# Guide

You are writing a Pinocchio handler that stores a growable list inside a Solana account and appends to it in place. This guide walks through the quickstart program (`programs/blubber-quickstart/src/lib.rs`) and its Mollusk test (`tests/mollusk/tests/quickstart.rs`), then explains the layout, sizing and funding rules for your own accounts.

If you are deciding whether Blubber fits your program, see the [README](../README.md) and the [performance numbers](performance.md#find-the-numbers). If you are checking safety assumptions or writer history, see the [trust model](trust-model.md#choose-a-load).

## Create an account

The crates require Rust 1.89.0<!-- num:release.rust.minimum --> or newer. Add `blubber` 0.1.0<!-- num:release.blubber.version --> with the `pinocchio` feature to your program crate:

`cargo add blubber@0.1.0 --features pinocchio`<!-- num-code:"cargo add blubber@{{num:release.blubber.version}} --features pinocchio" -->

The crate is `no_std` by default and leaves heap allocation off. The default `derive` feature re-exports `#[blubber::schema]` and `#[derive(blubber::Fixed)]`. Enabling `pinocchio` adds the Pinocchio `AccountView` guards, account creation helpers, sizing options and rent settlement. Enable `alloc` if you need `VecStore` or owned values.

First, declare the account schema. A schema describes the fields of a type and how to encode them. The account root describes the whole account payload, while a nested schema describes a field or item inside it.

<!-- code:schema -->
```rust
use blubber::List;

/// An append-only queue of numbers.
#[blubber::schema(account)]
pub struct Queue {
    /// Signer allowed to append.
    pub authority: Address,
    /// Values in insertion order.
    pub values: List<u64>,
}
```

`#[blubber::schema(account)]` generates an initializer and typed accessors for `Queue`. An initializer is the input struct for writing a new account, and it can borrow strings and slices from the caller. Here it is `QueueInit`, which takes a 32<!-- num:wire.address.bytes -->-byte `Address` and a slice of `u64` values.

Next, write the creation handler:

<!-- code:create -->
```rust
fn create(
    program_id: &Address,
    state: &mut AccountView,
    authority: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
) -> ProgramResult {
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Queue::create(
        state,
        payer,
        system_program,
        program_id,
        QueueInit {
            authority: *authority.address(),
            values: &[],
        },
    )?;
    Ok(())
}
```

In `programs/blubber-quickstart/src/lib.rs`, `process_instruction` takes four accounts in order: the writable queue, the read-only authority signer, the writable payer signer and the System Program. Opcode 0<!-- num:quickstart.opcode.create --> runs `create` and requires the queue to sign too.

Check `authority.is_signer()` before calling `Queue::create`, or a caller can record an authority address they do not control. `Queue::create` measures `QueueInit`, allocates the 48<!-- num:quickstart.queue.empty_bytes --> bytes an empty `Queue` needs, assigns the account to `program_id`, transfers rent from `payer` and writes the header and payload.

Generated `Queue::create` and `Queue::create_pda` require zero starting lamports so they do not overwrite a funded account. For prefunded, data-empty System accounts, call generated `Queue::allocate_account` or `Queue::allocate_pda`, which keep existing lamports and top up the difference with the SIMD-0312 System instruction. The generated methods on `Queue` take `QueueInit` and initialise the account.

## Grow the queue

Opcode 1<!-- num:quickstart.opcode.push --> reads a little-endian `u64` from the instruction data and calls `push` with the authority and payer signatures:

<!-- code:push -->
```rust
fn push(
    program_id: &Address,
    state: &mut AccountView,
    authority: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    value: u64,
) -> ProgramResult {
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let signer = *authority.address();
    // This program initializes the queue and is its only writer.
    Queue::with_edit_options(
        state,
        program_id,
        EditOptions {
            funding: Funding::TopUp {
                payer,
                system_program,
            },
            headroom: Headroom::UpTo(8), // One u64, with no spare bytes at opening.
            refund: None,
        },
        |queue| {
            if queue.try_authority()? != signer {
                return Err(ErrorKind::Unauthorized.into());
            }
            queue.values_mut().push(value)
        },
    )?;
    Ok(())
}
```

`blubber::pinocchio_sdk` provides `AccountView` as the runtime account handle. An SDK guard is the runtime borrow guard that controls shared reads and exclusive edits to account data. `Queue::with_edit_options` checks that `state` is owned by `program_id`, opens a mutable SDK guard, checks the account header and runs the closure on a temporary `QueueMut` editor without decoding the list into a `Vec`.

Owner authentication and authority checks establish different facts. `with_edit_options` checks that your program owns `state`, while your handler must still check that `authority` signed and matches the stored `queue.try_authority()?` key, or any caller can append to anyone's queue.

`try_authority()` and `try_get` return a `Result`, and `try_iter` yields a `Result` per item, so detected corruption becomes an error. On a list or map, `try_get` returns `Ok(None)` for a missing index or key in valid data.

This handler uses a lazy load (`with_edit_options`) because this program initialises `Queue` and is its only writer. Switch to a checked load (`with_edit_checked` or `with_edit_checked_options`) when you cannot vouch for every program version that wrote the account; see [Choose a load](trust-model.md#choose-a-load).

Because `create` sized the account to 48<!-- num:quickstart.queue.empty_bytes --> bytes with zero spare bytes, appending one `u64` needs 8<!-- num:quickstart.queue.append_bytes --> new bytes. `Headroom::UpTo(8)` asks the runtime to resize the account by up to 8<!-- num:quickstart.queue.append_bytes --> bytes when the edit opens. `Funding::TopUp` transfers the extra rent from `payer` when the closure commits.

Build the quickstart for both SBF architectures and run the Mollusk tests with:

`bash scripts/reproduce.sh sbf`

In `tests/mollusk/tests/quickstart.rs`, `creates_and_pushes_with_exact_lengths_and_rent` checks that creating the queue produces 48<!-- num:quickstart.queue.empty_bytes --> bytes, the first push grows it to 56<!-- num:quickstart.queue.first_append_bytes --> bytes and the second grows it to 64<!-- num:quickstart.queue.second_append_bytes --> bytes, with signer-funded rent and the expected values. The same test checks that a wrong authority returns `Unauthorized` and an empty payer returns `ProgramError::AccountNotRentExempt` without changing the account. See [errors and offsets](../SPEC.md#errors-and-offsets) for the runtime error mapping.

## Define a schema

Mark an account root with `#[blubber::schema(account)]` and a nested schema with `#[blubber::schema]`. A persistent account root must be a concrete struct because its layout determines the account discriminator, while a nested schema can use type and const generics. For a struct whose fields are all fixed-size, derive `#[derive(blubber::Fixed)]` to pack its fields consecutively without Rust struct padding.

`Str`, `List<T>` and `Map<K, V>` are schema markers for UTF-8 strings, ordered lists and sorted maps. `Address` is the fixed-size 32<!-- num:wire.address.bytes -->-byte address type. Built-in schemas cover unsigned and signed integers, `bool`, `[u8; N]`, `Address`, `Fixed` structs, `Str`, `Option<T>`, `List<T>` and `Map<K, V>`.

In a `Map<K, V>`, keys stay in sorted order so lookups can binary-search the key table without scanning every entry. Map keys must have an ordered encoding of fixed positive width (`MapKey`, implemented for integers, `bool`, `[u8; N]` and `Address`). Strings and variable-size types cannot be map keys in 0.1.0<!-- num:release.blubber.version -->.

Every type in a nested layout is a node. Schema depth counts the nodes from the account root to a leaf, inclusive. By default, schemas support a depth of 8<!-- num:schema.default.depth --> nodes. Enable `max-depth-16` to raise the limit to 16<!-- num:schema.depth16.depth --> nodes without changing the encoded bytes. When you change a schema on an existing deployment, see [Migrate an account](trust-model.md#migrate-an-account).

## How the bytes are laid out

Every account starts with a 12<!-- num:wire.account.header_bytes -->-byte header: an 8<!-- num:wire.account.discriminator_bytes -->-byte discriminator that identifies the schema, followed by a 4<!-- num:wire.account.used_len_bytes -->-byte little-endian `used_len` that counts the header plus the root payload. Capacity is the account's total physical storage (`storage_len`). Spare is the physical space after `used_len`.

```text
|<------------------ storage_len (physical capacity) ------------------>|
+----------------------+-----------------------+------------------------+
| Header (12 bytes)    | Root payload          | Spare capacity         |
| disc (8) | used (4)  | (used_len - 12 bytes) | (storage_len - used)   |
+----------------------+-----------------------+------------------------+
|<------------- used_len --------------------->|
```

Fixed-width lists such as `List<u64>` pack their items directly after a 4<!-- num:wire.list.count_bytes -->-byte little-endian count. Variable-width collections such as `List<Str>` use a directory. A directory is the offset table at the front of a variable-width collection that locates each item. After the 4<!-- num:wire.list.count_bytes -->-byte item count, the directory stores one 4<!-- num:wire.list.end_bytes -->-byte relative end offset (a stored end) for each item except the last. The last item needs no stored end because the parent region's own end already marks where it stops. That boundary is its implicit end.

Here is how a `List<Str>` changes across the first two appends:

```text
Empty List<Str>:
[ count = 0 (4B) ]

After pushing "old" (3B):
[ count = 1 (4B) ][ "old" (3B) ]
(No stored end word: the list's own end marks the end of the payload.)

After pushing "new" (3B):
[ count = 2 (4B) ][ end_0 = 3 (4B) ][ "old" (3B) ][ "new" (3B) ]
```

Appending `"old"` to an empty `List<Str>` adds only its payload bytes. Appending `"new"` after `"old"` adds 7<!-- num:recipe.indexed_append.encoded_growth_bytes --> encoded bytes: 3<!-- num:recipe.indexed_append.payload_bytes --> bytes for `"new"` and 4<!-- num:wire.list.end_bytes --> bytes in the directory to record where `"old"` ends.

An ancestor is an enclosing field or collection whose size or offsets depend on a child inside it. Inserting that new 4<!-- num:wire.list.end_bytes -->-byte stored end into the directory shifts `"old"` forward by 4<!-- num:wire.list.end_bytes --> bytes. Bytes after the list move by the full 7<!-- num:recipe.indexed_append.encoded_growth_bytes --> bytes, and Blubber updates existing end words in enclosing ancestors without adding any end words there. That is why an in-place edit can still move bytes after the edit point. Verus checks that a byte move preserves bytes outside its destination range.

## Size and fund an edit

Before an edit grows an account, separate three quantities: the spare bytes already sitting after `used_len`, the headroom (the extra bytes your handler asks the runtime to reserve) and the rent in lamports needed to keep the final physical size rent-exempt.

Take the second append above (`"new"` after `"old"`), which adds 7<!-- num:recipe.indexed_append.encoded_growth_bytes --> encoded bytes. If the account opens with 4<!-- num:recipe.indexed_append.opening_spare_bytes --> spare bytes, the edit needs 3<!-- num:recipe.indexed_append.additional_room_bytes --> more physical bytes, so you request `Headroom::UpTo(3)` (see [Grow an indexed string list](recipes.md#grow-an-indexed-string-list)). If the buffer already has 7<!-- num:recipe.indexed_append.host_spare_bytes --> spare bytes at opening, it needs no new physical bytes and fits inside `Headroom::UpTo(0)`<!-- num-code:"Headroom::UpTo({{num:runtime.headroom.default_bytes}})" -->.

`Queue::with_edit` and `EditOptions::from(funding)` use `Headroom::UpTo(0)` by default, which uses the existing physical allocation when the edit opens. A lease is a mutable edit session over an account's bytes. When you pass `EditOptions` to `with_edit_options` or `with_edit_checked_options`, pick the `Headroom` variant for your handler:

- **`Headroom::UpTo(bytes)`** - asks the runtime to resize the account by up to `bytes` when the lease opens.
- **`Headroom::Max`** - discovers the largest local growth window the SDK guard allows in this instruction.
- **`Headroom::OnDemand`** - requires the `alloc` feature and a working allocator and re-leases the account during the edit when a write needs more room.

Do not treat a requested `Headroom` as a guarantee that the runtime will grant that many bytes. Solana caps an account at 10<!-- num:runtime.account.max_mib --> MiB and caps growth per invocation at 10,240<!-- num:runtime.invocation.growth_bytes --> bytes above the account's length at invocation entry. Earlier growth of the same account can leave less room, and the SDK's local headroom check cannot predict the shared transaction growth limit.

For initial sizing before an account exists, `Queue::space(reserve)` and `Queue::encoded_len_of(&init)` measure different things. `Queue::space(reserve)` is a `const fn` that adds `reserve` to the minimum empty account encoding without looking at an initializer. `Queue::encoded_len_of(&init)` validates a `QueueInit` value and returns the exact byte length its header and initial items will take.

Commit is the step that finishes a mutable session. On a Pinocchio store it drops the SDK guard, attempts any physical trim and settles rent if needed. Scoped helpers (`with_edit`, `with_edit_options`, `with_edit_checked`, `with_edit_checked_options`) commit once when your closure returns `Ok(value)` and return `(value, CommitInfo)`. If you open a direct mutable session with `Queue::load_mut`, `Queue::load_checked_mut` or `Queue::initialize`, call `.commit()` yourself to finish it. On-chain, dropping an uncommitted session skips rent settlement. If your edit closure returns `Err(...)`, return that error from your handler so the Solana runtime rolls back any bytes changed during the callback.

`Funding::Prepaid` is the default `Funding` value and uses existing lamports to cover rent. `Funding::TopUp { payer, system_program }` transfers any net rent deficit from `payer`, while `refund: Some(recipient)` in `EditOptions` sends excess lamports above the final rent requirement to `recipient`; see [Pay rent or refund excess](recipes.md#pay-rent-or-refund-excess).

`CommitInfo` is the commit summary returned with your closure result. It carries `used_len`, `storage_len`, `topped_up`, `refunded` and `shrink_error: Option<Error>`, which records any physical trim error while keeping the logical shrink valid.

## Try it on the host

Host storage lets you run the same schema and edit code in unit tests on your machine without a Solana runtime. `SliceStore::new(&mut buf)` wraps a mutable byte slice for `no_std` tests, and `VecStore::new(&mut vec)` (with `alloc`) wraps a growable `Vec<u8>`. Call `Queue::initialize(&mut store, QueueInit { ... })`, run `values_mut().push(...)` and call `.commit()`. For a runnable host round trip, see the [crate doctest](../crates/blubber/src/lib.rs).

## Read an account from a Rust client

When an off-chain Rust client fetches account bytes, `Queue::load(bytes)` does not see the `AccountView` metadata that an on-chain handler checks. Check that the account address matches the address you expected and that its owner equals your program ID before calling `Queue::load`, or an RPC response can hand you bytes from an account your program does not own.

<!-- code:queue-client-read -->
```rust
fn read_queue(address: Pubkey, account: &Account) -> Option<QueueRef<'_>> {
    if address != STATE || account.owner != PROGRAM {
        return None;
    }
    let queue = Queue::load(&account.data).ok()?;
    (queue.authority().to_bytes() == AUTHORITY.to_bytes()).then_some(queue)
}
```

Once you have checked the address and owner, pass the byte slice to `Queue::load(bytes)`. `Queue::load` validates the 12<!-- num:wire.account.header_bytes -->-byte header and scans the whole used root against `Queue`'s schema before returning `QueueRef`, so truncated or bad bytes return an error on load. You can then read `queue.authority()` and iterate `queue.values().try_iter()`.

## Fix a version mismatch

Blubber 0.1.0<!-- num:release.blubber.version --> builds on Pinocchio 0.12. With the `pinocchio` feature, Blubber re-exports the SDK as `blubber::pinocchio_sdk`. Import the entrypoint macros, `AccountView`, `Address` and CPI types from there so your program uses the same Pinocchio as Blubber.

Rust treats two copies of a crate at different major versions as different types, even when the type has the same name. Run `cargo tree -d` to list the duplicate crates.

Passing a Pinocchio 0.11 `AccountView` to `ReadStore::new` gives this error:

```text
error[E0308]: mismatched types
expected `blubber::pinocchio::AccountView`, found `pinocchio::AccountView`
note: there are multiple different versions of crate `solana_account_view` in the dependency graph
```

Import the entrypoint and handler types from Blubber's re-export instead:

```rust,no_run
use blubber::pinocchio_sdk::{program_entrypoint, AccountView, Address};
```

If a CPI crate supplies the accounts, align it to the same account-view version.

Schema keys use `blubber::Address`, the Solana Address 2.x type. Pinocchio 0.11 and 0.12 and `solana-pubkey` 4's `solana_pubkey::Pubkey` use that same type, so those values pass straight in. A value from `solana-address = "=1.0.0"` fails to compile:

```text
error[E0308]: mismatched types
expected `blubber::Address`, found `solana_address::Address`
note: there are multiple different versions of crate `solana_address` in the dependency graph
```

With Blubber's `Address` in scope, copy the 32<!-- num:wire.address.bytes --> bytes:

<!-- doctest-ignore: old_key is a value from solana-address 1.0.0, checked in the version-mismatch reproduction. -->
```rust,ignore
let key = Address::from(old_key.to_bytes());
```
