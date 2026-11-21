# Handler task index

Start with your handler's task below, then run the checklist before you finish.

## Find the task

- **When creating or allocating an account** - read [`docs/guide.md#create-an-account`](guide.md#create-an-account), because `create` requires zero starting lamports while `allocate_account` and `allocate_pda` keep prefunded balances.
- **When choosing a lazy or checked load** - read [`docs/trust-model.md#choose-a-load`](trust-model.md#choose-a-load), because a lazy load checks only what you read and needs a known writer history.
- **When sizing and funding an edit** - read [`docs/guide.md#size-and-fund-an-edit`](guide.md#size-and-fund-an-edit), because default headroom requests no extra space and edits that outgrow spare bytes fail.
- **When growing a string or variable-size list** - read [`docs/recipes.md#grow-an-indexed-string-list`](recipes.md#grow-an-indexed-string-list), because peak growth can include new directory words alongside the payload.
- **When paying rent or refunding excess lamports** - read [`docs/recipes.md#pay-rent-or-refund-excess`](recipes.md#pay-rent-or-refund-excess), because top-up and refund settle against the final physical account length.
- **When calling another program during a handler** - read [`docs/recipes.md#call-another-program`](recipes.md#call-another-program), because you must finish the edit and release the account borrow before the call.
- **When changing a deployed schema or its meaning** - read [`docs/trust-model.md#migrate-an-account`](trust-model.md#migrate-an-account), because layout and semantic epoch changes alter automatic account identity. See [Migrate a Borsh account](recipes.md#migrate-a-borsh-account).
- **When recovering after a handoff or mid-session error** - read [`docs/trust-model.md#reopen-after-a-handoff-or-error`](trust-model.md#reopen-after-a-handoff-or-error), because a checked reopen validates the current bytes, which can include earlier edits.
- [Fix a version mismatch](guide.md#fix-a-version-mismatch)
- **When budgeting instruction CU or heap memory** - read [`docs/performance.md#budget-growth-and-memory`](performance.md#budget-growth-and-memory), because staging replacements and `OnDemand` growth can use the heap.

## Check the handler

<!-- agent-rules:start -->
- **Owner and authority** - Pass the expected program ID as the account owner, and verify the application signer or authority in your handler. Blubber checks the owner against the program ID you pass, while your code checks who may act.
- **Lazy load vs checked load** - A lazy load checks the account header up front and checks fields as you read them. A checked load scans the whole used account before your code runs. Use `with_read`, `with_edit` or `with_edit_options` when your program created or migrated the account under the current schema and every writer since kept it valid. Otherwise use `with_read_checked`, `with_edit_checked`, `with_edit_checked_options` or `load_checked_mut`.
- **Headroom** - Headroom controls how much extra physical space an edit can request and when it requests it. The default is `Headroom::UpTo(0)`. When an edit can outgrow opening spare, pass `Headroom::UpTo` with the peak additional encoded bytes minus opening spare, `Headroom::Max`, or `Headroom::OnDemand` with `alloc` and a working allocator.
- **Rent and funding** - Pass `Funding::Prepaid` to use existing lamports, or `Funding::TopUp` with a writable, signing, System-owned, data-empty payer and System Program to cover any rent deficit. If you set `EditOptions::refund` to `Some(recipient)`, commit sends excess lamports above the final rent requirement to that writable account.
- **Fallible reads** - Fallible reads return a `Result` for bad bytes instead of panicking. Use `try_get`, `try_iter` and `try_<field>` so malformed data becomes an error.
- **Borrows and commit** - Commit finishes the edit, releases the borrow, attempts any trim, settles rent if needed and returns `CommitInfo` on success. Scoped edits (`with_edit` and its variants) commit on `Ok`. Commit direct edits before a CPI that uses the account or a successful exit. Keep one editor per account and one child borrow per parent at a time.
- **Rollback and error propagation** - Edits made before a callback fails stay in the account bytes until transaction rollback. Return the error with `?` to roll back, or restore the bytes and reopen with `load_checked_mut`.
<!-- agent-rules:end -->

In an unpacked `blubber` crate archive, see the host doctest in `src/lib.rs`.

## Test the handler

1. Growing paths set `Headroom` and `Funding`, and fallible calls propagate errors with `?`.
2. Exposed load paths have tests for valid state and for rejecting a bad owner or malformed payload where those checks apply. Handlers that require authority have tests for bad signers.
3. The SBF build log has no stack frame diagnostics.
