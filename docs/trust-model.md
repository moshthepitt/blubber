# Trust model

When you open an account in a handler, you choose how much of it to check and what to trust about the programs that wrote it before you. If you pick the wrong load, or keep reading after a failure, bad bytes can hide in untouched fields or survive inside storage.

For the wire contract and identity hashes, read [`SPEC.md`](../SPEC.md). For the Verus proofs and test scope, read [`ASSURANCE.md`](../ASSURANCE.md).

## Choose a load

A load creates a typed reader or editor over existing bytes. Reopen means loading the account again after ending an earlier borrow or CPI handoff.

A lazy load authenticates the runtime account and checks its header up front, then checks fields as you read or edit them. A checked load scans the complete used encoding against the schema before giving you the view or calling your edit closure. An account has an approved writer history when your program initialised it or migrated it under the exact current schema, and every writer since then has used the typed editor. A migration must be followed by a separate checked reopen. Raw means byte-level access outside the normal typed editor.

Use a lazy load when your program created or migrated the account under the current schema and only approved writers have touched it since. Use a checked load after an unknown writer, a raw write, or a CPI handoff that could modify the account. A default lazy load leaves untouched fields uninspected. It can miss a key in an unsorted map or follow corrupt directory offsets into sibling content.

| Account history and entry point | What to call | What it checks |
| --- | --- | --- |
| Known valid initialisation or migration under this schema, followed only by approved writers | `with_read`, `with_edit`, `with_edit_options` | Runtime owner, non-executable state, write permission for edits, identity header, and borrow exclusion up front. Checks fields as you touch them. |
| Unknown writer, raw write, recovery after corruption, or CPI handoff that could write outside that history | `with_read_checked`, `with_edit_checked`, `with_edit_checked_options`, `ReadStore::load_checked`, `Store::load_checked` | Everything above, plus a full scan of the complete used encoding before your code runs. |
| Raw byte slice or host store (`&[u8]` or `&mut dyn Resize`) | `T::load`, `View::load`, `T::load_mut`, `T::load_checked_mut` | Full scan of the active root before returning. |
| Explicit trusted byte slice or host store where you vouch for writer history | `T::load_trusted`, `T::load_trusted_mut` | Header only by default. You carry the obligation that the bytes match the schema and come from approved writers. |

In raw byte APIs (`load_trusted`, `load_trusted_mut`) and benchmark CSVs, `trusted` means a lazy load where the caller carries this writer-history obligation.

