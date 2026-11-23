# `blubber-derive`

Procedural macros for Blubber.

## Choose a macro

A schema describes your fields and how to encode them: an account root covers the whole account payload, while a nested schema covers an item or field. An initializer is the input for writing a new value and can borrow strings and list items from the caller.

- **`#[blubber::schema]`** - an attribute macro for nonempty named structs whose fields implement `Schema`. It generates the initializer, reader and editor types. Pass `#[blubber::schema(account)]` on a concrete struct for an account root implementing `AccountSchema`, or omit `account` for a nested schema (which may use generics).
- **`#[derive(Fixed)]`** - a derive macro for nonempty named nongeneric `Copy` structs whose fields implement `Fixed` and fixed-kind `Schema`. It packs fields in declaration order without native padding, so you can store the struct as a fixed-width record or map value.

See the [guide](../../docs/guide.md#define-a-schema) for field types and account identity.

## Depend on `blubber` or invoke the macros directly

Depend on [`blubber`](../blubber/README.md#install) with its default `derive` feature, because it re-exports `schema` and `Fixed` alongside the traits their expanded code uses.

If you invoke `blubber_derive` directly, you must still depend on `blubber`, because macro expansion resolves `blubber` as its companion crate. See the example in the [crate rustdoc](https://docs.rs/blubber-derive/latest/blubber_derive/#invoke-the-macros-directly).

## Find generated methods

See the generated item table on the [`schema` macro page](https://docs.rs/blubber-derive/latest/blubber_derive/attr.schema.html#generated-account-api) and the rendered [`blubber::example_generated`](https://docs.rs/blubber/latest/blubber/example_generated/index.html) module on docs.rs.
