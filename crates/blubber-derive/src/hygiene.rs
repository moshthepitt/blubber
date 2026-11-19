//! Keep generated bindings independent of the consumer's value namespace.

use std::collections::BTreeMap;

use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    ext::IdentExt, parse::Parser, punctuated::Punctuated, spanned::Spanned, visit::Visit,
    visit_mut::VisitMut, Expr, Item, Token,
};

/// Renames generated bindings and masks unseen enclosing constants with functions.
///
/// Rust resolves constants in patterns even at mixed-site spans. A function item
/// shadows such a constant but permits a same-named local binding. The masks live
/// in an anonymous const around implementations, so no helper escapes into the
/// consumer's namespace. Field names and input types keep their original spans
/// and resolution; in particular, array lengths must never follow local renames.
pub(crate) fn bindings(
    input: &impl ToTokens,
    output: TokenStream,
    types: TokenStream,
) -> syn::Result<TokenStream> {
    let mut file: syn::File = syn::parse2(output.clone())?;
    let mut collect = Bindings::default();
    collect.visit_file(&file);
    if let Some(error) = collect.error {
        return Err(error);
    }
    let mut occupied = quote!(#input #output);
    let mut names = BTreeMap::new();
    for (name, ()) in collect.names {
        let ident = crate::fresh_ident(&occupied, &format!("__blubber_local_{name}"));
        occupied.extend(quote!(#ident));
        names.insert(name, ident);
    }
    let mut rename = Rename { names };
    rename.visit_file_mut(&mut file);
    let masks = rename.names.values().map(|ident| {
        quote! {
            #[allow(dead_code, non_snake_case)]
            fn #ident() {}
        }
    });
    let (implementations, declarations): (Vec<_>, Vec<_>) = file
        .items
        .into_iter()
        .partition(|item| matches!(item, Item::Impl(_) | Item::Const(_)));
    Ok(quote! {
        #(#declarations)*
        #[forbid(unsafe_code)]
        const _: () = {
            #types
            const _: () = { #(#masks)* #(#implementations)* };
        };
    })
}

#[derive(Default)]
struct Bindings {
    names: BTreeMap<String, ()>,
    error: Option<syn::Error>,
}
impl<'ast> Visit<'ast> for Bindings {
    fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
        if pat.ident != "self" {
            self.names.insert(pat.ident.to_string(), ());
        }
        syn::visit::visit_pat_ident(self, pat);
    }
    fn visit_type(&mut self, _: &'ast syn::Type) {}
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // Macro bodies are owned here, rather than borrowed from `mac`.
        let mut other = Bindings::default();
        visit_macro(&mut other, mac);
        self.names.extend(other.names);
        if let Some(error) = other.error {
            if let Some(previous) = &mut self.error {
                previous.combine(error);
            } else {
                self.error = Some(error);
            }
        }
    }
}

struct Rename {
    names: BTreeMap<String, Ident>,
}
impl VisitMut for Rename {
    fn visit_pat_ident_mut(&mut self, pat: &mut syn::PatIdent) {
        if let Some(ident) = self.names.get(&pat.ident.to_string()) {
            pat.ident = ident.clone();
        }
        syn::visit_mut::visit_pat_ident_mut(self, pat);
    }
    fn visit_expr_path_mut(&mut self, expr: &mut syn::ExprPath) {
        if expr.qself.is_none()
            && expr.path.leading_colon.is_none()
            && expr.path.segments.len() == 1
        {
            let id = &mut expr.path.segments[0].ident;
            if let Some(ident) = self.names.get(&id.to_string()) {
                *id = ident.clone();
            }
        }
        syn::visit_mut::visit_expr_path_mut(self, expr);
    }
    fn visit_field_value_mut(&mut self, field: &mut syn::FieldValue) {
        syn::visit_mut::visit_field_value_mut(self, field);
        if field.colon_token.is_none() {
            field.colon_token = Some(Token![:](field.member.span()));
        }
    }
    fn visit_field_pat_mut(&mut self, field: &mut syn::FieldPat) {
        syn::visit_mut::visit_field_pat_mut(self, field);
        if field.colon_token.is_none() {
            field.colon_token = Some(Token![:](field.member.span()));
        }
    }
    fn visit_type_mut(&mut self, _: &mut syn::Type) {}
    fn visit_generics_mut(&mut self, _: &mut syn::Generics) {}
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        if let Some(mut body) = macro_body(mac) {
            match &mut body {
                MacroBody::Items(items) => self.visit_file_mut(items),
                MacroBody::Expressions(exprs) => {
                    for expr in exprs.iter_mut() {
                        self.visit_expr_mut(expr);
                    }
                }
                MacroBody::Matches(expr, pat, guard) => {
                    self.visit_expr_mut(expr);
                    self.visit_pat_mut(pat);
                    if let Some(guard) = guard {
                        self.visit_expr_mut(guard);
                    }
                }
            }
            mac.tokens = body.into_token_stream();
        }
    }
}

// Own parsed macro bodies so both visitors can use ordinary syn traversal.
enum MacroBody {
    Items(syn::File),
    Expressions(Punctuated<Expr, Token![,]>),
    Matches(Box<Expr>, Box<syn::Pat>, Option<Box<Expr>>),
}
impl ToTokens for MacroBody {
    fn to_tokens(&self, out: &mut TokenStream) {
        match self {
            Self::Items(file) => file.to_tokens(out),
            Self::Expressions(exprs) => exprs.to_tokens(out),
            Self::Matches(expr, pat, guard) => {
                let guard = guard.as_ref().map(|expr| quote!(if #expr));
                out.extend(quote!(#expr, #pat #guard));
            }
        }
    }
}
fn macro_body(mac: &syn::Macro) -> Option<MacroBody> {
    let name = mac.path.segments.last()?.ident.to_string();
    match name.as_str() {
        "with_alloc" | "with_pinocchio" => {
            syn::parse2(mac.tokens.clone()).ok().map(MacroBody::Items)
        }
        "assert" => Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(mac.tokens.clone())
            .ok()
            .map(MacroBody::Expressions),
        "matches" => (|input: syn::parse::ParseStream<'_>| {
            let expr = input.parse()?;
            input.parse::<Token![,]>()?;
            let pat = syn::Pat::parse_multi(input)?;
            let guard = if input.peek(Token![if]) {
                input.parse::<Token![if]>()?;
                Some(input.parse()?)
            } else {
                None
            };
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
            Ok(MacroBody::Matches(
                Box::new(expr),
                Box::new(pat),
                guard.map(Box::new),
            ))
        })
        .parse2(mac.tokens.clone())
        .ok(),
        _ => None,
    }
}
fn visit_macro(visitor: &mut Bindings, mac: &syn::Macro) {
    match macro_body(mac) {
        Some(MacroBody::Items(file)) => visitor.visit_file(&file),
        Some(MacroBody::Expressions(exprs)) => {
            for expr in &exprs {
                visitor.visit_expr(expr);
            }
        }
        Some(MacroBody::Matches(expr, pat, guard)) => {
            visitor.visit_expr(&expr);
            visitor.visit_pat(&pat);
            if let Some(guard) = guard {
                visitor.visit_expr(&guard);
            }
        }
        None => {
            if mac.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "with_alloc" | "with_pinocchio" | "assert" | "matches"
                )
            }) {
                visitor.error = Some(syn::Error::new_spanned(
                    mac,
                    "cannot parse generated macro body for hygiene",
                ));
            }
        }
    }
}

/// Rejects companion collisions without changing consumer tokens, then fixes `Self`.
pub(crate) fn normalize(item: &mut syn::ItemStruct) -> syn::Result<()> {
    let stem = item.ident.unraw().to_string();
    let generated_items =
        ["", "Ref", "Mut", "Init", "Owned"].map(|suffix| format!("{stem}{suffix}"));
    for parameter in &item.generics.params {
        let (kind, ident) = match parameter {
            syn::GenericParam::Type(param) => ("type", &param.ident),
            syn::GenericParam::Const(param) => ("const", &param.ident),
            syn::GenericParam::Lifetime(param) => ("lifetime", &param.lifetime.ident),
        };
        let name = ident.unraw().to_string();
        if generated_items.contains(&name) {
            return Err(syn::Error::new_spanned(
                parameter,
                format!("consumer {kind} generic `{name}` conflicts with generated item `{name}`; rename the consumer generic parameter"),
            ));
        }
    }
    let name = &item.ident;
    let (_, args, _) = item.generics.split_for_impl();
    let owner: syn::Type = syn::parse_quote!(#name #args);

    let mut owner = Owner(owner, None);
    owner.visit_item_struct_mut(item);
    owner.result()
}

pub(crate) fn lifetime(input: &impl ToTokens) -> syn::Lifetime {
    let ident = crate::fresh_ident(input, "__blubber_lifetime");
    syn::Lifetime::new(&format!("'{ident}"), ident.span())
}

/// Normalizes field `Self` before Fixed metadata leaves the native struct's scope.
pub(crate) fn fixed_self(input: &mut syn::DeriveInput) -> syn::Result<()> {
    let name = &input.ident;
    let (_, arguments, _) = input.generics.split_for_impl();
    let mut owner = Owner(syn::parse_quote!(#name #arguments), None);
    owner.visit_derive_input_mut(input);
    owner.result()
}

struct Owner(syn::Type, Option<syn::Error>);
impl Owner {
    fn result(self) -> syn::Result<()> {
        self.1.map_or(Ok(()), Err)
    }
}
impl VisitMut for Owner {
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        // A type macro can inspect or reinterpret its tokens. Rewriting opaque
        // `Self` tokens cannot preserve that contract; require the explicit owner.
        fn contains_self(tokens: TokenStream) -> bool {
            tokens.into_iter().any(|token| match token {
                proc_macro2::TokenTree::Ident(id) => id == "Self",
                proc_macro2::TokenTree::Group(group) => contains_self(group.stream()),
                _ => false,
            })
        }
        if contains_self(mac.tokens.clone()) {
            let owner = &self.0;
            self.1 = Some(syn::Error::new_spanned(mac,
                format!("Self inside a field macro cannot be relocated; use the explicit owner type `{}` instead", quote!(#owner))));
        }
    }
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Path(path) = ty {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments[0].ident == "Self"
            {
                let owner = &self.0;
                if path.path.segments.len() == 1 {
                    *ty = self.0.clone();
                } else {
                    let rest: Punctuated<syn::PathSegment, Token![::]> =
                        path.path.segments.iter().skip(1).cloned().collect();
                    *ty = syn::parse_quote!(<#owner>::#rest);
                }
                return;
            }
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
    fn visit_expr_path_mut(&mut self, expr: &mut syn::ExprPath) {
        if expr.qself.is_none()
            && expr.path.leading_colon.is_none()
            && expr.path.segments[0].ident == "Self"
            && expr.path.segments.len() > 1
        {
            let owner = &self.0;
            let rest: Punctuated<syn::PathSegment, Token![::]> =
                expr.path.segments.iter().skip(1).cloned().collect();
            *expr = syn::parse_quote!(<#owner>::#rest);
            return;
        }
        syn::visit_mut::visit_expr_path_mut(self, expr);
    }
}

/// Resolve consumer field types outside binding masks, with no named helpers.
///
/// Qualified projections let public companions remain ordinary module items.
/// Only the trait implementations contain consumer field expressions; they live
/// in the anonymous scope outside generated bindings and constants.
pub(crate) fn field_types(
    input: &mut syn::DeriveInput,
    library: &TokenStream,
) -> syn::Result<TokenStream> {
    let syn::Data::Struct(data) = &input.data else {
        return Ok(TokenStream::new());
    };
    let owner = &input.ident;
    let conditions = input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"));
    let conditions = conditions.collect::<Vec<_>>();
    let mut bounded = input.generics.clone();
    for param in bounded.type_params_mut() {
        param.bounds.push(syn::parse_quote!('static));
    }
    let (_, arguments, _) = bounded.split_for_impl();
    let mut implementations = TokenStream::new();
    for (index, field) in data.fields.iter().enumerate() {
        let ty = &field.ty;
        let mut generics = bounded.clone();
        generics
            .make_where_clause()
            .predicates
            .push(syn::parse_quote_spanned!(ty.span()=> #ty: #library::Schema));
        let (ig, _, wc) = generics.split_for_impl();
        implementations.extend(quote::quote_spanned!(ty.span()=>
            #(#conditions)*
            #[forbid(unsafe_code)]
            impl #ig #library::__private::SchemaField<#index> for #owner #arguments #wc {
                type Type = #ty;
            }
        ));
    }
    let mut bare = input.generics.clone();
    bare.where_clause = None;
    for param in bare.type_params_mut() {
        param.bounds = param.bounds.iter().filter(|b| matches!(b, syn::TypeParamBound::Trait(bound) if matches!(bound.modifier, syn::TraitBoundModifier::Maybe(_)))).cloned().collect();
    }
    let syn::Data::Struct(data) = &mut input.data else {
        unreachable!()
    };
    for (index, field) in data.fields.iter_mut().enumerate() {
        if !bare.params.is_empty() {
            bare.make_where_clause().predicates.push(
                syn::parse_quote!(#owner #arguments: #library::__private::SchemaField<#index>),
            );
        }
        field.ty = syn::parse_quote_spanned!(field.ty.span()=> <#owner #arguments as #library::__private::SchemaField<#index>>::Type);
    }
    if !input.generics.params.is_empty() {
        input.generics = bare;
    }
    Ok(implementations)
}