You can also scan on demand. Calling `verify()` on the account root scans the full root, and calling `validate()` on a selected child scans that subtree. Enabling `verify-on-load` adds a full scan to lazy loads and initialisation, independent of debug assertions. See [`SPEC.md#loads-and-writer-history`](../SPEC.md#loads-and-writer-history).

## Check who may write

The Solana account owner controls writes. The runtime program ID supplies the expected owner for your program's state, and your application checks who may act.

Owner, identity, byte validity, and application authority answer four different questions:

- **Owner** - which program may modify the account's data. Pass the expected owner from a trusted constant or the authenticated runtime `program_id`, never from untrusted instruction data.
- **Identity** - which schema and meaning version the bytes claim to have.
- **Byte validity** - whether the used bytes conform to that schema's wire rules.
- **Application authority** - which user or PDA is allowed to run this instruction.

Checking the bytes or the account owner never checks whether a signer is allowed to act. In your handler, verify that the authority account is a signer and matches the expected address or PDA. See [`docs/guide.md`](guide.md).

## Reopen after a handoff or error

End every reader and editor borrow before making a CPI call. When the call returns, reopen the account with the load policy that matches what the callee could have done.

If an edit fails because of bad input, too little capacity, or too few lamports for funding, nothing is written to the payload and the poison state stays unchanged. In a detached resize, the edit releases the borrow, grows the physical storage, reacquires the borrow, and then runs the detached readiness check. If that check rejects the edit, the physical storage may already be larger, but the old used encoding is preserved.

Poison blocks ordinary typed reuse after certain failures. A closed session has ended. If a lazy mutable session detects corrupt source bytes during a read or edit, it sets sticky poison even before a write. A late corruption or custom encoder failure can also leave a partial edit in storage.

Rollback means the Solana runtime restores account state when the whole transaction fails. Partial progress means edits completed before a callback error remain in storage until that rollback happens. If you catch an error inside your handler, return `Ok(())` and the transaction succeeds, those earlier writes stay on chain. Always return callback and commit errors from your handler so the runtime rolls the transaction back.

In recoverable storage, such as an off-chain host `Store` or an in-place recovery handler, a fresh lazy session or a root `verify()` call will not clear the problem. Calling `verify()` preserves poison, and dropping a `Store` loses its in-memory poison marker without fixing the corrupt bytes underneath. Restore or repair any invalid bytes, then reopen through an explicit checked mutable load (`with_edit_checked`, `Store::load_checked`, or `load_checked_mut`). `verify-on-load` adds scans to lazy loads, but it does not replace an explicit checked mutable load for recovery. See [`SPEC.md#typed-edits-and-atomicity`](../SPEC.md#typed-edits-and-atomicity).

## Account identity

A namespace is a stable schema name, and a semantic epoch is a version number you bump when the meaning of the fields changes. Together with the wire layout, they form the account's identity. The discriminator stores that identity in the first bytes of the account. A descriptor is a canonical description of the encoding.

By default, `#[blubber::schema(account)]` sets `AccountSchema::NAMESPACE` to `account:<Name>` and `AccountSchema::SEMANTIC_EPOCH` to `0`<!-- num-code:"{{num:schema.identity.default_epoch}}" -->. Automatic identity hashes the wire format version, `NAMESPACE`, `SEMANTIC_EPOCH`, and `AccountSchema::WIRE_DESCRIPTOR_BYTES`.

If you rename a struct in Rust, freeze `namespace = "..."` so deployed accounts still match. If you change what fields mean without changing their types - such as swapping `debit: u64` and `credit: u64` - `WIRE_DESCRIPTOR_BYTES` does not change. Bump `semantic_epoch = ...` so old accounts cannot be loaded under the new meaning.

An explicit discriminator override turns off automatic layout binding, so you must version it yourself whenever the wire contract or field meaning changes. See [`SPEC.md#layout-bound-identity`](../SPEC.md#layout-bound-identity).

## Migrate an account

Migration reads old data, converts it, validates the replacement, and writes the new identity. Across its lifecycle an account is uninitialized, prefunded, or ready. At open, a lazy reader checks the account owner and that the account is not executable, takes a shared borrow, and checks the account identity and header without scanning the full encoding. Because the all-zero discriminator is reserved, uninitialized accounts fail that identity check. Full validity of the bytes rests on approved writer history, or on a checked load that scans the full encoding.

When you change an account's schema or bump its semantic epoch:

1. Freeze the old schema struct and its identity (`namespace` and `semantic_epoch`, or the explicit discriminator).
2. Authenticate the account owner and the application authority allowed to migrate it.
3. Decode the existing bytes with the old schema's checked loader.
4. Convert the values into the new schema's initializer or target encoding.
5. Validate the complete new encoding, and write the new discriminator last.
6. Reopen the migrated account with a checked load.

If a migration spans multiple instructions or transactions, write into a new account or mark the account non-ready until conversion and validation finish, or a normal handler could lazy-load half-migrated bytes. For a test that swaps equal-layout `debit` and `credit` fields across epochs, see [`crates/blubber/tests/migration.rs`](../crates/blubber/tests/migration.rs). See [Migrate a Borsh account](recipes.md#migrate-a-borsh-account).

## Use raw writes or custom codecs

Verus checks that Blubber's byte-moving core preserves bytes outside the edited region and keeps layouts valid under its stated assumptions. SDK calls, compiler lowering, raw byte writes, custom codecs, and application authority checks sit outside those proofs and rely on their own contracts and tests. See [`ASSURANCE.md#trust-boundaries`](../ASSURANCE.md#trust-boundaries).

If you use the raw gateways or low-level byte operations to write outside the typed editor, approved writer history ends. Restore a valid typed layout and reopen with a checked load before using lazy readers or editors again.

If you implement `Schema`, `Fixed`, or `MapKey` by hand, you must uphold the wire contract:

- Measure the exact byte length, reject bad input before writing, fill the entire selected region with valid bytes, and replay deterministically.
- Select the exact child byte region in field projections.
- Encode custom `MapKey` types injectively in canonical key order so binary search and uniqueness checks hold.
- Leave the hidden completeness and replay markers at `false` unless your encoder meets those exact obligations. Setting them to `true` lets native lazy initialisation skip redundant replay and post-write scans, while checked loads and `verify-on-load` still scan in full.
- For automatic account identity, give handwritten schemas a stable `WireDescriptor::Custom` with a non-empty codec namespace, epoch, and child descriptor tree. Give custom map keys a `MapKey::KEY_DESCRIPTOR` with their `CodecIdentity`, and bump the codec epoch when their encoding changes.

See [`SPEC.md#strict-raw-gateways`](../SPEC.md#strict-raw-gateways).
