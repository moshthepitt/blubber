//! Schema declarations become markers and borrowed inputs with explicit codecs.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{
    ext::IdentExt, parse::Parser, punctuated::Punctuated, spanned::Spanned, Expr, Fields, Ident,
    Item, Lit, Meta, Token,
};

#[derive(Default)]
struct Options {
    account: bool,
    discriminator: Option<[u8; 8]>,
    constraint_fields: Option<bool>,
    namespace: Option<syn::LitStr>,
    semantic_epoch: Option<u32>,
}

impl Options {
    fn parse(tokens: TokenStream) -> syn::Result<Self> {
        let mut options = Self::default();
        for meta in Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens)? {
            match meta {
                Meta::Path(path) if path.is_ident("account") && !options.account => {
                    options.account = true;
                }
                Meta::NameValue(value) if value.path.is_ident("namespace") && options.namespace.is_none() => {
                    let Expr::Lit(syn::ExprLit { lit: Lit::Str(namespace), .. }) = value.value else {
                        return Err(syn::Error::new_spanned(value, "namespace requires a string literal"));
                    };
                    options.namespace = Some(namespace);
                }
                Meta::NameValue(value) if value.path.is_ident("semantic_epoch") && options.semantic_epoch.is_none() => {
                    let Expr::Lit(syn::ExprLit { lit: Lit::Int(epoch), .. }) = value.value else {
                        return Err(syn::Error::new_spanned(value, "semantic_epoch requires a u32 literal"));
                    };
                    if epoch.base10_digits().starts_with('-') {
                        return Err(syn::Error::new_spanned(epoch, "semantic_epoch requires a u32 literal"));
                    }
                    options.semantic_epoch = Some(epoch.base10_parse::<u32>()?);
                }
                Meta::NameValue(value)
                    if value.path.is_ident("constraint_fields") && options.constraint_fields.is_none() =>
                {
                    let Expr::Lit(syn::ExprLit { lit: Lit::Bool(value), .. }) = value.value else {
                        return Err(syn::Error::new_spanned(value, "constraint_fields requires true or false"));
                    };
                    options.constraint_fields = Some(value.value);
                }
                Meta::NameValue(value)
                    if value.path.is_ident("discriminator") && options.discriminator.is_none() =>
                {
                    let Expr::Array(array) = &value.value else {
                        return Err(syn::Error::new_spanned(
                            value.value,
                            "discriminator requires exactly eight byte literals",
                        ));
                    };
                    if array.elems.len() != 8 {
                        return Err(syn::Error::new_spanned(
                            array,
                            "discriminator requires exactly eight byte literals",
                        ));
                    }
                    let mut bytes = [0; 8];
                    for (slot, expr) in bytes.iter_mut().zip(&array.elems) {
                        let Expr::Lit(literal) = expr else {
                            return Err(syn::Error::new_spanned(expr, "expected a byte literal"));
                        };
                        let Lit::Int(value) = &literal.lit else {
                            return Err(syn::Error::new_spanned(expr, "expected a byte literal"));
                        };
                        *slot = value.base10_parse::<u8>()?;
                    }
                    if bytes == [0; 8] {
                        return Err(syn::Error::new_spanned(
                            array,
                            "all-zero discriminator is reserved for uninitialized storage",
                        ));
                    }
                    options.discriminator = Some(bytes);
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "expected account, discriminator = [eight bytes], namespace = \"...\", semantic_epoch = N or constraint_fields = false, without duplicates",
                    ))
                }
            }
        }
        if (options.namespace.is_some() || options.semantic_epoch.is_some()) && !options.account {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "namespace and semantic_epoch require account",
            ));
        }
        if options.discriminator.is_some() && !options.account {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "discriminator requires account",
            ));
        }
        if options.constraint_fields.is_some() && !options.account {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "constraint_fields requires account",
            ));
        }
        Ok(options)
    }
}

