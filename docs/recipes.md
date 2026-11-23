# Editing recipes

This page is for a developer who already has a typed account editor open and needs to size a growth budget, settle rent, edit nested fields, replace a collection in one pass, or call another program.

If you are setting up your first schema or loading an account, start with [`docs/guide.md`](guide.md). For benchmark numbers, see [`docs/performance.md`](performance.md). Coding agents should also read [`docs/agents.md`](agents.md).

## Grow an indexed string list

You want to call `push("new")` on a `List<Str>` that already contains `"old"`. How much extra room do you ask for in `EditOptions`?

Capacity is the physical storage available to the account. The account header takes 12<!-- num:wire.account.header_bytes --> bytes: an 8<!-- num:wire.account.discriminator_bytes -->-byte discriminator and a 4<!-- num:wire.account.used_len_bytes -->-byte little-endian `used_len`. Used length includes the header and the live payload. Spare is the physical space beyond `used_len`. `encoded_len_of` measures and checks an initializer with its header, and account creation uses that measured length.

Variable-width collections use a directory. A directory is an offset table that finds variable-sized items. An indexed list starts with a 4<!-- num:wire.list.count_bytes -->-byte count and stores one 4<!-- num:wire.list.end_bytes -->-byte relative end per item except the last. The parent gives the last item's end, so that implicit end needs no stored entry.

Appending to an empty indexed list adds only the item's payload bytes, because its end is implicit. When you append `"new"` after `"old"`, `"old"` stops being the last item and now needs a stored end word.

So appending `"new"` after `"old"` takes 7<!-- num:recipe.indexed_append.encoded_growth_bytes --> bytes. 3<!-- num:recipe.indexed_append.payload_bytes --> hold the text `"new"`. 4<!-- num:wire.list.end_bytes --> store the new end word for `"old"`. Existing parent end words may move or receive updated offsets when the list grows, which does not add bytes to the encoding. Verus checks the move contract: a byte move preserves every byte outside its destination range.

Peak encoded growth is the largest addition to used bytes during your edits, including structural directory bytes. To budget extra allocation, subtract the opening spare from the peak encoded growth, floored at zero.

Headroom is the extra physical space an edit session asks for. The default is `Headroom::UpTo(0)`<!-- num-code:"Headroom::UpTo({{num:runtime.headroom.default_bytes}})" -->, which uses the existing allocation when the edit opens. If your account opens with 4<!-- num:recipe.indexed_append.opening_spare_bytes --> spare bytes, appending `"new"` needs 3<!-- num:recipe.indexed_append.additional_room_bytes --> more physical bytes, so you pass `Headroom::UpTo(3)`<!-- num-code:"Headroom::UpTo({{num:recipe.indexed_append.additional_room_bytes}})" -->.

<!-- code:indexed-append -->
```rust
#[test]
fn indexed_append_preserves_sibling() -> blubber::Result<()> {
    let init = NotesInit {
        messages: &["old"],
        following: "keep",
    };
    let used = Notes::encoded_len_of(&init)? as usize;
    let mut bytes = vec![0; used + 7];
    let mut store = SliceStore::new(&mut bytes);
    let mut notes = Notes::initialize(&mut store, init)?;
    let new = "new";
    notes.messages_mut().push(new)?;
    let growth = notes.commit()?.used_len as usize - used;
    assert_eq!(new.len(), 3);
    assert_eq!(growth, 7);
    let runtime_spare = 4;
    assert_eq!(growth - runtime_spare, 3);

    let notes = Notes::load(&bytes)?;
    assert_eq!(notes.messages().try_get(1)?, Some("new"));
    assert_eq!(notes.following(), "keep");
    Ok(())
}
```

