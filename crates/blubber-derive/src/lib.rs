//! Schema and fixed-codec macros for Blubber.
//!
//! Prefer the default macro re-exports from `blubber`. [`schema`] generates a
//! marker, borrowed initializer, reader and editor. Concrete `account` roots also
//! get identity, sizing, initialization and load/commit helpers. Nested schemas
//! support type and const generics. [`Fixed`] encodes copied native structs
//! without Rust padding; it accepts nonempty named structs with fixed fields.
//!
//! Add `blubber` as a dependency when invoking these macros directly:
//!
//! ```
//! #[blubber_derive::schema]
//! struct Position { x: u64, y: u64 }
//!
//! let input = PositionInit { x: 3, y: 4 };
//! assert_eq!(<Position as blubber::Schema>::measure(&input)?, 16);
//! # Ok::<(), blubber::Error>(())
//! ```

#![forbid(unsafe_code)]

mod account;
mod fixed;
mod generic_schema;
mod hygiene;
mod product;
mod schema;
mod schema_read;

// Const items have call-site name resolution even inside an anonymous const.
// Avoid capturing identifiers embedded in consumer field types or const exprs.
fn fresh_ident(input: &impl quote::ToTokens, base: &str) -> proc_macro2::Ident {
    fn collect(tokens: proc_macro2::TokenStream, names: &mut std::collections::BTreeSet<String>) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(ident) => {
                    use syn::ext::IdentExt;
                    names.insert(ident.unraw().to_string());
                }
                proc_macro2::TokenTree::Group(group) => collect(group.stream(), names),
                _ => {}
            }
        }
    }
    let mut names = std::collections::BTreeSet::new();
    collect(input.to_token_stream(), &mut names);
    let mut candidate = base.to_owned();
    let mut suffix = 0;
    while names.contains(&candidate) {
        suffix += 1;
        candidate = format!("{base}_{suffix}");
    }
    proc_macro2::Ident::new(&candidate, proc_macro2::Span::mixed_site())
}

fn library_path() -> syn::Result<proc_macro2::TokenStream> {
    match proc_macro_crate::crate_name("blubber") {
        // The absolute name works in both the library and its integration tests.
        Ok(proc_macro_crate::FoundCrate::Itself) => Ok(quote::quote!(::blubber)),
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            Ok(quote::quote!(::#ident))
        }
        Err(error) => Err(syn::Error::new(proc_macro2::Span::call_site(), error)),
    }
}

/// Derives packed fixed-width codecs and schema metadata for a native struct.
///
/// Accepts nonempty, named, nongeneric structs that implement `Copy`. Every
/// field must implement Blubber's `Fixed` and fixed-kind `Schema` contracts.
/// Fields encode in declaration order; native padding is not encoded.
/// Inside field macros, spell the owner type explicitly instead of `Self`.
#[proc_macro_derive(Fixed)]
pub fn derive_fixed(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    (|| {
        hygiene::fixed_self(&mut input)?;
        let library = library_path()?;
        let types = hygiene::field_types(&mut input, &library)?;
        let output = fixed::expand(&input, &library)?;
        hygiene::bindings(&input, output, types)
    })()
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Generates a schema marker, borrowed input and local checked reader types.
///
/// Accepts nonempty, named structs. Type and const generics are supported for
/// nested fields. A generic named after a generated marker or companion is
/// rejected with a rename diagnostic; consumer generic tokens stay unchanged.
/// Generic structs do not implement `RootSchema` and cannot use
/// `account`. Inside field macros, spell the owner type explicitly instead of
/// `Self`. Fields must implement `Schema`; their metadata selects the packed
/// fixed prefix and variable payloads. Concrete `account` schemas also emit
/// account constructors and a mutable root
/// session with checked reads and explicit commit. Accessor names can be
/// overridden with `#[blubber(accessor = "other_name")]`.
/// Account identity hashes the resolved canonical wire descriptor, namespace,
/// format and semantic epoch in constant evaluation. Set `namespace = "..."`
/// and `semantic_epoch = N` for a stable application contract; omission uses
/// the literal `account:<Name>` and epoch zero. Explicit `discriminator = [..]`
/// overrides retain exact bytes and require deliberate version changes.
/// Account schemas implement SDK-independent `AccountSchema` metadata. Their
/// copied constraint prefix includes fixed fields by default, up to 1,024 native
/// bytes. Use `#[blubber::schema(account, constraint_fields = false)]` to disable
/// it. `space(reserve)` checks minimum-envelope sizing without an initializer.
#[proc_macro_attribute]
pub fn schema(
    attributes: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    library_path()
        .and_then(|library| {
            let input: proc_macro2::TokenStream = input.into();
            schema::expand(attributes.into(), input, &library)
        })
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
