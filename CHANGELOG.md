# Changelog

## 0.1.0

Initial release candidate for `blubber-core`, `blubber-derive` and `blubber`.

- Borrowed typed readers and editors for format-1 integers, booleans, byte arrays,
  public keys, fixed structs, strings, options, lists and sorted maps.
- Schema derives with borrowed inputs, nested type/const generics and
  namespace/semantic-epoch account identity. Account roots are concrete.
- Pinocchio account guards, creation helpers, rent settlement and explicit
  commit receipts. Default headroom is `UpTo(0)`; larger budgets are explicit.
- Authenticated lazy runtime entry under valid current-schema writer history,
  checked alternatives, root `verify()` and opt-in `verify-on-load`.
- Scoped child edits, lending cursors, entry combinators and whole-container
  fill/rewrite operations, with documented partial-progress and poison rules.
- A Verus-checked byte kernel, bounded typed checks and runtime evidence whose
  scope and assumptions are documented in [ASSURANCE.md](ASSURANCE.md).

The separate `blubber-anchor` adapter is unpublished and uses an exact SDK git
revision. Its [README](crates/blubber-anchor/README.md) describes availability.