pub(crate) fn expand(
    attributes: TokenStream,
    input: TokenStream,
    library: &TokenStream,
) -> syn::Result<TokenStream> {
    let options = Options::parse(attributes)?;
    let item: Item = syn::parse2(input)?;
    let Item::Struct(mut structure) = item else {
        return Err(syn::Error::new_spanned(
            item,
            "schema requires a struct with named fields",
        ));
    };
    // Normalize Self in the native owner before moving types to their own scope.
    crate::hygiene::normalize(&mut structure)?;
    let mut item: syn::DeriveInput = syn::parse2(quote!(#structure))?;
    let types = crate::hygiene::field_types(&mut item, library)?;
    let generated = expand_inner(options, quote!(#item), library)?;
    crate::hygiene::bindings(&item, generated, types)
}

fn expand_inner(
    options: Options,
    input: TokenStream,
    library: &TokenStream,
) -> syn::Result<TokenStream> {
    let item = syn::parse2::<Item>(input)?;
    let Item::Struct(mut item) = item else {
        return Err(syn::Error::new_spanned(
            item,
            "schema requires a struct with named fields",
        ));
    };
    crate::hygiene::normalize(&mut item)?;
    let lifetime = crate::hygiene::lifetime(&item);
    let Fields::Named(fields) = &item.fields else {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "schema requires a nonempty struct with named fields",
        ));
    };
    if fields.named.is_empty() || fields.named.len() > u16::MAX as ::core::primitive::usize {
        return Err(syn::Error::new_spanned(
            fields,
            "schema requires 1 through 65535 named fields",
        ));
    }

    let fixed_count = crate::fresh_ident(&item, "__BLUBBER_FIXED_COUNT");
    let variable_count = crate::fresh_ident(&item, "__BLUBBER_VARIABLE_COUNT");
    let directory = crate::fresh_ident(&item, "__BLUBBER_DIRECTORY");
    let fixed = crate::fresh_ident(&item, "__BLUBBER_FIXED");
    let variable = crate::fresh_ident(&item, "__BLUBBER_VARIABLE");
    let minimum = crate::fresh_ident(&item, "__BLUBBER_MINIMUM");
    if !item.generics.params.is_empty() {
        if options.account {
            return Err(syn::Error::new_spanned(
                &item.generics,
                "generic schemas are nested-only; account requires a concrete struct",
            ));
        }
        return crate::generic_schema::expand(&item, library);
    }
    let name = &item.ident;
    let init_name = format_ident!("{}Init", name.unraw());
    let ref_name = format_ident!("{}Ref", name.unraw());
    let mut_name = format_ident!("{}Mut", name.unraw());
    let owned_name = format_ident!("{}Owned", name.unraw());
    let readers = crate::schema_read::expand(&item, library, options.account)?;
    let read_storage = options.account.then(|| {
        quote! {
            storage_len: #library::__private::checked_add(12, view.bytes().len() as ::core::primitive::u32)?,
            root_origin: false,
        }
    });
    let visibility = &item.vis;
    let attributes = &item.attrs;
    let conditional = item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        .collect::<Vec<_>>();
    let lints = item
        .attrs
        .iter()
        .filter(|attr| {
            ["allow", "warn", "deny", "forbid"]
                .iter()
                .any(|name| attr.path().is_ident(name))
        })
        .collect::<Vec<_>>();
    let marker_doc = format!("Schema marker for `{}`.", name.unraw());
    let init_doc = format!(
        "Borrowed, allocation-free encoding input for `{}`.",
        name.unraw()
    );
    let mut init_fields = Vec::new();
    let mut owned_fields = Vec::new();
    let mut owned_decodings = Vec::new();
    let mut fixed_copies = Vec::new();
    for (index, field) in fields.named.iter().enumerate() {
        let ident = field.ident.as_ref().expect("named field");
        let ty = &field.ty;
        let vis = &field.vis;
        let ordinal = index as u16;
        fixed_copies.push(quote_spanned! { field.span()=>
            #ident: <#ty as #library::Schema>::__read_fixed(view.field(#ordinal)?)?
        });
        let mut attrs = Vec::new();
        for attribute in &field.attrs {
            if attribute.path().is_ident("blubber") {
                // Validated and consumed by the reader generator.
            } else if attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr") {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "conditional schema fields are unsupported; condition the whole schema instead",
                ));
            } else {
                attrs.push(attribute);
            }
        }
        let field_doc = format!("Input for `{}`.", ident.unraw());
        init_fields.push(quote_spanned! { ty.span()=>
            #(#attrs)*
            #[doc = #field_doc]
            #vis #ident: <#ty as #library::Schema>::Init<#lifetime>
        });
        owned_fields.push(quote_spanned! { ty.span()=>
            #(#attrs)*
            #[doc = #field_doc]
            #vis #ident: <#ty as #library::Schema>::Owned
        });
        owned_decodings.push(quote_spanned! { ty.span()=>
            #ident: <#ty as #library::Schema>::decode_owned(view.field(#ordinal)?)?
        });
    }
    let field_refs = fields.named.iter().collect::<Vec<_>>();
    let field_types = field_refs.iter().map(|field| &field.ty).collect::<Vec<_>>();
    let representation = crate::product::types(&field_types, library);
    let borrowed =
        crate::product::project(&field_refs, quote!(init), quote!(borrowed), library, false);
    let owned = crate::product::project(&field_refs, quote!(value), quote!(owned), library, false);
    let children =
        fields
            .named
            .iter()
            .rev()
            .fold(quote!(#library::__private::Nil), |tail, field| {
                let ty = &field.ty;
                quote!(#library::__private::Cons<#ty, #tail>)
            });

    let field_count = fields.named.len();
    let metadata = options.account.then(|| {
        crate::account::expand(
            &item,
            library,
            options.discriminator,
            options.constraint_fields.unwrap_or(true),
        )
    });
    let account = if options.account {
        let namespace = options
            .namespace
            .as_ref()
            .map(|value| value.value())
            .unwrap_or_else(|| format!("account:{}", name.unraw()));
        let semantic_epoch = options.semantic_epoch.unwrap_or(0);
        let discriminator = match options.discriminator {
            Some(bytes) => quote!([#(#bytes),*]),
            None => quote!(#library::identity::account_identity(
                Self::NAMESPACE, Self::SEMANTIC_EPOCH, Self::WIRE_DESCRIPTOR_BYTES)),
        };
        let creation_helpers = [
            ("create", false, false),
            ("create_pda", false, true),
            ("allocate_account", true, false),
            ("allocate_pda", true, true),
        ].map(|(method, prefunded, pda)| {
            let method = Ident::new(method, name.span());
            let seeds_arg = pda.then(|| quote!(seeds_with_bump: &[&[::core::primitive::u8]],));
            let seeds = if pda { quote!(Some(seeds_with_bump)) } else { quote!(None) };
            quote! {
                /// Creates or allocates the account, initializes it, and commits rent.
                ///
                /// Validates input before CPI. Allocation retains prefunded lamports;
                /// creation requires zero lamports. PDA methods take the complete
                /// seeds including bump, derived against the calling program_id.
                /// Propagate errors on-chain to roll back CPIs and initialization.
                pub fn #method(
                    state: &mut #library::pinocchio_sdk::AccountView,
                    payer: &#library::pinocchio_sdk::AccountView,
                    system_program: &#library::pinocchio_sdk::AccountView,
                    program_id: &#library::pinocchio_sdk::Address,
                    #seeds_arg
                    init: #init_name<'_>,
                ) -> #library::Result<#library::CommitInfo> {
                    let space = Self::encoded_len_of(&init)?;
                    #library::__private::create(state, payer, system_program, program_id, space, #prefunded, #seeds)?;
                    let mut store = #library::pinocchio::Store::new_with_headroom(
                        state, program_id, #library::pinocchio::Funding::Prepaid,
                        #library::pinocchio::Headroom::UpTo(0),
                    )?;
                    Self::initialize(&mut store, init)?.commit()
                }
            }
        });
        quote! {
            impl #name {
                /// Account identity, excluding the used-length word and root bytes.
                pub const DISCRIMINATOR: [::core::primitive::u8; 8] = #discriminator;
                /// Stable application namespace; freeze it before renaming an account.
                pub const NAMESPACE: &'static ::core::primitive::str = #namespace;
                /// Semantic contract epoch, independent of byte layout changes.
                pub const SEMANTIC_EPOCH: ::core::primitive::u32 = #semantic_epoch;
                /// Version-one canonical wire descriptor, computed at compile time.
                pub const WIRE_DESCRIPTOR_BYTES: &'static [::core::primitive::u8] =
                    &<Self as #library::Schema>::WIRE_DESCRIPTOR.encode::<{
                        <Self as #library::Schema>::WIRE_DESCRIPTOR.encoded_len()
                    }>();
                /// Smallest account encoding, including its twelve-byte envelope.
                pub const MIN_ACCOUNT_LEN: ::core::primitive::u32 = match
                    <Self as #library::Schema>::MIN_SIZE.checked_add(12) {
                        ::core::option::Option::Some(size) => size,
                        ::core::option::Option::None => ::core::panic!("account minimum exceeds u32"),
                    };
                /// Adds reservation to the empty account size, including the envelope.
                ///
                /// Reports ArithmeticOverflow if the result exceeds u32. SDK
                /// allocation limits apply separately. Usable in account space constraints.
                pub const fn space(reserve: ::core::primitive::usize) -> #library::Result<::core::primitive::usize> {
                    #library::__private::account_space(Self::MIN_ACCOUNT_LEN, reserve)
                }
                /// Measures and validates borrowed input, including its envelope.
                pub fn encoded_len_of(init: &#init_name<'_>) -> #library::Result<::core::primitive::u32> {
                    #library::__private::checked_add(12, <Self as #library::Schema>::measure(init)?)
                }
                /// Loads and validates the complete account encoding.
                #[inline(always)]
                pub fn load(bytes: &[::core::primitive::u8]) -> #library::Result<#ref_name<'_>> {
                    let view = #library::View::load::<Self>(bytes, Self::DISCRIMINATOR)?;
                    ::core::result::Result::Ok(#ref_name {
                        view, storage_len: bytes.len() as ::core::primitive::u32, root_origin: true,
                    })
                }
                /// Loads under the explicit valid-current-schema caller obligation.
                /// With `verify-on-load`, scans the complete root before return.
                ///
                /// Trust requires bytes written by this program through blubber,
                /// with owner and discriminator checked. Reads and edits check only
                /// touched structure; malformed directories can alias siblings
                /// within the account.
                #[inline(always)]
                pub fn load_trusted(bytes: &[::core::primitive::u8]) -> #library::Result<#ref_name<'_>> {
                    let view = #library::View::load_trusted::<Self>(bytes, Self::DISCRIMINATOR)?;
                    ::core::result::Result::Ok(#ref_name {
                        view, storage_len: bytes.len() as ::core::primitive::u32, root_origin: true,
                    })
                }
                /// Opens an exclusive root: checked on hosts, lazy on authenticated SDK stores.
                ///
                /// SDK callers guarantee checked initialization/migration for this schema
                /// followed only by approved writers. Touched checks do not certify
                /// global map order or disjoint directories. Use `load_checked_mut`
                /// for untrusted contents or explicit poison recovery. `verify-on-load`
                /// also scans SDK roots; it does not clear their poison.
                #[inline(always)]
                pub fn load_mut(store: &mut dyn #library::Resize) -> #library::Result<#mut_name<'_>> {
                    #library::__private::Projection::<Self>::load(store)
                        .map(|edit| #mut_name { edit })
                }
                /// Fully validates an exclusive root, recovering valid poisoned storage.
                #[inline(always)]
                pub fn load_checked_mut(store: &mut dyn #library::Resize) -> #library::Result<#mut_name<'_>> {
                    #library::__private::Projection::<Self>::load_checked(store)
                        .map(|edit| #mut_name { edit })
                }
                /// Opens under the explicit valid-current-schema caller obligation.
                /// With `verify-on-load`, scans the complete root before return.
                ///
                /// Trust requires bytes written by this program through blubber,
                /// with owner and discriminator checked. Edits check only touched
                /// structure; malformed directories can alias siblings within the account.
                #[inline(always)]
                pub fn load_trusted_mut(store: &mut dyn #library::Resize) -> #library::Result<#mut_name<'_>> {
                    #library::__private::Projection::<Self>::load_trusted(store)
                        .map(|edit| #mut_name { edit })
                }
                /// Measures and initializes zero-identity storage, publishing its envelope last.
                ///
                /// Preflight failures preserve storage. An unexpected encoder
                /// panic leaves the session poisoned until full validation.
                pub fn initialize<#lifetime>(store: &#lifetime mut dyn #library::Resize, init: #init_name<'_>)
                    -> #library::Result<#mut_name<#lifetime>> {
                    #library::__private::Projection::<Self>::initialize(store, init)
                        .map(|edit| #mut_name { edit })
                }
                #library::__private::with_pinocchio! {
                    /// Authenticates and reads an account under a scoped shared guard.
                    /// The result cannot borrow account data beyond the callback.
                    ///
                    /// Checks schema identity/envelope before the callback; contents are lazy.
                    /// Requires checked initialization/migration for this schema and only
                    /// approved writers thereafter. Touched checks do not certify global
                    /// map order or disjoint directories. Use `with_read_checked` for
                    /// untrusted contents. `verify-on-load` adds a complete scan.
                    pub fn with_read<R>(
                        account: &#library::pinocchio_sdk::AccountView,
                        program_id: &#library::pinocchio_sdk::Address,
                        read: impl ::core::ops::FnOnce(#ref_name<'_>) -> #library::Result<R>,
                    ) -> #library::Result<R> {
                        let store = #library::pinocchio::ReadStore::new(account, program_id)?;
                        read(store.load::<Self>()?)
                    }

                    /// Opens an edit session and commits once when the callback succeeds.
                    ///
                    /// On callback Err, skips commit and drops the store: completed
                    /// edits remain and Drop only attempts trimming, without rent
                    /// settlement. Propagate errors on-chain for transaction rollback.
                    /// Default opening uses existing spare without speculative growth.
                    /// Use `with_edit_options` to request a growth budget.
                    /// The result cannot borrow the temporary account session.
                    /// Checks schema identity/envelope before the callback; contents are lazy.
                    /// Requires checked initialization/migration for this schema and only
                    /// approved writers thereafter. Touched checks do not certify global
                    /// map order or disjoint directories. Use `with_edit_checked` for
                    /// untrusted contents. `verify-on-load` adds a complete scan.
                    pub fn with_edit<R>(
                        account: &mut #library::pinocchio_sdk::AccountView,
                        program_id: &#library::pinocchio_sdk::Address,
                        funding: #library::pinocchio::Funding<'_>,
                        edit: impl ::core::ops::FnOnce(&mut #mut_name<'_>) -> #library::Result<R>,
                    ) -> #library::Result<(R, #library::CommitInfo)> {
                        let mut store = #library::pinocchio::Store::new(account, program_id, funding)?;
                        let mut account = Self::load_mut(&mut store)?;
                        let value = edit(&mut account)?;
                        let receipt = account.commit()?;
                        ::core::result::Result::Ok((value, receipt))
                    }

                    /// Edits under an explicit bounded lease and commits on callback success.
                    ///
                    /// Uses the same authenticated lazy policy as `with_edit`, including
                    /// `verify-on-load`. Use `Headroom::UpTo(0)` for equal-width edits,
                    /// or sum positive encoded growth for a known sequence, including
                    /// directory and ancestor bytes, less existing opening spare.
                    /// UpTo may grant less than requested.
                    /// Edits exceeding the lease fail before writes. Select Max
                    /// for unknown sequences. Callback errors skip
                    /// settlement and retain completed edits; propagate them for rollback.
                    /// No result can borrow the temporary session. Refund recipients
                    /// must be authorized by the application.
                    ///
                    #[doc = concat!(
                        "```\nuse ", stringify!(#library), "::{schema, List};\n",
                        "use ", stringify!(#library), "::pinocchio::{EditOptions, Funding, Headroom};\n",
                        "use ", stringify!(#library), "::pinocchio_sdk::{AccountView, Address};\n",
                        "#[schema(account)]\nstruct Queue { values: List<u64> }\n",
                        "fn push_two(account: &mut AccountView, owner: &Address) -> ",
                        stringify!(#library), "::Result<()> {\n",
                        "    Queue::with_edit_checked_options(account, owner, EditOptions {\n",
                        "        funding: Funding::Prepaid, headroom: Headroom::UpTo(16), refund: None,\n",
                        "    }, |queue| {\n",
                        "        queue.values_mut().push(1)?;\n",
                        "        queue.values_mut().push(2)\n",
                        "    })?;\n    Ok(())\n}\n```",
                    )]
                    #[inline(always)]
                    pub fn with_edit_options<'a, R>(
                        account: &'a mut #library::pinocchio_sdk::AccountView,
                        program_id: &#library::pinocchio_sdk::Address,
                        options: #library::pinocchio::EditOptions<'a>,
                        edit: impl ::core::ops::FnOnce(&mut #mut_name<'_>) -> #library::Result<R>,
                    ) -> #library::Result<(R, #library::CommitInfo)> {
                        let mut store = #library::pinocchio::Store::new_with_headroom(
                            account, program_id, options.funding, options.headroom,
                        )?;
                        if let ::core::option::Option::Some(recipient) = options.refund {
                            store = store.with_refund(recipient)?;
                        }
                        let mut account = Self::load_mut(&mut store)?;
                        let value = edit(&mut account)?;
                        let receipt = account.commit()?;
                        ::core::result::Result::Ok((value, receipt))
                    }

                    /// Authenticates and reads an account under a scoped shared guard.
                    /// The result cannot borrow account data beyond the callback.
                    /// Validates the complete encoding before invoking the callback.
                    pub fn with_read_checked<R>(
                        account: &#library::pinocchio_sdk::AccountView,
                        program_id: &#library::pinocchio_sdk::Address,
                        read: impl ::core::ops::FnOnce(#ref_name<'_>) -> #library::Result<R>,
                    ) -> #library::Result<R> {
                        let store = #library::pinocchio::ReadStore::new(account, program_id)?;
                        read(Self::load(store.bytes())?)
                    }

                    /// Opens an edit session and commits once when the callback succeeds.
                    ///
                    /// On callback Err, skips commit and drops the store: completed
                    /// edits remain and Drop only attempts trimming, without rent
                    /// settlement. Propagate errors on-chain for transaction rollback.
                    /// The result cannot borrow the temporary account session.
                    /// Validates the complete encoding before invoking the callback.
                    pub fn with_edit_checked<R>(
                        account: &mut #library::pinocchio_sdk::AccountView,
                        program_id: &#library::pinocchio_sdk::Address,
                        funding: #library::pinocchio::Funding<'_>,
                        edit: impl ::core::ops::FnOnce(&mut #mut_name<'_>) -> #library::Result<R>,
                    ) -> #library::Result<(R, #library::CommitInfo)> {
                        let mut store = #library::pinocchio::Store::new(account, program_id, funding)?;
                        let mut account = Self::load_checked_mut(&mut store)?;
                        let value = edit(&mut account)?;
                        let receipt = account.commit()?;
                        ::core::result::Result::Ok((value, receipt))
                    }

                    /// Fully validates the encoding before invoking an explicitly budgeted edit.
                    ///
                    /// Uses the same lease, refund and completion rules as `with_edit_options`.
                    /// The compiled example on that helper selects this checked twin.
                    /// Default options use existing spare without speculative growth.
                    /// Callback errors retain completed edits and skip rent settlement;
                    /// propagate them on-chain for rollback. The result cannot borrow
                    /// the temporary session or its refund recipient.
                    #[inline(always)]
                    pub fn with_edit_checked_options<'a, R>(
                        account: &'a mut #library::pinocchio_sdk::AccountView,
                        program_id: &#library::pinocchio_sdk::Address,
                        options: #library::pinocchio::EditOptions<'a>,
                        edit: impl ::core::ops::FnOnce(&mut #mut_name<'_>) -> #library::Result<R>,
                    ) -> #library::Result<(R, #library::CommitInfo)> {
                        let mut store = #library::pinocchio::Store::new_with_headroom(
                            account, program_id, options.funding, options.headroom,
                        )?;
                        if let ::core::option::Option::Some(recipient) = options.refund {
                            store = store.with_refund(recipient)?;
                        }
                        let mut account = Self::load_checked_mut(&mut store)?;
                        let value = edit(&mut account)?;
                        let receipt = account.commit()?;
                        ::core::result::Result::Ok((value, receipt))
                    }

                    #(#creation_helpers)*
                }
            }
            const _: () = { let _ = #name::MIN_ACCOUNT_LEN; };
        }
    } else {
        TokenStream::new()
    };

    Ok(quote! {
        #(#attributes)*
        #[doc = #marker_doc]
        #[forbid(unsafe_code)]
        #visibility struct #name;

        #(#conditional)*
        #(#lints)*
        #[doc = #init_doc]
        #[derive(::core::clone::Clone, ::core::marker::Copy)]
        #[forbid(unsafe_code)]
        #visibility struct #init_name<#lifetime> { #(#init_fields),* }

        #library::__private::with_alloc! {
            #(#conditional)*
            #(#lints)*
            /// Independent owned mirror with fallible recursive wire codecs.
            #[forbid(unsafe_code)]
            #visibility struct #owned_name { #(#owned_fields),* }
        }

        #readers
        #metadata

        #(#conditional)*
        #[forbid(unsafe_code)]
        const _: () = {
            const #fixed_count: ::core::primitive::usize =
                #library::__private::product::fixed_count::<#representation>();
            const #variable_count: ::core::primitive::usize =
                #library::__private::product::variable_count::<#representation>();
            const #directory: ::core::primitive::u32 =
                #library::__private::product::directory::<#representation>();
            const #fixed: ([#library::__private::FixedField; #fixed_count], ::core::primitive::u32) = (
                #library::__private::product::fixed::<#representation, #fixed_count>(),
                #library::__private::product::fixed_bytes::<#representation>(),
            );
            const #variable: [#library::__private::VariableField; #variable_count] =
                #library::__private::product::variable::<#representation, #variable_count>();
            const #minimum: ::core::primitive::u32 =
                #library::__private::product::minimum::<#representation>();
            const _: () = {
                let _ = #minimum;
                #library::__private::assert_schema::<#name>(#library::__private::MAX_DEPTH);
            };

            impl #library::RootSchema for #name {
                #[inline(always)]
                fn __layout() -> #library::__private::CheckedLayout<'static> {
                    #library::__layout_of!(#name)
                }
            }

            impl #library::Schema for #name {
                const __PROJECT_FIELDS: ::core::primitive::bool = true;
                const __MEASURE_REUSABLE: ::core::primitive::bool =
                    <#representation as #library::__private::product::Types>::META.markers.replay;
                const __ENCODER_AUDITED: ::core::primitive::bool =
                    <#representation as #library::__private::product::Types>::META.markers.encoder;
                const WIRE_DESCRIPTOR: &'static #library::identity::WireDescriptor =
                    &#library::identity::WireDescriptor::Struct(
                        &#library::__private::product::wire::<#representation, #field_count>());
                fn __read_fixed(view: #library::View<'_>) -> #library::Result<Self::Init<'static>> {
                    if <Self as #library::Schema>::FIXED_SIZE.is_none() {
                        return ::core::result::Result::Err(#library::ErrorKind::InvalidSchema.into());
                    }
                    ::core::result::Result::Ok(#init_name { #(#fixed_copies),* })
                }
                const KIND: #library::Kind = if #variable_count == 0 {
                    #library::Kind::Fixed
                } else {
                    #library::Kind::Variable
                };
                const FIXED_SIZE: ::core::option::Option<::core::primitive::u32> = if #variable_count == 0 {
                    ::core::option::Option::Some(#fixed.1)
                } else {
                    ::core::option::Option::None
                };
                const MIN_SIZE: ::core::primitive::u32 = #minimum;
                const LAYOUT: &'static #library::__private::Layout =
                    &#library::__private::Layout::Struct {
                        fixed_bytes: #fixed.1,
                        fixed: &#fixed.0,
                        variable: &#variable,
                    };
                const __LEN: ::core::primitive::usize =
                    <Self::__Children as #library::__private::SchemaList>::LEN + 8*(1+#field_count);
                const __HEADER: &'static [::core::primitive::u32] =
                    &#library::__private::product::header::<#representation, { 8 + 2 * #field_count }>();
                type __Children = #children;
                type Read<#lifetime> = #ref_name<#lifetime>;
                type Mut<#lifetime> = #mut_name<#lifetime>;

                fn project<#lifetime>(edit: #library::__private::Projection<#lifetime, Self>) -> Self::Mut<#lifetime> {
                    #mut_name { edit }
                }
                type Init<#lifetime> = #init_name<#lifetime>;
                type Removed = ();

                #library::__private::with_alloc! {
                    type Owned = #owned_name;

                    fn measure_owned(value: &Self::Owned) -> #library::Result<::core::primitive::u32> {
                        #library::__private::product::measure_owned::<#representation>(&#owned)
                    }

                    fn encode_owned(value: &Self::Owned, destination: &mut [::core::primitive::u8]) {
                        #library::__private::product::encode_owned::<#representation>(&#owned, destination);
                    }

                    fn decode_owned(view: #library::View<'_>) -> #library::Result<Self::Owned> {
                        let view = view;
                        view.validate()?;
                        ::core::result::Result::Ok(#owned_name { #(#owned_decodings),* })
                    }
                }

                fn read<#lifetime>(view: #library::View<#lifetime>) -> #library::Result<Self::Read<#lifetime>> {
                    let view = view;
                    ::core::result::Result::Ok(#ref_name { view, #read_storage })
                }

                fn read_removed(view: #library::View<'_>) -> #library::Result<()> {
                    <Self as #library::Schema>::read(view)?.validate()
                }

                fn measure(init: &Self::Init<'_>) -> #library::Result<::core::primitive::u32> {
                    #library::__private::product::measure::<#representation>(&#borrowed)
                }

                fn encode(init: &Self::Init<'_>, destination: &mut [::core::primitive::u8]) {
                    #library::__private::product::encode::<#representation>(&#borrowed, destination);
                }
            }
            #account
        };
    })
}
