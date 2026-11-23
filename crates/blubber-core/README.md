# blubber-core

`blubber-core` is Blubber's allocator-free byte kernel. A byte kernel is the low-level byte code for checked layouts, validation, encoding and byte operations.

## Work with encoded bytes

This crate has `validate`, `validate_utf8` and the shared `Error` and `ErrorKind` types, which report the error kind and byte offset. `validate_utf8` checks a byte slice in constant space without allocating. See the validation example in [crate rustdoc](https://docs.rs/blubber-core/latest/blubber_core/#validate-bytes).

## Depend on `blubber` in program code

Depend on `blubber` in application code instead of calling `blubber-core` directly, because these raw byte operations have local contracts you must uphold yourself. `blubber` generates typed readers and editors that satisfy those contracts for you; see [`../blubber/README.md#what-this-crate-gives-you`](../blubber/README.md#what-this-crate-gives-you).

## Find the contracts and proofs

Verus is the contract verifier used on this crate. Verus checks the byte operations under their premises, and `validate` accepts the specified encoding. SDK behaviour, compiler lowering, custom codecs and application authority sit outside those proofs.

See [`../../SPEC.md#wire-formats`](../../SPEC.md#wire-formats) for the byte contracts and [`../../ASSURANCE.md#production-proofs-and-bounds`](../../ASSURANCE.md#production-proofs-and-bounds) for the proof scope.
