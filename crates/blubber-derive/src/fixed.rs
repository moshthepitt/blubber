//! Native fixed structs, encoded field by field without inspecting their layout.

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, Data, DeriveInput, Fields};

pub(crate) fn expand(input: &DeriveInput, library: &TokenStream) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "Fixed requires a nongeneric struct",
        ));
    }
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) if !fields.named.is_empty() => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &data.fields,
                    "Fixed requires a nonempty struct with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "Fixed requires a struct with named fields",
            ));
        }
    };
    if fields.len() > u16::MAX as usize {
        return Err(syn::Error::new_spanned(
            fields,
            "Fixed supports at most 65535 fields",
        ));
    }

    let offsets = crate::fresh_ident(input, "__BLUBBER_OFFSETS");
    let lifetime = crate::hygiene::lifetime(input);
    let name = &input.ident;
    let count = fields.len();
    let boundaries = count + 1;
    let mut decode = Vec::with_capacity(count);
    for (index, field) in fields.iter().enumerate() {
        let ty = &field.ty;
        let ident = field.ident.as_ref().expect("named field");
        let end = index + 1;
        decode.push(quote_spanned! { ty.span()=>
            #ident: <#ty as #library::Fixed>::decode(
                &bytes[#offsets[#index] as ::core::primitive::usize..#offsets[#end] as ::core::primitive::usize],
            ).map_err(|error| error.rebase(#offsets[#index]))?
        });
    }
    let field_refs = fields.iter().collect::<Vec<_>>();
    let field_types = field_refs.iter().map(|field| &field.ty).collect::<Vec<_>>();
    let representation = crate::product::types(&field_types, library);
    let projection =
        crate::product::project(&field_refs, quote!(self), quote!(native), library, true);

    let children = fields
        .iter()
        .rev()
        .fold(quote!(#library::__private::Nil), |tail, field| {
            let ty = &field.ty;
            quote!(#library::__private::Cons<#ty, #tail>)
        });
    Ok(quote! {
        // Keep generated constants out of the application's item namespace.
        #[forbid(unsafe_code)]
        const _: () = {
            const #offsets: [::core::primitive::u32; #boundaries] = {
                let () = <#representation as #library::__private::product::NativeTypes>::CHECK;
                #library::__private::product::offsets::<#representation, #boundaries>()
            };

            // Force width and depth checks even when no codec is called.
            const _: () = { let _ = #offsets; };

            impl #library::Fixed for #name {
                const __ENCODER_AUDITED: ::core::primitive::bool = <#representation as #library::__private::product::NativeTypes>::AUDITED;
                const SIZE: ::core::primitive::u32 = #offsets[#count];
                type Encoded = [::core::primitive::u8; #offsets[#count] as ::core::primitive::usize];

                fn decode(bytes: &[::core::primitive::u8]) -> #library::Result<Self> {
                    let size = <Self as #library::Fixed>::SIZE as ::core::primitive::usize;
                    if bytes.len() != size {
                        let (kind, offset) = if bytes.len() < size {
                            (#library::ErrorKind::Truncated, bytes.len())
                        } else {
                            (#library::ErrorKind::InvalidBoundary, size)
                        };
                        return ::core::result::Result::Err(#library::Error::new(
                            kind, offset as ::core::primitive::u32,
                        ));
                    }
                    ::core::result::Result::Ok(Self { #(#decode),* })
                }

                fn encode(self) -> Self::Encoded {
                    let mut destination = [0u8; #offsets[#count] as ::core::primitive::usize];
                    #library::__private::product::pack::<#representation>(&#projection, &mut destination);
                    destination
                }
            }

            impl #library::RootSchema for #name {
                #[inline(always)]
                fn __layout() -> #library::__private::CheckedLayout<'static> {
                    #library::__layout_of!(#name)
                }
            }

            impl #library::Schema for #name {
                const __ENCODER_AUDITED: ::core::primitive::bool = <Self as #library::Fixed>::__ENCODER_AUDITED;
                const __MEASURE_REUSABLE: ::core::primitive::bool = true;
                const WIRE_DESCRIPTOR: &'static #library::identity::WireDescriptor =
                    &#library::identity::WireDescriptor::Struct(
                        &#library::__private::product::wire::<#representation, #count>());
                fn __read_fixed(view: #library::View<'_>) -> #library::Result<Self::Init<'static>> {
                    view.read_fixed::<Self>()
                }
                const KIND: #library::Kind = #library::Kind::Fixed;
                const FIXED_SIZE: ::core::option::Option<::core::primitive::u32> =
                    ::core::option::Option::Some(<Self as #library::Fixed>::SIZE);
                const MIN_SIZE: ::core::primitive::u32 = <Self as #library::Fixed>::SIZE;
                const LAYOUT: &'static #library::__private::Layout =
                    &#library::__private::Layout::Struct {
                        fixed_bytes: <Self as #library::Fixed>::SIZE,
                        fixed: &#library::__private::product::fixed::<#representation, #count>(),
                        variable: &[],
                    };
                const __LEN: ::core::primitive::usize =
                    <Self::__Children as #library::__private::SchemaList>::LEN + 8*(1+#count);
                const __HEADER: &'static [::core::primitive::u32] =
                    &#library::__private::product::header::<#representation, {8+2*#count}>();
                type __Children = #children;
                type Read<#lifetime> = Self;
                type Mut<#lifetime> = #library::ScalarMut<#lifetime, Self>;

                fn project<#lifetime>(edit: #library::__private::Projection<#lifetime, Self>) -> Self::Mut<#lifetime> {
                    #library::ScalarMut::from_edit(edit)
                }
                type Init<#lifetime> = Self;
                type Removed = Self;

                #library::__private::with_alloc! {
                    type Owned = Self;
                    fn measure_owned(value: &Self) -> #library::Result<::core::primitive::u32> {
                        <Self as #library::Schema>::measure(value)
                    }
                    fn encode_owned(value: &Self, destination: &mut [::core::primitive::u8]) {
                        <Self as #library::Schema>::encode(value, destination);
                    }
                    fn decode_owned(view: #library::View<'_>) -> #library::Result<Self> {
                        <Self as #library::Schema>::read(view)
                    }
                }

                fn read<#lifetime>(view: #library::View<#lifetime>) -> #library::Result<Self> {
                    view.read_fixed::<Self>()
                }

                const __READ_REGION_NEEDS_LAYOUT: ::core::primitive::bool = false;

                #[inline(always)]
                fn read_region<#lifetime>(bytes: &#lifetime [::core::primitive::u8],
                    _layout: #library::__private::Layout, base: ::core::primitive::u32) -> #library::Result<Self> {
                    <Self as #library::Fixed>::__decode_region(bytes).map_err(|error| error.rebase(base))
                }

                fn read_removed(view: #library::View<'_>) -> #library::Result<Self> {
                    view.read_fixed::<Self>()
                }

                fn measure(_: &Self) -> #library::Result<::core::primitive::u32> {
                    ::core::result::Result::Ok(<Self as #library::Fixed>::SIZE)
                }

                fn encode(init: &Self, destination: &mut [::core::primitive::u8]) {
                    let encoded = <Self as #library::Fixed>::encode(*init);
                    destination.copy_from_slice(::core::convert::AsRef::<[::core::primitive::u8]>::as_ref(&encoded));
                }
            }
        };
    })
}