In this host test, the `SliceStore` opens with 7<!-- num:recipe.indexed_append.host_spare_bytes --> spare bytes, so the append fits `Headroom::UpTo(0)`<!-- num-code:"Headroom::UpTo({{num:runtime.headroom.default_bytes}})" --> on-chain. See [`docs/guide.md#how-the-bytes-are-laid-out`](guide.md#how-the-bytes-are-laid-out) for the full layout rules.

## Pay rent or refund excess

The rent-exempt minimum is the lamport balance required for the physical allocation. Pinocchio's `Rent::get()?.try_minimum_balance(len)?` quotes it under SIMD-0194. When an account's physical allocation changes, its lamports must still cover rent exemption.

You configure funding and headroom in `EditOptions`:
- `Funding::Prepaid` uses the account's existing lamport balance. Growth that the balance cannot cover is rejected before the payload changes.
- `Funding::TopUp { payer, system_program }` transfers any net rent deficit from `payer` using at most one System Program transfer. The `payer` must be writable, a signer, System-owned, and data-empty, or Blubber rejects the payer.
- `refund: Some(recipient)` sends excess lamports above the final physical allocation's rent requirement to `recipient`. Your handler must check that the recipient is authorised to receive those funds, and Blubber checks writability and aliasing.

Passing `EditOptions::from(funding)` keeps your `Funding` choice with `Headroom::UpTo(0)`<!-- num-code:"Headroom::UpTo({{num:runtime.headroom.default_bytes}})" --> and `refund: None`.

Commit finishes the edit session and releases the Pinocchio account borrow. A successful scoped edit commits once for you and returns your closure's result alongside `CommitInfo`. Direct `commit` consumes the root editor, closes the store, releases the borrow guard, attempts to trim to the used length or retained reservation, and settles rent if needed at the actual final `storage_len`.

If an edit shrinks the logical `used_len` without trimming physical space, the extra capacity stays as spare bytes. If a requested physical trim fails, the valid encoded bytes stay intact and `CommitInfo` reports it in `shrink_error` if commit otherwise succeeds. If your edit callback returns an error, Blubber skips rent settlement and only attempts physical trim on drop.

See [`docs/guide.md#size-and-fund-an-edit`](guide.md#size-and-fund-an-edit) for the entry walkthrough.

## Edit a selected value

A selector names a field, key, or index. A child editor changes one selected value, and a scoped callback borrows that child temporarily.

<!-- code:nested-string-edit -->
```rust
acct.records_mut().at_mut(1)?.name_mut().set("longer")?;
```

Here `at_mut` selects an existing list record and `set` replaces its nested string in place. If the new string has a different length, Blubber shifts the following bytes and updates the enclosing directory offsets.

Only one child editor can be live under a parent at a time. A child editor cannot change its parent collection's count or map keys, because that would invalidate the parent directory while you are navigating it. To step through items one by one, use a lending cursor (`cursor_mut`) or `try_for_each_mut`. For the other selector methods (`get_mut`, `into_get_mut`, `with_mut`, `try_get`), see the `ListMut` and `MapMut` rustdoc.

## Handle absence and callback errors

Choose your selector based on whether a missing item is normal or a bug:
- `get_mut` returns `Ok(None)` when the index or key is absent, as does `into_get_mut` for a missing map key. `with_mut` on a list skips its closure and returns `Ok(None)` when the index is out of bounds.
- `at_mut` requires the target to exist. It returns `IndexOutOfBounds` for a missing list index and `MissingKey` for a missing map key.

Rollback is how the Solana runtime restores account data and lamports when the transaction fails. Partial progress is what happens inside your session before that. If a callback edits an account and returns `Err` later in the same pass, the bytes written before the error are already in the account buffer. `try_for_each_mut` also stops on the first selection or callback error and leaves earlier completed item writes in storage.

<!-- code:callback-progress -->
```rust
#[test]
fn list_callback_absence_and_partial_progress() -> blubber::Result<()> {
    let mut bytes = [0; 96];
    let mut store = SliceStore::new(&mut bytes);
    let mut notes = Notes::initialize(
        &mut store,
        NotesInit {
            messages: &["old"],
            following: "keep",
        },
    )?;
    let absent = notes.messages_mut().with_mut(9, |_| {
        Err::<(), blubber::Error>(ErrorKind::InvalidBoundary.into())
    })?;
    assert_eq!(absent, None);

    let edit = notes.messages_mut().with_mut(0, |message| {
        message.set("written")?;
        Err::<(), blubber::Error>(ErrorKind::InvalidBoundary.into())
    });
    assert_eq!(edit, Err(ErrorKind::InvalidBoundary.into()));
    assert_eq!(notes.messages().try_get(0)?, Some("written"));
    Ok(())
}
```

In an on-chain handler, return the callback error with `?` so the transaction fails and the runtime rolls back the partial writes. If you ignore the error and the transaction succeeds, those partial writes stay on-chain. See [`docs/trust-model.md#reopen-after-a-handoff-or-error`](trust-model.md#reopen-after-a-handoff-or-error) for recovery rules in host code.

## Replace a collection

Replacing items one by one in a loop can shift the tail of the account on each length change. When you replace every value in a list or map, use a bulk method so Blubber lays out the collection in one edit:

- Use `fill` on a `ListMut` or `fill_values` on a `MapMut` to set every existing item to the same value.
- Use `rewrite_from` on a `ListMut` or `rewrite_values` on a `MapMut` when you have a slice of replacement inputs, one per existing item in entry order. Both check that your slice length matches the collection count before writing any bytes.
- Use `rewrite_values_from` on a `MapMut` when your slice carries `(K, V::Init<'_>)` pairs. It checks the count and checks that the supplied keys match the map's current canonical key order before writing.
- Use `rewrite_values_with` on a `MapMut<K, Str>` (with the `alloc` feature) when each new string is computed from the old string.

Writing longer replacements straight into the packed payload can overwrite unread bytes. Instead, `rewrite_values_with` stages the new strings in temporary heap memory before updating the account. You pass an `output_capacity` in bytes for one shared staging buffer and a closure `FnMut(&str, &mut [u8]) -> Result<usize>`. For each entry in key order, the closure receives the old string and the remaining slice of the staging buffer, writes the replacement bytes at the start of that slice, and returns the length written. If replacement lengths differ, Blubber also allocates a temporary ends buffer on the heap.

Budget the staging heap and the account headroom separately. `output_capacity` comes from the SBF heap, while net growth in the encoded map comes from the account's spare capacity or `Headroom`.

<!-- code:map-transform -->
```rust
#[test]
fn doc_map_transform_and_short_output() -> blubber::Result<()> {
    let mut bytes = [0; 64];
    initialized(&mut bytes, &[(1, "aa"), (3, "bb")]);
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    let mut root = OldStrings::load_checked_mut(&mut store)?;
    let upper = |old: &str, output: &mut [u8]| {
        let output = output.get_mut(..old.len()).ok_or(ErrorKind::Capacity)?;
        output.copy_from_slice(old.as_bytes());
        output.make_ascii_uppercase();
        Ok(output.len())
    };
    let short = root.values_mut().rewrite_values_with(3, upper);
    assert_eq!(short, Err(ErrorKind::Capacity.into()));
    root.commit()?;
    assert_eq!(bytes, before);

    let mut store = SliceStore::new(&mut bytes);
    let mut root = OldStrings::load_checked_mut(&mut store)?;
    root.values_mut().rewrite_values_with(4, upper)?;
    root.commit()?;
    let root = OldStrings::load(&bytes)?;
    assert_eq!(
        root.values().iter().collect::<Vec<_>>(),
        [(1, "AA"), (3, "BB")]
    );
    Ok(())
}
```

If the callback returns an error, writes invalid UTF-8, or exceeds `output_capacity` (returning `Capacity`), `rewrite_values_with` stops before modifying the account bytes and leaves the session unpoisoned. On a lazy mutable path, a detected source fault instead poisons the session and prevents commit.

For measured bulk-edit CU and heap costs, see [`docs/performance.md#replace-many-values`](performance.md#replace-many-values) and [`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](../benchmarks/TRAVERSAL.md#ordinary-caller-costs).

## Migrate a Borsh account

You can convert an existing Borsh account to Blubber in place at the same address. Instruction opcode 2 in [`programs/blubber-quickstart/src/lib.rs`](../programs/blubber-quickstart/src/lib.rs) converts a program-owned Borsh `Queue` to Blubber's `Queue` without a heap allocator, and [`tests/mollusk/tests/quickstart.rs`](../tests/mollusk/tests/quickstart.rs) runs it on both SBF architectures (v0 and v3). This example shifts the payload in place because Borsh `Vec<u64>` and Blubber `List<u64>` both store a little-endian `u32` count followed by packed `u64` values.

<!-- code:borsh-migrate -->
```rust
fn migrate(
    program_id: &Address,
    state: &mut AccountView,
    authority: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
) -> blubber::Result<()> {
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    if !state.is_writable() {
        return Err(ErrorKind::NotWritable.into());
    }
    let new_len = checked_migration_len(state, program_id, authority.address())?;
    let top_up = migration_top_up(state, payer, system_program, new_len)?;
    state.resize(new_len as usize)?;
    if top_up != 0 {
        Transfer {
            from: payer,
            to: state,
            lamports: top_up,
        }
        .invoke()?;
    }
    publish_queue(state, new_len)?;
    Queue::with_read_checked(state, program_id, |_| Ok(()))
}
```

The caller passes four accounts in order: the writable queue, the authority (signer), a writable payer (signer), and the System Program. The queue address does not sign. The handler only validates and charges the payer when the account needs more rent; when it does, the payer must be a System-owned account with empty data and sign.

A valid queue with `n` values grows from `44 + 8n` bytes to `48 + 8n` bytes: 4<!-- num:wire.account.used_len_bytes --> more bytes for Blubber's used-length word, inside Solana's 10,240<!-- num:runtime.invocation.growth_bytes -->-byte per-instruction growth window. Moving the payload costs CU in proportion to its length; see [`docs/performance.md`](performance.md).

The handler runs in this order so a failure leaves no bad state:

1. Check the authority signature and account writability first, then check the account owner, the old discriminator and the exact Borsh length before any write (this example rejects all trailing bytes; if your old layout reserved padding, apply its own layout rules).
2. Compute the new length and confirm the payer can cover the rent deficit with Pinocchio's `Rent::get()` and `try_minimum_balance`, so you do not resize an account you cannot fund.
3. Resize the account and transfer the rent deficit from the payer.
4. Shift the validated payload 4<!-- num:wire.account.used_len_bytes --> bytes, write the used length, and validate the destination payload.
5. Write the new discriminator last, because until it is written, no normal handler can load the half-written account.
6. Reopen the account with a checked load, and return any error to the runtime so the whole instruction rolls back.

Running the migration a second time fails the old-discriminator check. The address and any PDA seeds stay the same. Clients that decode the account data must switch to the new format.

### When your layout differs

An in-place shift only works when the wire layouts match. Borsh strings carry a length word while Blubber gives `Str` an exact byte region, map key encodings and ordering can differ, and reordered fields change wire positions.

For those accounts, parse and validate the old values, build the new schema's typed input, and call `initialize` into a staging buffer. For schemas with variable-size fields, the generated `encoded_len_of` gives the staging length, while [`crates/blubber/tests/migration.rs`](../crates/blubber/tests/migration.rs) shows the staging and publish order in a fixed-size buffer. On chain, check growth and funding before writing, copy the verified encoding from byte 8<!-- num:wire.account.discriminator_bytes --> onward under an exclusive borrow, verify the destination, write the new discriminator last, and reopen with a checked load.

Blubber 0.1.1<!-- num:release.blubber.version --> has no allocator-free general converter; your program supplies the old decoder and the staging space. A larger conversion may need an allocator or a migration planned across instructions; see [`docs/trust-model.md#migrate-an-account`](trust-model.md#migrate-an-account).

## Call another program

A CPI is a call to another on-chain program, and a handoff is any point where your handler yields control over the account. A live Blubber editor can conflict with a CPI that reads or writes the account, and a borrowed read can conflict with a CPI that writes it.

Before making a CPI:
1. Finish your scoped edit (or call `commit` on a direct editor) and drop every borrowed view into the account data.
2. Invoke the other program.
3. Reload the account afterwards.

If the other program could have modified the account or broken your writer history, reopen with a checked load (`ReadStore::load_checked` or `with_edit_checked`) so Blubber scans the full encoding before you touch it again. See [`docs/trust-model.md#reopen-after-a-handoff-or-error`](trust-model.md#reopen-after-a-handoff-or-error) for the policy.

Closing, trimming, or making a CPI and reopening does not reset Solana's per-invocation growth budget. The tests in [`tests/mollusk/tests/tour.rs`](../tests/mollusk/tests/tour.rs) and [`tests/mollusk/tests/invocation_boundaries.rs`](../tests/mollusk/tests/invocation_boundaries.rs) exercise the 10,240<!-- num:runtime.invocation.growth_bytes -->-byte growth window above the account's length at the start of the top-level instruction. The separate account maximum is 10<!-- num:runtime.account.max_mib --> MiB.
