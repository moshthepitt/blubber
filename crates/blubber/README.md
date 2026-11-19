# blubber

Typed borrowed readers and editors for contiguous Solana account storage.
The library is `no_std`; default `derive` generates schemas and accessors.

```toml
[dependencies]
blubber = { version = "0.1.0", features = ["pinocchio"] }
```

See the [compiling quickstart](https://github.com/moshthepitt/blubber/blob/v0.1.0/README.md#initialize-edit-and-read),
[guide](https://github.com/moshthepitt/blubber/blob/v0.1.0/docs/guide.md), [recipes](https://github.com/moshthepitt/blubber/blob/v0.1.0/docs/recipes.md) and
[trust model](https://github.com/moshthepitt/blubber/blob/v0.1.0/docs/trust-model.md). `UpTo(0)` is the default headroom;
request additional growth explicitly and propagate edit/commit errors.

## For coding agents

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

Find examples and tests in the [agent guide](https://github.com/moshthepitt/blubber/blob/v0.1.0/docs/agents.md).
