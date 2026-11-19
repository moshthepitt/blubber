# Blubber

A Solana account gives you one byte slice. If you want to push to a list in
that slice, Blubber gives you a typed editor that works in place. A derive
generates the readers and editors for your schema. Pinocchio support handles
account borrows, growth and rent settlement.

One maintainer works on this 0.1.0 release. Verus checks the byte kernel. The
typed APIs and runtime account behaviour have separate checks and tests.
[Assurance](ASSURANCE.md) explains the bounds of each claim.

I built it because I wanted to see what is possible. I think the idea of simple
and friendly Solana unsized types is a worthy goal, and I wanted to contribute
to this area.

## Install

For the 0.1.0 release:

```sh
cargo add blubber@0.1.0 --features pinocchio
```

The equivalent dependency entry is:

```toml
[dependencies]
blubber = { version = "0.1.0", features = ["pinocchio"] }
```

The default `derive` feature generates schemas and accessors. An unpublished
[Anchor adapter](crates/blubber-anchor/README.md) is available from the repository. Rust 1.89 or newer is required. The library is `no_std` and forbids unsafe
code. The gate suite checks both the minimum Rust version and the pinned host
compiler.

## Initialize, edit and read

This example runs on host storage without an SDK or allocator:

```rust
use blubber::{List, SliceStore, Str};

#[blubber::schema(account, namespace = "example:queue", semantic_epoch = 1)]
struct Queue {
    count: u64,
    label: Str,
    values: List<u64>,
}

let mut bytes = [0; 256];
let mut store = SliceStore::new(&mut bytes);
let mut queue = Queue::initialize(&mut store, QueueInit {
    count: 0,
    label: "pending",
    values: &[10, 20],
})?;
queue.count_mut().set(1)?;
queue.label_mut().set("ready")?;
queue.values_mut().push(30)?;
let receipt = queue.commit()?;

let queue = Queue::load(&bytes[..receipt.used_len as usize])?;
assert_eq!(queue.count(), 1);
assert_eq!(queue.label(), "ready");
let sum = queue.values().try_iter().try_fold(0, |sum, value| {
    value.map(|value| sum + value)
})?;
assert_eq!(sum, 60);
# Ok::<(), blubber::Error>(())
```

