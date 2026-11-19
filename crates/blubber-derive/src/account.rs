//! Account metadata and semantically selected, owned fixed-prefix fields.

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{ext::IdentExt, spanned::Spanned, ItemStruct};

pub(crate) fn expand(
    item: &ItemStruct,
    library: &TokenStream,
    override_bytes: Option<[u8; 8]>,
    constraint_fields: bool,
) -> TokenStream {
    let name = &item.ident;
    let vis = &item.vis;
    let prefix = format_ident!("{}Prefix", name.unraw());
    let conditional = item
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect::<Vec<_>>();
    let lints = item
        .attrs
        .iter()
        .filter(|a| {
            ["allow", "warn", "deny", "forbid"]
                .iter()
                .any(|name| a.path().is_ident(name))
        })
        .collect::<Vec<_>>();
    let override_value = match override_bytes {
        Some(bytes) => quote!(::core::option::Option::Some([#(#bytes),*])),
        None => quote!(::core::option::Option::None),
    };
    let mut nodes = Vec::new();
    let mut copies = Vec::new();
    let mut tail = quote!(());
    let tail_field = crate::fresh_ident(item, "__blubber_prefix_tail");
    if constraint_fields {
        for (index, field) in item.fields.iter().enumerate().rev() {
            let id = field.ident.as_ref().expect("named field");
            let ty = &field.ty;
            let fv = &field.vis;
            let ordinal = index as u16;
            let node = format_ident!("__BlubberPrefixField{index}");
            let selected = format_ident!("__BlubberPrefixTail{index}");
            let value = quote!(<#ty as #library::Schema>::Init<'static>);
            let choice = quote!(#library::__private::PrefixChoice<{
                <#ty as #library::Schema>::FIXED_SIZE.is_some()
            }>);
            let docs = field.attrs.iter().filter(|a| a.path().is_ident("doc"));
            nodes.push(quote_spanned! { field.span()=>
                #[doc(hidden)]
                #[allow(dead_code)]
                #[derive(::core::clone::Clone, ::core::marker::Copy)]
                pub struct #node<Value: ::core::marker::Copy, Tail: ::core::marker::Copy> {
                    #(#docs)*
                    #fv #id: Value,
                    #tail_field: Tail,
                }
                impl<Value: ::core::marker::Copy, Tail: ::core::marker::Copy>
                    ::core::ops::Deref for #node<Value, Tail> {
                    type Target = Tail;
                    fn deref(&self) -> &Tail { &self.#tail_field }
                }
                type #selected = <#choice as #library::__private::SelectPrefix<
                    #node<#value, #tail>, #tail
                >>::Output;
            });
            copies.push(quote_spanned! { field.span()=>
                let tail = <#choice as #library::__private::SelectPrefix<
                    #node<#value, #tail>, #tail
                >>::select(tail, |tail| {
                    ::core::result::Result::Ok(#node {
                        #id: <#ty as #library::Schema>::__read_fixed(view.__account_fixed_field::<Self>(#ordinal)?)?,
                        #tail_field: tail,
                    })
                })?;
            });
            tail = quote!(#selected);
        }
    }
    let read = if constraint_fields {
        quote! {
            let tail = ();
            #(#copies)*
            ::core::result::Result::Ok(tail)
        }
    } else {
        quote! { let _ = view; ::core::result::Result::Ok(()) }
    };
    quote! {
        #(#conditional)*
        #(#lints)*
        /// Owned fixed fields available to SDK account constraints.
        #vis type #prefix = <#name as #library::AccountSchema>::Prefix;

        #(#conditional)*
        #(#lints)*
        #[forbid(unsafe_code)]
        const _: () = {
            #(#nodes)*
            impl #library::AccountSchema for #name {
                const DISCRIMINATOR: [::core::primitive::u8; 8] = Self::DISCRIMINATOR;
                const DISCRIMINATOR_OVERRIDE: ::core::option::Option<[::core::primitive::u8; 8]> = #override_value;
                const NAMESPACE: &'static ::core::primitive::str = Self::NAMESPACE;
                const SEMANTIC_EPOCH: ::core::primitive::u32 = Self::SEMANTIC_EPOCH;
                const WIRE_DESCRIPTOR_BYTES: &'static [::core::primitive::u8] = Self::WIRE_DESCRIPTOR_BYTES;
                const MIN_ACCOUNT_LEN: ::core::primitive::u32 = Self::MIN_ACCOUNT_LEN;
                type Prefix = #tail;
                #[inline(always)]
                fn __read_account(bytes: &[::core::primitive::u8], checked: ::core::primitive::bool) -> #library::Result<Self::Read<'_>> {
                    if checked { Self::load(bytes) } else { Self::load_trusted(bytes) }
                }
                #[inline(always)]
                fn read_prefix(view: #library::View<'_>) -> #library::Result<Self::Prefix> {
                    #read
                }
            }
            const _: () = ::core::assert!(
                ::core::mem::size_of::<<#name as #library::AccountSchema>::Prefix>()
                    <= #library::MAX_CONSTRAINT_PREFIX_SIZE,
                "copied constraint prefix exceeds 1024 bytes; use #[blubber::schema(account, constraint_fields = false)]"
            );
        };
    }
}
