# Trust model

Raw `State::load(bytes)` and host mutable loads scan the complete active
encoding. Authenticated Pinocchio loads default to lazy checks of the expected
owner, layout-bound schema identity and envelope, followed by bounds/domain
checks on touched paths. The program supplies valid initialization or migration
for the current schema, followed only by approved writers.

| API | Load policy |
| --- | --- |
| Raw `State::load`, host `State::load_mut` | Full root scan |
| Typed `ReadStore::load` / `Store::load`, scoped `with_read` / `with_edit` | Lazy |
| Checked Store loads, `load_checked_mut`, checked scoped helpers | Full root scan |
| `verify-on-load` | Adds full scans to lazy loads and initialization; opt-in |

Lazy loading does not certify untouched bool/Option domains, UTF-8, directories,
or map order. A map with invalid ordering can yield a false miss; a local edit
can leave distant invalid bytes intact. Owner authentication alone does not
establish writer history. Keep the expected owner independent of instruction
contents and check application authority separately.

Account-root `verify()` checks all boundaries, domains, UTF-8, key order and
uniqueness, excluding spare capacity. It returns the first fault at its account
byte offset. It preserves poison and creates no reusable validation evidence.
`validate()` checks a selected subtree. A bare `Schema::read(View)` without an
enclosing account envelope cannot use root `verify()`; use subtree `validate()`.

Both checked and lazy policies keep touched checks and SDK exclusion. Checked
scoped helpers scan before invoking the callback. Their result cannot borrow
the temporary session. Readonly full scans can exceed an instruction budget.
Select and measure the policy for your actual schema.

## Approved writers

Generated editors select fields through schema-bound projections. Ordinary
`Resize` is read-only; byte mutation, supplied used lengths and untyped projection
belong to the explicit `raw` boundary. Typed constructors obtain identity from
the matching account schema. These controls preserve the root only when the
starting root is valid and codecs meet their contracts.

## Explicit raw boundary

`blubber::raw` exposes untyped editing, backend mutation and rewrite helpers.
`raw::backend` obtains a storage object's raw backend. `raw::into_edit`,
`raw::project` and `raw::scalar` discard or replace schema authority explicitly.
`blubber_core::raw` provides byte moves, word writes, plans and destination
loans. Their local bounds and phase contracts do not establish typed root
preservation. Same-owner external writes and upgrades also require a writer audit.

## Custom codecs

Handwritten `Schema`, `Fixed`, `MapKey` and field mappings are implementer
boundaries. Measurement must check inputs and give the exact representable
length. Encoding must fill that region with schema-valid bytes and replay
deterministically. Map keys must encode injectively in canonical order.
Projection metadata must select the matching schema and contained bytes.

Hidden encoder markers default to false for handwritten codecs. Built-ins set
them true; derives and collections combine relevant child/key markers. Fully
covered graphs omit redundant native/fixed-child measurement replay and lazy
post-write initialization scans. Variable-child sizing remains when it selects
exact regions. Checked initialization and `verify-on-load` keep full validation.
A handwritten true marker is an implementer contract, not a proof of its implementation.

Handwritten schemas default to `Unspecified`; automatic identity generation
rejects them until they supply a stable descriptor. Use `WireDescriptor::Custom`
with a nonempty codec namespace, epoch and underlying structural/domain tree.
Handwritten map keys supply `MapKey::KEY_DESCRIPTOR` with `WireKey::codec` set
to their explicit `CodecIdentity`. A contract change requires a new codec epoch.
The declared metadata cannot prove a handwritten implementation correct.

## Reopen after a handoff or error