For runtime accounts, use the [Pinocchio quickstart](docs/guide.md#pinocchio-accounts).
It covers owner and application authority checks, lazy and checked loads,
explicit growth budgets and commit propagation. The [complete edit tour](programs/blubber-demo/src/exercise.rs)
also runs on host stores and SBF. Treat every SBF stack warning as an error:
frames must fit 4,096 bytes. Direct mapping removes stack frame gaps on v0.
The repository's [stack checker](scripts/sbf-stack.sh) enforces this during builds.

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

Find examples and tests in the [agent guide](docs/agents.md).

## Choose features and schemas

| Feature | Adds |
| --- | --- |
| Default `derive` | `#[blubber::schema]`, `#[derive(blubber::Fixed)]` and generated accessors |
| No default features | Typed APIs with handwritten schema contracts; no SDK dependency |
| `alloc` | Owned mirrors, extraction helpers and `VecStore` |
| `pinocchio` | Safe account guards, headroom leases, rent settlement and signer/PDA creation |
| `account-runtime` | Support for runtime adapters; applications normally use `pinocchio` |
| `max-depth-16` | A sixteen-node depth budget instead of eight; identical wire format |
| `verify-on-load` | Full scans on lazy loads and initialization; opt-in |

Format 1 supports signed and unsigned integers through 128 bits, booleans,
fixed byte arrays, public keys, packed fixed structs, UTF-8 strings, options,
lists and sorted maps. Nested schemas support type and const generics; account
roots must be concrete. Enums and floating-point codecs are not provided.
See the [schema guide](docs/guide.md#schemas) and [identity and migration](docs/trust-model.md#account-identity).

Account editing defaults to `Headroom::UpTo(0)`. Equal-width edits and growth
within existing spare need no new reservation. Request positive encoded growth
beyond that spare with `EditOptions`, including directory and parent bytes.
`Max` and allocator-dependent `OnDemand` require explicit selection.

Raw byte loads and host mutable loads scan the complete active encoding.
Authenticated Pinocchio loads check owner, identity and envelope, then check
touched contents. Lazy entry requires valid initialization or migration for the
current schema followed only by approved writers. It does not certify untouched
bytes or global map order. Use checked entry after raw writes or an unaudited
handoff. The [trust model](docs/trust-model.md) explains recovery and proof limits.

Read the [guide](docs/guide.md), [recipes](docs/recipes.md),
[trust model](docs/trust-model.md) and [performance guide](docs/performance.md).
[SPEC.md](SPEC.md) defines the format and operation contracts.

## Performance

Whole instruction CU at n=1000 on SBF v0, generated from the retained comparison
data. List strings grow from eight to seventeen bytes; map growth derives new
values from old values and includes staging. Each library keeps its measured
load and capacity contract. Fixed baselines reserve 1000 slots; PinaPod reserves
seventeen-byte string slots, and Critbit uses the fixture's 128-bit key projection.

<!-- compare-headline:start -->

| Library | List sum | Map lookup | List grow all | Map grow all |
|---|---|---|---|---|
| Blubber lazy | 10,566 | 974 | 45,185 | 521,411 |
| Blubber checked | 10,854 | 22,401 | 49,177 | 525,376 |
| Borsh | 27,481 | 188,944 | 397,094 | OOM |
| Star Frame | 17,341 | 537 | 202,917 | 1,287,238 |
| Bytemuck | 10,295 | n/a | n/a | n/a |
| Sokoban | n/a | 583 | n/a | n/a |
| Zeropod | 11,306 | 426 | 516,682 | 878,116 |
| Wincode | 10,332 | 632 | 424,705 | 1,328,841 |
| PinaPod | 10,303 | n/a | 786,919 | n/a |
| Light | 10,297 | n/a | n/a | n/a |
| Anchor slab | 10,298 | n/a | n/a | n/a |
| Critbit | n/a | 1,268 | n/a | n/a |
| Manifest | n/a | 573 | n/a | n/a |
| Manifest checked | n/a | 835,565 | n/a | n/a |

<!-- compare-headline:end -->

<!-- compare-summary:start -->

Blubber lazy list sum costs 10,566 CU versus 10,295 for Bytemuck; map lookup costs 974 versus 426 for Zeropod. Blubber lazy string-list growth costs 45,185 CU versus 202,917 for Star Frame. Old-dependent string-map growth costs 521,411 versus 878,116 for Zeropod. These are v0 whole-instruction totals for the measured methods and load policies.

<!-- compare-summary:end -->

<!-- compare-arch-note:start -->

Most v3 headline values are within a few CU of v0. Blubber string-list growth differs by 3,978 to 3,984 CU. The other headline differences are at most 12 CU. See the [full data](benchmarks/COMPARE.md).

<!-- compare-arch-note:end -->

[Per-size tables](docs/performance.md#comparison-by-size) show both architectures.
The lazy list sum and Bytemuck both add 10 CU per item over the sampled
nonempty sizes. The headline map lookup hits the middle key on Blubber's
first probe. That lazy lookup and Zeropod's both have a flat cost across those
sizes. In both comparisons the whole-instruction gap is fixed overhead.
[Comparison measurements](benchmarks/compare/results/all.csv).

The [full comparison](benchmarks/COMPARE.md) includes load policies, matching idles,
heap, account sizes and failed operations. Measurements use platform-tools v1.53,
Mollusk 0.16.2, a 40,000,000-CU meter and 256 KiB heap; expensive rows can exceed a
production transaction's budget. The [receipt](benchmarks/compare/results/receipt.json)
binds measured sources, tools, ELFs and independent byte/read checks.

## Roadmap

After 0.1.0 I want to bring down the cost of a single map lookup and let the
benchmarks guide the rest of the speed work. I plan to work on client and IDL
tooling, including account decoders where the library can describe the wire
format. A TypeScript decoder and variable-size string keys for maps are on the
list. I am aiming to add enums in 0.2.

I want an external review before 1.0, which I am aiming to ship in the first
quarter of 2027.

## License

[MIT](LICENSE).
