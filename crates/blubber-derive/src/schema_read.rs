//! Local reader methods and account root handles.

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{ext::IdentExt, spanned::Spanned, ItemStruct};

pub(crate) fn expand(
    item: &ItemStruct,
    library: &TokenStream,
    account: bool,
) -> syn::Result<TokenStream> {
    let lifetime = crate::hygiene::lifetime(item);
    let name = &item.ident;
    let ref_name = format_ident!("{}Ref", name.unraw());
    let mut_name = format_ident!("{}Mut", name.unraw());
    let owned_name = format_ident!("{}Owned", name.unraw());
    let visibility = &item.vis;
    let attributes = item
        .attrs
        .iter()
        .filter(|attr| {
            ["cfg", "cfg_attr", "allow", "warn", "deny", "forbid"]
                .iter()
                .any(|name| attr.path().is_ident(name))
        })
        .collect::<Vec<_>>();
    let reserved = [
        "reborrow",
        "validate",
        "commit",
        "used_len",
        "storage_len",
        "reserve",
        "shrink_to_fit",
        "to_owned",
        "try_used_len",
        "try_storage_len",
    ];
    let mut occupied = reserved
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if account {
        occupied.insert("verify".to_owned());
    }
    let mut reads = Vec::new();
    let mut mutable_reads = Vec::new();
    let mut projections = Vec::new();
    let mut offset_assertions = Vec::new();
    let mut field_contracts = Vec::new();
    let index_local = crate::fresh_ident(item, "__blubber_index");
    let offset_local = crate::fresh_ident(item, "__blubber_offset");
    for (index, field) in item.fields.iter().enumerate() {
        let ident = field.ident.as_ref().expect("named field");
        let mut accessor = None;
        for attr in field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("blubber"))
        {
            let mut found = false;
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("accessor") || accessor.is_some() {
                    return Err(meta.error("expected accessor = \"name\", without duplicates"));
                }
                let value: syn::LitStr = meta.value()?.parse()?;
                let mut name = syn::parse_str::<syn::Ident>(&value.value()).map_err(|_| {
                    syn::Error::new_spanned(&value, "accessor must be a Rust identifier")
                })?;
                name.set_span(value.span());
                accessor = Some(name);
                found = true;
                Ok(())
            })?;
            if !found {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected accessor = \"name\"",
                ));
            }
        }
        let accessor = accessor.unwrap_or_else(|| ident.clone());
        let plain = accessor.unraw().to_string();
        let try_name = format_ident!("try_{}", plain, span = accessor.span());
        for method in [
            plain.clone(),
            format!("try_{plain}"),
            format!("{plain}_mut"),
        ] {
            if !occupied.insert(method.clone()) {
                return Err(syn::Error::new_spanned(field,
                    format!("accessor collision for `{method}`; use #[blubber(accessor = \"other_name\")]")));
            }
        }
        let ty = &field.ty;
        let vis = &field.vis;
        let ordinal = index as u16;
        let docs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"))
            .collect::<Vec<_>>();
        let read_doc = format!("Reads `{}`; panics on detected corruption.", ident.unraw());
        let try_doc = format!(
            "Reads `{}`, reporting detected corruption with its account offset.",
            ident.unraw()
        );
        let receiver = if account { quote!(&self) } else { quote!(self) };
        let read_field = if item.generics.params.is_empty() {
            quote!(self.view.field_tail::<#name>(#ordinal)?)
        } else {
            quote!(self.view.field(#ordinal)?)
        };
        reads.push(quote_spanned! { field.span()=>
            #(#docs)*
            #[doc = #read_doc]
            #[allow(dead_code)]
            #[inline(always)]
            #vis fn #accessor(#receiver) -> <#ty as #library::Schema>::Read<#lifetime> {
                self.#try_name().expect("invalid Blubber field")
            }
            #[doc = #try_doc]
            #[allow(dead_code)]
            #[inline(always)]
            #vis fn #try_name(#receiver) -> #library::Result<<#ty as #library::Schema>::Read<#lifetime>> {
                #read_field.read::<#ty>()
            }
        });
        let mut_accessor = format_ident!("{}_mut", plain, span = accessor.span());
        let projection_doc = format!(
            "Exclusively projects `{}` by its declaration ordinal.",
            ident.unraw()
        );
        let preceding = item
            .fields
            .iter()
            .take(index)
            .map(|f| &f.ty)
            .collect::<Vec<_>>();
        let (fixed_position, variable_position) = if preceding.is_empty() {
            (quote!((0u16, 0u32)), quote!(0u16))
        } else {
            (
                quote!({
                    let mut #index_local = 0u16;
                    let mut #offset_local = 0u32;
                    #(if let ::core::option::Option::Some(width) = <#preceding as #library::Schema>::FIXED_SIZE {
                        #index_local += 1;
                        #offset_local += width;
                    })*
                    (#index_local, #offset_local)
                }),
                quote!({
                    let mut #index_local = 0u16;
                    #(if <#preceding as #library::Schema>::FIXED_SIZE.is_none() {
                        #index_local += 1;
                    })*
                    #index_local
                }),
            )
        };
        // Check the exact offset expression passed to the certified fast path
        // against the flattened, certified table, even for an unused accessor.
        // Generic schemas use checked field traversal and emit no const offset.
        if item.generics.params.is_empty() {
            offset_assertions.push(quote! {
                #(#attributes)*
                const _: () = {
                    if <#ty as #library::Schema>::FIXED_SIZE.is_some() {
                        let (index, offset) = #fixed_position;
                        let table = #library::__layout_of!(#name);
                        ::core::assert!(offset == table.words()[table.header(5) as ::core::primitive::usize
                            + index as ::core::primitive::usize * 8 + 2],
                            "generated const offset differs from schema-table field offset");
                    }
                };
            });
        }
        field_contracts.push(quote! {
            #(#attributes)*
            impl #library::__private::ProjectField<#ordinal> for #name {
                type Field = #ty;
                const FIXED: ::core::option::Option<(::core::primitive::u16, ::core::primitive::u32)> =
                    if <#ty as #library::Schema>::FIXED_SIZE.is_some() {
                        ::core::option::Option::Some(#fixed_position)
                    } else { ::core::option::Option::None };
                const VARIABLE: ::core::primitive::u16 = #variable_position;
                const PARENT: ::core::option::Option<#library::__private::CheckedLayout<'static>> =
                    ::core::option::Option::Some(#library::__layout_of!(#name));
            }
        });
        let projection = quote! {
            <#ty as #library::Schema>::project(self.edit.field::<#ordinal>())
        };
        projections.push(quote_spanned! { field.span()=>
            #(#docs)*
            #[doc = #projection_doc]
            ///
            /// To retain a deeper field handle, bind each intermediate Mut:
            /// `let mut child = account.child_mut(); let tags = child.tags_mut();`.
            /// `let tags = account.child_mut().tags_mut();` borrows a temporary
            /// and fails with E0716. Immediate chained edits are valid.
            #[allow(dead_code)]
            #[inline(always)]
            #vis fn #mut_accessor(&mut self) -> <#ty as #library::Schema>::Mut<'_> {
                #projection
            }
        });
        mutable_reads.push(quote_spanned! { field.span()=>
            #(#docs)*
            #[doc = #read_doc]
            #[allow(dead_code)]
            #[inline(always)]
            #vis fn #accessor(&self) -> <#ty as #library::Schema>::Read<'_> {
                self.#try_name().expect("invalid Blubber field")
            }
            #[doc = #try_doc]
            #[allow(dead_code)]
            #[inline(always)]
            #vis fn #try_name(&self) -> #library::Result<<#ty as #library::Schema>::Read<'_>> {
                self.edit.view()?.field(#ordinal)?.read::<#ty>()
            }
        });
    }
    let storage_field = account.then(
        || quote! { storage_len: ::core::primitive::u32, root_origin: ::core::primitive::bool, },
    );
    let root_verify = account.then(|| {
        quote! {
            /// Verifies the complete loaded account root, excluding spare storage.
            /// Bare account subtrees return `InvalidBoundary`; use `validate()` there.
            pub fn verify(&self) -> #library::Result<()> {
                if !self.root_origin {
                    return ::core::result::Result::Err(#library::ErrorKind::InvalidBoundary.into());
                }
                self.view.validate()
            }
        }
    });
    let lengths = account.then(|| {
        quote! {
            /// Logical account length, including the twelve-byte envelope.
            pub fn used_len(&self) -> ::core::primitive::u32 {
                12 + self.view.bytes().len() as ::core::primitive::u32
            }
            /// Loaded storage length, or envelope plus root length for a bare schema read.
            pub fn storage_len(&self) -> ::core::primitive::u32 { self.storage_len }
        }
    });
    let ref_doc = format!(
        "Copyable checked reader for `{}`; fields validate lazily.",
        name.unraw()
    );
    let lifecycle = account.then(|| {
        quote! {
                /// Verifies the complete root without clearing poison or changing state.
                /// Selected account subtrees return `InvalidBoundary`; use `validate()` there.
                pub fn verify(&self) -> #library::Result<()> { self.edit.verify() }
                /// Logical length including the envelope; panics on invalid state.
                pub fn used_len(&self) -> ::core::primitive::u32 {
                    self.try_used_len().expect("invalid Blubber account")
                }
                /// Returns the logical length after checking state and the envelope.
                pub fn try_used_len(&self) -> #library::Result<::core::primitive::u32> { self.edit.used_len() }
                /// Physical storage length; panics on invalid state.
                pub fn storage_len(&self) -> ::core::primitive::u32 {
                    self.try_storage_len().expect("invalid Blubber account")
                }
                /// Returns the physical length after checking session state.
                pub fn try_storage_len(&self) -> #library::Result<::core::primitive::u32> { self.edit.storage_len() }
                /// Ensures physical room beyond the logical account length.
                pub fn reserve(&mut self, additional: ::core::primitive::u32) -> #library::Result<()> {
                    self.edit.reserve(additional)
                }
                /// Attempts or schedules physical compaction to the logical length.
                pub fn shrink_to_fit(&mut self) -> #library::Result<::core::primitive::bool> {
                    self.edit.shrink_to_fit()
                }
                /// Consumes the root and completes its storage session.
                #[inline(always)]
                pub fn commit(self) -> #library::Result<#library::CommitInfo> { self.edit.commit() }
        }
    });
    let mut_doc = format!(
        "Exclusive selection for `{}` with short read borrows.",
        name.unraw()
    );
    let receiver = if account { quote!(&self) } else { quote!(self) };
    Ok(quote! {
        #(#offset_assertions)*
        #(#field_contracts)*
        #(#attributes)*
        #[doc = #ref_doc]
        #[derive(::core::clone::Clone, ::core::marker::Copy)]
        #[forbid(unsafe_code)]
        #visibility struct #ref_name<#lifetime> { view: #library::View<#lifetime>, #storage_field }
        #(#attributes)*
        impl<#lifetime> ::core::fmt::Debug for #ref_name<#lifetime> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(::core::stringify!(#ref_name)).finish_non_exhaustive()
            }
        }


        #(#attributes)*
        #[forbid(unsafe_code)]
        impl<#lifetime> #ref_name<#lifetime> {
            #(#reads)*
            /// Checks every boundary and leaf in this exact subtree.
            pub fn validate(#receiver) -> #library::Result<()> { self.view.validate() }
            #library::__private::with_alloc! {
                /// Validates and copies this subtree, returning allocation failures.
                #[allow(dead_code)]
                pub fn to_owned(&self) -> #library::Result<#owned_name> {
                    <#name as #library::Schema>::decode_owned(self.view)
                }
            }
            #root_verify
            #lengths
        }
        #(#attributes)*
        #[doc = #mut_doc]
        #[forbid(unsafe_code)]
        #visibility struct #mut_name<#lifetime> { edit: #library::__private::Projection<#lifetime, #name> }
        #(#attributes)*
        impl<#lifetime> ::core::fmt::Debug for #mut_name<#lifetime> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(::core::stringify!(#mut_name)).finish_non_exhaustive()
            }
        }


        #(#attributes)*
        #[forbid(unsafe_code)]
        impl<#lifetime> #mut_name<#lifetime> {
            #(#mutable_reads)*
            #(#projections)*
            /// Temporarily reborrows this selection for another edit.
            #[allow(dead_code)]
            pub fn reborrow(&mut self) -> #mut_name<'_> {
                #mut_name { edit: self.edit.reborrow() }
            }
            /// Checks every boundary and leaf in the selected subtree.
            #[allow(dead_code)]
            pub fn validate(&self) -> #library::Result<()> { self.edit.validate() }
            #library::__private::with_alloc! {
                /// Validates and copies this selection before any account mutation.
                #[allow(dead_code)]
                pub fn to_owned(&self) -> #library::Result<#owned_name> {
                    <#name as #library::Schema>::decode_owned(self.edit.view()?)
                }
            }
            #lifecycle
        }
    })
}
