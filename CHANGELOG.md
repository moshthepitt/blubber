# Changelog

## 0.1.0<!-- num:release.blubber.version -->

Released 2026-11-21.

### What ships

`blubber`, `blubber-core` and `blubber-derive` 0.1.0<!-- num:release.blubber.version --> are on crates.io and docs.rs. The package check passes an external archive consumer check with the package inventory's feature selections. See [`README.md#release-status`](README.md#release-status).

- Account schemas via `#[blubber::schema]` and `Fixed`, with encodings for scalars, `Str`, `List` and `Map`. Tests check that the default account identity depends on the canonical encoding description, namespace and application-controlled epoch.
- Typed borrowed views and in-place editors over account bytes, so edits do not decode the account into a `Vec`.
- Authenticated Pinocchio lifecycle and editing helpers. You can request extra bytes beyond the current length when an edit opens with `Headroom::UpTo`. Commit settles rent on the final physical size.

### Edits and initialisation

With `alloc`, `MapMut<K, Str>::rewrite_values_with` stages replacements computed from the old strings in one caller-sized byte buffer before rewriting the map. A callback error or insufficient output buffer leaves the account bytes untouched. See [`docs/recipes.md#replace-a-collection`](docs/recipes.md#replace-a-collection).

Verus is a formal verification tool for Rust. It checks the byte-moving core and the shared initialisation payload writer under their stated premises, including trusted projected input, an exact byte measurement and a destination slot of that size for the payload writer. See [`ASSURANCE.md#encoders-and-initialization`](ASSURANCE.md#encoders-and-initialization).
