//! Procedural macros for [`blubber`](https://docs.rs/blubber/0.1.0/blubber/). [`#[schema]`][macro@schema] generates initializers, borrowed views and editors for fixed- and variable-size records. [`#[derive(Fixed)]`][Fixed] generates packed fixed-width codecs without native Rust padding.
//!
//! ## Invoke the macros directly
//!
//! Most programs use the default `derive` feature on `blubber` and write `#[blubber::schema]`. If you depend on `blubber-derive` directly, also depend on `blubber`, because the expanded code resolves `Schema` and `Schema::measure` through that dependency.
//!
//! ```
//! // docs:start direct-schema
//! #[blubber_derive::schema]
//! struct Position { x: u64, y: u64 }
//!
//! let input = PositionInit { x: 3, y: 4 };
//! assert_eq!(<Position as blubber::Schema>::measure(&input)?, 16);
//! // docs:end direct-schema
//! # Ok::<(), blubber::Error>(())
//! ```
//!
//! See the [crate README](https://github.com/moshthepitt/blubber/blob/main/crates/blubber-derive/README.md#choose-a-macro).

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

/// Generates the schema implementation, initializer, borrowed reader and exclusive editor for a struct.
///
/// A schema describes the fields and how to encode them. Use `#[blubber::schema]` on a nested schema, which implements `Schema` and may take type and const generics. Concrete schemas also implement `RootSchema`. Use `#[blubber::schema(account)]` on a concrete account root to also implement `AccountSchema`, compute its canonical identity by default and generate `space`, `encoded_len_of`, `initialize` and `load`.
///
/// For an account root `Queue`, the macro generates `QueueInit`, `QueueRef`, `QueueMut` and `QueuePrefix`, plus `QueueOwned` when `alloc` is enabled on `blubber`. Enabling `pinocchio` on `blubber` adds the on-chain `create`, `create_pda`, `allocate_account`, `allocate_pda`, `with_read` and `with_edit` methods in the tables below. On docs.rs, [`blubber::example_generated`](https://docs.rs/blubber/0.1.0/blubber/example_generated/index.html) shows the full expansion for `Queue`; that module is built only for documentation. See the [guide](https://github.com/moshthepitt/blubber/blob/main/docs/guide.md#define-a-schema).
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
///
/// # Generated account API
///
/// The [example expansion](https://docs.rs/blubber/latest/blubber/example_generated/index.html) uses
/// a concrete account with two public fields, `authority: Address` and
/// `values: List<u64>`. Each field gets a reader, a fallible reader and a
/// mutable selector. The marker implements `Schema`, `RootSchema` and
/// `AccountSchema`, which describe its wire layout and account metadata.
///
/// ## Types and fields
///
/// | Generated item | Purpose |
/// | --- | --- |
/// | [`Schema`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#impl-Schema-for-Queue) | Defines the wire layout, projections and codecs. |
/// | [`RootSchema`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#impl-RootSchema-for-Queue) | Provides the layout for a concrete root. |
/// | [`AccountSchema`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#impl-AccountSchema-for-Queue) | Provides account identity and fixed-prefix reading. |
/// | [`Queue`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html) | Schema marker and account entry points. |
/// | [`QueueInit`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueInit.html) | Borrowed initializer with one field per schema field. |
/// | [`QueueRef`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html) | Copyable reader over borrowed bytes. |
/// | [`QueueMut`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html) | Exclusive editor over account storage. |
/// | [`QueueOwned`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueOwned.html) | Independent owned fields with `alloc`. |
/// | [`QueuePrefix`](https://docs.rs/blubber/latest/blubber/example_generated/type.QueuePrefix.html) | Copied fixed fields available to account constraints. |
/// | [`QueueInit.authority`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueInit.html#structfield.authority) | Public key copied into the new queue. |
/// | [`QueueInit.values`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueInit.html#structfield.values) | Borrowed slice of initial values. |
/// | [`QueueOwned.authority`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueOwned.html#structfield.authority) | Independent copy of the public key. |
/// | [`QueueOwned.values`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueOwned.html#structfield.values) | Owned vector of the queue values. |
///
/// ## Identity and sizing
///
/// | Generated item | Purpose |
/// | --- | --- |
/// | [`Queue::DISCRIMINATOR`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#associatedconstant.DISCRIMINATOR) | Eight-byte account identity. |
/// | [`Queue::NAMESPACE`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#associatedconstant.NAMESPACE) | Namespace used for account identity. |
/// | [`Queue::SEMANTIC_EPOCH`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#associatedconstant.SEMANTIC_EPOCH) | Application version used for account identity. |
/// | [`Queue::WIRE_DESCRIPTOR_BYTES`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#associatedconstant.WIRE_DESCRIPTOR_BYTES) | Canonical wire descriptor. |
/// | [`Queue::MIN_ACCOUNT_LEN`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#associatedconstant.MIN_ACCOUNT_LEN) | Minimum encoded size including the envelope. |
/// | [`Queue::space`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.space) | Add spare capacity to the minimum size. |
/// | [`Queue::encoded_len_of`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.encoded_len_of) | Measure and validate a specific initializer. |
///
/// ## Storage entry points
///
/// | Generated item | Purpose |
/// | --- | --- |
/// | [`Queue::initialize`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.initialize) | Write an initializer to zero-identity storage. |
/// | [`Queue::load`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.load) | Read bytes after validating the complete encoding. |
/// | [`Queue::load_trusted`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.load_trusted) | Read bytes lazily under the caller's current-schema writer obligation. |
/// | [`Queue::load_mut`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.load_mut) | Edit storage with the backend's default load policy. |
/// | [`Queue::load_checked_mut`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.load_checked_mut) | Validate and open an editor, recovering valid poisoned storage. |
/// | [`Queue::load_trusted_mut`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.load_trusted_mut) | Open a lazy editor under the caller's current-schema writer obligation. |
///
/// ## On-chain account helpers (with `pinocchio`)
///
/// | Generated item | Purpose |
/// | --- | --- |
/// | [`Queue::create`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.create) | Create and initialize a signer-provided account with exact measured space. |
/// | [`Queue::create_pda`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.create_pda) | Create and initialize a program-derived account using seeds and bump. |
/// | [`Queue::allocate_account`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.allocate_account) | Allocate and initialize a prefunded signer-provided account. |
/// | [`Queue::allocate_pda`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.allocate_pda) | Allocate and initialize a prefunded program-derived account. |
/// | [`Queue::with_read`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.with_read) | Authenticate and read lazily within a shared account borrow. |
/// | [`Queue::with_read_checked`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.with_read_checked) | Validate the complete account before the read callback. |
/// | [`Queue::with_edit`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.with_edit) | Edit lazily with default headroom and commit on callback success. |
/// | [`Queue::with_edit_options`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.with_edit_options) | Edit lazily with explicit growth, funding and refund options. |
/// | [`Queue::with_edit_checked`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.with_edit_checked) | Validate the complete account before editing with default headroom. |
/// | [`Queue::with_edit_checked_options`](https://docs.rs/blubber/latest/blubber/example_generated/struct.Queue.html#method.with_edit_checked_options) | Validate the complete account before editing with explicit options. |
///
/// ## Reader methods
///
/// | Generated item | Purpose |
/// | --- | --- |
/// | [`QueueRef::authority`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.authority) | Read the public key, panicking on invalid bytes. |
/// | [`QueueRef::try_authority`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.try_authority) | Read the public key with error propagation. |
/// | [`QueueRef::values`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.values) | Read the borrowed values list, panicking on invalid bytes. |
/// | [`QueueRef::try_values`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.try_values) | Read the borrowed values list with error propagation. |
/// | [`QueueRef::validate`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.validate) | Check the selected subtree. |
/// | [`QueueRef::verify`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.verify) | Check the complete active account encoding. |
/// | [`QueueRef::used_len`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.used_len) | Report the logical size including the envelope. |
/// | [`QueueRef::storage_len`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.storage_len) | Report the loaded storage length. |
/// | [`QueueRef::to_owned`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueRef.html#method.to_owned) | Validate and copy the selected subtree with `alloc`. |
///
/// ## Editor methods
///
/// | Generated item | Purpose |
/// | --- | --- |
/// | [`QueueMut::authority`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.authority) | Read the current public key, panicking on invalid state. |
/// | [`QueueMut::try_authority`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.try_authority) | Read the current public key with error propagation. |
/// | [`QueueMut::authority_mut`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.authority_mut) | Borrow the public key editor. |
/// | [`QueueMut::values`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.values) | Read the current values list, panicking on invalid state. |
/// | [`QueueMut::try_values`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.try_values) | Read the current values list with error propagation. |
/// | [`QueueMut::values_mut`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.values_mut) | Borrow the list editor. |
/// | [`QueueMut::reborrow`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.reborrow) | Open a shorter exclusive borrow. |
/// | [`QueueMut::validate`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.validate) | Check the selected subtree. |
/// | [`QueueMut::verify`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.verify) | Check the active account encoding without changing state. |
/// | [`QueueMut::used_len`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.used_len) | Report logical size, panicking on invalid state. |
/// | [`QueueMut::try_used_len`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.try_used_len) | Report logical size after checking the session and envelope. |
/// | [`QueueMut::storage_len`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.storage_len) | Report physical size, panicking on invalid state. |
/// | [`QueueMut::try_storage_len`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.try_storage_len) | Report physical size after checking the session. |
/// | [`QueueMut::reserve`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.reserve) | Ensure physical room beyond the logical size. |
/// | [`QueueMut::shrink_to_fit`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.shrink_to_fit) | Attempt or schedule compaction to the logical size. |
/// | [`QueueMut::commit`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.commit) | Finish the exclusive storage session and return its receipt. |
/// | [`QueueMut::to_owned`](https://docs.rs/blubber/latest/blubber/example_generated/struct.QueueMut.html#method.to_owned) | Validate and copy the selected subtree with `alloc`. |
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
