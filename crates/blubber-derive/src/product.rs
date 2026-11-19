//! Balanced field type declarations and trusted projections.

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn types(types: &[&syn::Type], library: &TokenStream) -> TokenStream {
    if types.len() == 1 {
        let ty = types[0];
        quote!(#library::__private::product::Field<#ty>)
    } else {
        let (left, right) = types.split_at(types.len() / 2);
        let left = self::types(left, library);
        let right = self::types(right, library);
        quote!(#library::__private::Pair<#left, #right>)
    }
}

pub(crate) fn project(
    fields: &[&syn::Field],
    source: TokenStream,
    adapter: TokenStream,
    library: &TokenStream,
    copied: bool,
) -> TokenStream {
    if fields.len() == 1 {
        let ty = &fields[0].ty;
        let name = fields[0].ident.as_ref().expect("named field");
        let value = if copied {
            // Native Fixed fields are Copy. A value projection also supports
            // packed structs without ever borrowing an unaligned field.
            quote!(&{ #source.#name })
        } else {
            quote!(&#source.#name)
        };
        quote!(#library::__private::product::#adapter::<#ty>(#value))
    } else {
        let (left, right) = fields.split_at(fields.len() / 2);
        let left = project(left, source.clone(), adapter.clone(), library, copied);
        let right = project(right, source, adapter, library, copied);
        quote!(#library::__private::Pair { left: #left, right: #right })
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::visit::Visit;

    struct Delegates {
        methods: usize,
        descriptors: usize,
    }

    impl<'ast> Visit<'ast> for Delegates {
        fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
            let Some((_, path, _)) = &item.trait_ else {
                return;
            };
            let name = &path.segments.last().unwrap().ident;
            if name != "Schema" && name != "Fixed" {
                return;
            }
            for member in &item.items {
                match member {
                    syn::ImplItem::Fn(method)
                        if ["measure", "encode", "measure_owned", "encode_owned"]
                            .iter()
                            .any(|name| method.sig.ident == name) =>
                    {
                        let body = &method.block;
                        let tokens = quote!(#body).to_string();
                        assert!(!tokens.contains("split_at_mut"), "{tokens}");
                        assert!(!tokens.contains("checked_add"), "{tokens}");
                        assert!(!tokens.contains("to_le_bytes"), "{tokens}");
                        assert!(!tokens.contains("while"), "{tokens}");
                        self.methods += 1;
                    }
                    syn::ImplItem::Const(value) if value.ident == "WIRE_DESCRIPTOR" => {
                        let expr = &value.expr;
                        let tokens = quote!(#expr).to_string();
                        assert!(tokens.contains("product :: wire"), "{tokens}");
                        self.descriptors += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn derives_project_fields_and_delegate_encoding_and_metadata() {
        let library = quote!(::blubber);
        let regular = crate::schema::expand(
            quote!(),
            quote!(
                struct Record {
                    a: u32,
                    b: ::blubber::Str,
                }
            ),
            &library,
        )
        .unwrap();
        let generic = crate::schema::expand(
            quote!(),
            quote!(
                struct Record<T: ::blubber::Schema> {
                    a: T,
                    b: u32,
                }
            ),
            &library,
        )
        .unwrap();
        let fixed = crate::fixed::expand(
            &syn::parse_quote!(
                struct Native {
                    a: u32,
                    b: bool,
                }
            ),
            &library,
        )
        .unwrap();
        let mut delegates = Delegates {
            methods: 0,
            descriptors: 0,
        };
        for tokens in [regular, generic, fixed] {
            let file: syn::File = syn::parse2(tokens).unwrap();
            delegates.visit_file(&file);
        }
        assert_eq!(delegates.descriptors, 3);
        // Owned methods inside the feature macro are tested by ordinary builds.
        assert_eq!(delegates.methods, 7);
    }
}
