# Using Blubber in a program

Choose a load policy before writing the handler. The examples and tests below
cover the cases where a routine edit differs from an edit on arbitrary bytes.

## Rules

<!-- agent-rules:start -->

1. Load. Use `State::with_read` and `State::with_edit` for a valid current
   schema written only by approved writers. After a raw write, an unaudited
   CPI writer or a program upgrade, use `with_read_checked`,
   `with_edit_checked` or `with_edit_checked_options`.
2. Authority. Take the expected owner from a program constant or the
   runtime-supplied program ID. Check application authority, including signer
   and PDA seeds as applicable, before changing bytes.
3. Headroom. The default is `Headroom::UpTo(0)`. For growth, set
   `Headroom::UpTo(n)` to the peak additional encoded bytes beyond opening
   spare. Include new directory words and changed ancestor sizes. Pushing to
   a nonempty indexed string list adds one four-byte end word plus UTF-8 bytes.
4. Funding. Use `Funding::Prepaid` if the account already has final rent.
   Use `Funding::TopUp { payer, system_program }` to fund a deficit.
5. Commit. Commit a direct mutable session with `root.commit()?` before CPI.
   Scoped edit helpers commit on callback success. Propagate the commit error
   and use the returned receipt if you need the published length.
6. Errors. Return edit errors from the instruction so the transaction rolls
   back. A callback error alone does not undo completed writes. Repair or roll
   back poisoned bytes and use an explicit checked mutable load to recover.
7. Reads. Use `try_get` and `try_iter` on lazy runtime collections to
   propagate invalid input. Use fallible field accessors where provided.
8. Borrows. Finish reads before editing or resizing the same storage. Keep
   one child editor live at a time. Use `with_mut`, `cursor_mut` or
   `try_for_each_mut` for scoped edits and passes.
9. SBF. Start with `no_std` and enable `alloc` when using
   `Headroom::OnDemand`. Reject stack diagnostics. Frames must fit 4,096
   bytes. Split large handlers with `#[inline(never)]` and rebuild.
10. Schema. Use a concrete account root. Set `namespace` and `semantic_epoch`
    before deployment. Bump the epoch and migrate when the meaning changes.
    Use supported codecs for enum or float representations.

<!-- agent-rules:end -->

| When | Read or check |
| --- | --- |
| First schema | [Host example](../README.md#initialize-edit-and-read) and [schema compiler tests](../crates/blubber/tests/ui.rs) |
| Runtime handler or authority | [Handler](../programs/blubber-demo/src/lib.rs), [authority check](../programs/blubber-demo/src/exercise.rs) and [quickstart](guide.md#pinocchio-accounts) |
| Load policy or poison | [Trust model](trust-model.md) and [policy tests](../crates/blubber/tests/verification_policy.rs) |
| Growth or funding | [Indexed string recipe](recipes.md#indexed-string-growth), [headroom tests](../tests/mollusk/tests/borrow_resize.rs) and [funding tests](../tests/mollusk/tests/tour.rs) |
| Commit, CPI or rollback | [Invocation tests](../tests/mollusk/tests/invocation_boundaries.rs) and [callback example](recipes.md#a-missing-index-and-an-error-after-an-edit) |
| Reading or borrowing | [Reader tests](../crates/blubber/tests/readers.rs) and [borrow compiler tests](../crates/blubber/tests/ui.rs) |
| Stack or allocation | [Build rules](guide.md#errors-and-completion) and [stack tests](../tests/mollusk/tests/stack_matrix.rs) |
| Existing on-chain schema | [Migration](trust-model.md#overrides-and-migration) and [migration test](../crates/blubber/tests/migration.rs) |
| Anchor handler | [Adapter guide](../crates/blubber-anchor/README.md) |
| Instruction budget | [Performance guide](performance.md) |

## Completion check

Confirm every handler path propagates Blubber errors and every growing edit
chooses its headroom. Run handler tests for the lazy and checked paths you
expose. Build the SBF program and check that the log has no stack diagnostics.
