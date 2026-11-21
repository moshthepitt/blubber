#![forbid(unsafe_code)]
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use std::{
    env, fs,
    io::{self, Read, Write},
    path::Path,
};
use syn::{Fields, ImplItem, Item, TraitItem, Visibility};

fn public(v: &Visibility) -> bool {
    matches!(v, Visibility::Public(_))
}
fn attrs(a: &[syn::Attribute]) -> String {
    a.iter()
        .filter(|a| {
            !["doc", "inline", "allow", "warn", "deny", "forbid", "cold"]
                .iter()
                .any(|name| a.path().is_ident(name))
        })
        .map(|a| a.to_token_stream().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}
fn emit(
    out: &mut Vec<String>,
    file: &str,
    owner: &str,
    kind: &str,
    value: impl ToTokens,
    a: &[syn::Attribute],
) {
    out.push(
        format!(
            "{file}\t{owner}\t{kind}\t{} {}",
            attrs(a),
            value.to_token_stream()
        )
        .trim_end()
        .to_string(),
    );
}
fn fields(fields: &Fields, all: bool) -> TokenStream {
    let mut out = TokenStream::new();
    for (index, field) in fields.iter().enumerate() {
        if all || public(&field.vis) {
            let name = field
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or(index.to_string());
            let ty = &field.ty;
            let text = format!("{} {name}: {} ,", attrs(&field.attrs), ty.to_token_stream());
            // Field names may be tuple indices; this is display text, not input Rust.
            out.extend(text.parse::<TokenStream>().unwrap_or_default());
        }
    }
    out
}
fn items(out: &mut Vec<String>, file: &str, owner: &str, list: &[Item]) {
    for item in list {
        match item {
            Item::Fn(i) if public(&i.vis) => emit(out, file, owner, "fn", &i.sig, &i.attrs),
            Item::Struct(i) if public(&i.vis) => {
                let (id, g, w) = (&i.ident, &i.generics, &i.generics.where_clause);
                emit(
                    out,
                    file,
                    owner,
                    "struct",
                    quote::quote!(struct #id #g #w),
                    &i.attrs,
                );
                emit(
                    out,
                    file,
                    &format!("{owner}::{id}"),
                    "fields",
                    fields(&i.fields, false),
                    &[],
                );
            }
            Item::Enum(i) if public(&i.vis) => {
                let (id, g, w) = (&i.ident, &i.generics, &i.generics.where_clause);
                emit(
                    out,
                    file,
                    owner,
                    "enum",
                    quote::quote!(enum #id #g #w),
                    &i.attrs,
                );
                for v in &i.variants {
                    let id = &v.ident;
                    let f = fields(&v.fields, true);
                    let d = v
                        .discriminant
                        .as_ref()
                        .map(|(_, e)| e.to_token_stream())
                        .unwrap_or_default();
                    emit(
                        out,
                        file,
                        &format!("{owner}::{}", i.ident),
                        "variant",
                        quote::quote!(#id (#f) = #d),
                        &v.attrs,
                    );
                }
            }
            Item::Trait(i) if public(&i.vis) => {
                let (id, g, w, s, u, a) = (
                    &i.ident,
                    &i.generics,
                    &i.generics.where_clause,
                    &i.supertraits,
                    &i.unsafety,
                    &i.auto_token,
                );
                emit(
                    out,
                    file,
                    owner,
                    "trait",
                    quote::quote!(#u #a trait #id #g: #s #w),
                    &i.attrs,
                );
                let owner = format!("{owner}::{id}");
                for t in &i.items {
                    match t {
                        TraitItem::Fn(f) => emit(out, file, &owner, "fn", &f.sig, &f.attrs),
                        TraitItem::Type(t) => emit(out, file, &owner, "type", t, &t.attrs),
                        TraitItem::Const(t) => emit(out, file, &owner, "const", t, &t.attrs),
                        _ => {}
                    }
                }
            }
            Item::Impl(i) => {
                let (g, w, ty) = (&i.generics, &i.generics.where_clause, &i.self_ty);
                let trait_name = i
                    .trait_
                    .as_ref()
                    .map(|(neg, p, _)| quote::quote!(#neg #p for))
                    .unwrap_or_default();
                let header = format!(
                    "{} {}",
                    attrs(&i.attrs),
                    quote::quote!(impl #g #trait_name #ty #w)
                );
                for t in &i.items {
                    match t {
                        ImplItem::Fn(f) if public(&f.vis) || i.trait_.is_some() => {
                            emit(out, file, &header, "fn", &f.sig, &f.attrs)
                        }
                        ImplItem::Type(t) if public(&t.vis) || i.trait_.is_some() => {
                            emit(out, file, &header, "type", t, &t.attrs)
                        }
                        ImplItem::Const(t) if public(&t.vis) || i.trait_.is_some() => {
                            emit(out, file, &header, "const", t, &t.attrs)
                        }
                        _ => {}
                    }
                }
            }
            Item::Mod(i) => {
                if !attrs(&i.attrs).is_empty() {
                    emit(out, file, owner, "module-scope", &i.ident, &i.attrs);
                }
                if public(&i.vis) {
                    emit(out, file, owner, "mod", &i.ident, &i.attrs);
                }
                if let Some((_, content)) = &i.content {
                    items(out, file, &format!("{owner}::{}", i.ident), content);
                }
            }
            Item::Type(i) if public(&i.vis) => emit(out, file, owner, "type", i, &i.attrs),
            Item::Use(i) if public(&i.vis) => emit(out, file, owner, "use", i, &i.attrs),
            Item::Const(i) if public(&i.vis) => emit(out, file, owner, "const", i, &i.attrs),
            Item::Static(i) if public(&i.vis) => emit(out, file, owner, "static", i, &i.attrs),
            Item::Macro(i) if i.mac.path.is_ident("verus") => {
                verus_items(out, file, owner, i.mac.tokens.clone())
            }
            Item::Macro(i) if i.attrs.iter().any(|a| a.path().is_ident("macro_export")) => {
                emit(out, file, owner, "macro", i, &i.attrs)
            }
            _ => {}
        }
    }
}
// Verus declarations use ordinary token groups but extend function headers.
// Keep every declared public signature, including proof-only declarations.
fn verus_header(tokens: &[proc_macro2::TokenTree]) -> (proc_macro2::TokenStream, String, bool) {
    let mut header = proc_macro2::TokenStream::new();
    let mut kind = String::new();
    let mut public = false;
    let mut at = 0;
    while at < tokens.len() {
        if matches!(&tokens[at], TokenTree::Punct(p) if p.as_char() == '#') {
            if let Some(TokenTree::Group(g)) = tokens.get(at + 1) {
                if g.stream().to_string().starts_with("doc =") {
                    at += 2;
                    continue;
                }
            }
        }
        if let TokenTree::Ident(id) = &tokens[at] {
            let word = id.to_string();
            if [
                "requires",
                "ensures",
                "recommends",
                "decreases",
                "opens_invariants",
                "no_unwind",
            ]
            .contains(&word.as_str())
            {
                break;
            }
            if word == "pub" {
                public = !matches!(tokens.get(at + 1), Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Parenthesis);
            }
            if kind.is_empty()
                && [
                    "fn", "struct", "enum", "impl", "trait", "const", "type", "use",
                ]
                .contains(&word.as_str())
            {
                kind = word;
            }
        }
        header.extend([tokens[at].clone()]);
        at += 1;
    }
    (header, kind, public)
}
fn verus_items(out: &mut Vec<String>, file: &str, owner: &str, stream: TokenStream) {
    let tokens: Vec<_> = stream.into_iter().collect();
    let mut start = 0;
    for (at, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = token {
            if g.delimiter() == Delimiter::Brace {
                let (header, kind, public) = verus_header(&tokens[start..at]);
                if public || owner.split_whitespace().any(|part| part == "trait") {
                    emit(out, file, owner, "verus", header.clone(), &[]);
                    if kind == "enum" {
                        emit(out, file, &header.to_string(), "variants", g.stream(), &[]);
                    }
                    if kind == "struct" {
                        let mut field = TokenStream::new();
                        let mut angle_depth = 0usize;
                        for token in g.stream().into_iter().chain([TokenTree::Punct(
                            proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone),
                        )]) {
                            if matches!(&token, TokenTree::Punct(p) if p.as_char() == ',' && angle_depth == 0)
                            {
                                let parts: Vec<_> = field.clone().into_iter().collect();
                                let (cleaned, _, visible) = verus_header(&parts);
                                if visible {
                                    emit(out, file, &header.to_string(), "field", cleaned, &[]);
                                }
                                field = TokenStream::new();
                            } else {
                                if matches!(&token, TokenTree::Punct(p) if p.as_char() == '<') {
                                    angle_depth += 1;
                                }
                                if matches!(&token, TokenTree::Punct(p) if p.as_char() == '>') {
                                    angle_depth = angle_depth.saturating_sub(1);
                                }
                                field.extend([token]);
                            }
                        }
                    }
                }
                if kind == "impl" || (kind == "trait" && public) {
                    verus_items(out, file, &header.to_string(), g.stream());
                }
                start = at + 1;
            }
        } else if matches!(token, TokenTree::Punct(p) if p.as_char() == ';') {
            let (header, _, public) = verus_header(&tokens[start..=at]);
            if public || owner.split_whitespace().any(|part| part == "trait") {
                emit(out, file, owner, "verus", header, &[]);
            }
            start = at + 1;
        }
    }
}

struct Templates<'a> {
    out: &'a mut Vec<String>,
    file: &'a str,
}
impl<'ast> syn::visit::Visit<'ast> for Templates<'_> {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac
            .path
            .segments
            .last()
            .map(|s| s.ident == "quote" || s.ident == "quote_spanned")
            .unwrap_or(false)
        {
            verus_items(self.out, self.file, "template", mac.tokens.clone());
        }
        syn::visit::visit_macro(self, mac);
    }
}
fn walk(
    root: &Path,
    dir: &Path,
    output: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.exists() {
        return Ok(());
    }
    let mut paths = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(root, &path, output)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && path.file_name().and_then(|e| e.to_str()) != Some("tests.rs")
        {
            let source = fs::read_to_string(&path)?;
            let parsed = syn::parse_file(&source)?;
            if !attrs(&parsed.attrs).is_empty() {
                emit(
                    output,
                    &path.strip_prefix(root)?.to_string_lossy(),
                    "crate",
                    "file-scope",
                    TokenStream::new(),
                    &parsed.attrs,
                );
            }
            items(
                output,
                &path.strip_prefix(root)?.to_string_lossy(),
                "crate",
                &parsed.items,
            );
            let file = path.strip_prefix(root)?.to_string_lossy();
            syn::visit::Visit::visit_file(
                &mut Templates {
                    out: output,
                    file: &file,
                },
                &parsed,
            );
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::args().nth(1).ok_or("repository root required")?;
    if root == "--tokens" {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        let tokens: TokenStream = source.parse()?;
        println!("{tokens}");
        return Ok(());
    }
    let root = Path::new(&root);
    let mut output = Vec::new();
    for name in ["blubber-core", "blubber-derive", "blubber"] {
        walk(
            root,
            &root.join("crates").join(name).join("src"),
            &mut output,
        )?;
    }
    output.sort();
    let mut stdout = io::BufWriter::new(io::stdout());
    writeln!(
        stdout,
        "# Declared public API; generated by scripts/public-api.py --write"
    )?;
    for line in output {
        writeln!(stdout, "{line}")?;
    }
    Ok(())
}