Raw writes and unaudited same-owner writers end approved history. After a raw
edit, external write or an upgrade, finish all byte borrows and reopen
checked with `load_checked_mut`, `Store::load_checked` or a checked
scoped helper. Preserve the invocation growth allowance across resize/CPI and
reopen. A meaning change also requires [migration](#overrides-and-migration).

Source corruption detected on a lazy mutable path sets sticky poison before
returning, including read-only checks. Input, capacity and funding failures
stay unpoisoned and precede the first write. Later patch/encoder failures can
leave changed bytes with the old published length. Propagate these errors so
the transaction rolls back. Completed edits remain after callback errors.

`Poisoned` prevents ordinary reuse and commit. Repair or roll back bytes, then
use an explicit checked mutable load. Only a successful scan clears poison;
a failed checked load preserves the old frame. `verify()` preserves it.
Poison is volatile: dropping a store loses the marker without repairing bytes.
Opening a new lazy store is not recovery. `verify-on-load` does not change that
rule. The [separate runtime adapter](../crates/blubber-anchor/README.md) documents
its checked reload and terminal-session contracts.

## Account identity

`#[blubber::schema(account)]` binds the default eight-byte identity to the
resolved wire schema. A same-name `u64` to `i64` change gets a different identity.
Aliases of the same codec contract have the same descriptor. Rust field names,
native padding and optimisation metadata do not enter the descriptor. Reordering
fields with equal codec contracts keeps the descriptor and hash unchanged.
Such a meaning change requires a `semantic_epoch` bump and conversion; renaming
fields or stamping a new identity alone does not perform that conversion.

Declare a stable namespace before persisting accounts:

```rust
#[blubber::schema(account, namespace = "example:balances", semantic_epoch = 1)]
pub struct Balances {
    pub authority: blubber::Pubkey,
    pub values: blubber::Map<u16, u64>,
}
```

Omitting these attributes selects the literal `account:<Name>` and epoch zero.
Freeze that namespace explicitly before renaming the account. A change in units
or meaning with identical wire bytes requires a new semantic epoch. The
`AccountSchema` trait and generated account expose `NAMESPACE`, `SEMANTIC_EPOCH`
and `WIRE_DESCRIPTOR_BYTES`; `Schema::WIRE_DESCRIPTOR` is the resolved tree.
These constants need neither allocation nor an SDK.

The identity is the first eight bytes of SHA-256 over the versioned canonical
descriptor, namespace and semantic epoch. Hashing runs in constant evaluation;
runtime entry compares bytes. An all-zero identity is reserved for uninitialized
storage and is rejected. An eight-byte hash can collide: owner and application
authority checks remain required. [SPEC.md](../SPEC.md) defines the descriptor,
identity preimage and account wire format.

On an SDK identity mismatch, the error remains `Discriminator` at byte zero.
Instruction return data identifies the expected and found identities as
wire-order hex bytes. Formatting runs only on error.

## Overrides and migration

`discriminator = [eight bytes]` retains exact explicit identities and opts out
of automatic layout-change detection. Keep a reviewed namespace/epoch registry
and select a new identity deliberately when wire or semantic contracts change.
The explicit-discriminator contract in `crates/blubber/tests/identity.rs`
checks exact override bytes, automatic-identity rejection and unchanged payload
bytes when an application replaces the identity.

The executable [migration example](../crates/blubber/tests/migration.rs) stages an
old/new-schema conversion, verifies the staged and destination payloads, then
publishes the new identity last. For a future change:

1. Freeze the old namespace, semantic epoch, descriptor and explicit override,
   if any. Allocate a new identity for an incompatible wire or meaning change;
   update custom-codec epochs when their contract changes.
2. Authenticate the expected owner and application authority, match the old
   identity and strictly decode the complete old root with its old schema.
   Keep that decoder on the migration path only.
3. Convert the old meaning into the new typed input. Encode the new root and
   fully verify it against the new schema before publishing its new identity
   last. Restamping unchanged bytes requires old-contract validation and does
   not convert units or meaning.
4. If one invocation cannot finish, use a new account or an explicit non-ready
   migration state that ordinary lazy loads reject. Publish Ready only after
   conversion and full verification succeed; propagate errors for rollback.
5. Use a checked load under the new identity. Resume lazy access
   only while the new approved writer history holds; follow the
   [reopen recipe](#reopen-after-a-handoff-or-error) after raw
   writes, CPI or destructive errors.

Proof scope, QA evidence and reproduction commands are in [ASSURANCE.md](../ASSURANCE.md).
