# Changelog

## 0.1.1<!-- num:release.blubber.version -->

Released 2026-11-23.

### Smaller SBF stack frames

`blubber::edit::list_pass::stage_checked_map` and `blubber::edit::stage_bytes` are generic functions, so they compile into your program. In 0.1.0<!-- num:release.blubber.v0_1_0 -->, on SBF v3, both sat at the 4,096<!-- num:release011.frame.sbf_limit -->-byte SBF frame limit. That leaves no room when your own types or call sites add stack use.

Checked list and map edits now resolve preparation errors before handing the edit plan to the next stage. On v3, `stage_checked_map` drops from 4,096<!-- num:release011.frame.stage_checked_map.v3.before --> to 3,520<!-- num:release011.frame.stage_checked_map.v3.after --> bytes and `stage_bytes` drops from 4,096<!-- num:release011.frame.stage_bytes.v3.before --> to 4,032<!-- num:release011.frame.stage_bytes.v3.after --> bytes. On v0, `stage_checked_map` drops from 4,032<!-- num:release011.frame.stage_checked_map.v0.before --> to 3,456<!-- num:release011.frame.stage_checked_map.v0.after --> bytes. Every one of the 50<!-- num:release011.frame.checked_builds --> checked program builds in the repository now keeps at least 64<!-- num:release011.frame.headroom_floor --> bytes below the limit, and a repository build check fails if a frame comes closer or grows past its recorded size.

Compared against 0.1.0<!-- num:release.blubber.v0_1_0 -->'s code rebuilt and measured the same way, none of the 9,924<!-- num:release011.cu.compared_count --> whole-instruction CU measurements got more expensive on v0 or v3.

### Docs and publishing

- In the 0.1.0<!-- num:release.blubber.v0_1_0 --> rustdoc, the CU figures for `MapMut::try_iter` were wrong and the figures for `ListMut::get_mut` were slightly out of date. At 1,000 entries in an indexed map, `MapMut::try_iter` said the callback read pass took 158,109<!-- num:release011.rustdoc.callback_read.was --> CU instead of 233,069<!-- num:rustdoc.map_indexed.callback_read --> CU, and the cursor read pass took 176,229<!-- num:release011.rustdoc.cursor_read.was --> CU instead of 241,117<!-- num:rustdoc.map_indexed.cursor_read --> CU. It also said the iterator's own read pass took 660,767<!-- num:release011.rustdoc.iter_read_v0.was --> CU on v0 instead of 510,444<!-- num:rustdoc.map_indexed.iter_read_v0 --> CU. These figures are now generated from the benchmark tables and checked.
- `blubber`, `blubber-core` and `blubber-derive` are now published by [`.github/workflows/release.yml`](.github/workflows/release.yml) from a version tag such as `v0.1.1`<!-- num-code:"v{{num:release.blubber.version}}" --> through crates.io Trusted Publishing, with no long-lived crates.io token. Before publishing, the workflow checks each package digest against the release record committed in the repository.

### Upgrading

No public API, wire encoding or account identity changed, and `cargo-semver-checks` passes against 0.1.0<!-- num:release.blubber.v0_1_0 -->. Existing accounts read and edit as before.

```sh
cargo add blubber@0.1.1 --features pinocchio
```
<!-- num-fence:{"1": "cargo add blubber@{{num:release.blubber.version}} --features pinocchio"} -->

## 0.1.0<!-- num:release.blubber.v0_1_0 -->

Released 2026-11-21.

### What ships

`blubber`, `blubber-core` and `blubber-derive` 0.1.0<!-- num:release.blubber.v0_1_0 --> are on crates.io and docs.rs. The package check passes an external archive consumer check with the package inventory's feature selections. See [`README.md#release-status`](README.md#release-status).

- Account schemas via `#[blubber::schema]` and `Fixed`, with encodings for scalars, `Str`, `List` and `Map`. Tests check that the default account identity depends on the canonical encoding description, namespace and application-controlled epoch.
- Typed borrowed views and in-place editors over account bytes, so edits do not decode the account into a `Vec`.
- Authenticated Pinocchio lifecycle and editing helpers. You can request extra bytes beyond the current length when an edit opens with `Headroom::UpTo`. Commit settles rent on the final physical size.

### Edits and initialisation

With `alloc`, `MapMut<K, Str>::rewrite_values_with` stages replacements computed from the old strings in one caller-sized byte buffer before rewriting the map. A callback error or insufficient output buffer leaves the account bytes untouched. See [`docs/recipes.md#replace-a-collection`](docs/recipes.md#replace-a-collection).

Verus is a formal verification tool for Rust. It checks the byte-moving core and the shared initialisation payload writer under their stated premises, including trusted projected input, an exact byte measurement and a destination slot of that size for the payload writer. See [`ASSURANCE.md#encoders-and-initialization`](ASSURANCE.md#encoders-and-initialization).
