# `blubber`

`blubber` is the main crate for Blubber, a Rust library for Solana programs that edits lists, strings and maps in place.

## Use the facade

Your program's state lives in account data, which is the account's byte slice. A borrowed view reads those bytes while the borrow is live, copying scalars and borrowing strings. You can append to a list through a generated editor without decoding the account into owned values.

By default, `blubber` enables the `derive` feature and re-exports `#[blubber::schema]` and `#[derive(blubber::Fixed)]`. The schema macro generates initializers and typed accessors. `Fixed` encodes fixed-width structs without Rust struct padding.

`blubber` 0.1.0<!-- num:release.blubber.version --> is published under the MIT licence; see [release status](../../README.md#release-status). Some libraries with fixed slots or single-item edits are faster on individual benchmark rows; see [the comparison](../../benchmarks/COMPARE.md).

## Install

`blubber` is `no_std`, so it builds without the standard library and keeps allocation opt-in. The minimum supported Rust version is 1.89.0<!-- num:release.rust.minimum -->.

For a Pinocchio program, enable `pinocchio` so the crate exposes the account guards, funding, sizing and account creation APIs:

```sh
cargo add blubber@0.1.0 --features pinocchio
```
<!-- num-fence:{"1": "cargo add blubber@{{num:release.blubber.version}} --features pinocchio"} -->

```toml
[dependencies]
blubber = { version = "0.1.0", features = ["pinocchio"] }
```
<!-- num-fence:{"2": "blubber = { version = \"{{num:release.blubber.version}}\", features = [\"pinocchio\"] }"} -->

Enable `alloc` when you need `VecStore` or owned values.

## Try it on the host

Host storage runs off-chain in tests and tools. `SliceStore::new(room)` wraps a mutable byte slice. `VecStore::new(buffer)` wraps a growable vector when `alloc` is enabled.

The runnable host round-trip doctest ships in [`src/lib.rs`](src/lib.rs) and in the crate-level rustdoc under [`#try-it-on-the-host`](https://docs.rs/blubber/0.1.0/blubber/#try-it-on-the-host). For an on-chain handler, see [create an account](../../docs/guide.md#create-an-account) and [choose a load](../../docs/trust-model.md#choose-a-load).

## Check the handler

Before you finish a handler, run through the checklist below. Full task routing is in [`docs/agents.md`](../../docs/agents.md#find-the-task) and [`#check-the-handler`](../../docs/agents.md#check-the-handler).

<!-- agent-rules:start -->

- **Owner and authority** - Pass the expected program ID as the account owner, and verify the application signer or authority in your handler. Blubber checks the owner against the program ID you pass, while your code checks who may act.
- **Lazy load vs checked load** - A lazy load checks the account header up front and checks fields as you read them. A checked load scans the whole used account before your code runs. Use `with_read`, `with_edit` or `with_edit_options` when your program created or migrated the account under the current schema and every writer since kept it valid. Otherwise use `with_read_checked`, `with_edit_checked`, `with_edit_checked_options` or `load_checked_mut`.
- **Headroom** - Headroom controls how much extra physical space an edit can request and when it requests it. The default is `Headroom::UpTo(0)`. When an edit can outgrow opening spare, pass `Headroom::UpTo` with the peak additional encoded bytes minus opening spare, `Headroom::Max`, or `Headroom::OnDemand` with `alloc` and a working allocator.
- **Rent and funding** - Pass `Funding::Prepaid` to use existing lamports, or `Funding::TopUp` with a writable, signing, System-owned, data-empty payer and System Program to cover any rent deficit. If you set `EditOptions::refund` to `Some(recipient)`, commit sends excess lamports above the final rent requirement to that writable account.
- **Fallible reads** - Fallible reads return a `Result` for bad bytes instead of panicking. Use `try_get`, `try_iter` and `try_<field>` so malformed data becomes an error.
- **Borrows and commit** - Commit finishes the edit, releases the borrow, attempts any trim, settles rent if needed and returns `CommitInfo` on success. Scoped edits (`with_edit` and its variants) commit on `Ok`. Commit direct edits before a CPI that uses the account or a successful exit. Keep one editor per account and one child borrow per parent at a time.
- **Rollback and error propagation** - Edits made before a callback fails stay in the account bytes until transaction rollback. Return the error with `?` to roll back, or restore the bytes and reopen with `load_checked_mut`.

<!-- agent-rules:end -->
