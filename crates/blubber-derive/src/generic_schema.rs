//! Nested generic schemas use only const/type metadata; concrete parents own tables.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ext::IdentExt, parse_quote, GenericParam, ItemStruct};

pub(crate) fn expand(item: &ItemStruct, library: &TokenStream) -> syn::Result<TokenStream> {
    // Reuse the reader generator's accessor and attribute validation.
    crate::schema_read::expand(item, library, false)?;
    let lifetime = crate::hygiene::lifetime(item);
    let name = &item.ident;
    let init_name = format_ident!("{}Init", name.unraw());
    let ref_name = format_ident!("{}Ref", name.unraw());
    let mut_name = format_ident!("{}Mut", name.unraw());
    let owned_name = format_ident!("{}Owned", name.unraw());
    let vis = &item.vis;
    let attrs = &item.attrs;
    let mut generics = item.generics.clone();
    let mut args = Vec::new();
    let mut markers = Vec::new();
    for param in &mut generics.params {
        match param {
            GenericParam::Type(param) => {
                param.bounds.push(parse_quote!('static));
                let id = &param.ident;
                args.push(quote!(#id));
                markers.push(quote!(#id));
            }
            GenericParam::Const(param) => {
                let id = &param.ident;
                args.push(quote!(#id));
            }
            GenericParam::Lifetime(param) => {
                return Err(syn::Error::new_spanned(
                    param,
                    "schema markers require static types; use type or const parameters",
                ))
            }
        }
    }
    // Field projections target this marker; its declaration must not require them.
    let mut marker_generics = generics.clone();
    marker_generics.where_clause = None;
    for field in &item.fields {
        let ty = &field.ty;
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: #library::Schema));
    }
    let mut viewed = generics.clone();
    viewed.params.insert(0, parse_quote!(#lifetime));
    let (ig, tg, wc) = generics.split_for_impl();
    let (vig, vtg, vwc) = viewed.split_for_impl();
    let mut init_copy = viewed.clone();
    for field in &item.fields {
        let ty = &field.ty;
        init_copy
            .make_where_clause()
            .predicates
            .push(parse_quote!(<#ty as #library::Schema>::Init<#lifetime>: ::core::marker::Copy));
    }
    let (cig, ctg, cwc) = init_copy.split_for_impl();
    let count = item.fields.len();
    let mut init_fields = Vec::new();
    let mut owned_fields = Vec::new();
    let mut decode_owned = Vec::new();
    let mut reads = Vec::new();
    let mut mut_reads = Vec::new();
    let mut projections = Vec::new();
    let mut field_contracts = Vec::new();
    let mut fixed_copies = Vec::new();
    let children =
        item.fields
            .iter()
            .rev()
            .fold(quote!(#library::__private::Nil), |tail, field| {
                let ty = &field.ty;
                quote!(#library::__private::Cons<#ty,#tail>)
            });
    for (index, field) in item.fields.iter().enumerate() {
        let id = field.ident.as_ref().expect("named field");
        let ty = &field.ty;
        let fv = &field.vis;
        let ord = index as u16;
        fixed_copies.push(quote! {
            #id: <#ty as #library::Schema>::__read_fixed(view.field(#ord)?)?
        });
        let mut accessor = id.clone();
        for attr in field.attrs.iter().filter(|a| a.path().is_ident("blubber")) {
            attr.parse_nested_meta(|meta| {
                let s: syn::LitStr = meta.value()?.parse()?;
                accessor = syn::parse_str(&s.value())?;
                Ok(())
            })?;
        }
        let try_name = format_ident!("try_{}", accessor.unraw());
        let mut_accessor = format_ident!("{}_mut", accessor.unraw());
        let fa = field
            .attrs
            .iter()
            .filter(|a| !a.path().is_ident("blubber"))
            .collect::<Vec<_>>();
        if fa
            .iter()
            .any(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        {
            return Err(syn::Error::new_spanned(
                field,
                "conditional schema fields are unsupported",
            ));
        }
        init_fields.push(quote!(#(#fa)* #[doc = "Borrowed field input."] #fv #id: <#ty as #library::Schema>::Init<#lifetime>));
        owned_fields.push(
            quote!(#(#fa)* #[doc = "Owned field value."] #fv #id: <#ty as #library::Schema>::Owned),
        );
        decode_owned.push(quote!(#id:<#ty as #library::Schema>::decode_owned(view.field(#ord)?)?));
        reads.push(quote! {
            /// Reads this field, panicking on detected corruption.
            #fv fn #accessor(self)-><#ty as #library::Schema>::Read<#lifetime> { self.#try_name().expect("invalid Blubber field") }
            /// Reads this field with an account-relative error.
            #fv fn #try_name(self)->#library::Result<<#ty as #library::Schema>::Read<#lifetime>> { self.view.field(#ord)?.read::<#ty>() }
        });
        mut_reads.push(quote! {
            /// Reads this field, panicking on detected corruption.
            #fv fn #accessor(&self)-><#ty as #library::Schema>::Read<'_> { self.#try_name().expect("invalid Blubber field") }
            /// Reads this field with an account-relative error.
            #fv fn #try_name(&self)->#library::Result<<#ty as #library::Schema>::Read<'_>> { self.edit.view()?.field(#ord)?.read::<#ty>() }
        });
        field_contracts.push(quote! {
            impl #ig #library::__private::ProjectField<#ord> for #name #tg #wc {
                type Field = #ty;
                const FIXED: ::core::option::Option<(::core::primitive::u16, ::core::primitive::u32)> = ::core::option::Option::None;
                const VARIABLE: ::core::primitive::u16 = 0;
                const GENERIC: ::core::primitive::bool = true;
            }
        });
        projections.push(quote! {
            /// Reborrows the selected field for editing.
            #fv fn #mut_accessor(&mut self)-><#ty as #library::Schema>::Mut<'_> {
                <#ty as #library::Schema>::project(self.edit.field::<#ord>())
            }
        });
    }
    let field_refs = item.fields.iter().collect::<Vec<_>>();
    let field_types = field_refs.iter().map(|field| &field.ty).collect::<Vec<_>>();
    let representation = crate::product::types(&field_types, library);
    let borrowed =
        crate::product::project(&field_refs, quote!(init), quote!(borrowed), library, false);
    let owned = crate::product::project(&field_refs, quote!(value), quote!(owned), library, false);
    Ok(quote! {
        #(#field_contracts)*
        #(#attrs)*
        #[doc = "Nested generic schema marker. Use a concrete containing schema as a root."]
        #[forbid(unsafe_code)]
        #vis struct #name #marker_generics { marker: ::core::marker::PhantomData<fn()->(#(#markers,)*)> }
        /// Borrowed generic schema input.
        #[forbid(unsafe_code)]
        #vis struct #init_name #viewed #vwc { #(#init_fields),* }
        impl #cig ::core::marker::Copy for #init_name #ctg #cwc {}
        impl #cig ::core::clone::Clone for #init_name #ctg #cwc { fn clone(&self)->Self { *self } }
        impl #ig #name #tg #wc {
            const __BLUBBER_NATIVE_FIXED: [#library::__private::FixedField; #count] =
                #library::__private::product::fixed::<#representation, #count>();
            const __BLUBBER_NATIVE_VARIABLE: [#library::__private::VariableField; #count] =
                #library::__private::product::variable::<#representation, #count>();
        }
        /// Checked generic subtree reader.
        #[forbid(unsafe_code)]
        #vis struct #ref_name #viewed #vwc { view:#library::View<#lifetime>, marker: ::core::marker::PhantomData<fn()->#name #tg> }
        impl #vig ::core::fmt::Debug for #ref_name #vtg #vwc {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(::core::stringify!(#ref_name)).finish_non_exhaustive()
            }
        }

        impl #vig ::core::marker::Copy for #ref_name #vtg #vwc {}
        impl #vig ::core::clone::Clone for #ref_name #vtg #vwc { fn clone(&self)->Self { *self } }
        impl #vig #ref_name #vtg #vwc {
            #(#reads)*
            /// Validates this complete subtree.
            pub fn validate(self)->#library::Result<()> { self.view.validate() }
            #library::__private::with_alloc! {
                /// Validates and copies this complete subtree.
                pub fn to_owned(self)->#library::Result<#owned_name #tg> { <#name #tg as #library::Schema>::decode_owned(self.view) }
            }
        }
        /// Exclusive generic subtree selection.
        #[forbid(unsafe_code)]
        #vis struct #mut_name #viewed #vwc { edit:#library::__private::Projection<#lifetime, #name #tg>, marker: ::core::marker::PhantomData<fn()->#name #tg> }
        impl #vig ::core::fmt::Debug for #mut_name #vtg #vwc {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(::core::stringify!(#mut_name)).finish_non_exhaustive()
            }
        }

        impl #vig #mut_name #vtg #vwc {
            #(#mut_reads)* #(#projections)*
            /// Temporarily reborrows this subtree.
            pub fn reborrow(&mut self)->#mut_name<'_,#(#args),*> { #mut_name { edit:self.edit.reborrow(),marker: ::core::marker::PhantomData } }
            /// Validates this complete subtree.
            pub fn validate(&self)->#library::Result<()> { self.edit.validate() }
            #library::__private::with_alloc! {
                /// Validates and copies this complete subtree.
                pub fn to_owned(&self)->#library::Result<#owned_name #tg> { <#name #tg as #library::Schema>::decode_owned(self.edit.view()?) }
            }
        }
        #library::__private::with_alloc! {
            /// Independent generic owned mirror.
            #[forbid(unsafe_code)]
            #vis struct #owned_name #generics #wc { #(#owned_fields),* }
        }
        impl #ig #library::Schema for #name #tg #wc {
            const __PROJECT_FIELDS: ::core::primitive::bool = true;
            const __ENCODER_AUDITED: ::core::primitive::bool =
                <#representation as #library::__private::product::Types>::META.markers.encoder;
            const __MEASURE_REUSABLE: ::core::primitive::bool =
                <#representation as #library::__private::product::Types>::META.markers.replay;
                const WIRE_DESCRIPTOR: &'static #library::identity::WireDescriptor =
                    &#library::identity::WireDescriptor::Struct(
                        &#library::__private::product::wire::<#representation, #count>());
            fn __read_fixed(view: #library::View<'_>) -> #library::Result<Self::Init<'static>> {
                if <Self as #library::Schema>::FIXED_SIZE.is_none() {
                    return ::core::result::Result::Err(#library::ErrorKind::InvalidSchema.into());
                }
                ::core::result::Result::Ok(#init_name { #(#fixed_copies),* })
            }
            const __LEN: ::core::primitive::usize = <Self::__Children as #library::__private::SchemaList>::LEN+8*(1+#count);
            const __HEADER: &'static [::core::primitive::u32] =
                &#library::__private::product::header::<#representation, {8+2*#count}>();
            type __Children=#children;
            const KIND:#library::Kind=if Self::__HEADER[4]==0 { #library::Kind::Fixed } else { #library::Kind::Variable };
            const FIXED_SIZE: ::core::option::Option<::core::primitive::u32>=if Self::__HEADER[4]==0 { ::core::option::Option::Some(Self::__HEADER[1]) } else { ::core::option::Option::None };
            const MIN_SIZE: ::core::primitive::u32 = #library::__private::product::minimum::<#representation>();
            // Recursive metadata checks nested widths, domains and depth at compile time.
            const LAYOUT: &'static #library::__private::Layout=&#library::__private::Layout::Struct {
                fixed_bytes:Self::__HEADER[1],
                fixed: (&Self::__BLUBBER_NATIVE_FIXED).split_at(Self::__HEADER[3] as ::core::primitive::usize).0,
                variable: (&Self::__BLUBBER_NATIVE_VARIABLE).split_at(Self::__HEADER[4] as ::core::primitive::usize).0,
            };
            type Read<#lifetime>=#ref_name<#lifetime,#(#args),*>;
            type Mut<#lifetime>=#mut_name<#lifetime,#(#args),*>;
            type Init<#lifetime>=#init_name<#lifetime,#(#args),*>;
            type Removed=();
            fn project<#lifetime>(edit:#library::__private::Projection<#lifetime, Self>)->Self::Mut<#lifetime> { #mut_name {edit,marker: ::core::marker::PhantomData} }
            fn read<#lifetime>(view:#library::View<#lifetime>)->#library::Result<Self::Read<#lifetime>> { ::core::result::Result::Ok(#ref_name { view,marker: ::core::marker::PhantomData }) }
            fn read_removed(view:#library::View<'_>)->#library::Result<()> { view.validate() }
            fn measure(init:&Self::Init<'_>)->#library::Result<::core::primitive::u32> {
                #library::__private::check_schema_depth::<Self>()?;
                #library::__private::product::measure::<#representation>(&#borrowed)
            }
            fn encode(init:&Self::Init<'_>,destination:&mut [::core::primitive::u8]) {
                #library::__private::product::encode::<#representation>(&#borrowed, destination);
            }
            #library::__private::with_alloc! {
                type Owned=#owned_name #tg;
                fn measure_owned(value:&Self::Owned)->#library::Result<::core::primitive::u32> {
                    #library::__private::check_schema_depth::<Self>()?;
                    #library::__private::product::measure_owned::<#representation>(&#owned)
                }
                fn encode_owned(value:&Self::Owned,destination:&mut [::core::primitive::u8]) {
                    #library::__private::product::encode_owned::<#representation>(&#owned, destination);
                }
                fn decode_owned(view:#library::View<'_>)->#library::Result<Self::Owned> {
                    view.validate()?; ::core::result::Result::Ok(#owned_name { #(#decode_owned),* })
                }
            }
        }
    })
}
