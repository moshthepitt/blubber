# Blubber

Blubber is a Rust library for Solana programs that stores lists, strings and maps inside an account and edits them in place.

I have been writing Solana programs since the first version of [Anchor](https://github.com/otter-sec/anchor). Back then, if you wanted a map, the advice was to use a PDA per entry. That works, but every instruction and client call has to pass around all those extra accounts. The other options have their own costs: [`bytemuck`](https://github.com/Lokathor/bytemuck) and Anchor zero-copy make you pick a maximum length up front and pay rent for empty slots, while [Borsh](https://github.com/near/borsh-rs) copies the whole account onto the heap and writes it back. When I was one of the developers on the Star Atlas programs, we needed variable-size types badly enough that we left Anchor and wrote our own framework, [Star Frame](https://github.com/staratlasmeta/star_frame).

So I built Blubber to solve that problem as a library, not a framework. You declare an account struct with `List`, `Str` and `Map` fields, and Blubber generates typed readers and editors that work on the account bytes directly. That lets you keep a player inventory, an allowlist or a registry in one account. When data grows, the account grows and the handler pays the rent difference. It plugs into [Pinocchio](https://github.com/anza-xyz/pinocchio) today, and Anchor support is coming.

Here is a queue account with an authority and a list of `u64`:

<!-- code:schema -->
```rust
use blubber::{Address, List};

/// An append-only queue of numbers.
#[blubber::schema(account)]
pub struct Queue {
    /// Signer allowed to append.
    pub authority: Address,
    /// Values in insertion order.
    pub values: List<u64>,
}
```

And here is a Pinocchio handler that appends a value to that queue, checks the authority signed, and tops up rent from the payer when the account grows:

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

The account has no spare bytes when the edit opens, so `Headroom::UpTo(8)` asks for up to eight more, enough for one `u64`. `Funding::TopUp` moves the extra rent from `payer` when the account grows. The closure runs on the account bytes, so nothing is decoded into a `Vec`.

Other in-place libraries exist too, including Star Frame and [Zeropod](https://github.com/blueshift-gg/zeropod). Below is the measured cost of four operations on SBF v0 with 1,000 items:

| Operation (1,000 items, SBF v0) | Blubber (lazy load) | Blubber (checked load) | Borsh | Fastest other library |
| --- | --- | --- | --- | --- |
| Sum a `List<u64>` | 10,565<!-- num:cmp.v0.n1000.list_sum.lazy --> CU | 10,854<!-- num:cmp.v0.n1000.list_sum.checked --> CU | 27,481<!-- num:cmp.v0.n1000.list_sum.borsh --> CU | 10,295<!-- num:cmp.v0.n1000.list_sum.bytemuck --> CU (`bytemuck`) |
| Look up one key in a `Map` | 973<!-- num:cmp.v0.n1000.map_lookup.lazy --> CU | 22,401<!-- num:cmp.v0.n1000.map_lookup.checked --> CU | 188,944<!-- num:cmp.v0.n1000.map_lookup.borsh --> CU | 426<!-- num:cmp.v0.n1000.map_lookup.zeropod --> CU (Zeropod) |
| Push one `u64` to a `List<u64>` | 1,403<!-- num:cmp.v0.n1000.list_push.lazy --> CU | 1,739<!-- num:cmp.v0.n1000.list_push.checked --> CU | 50,642<!-- num:cmp.v0.n1000.list_push.borsh --> CU | 356<!-- num:cmp.v0.n1000.list_push.anchor_slab --> CU (Anchor slab) |
| Set every string in a `List<Str>` to a longer string | 45,110<!-- num:cmp.v0.n1000.list_grow_constant.lazy --> CU | 49,127<!-- num:cmp.v0.n1000.list_grow_constant.checked --> CU | 397,094<!-- num:cmp.v0.n1000.list_grow_constant.borsh --> CU | 202,917<!-- num:cmp.v0.n1000.list_grow_constant.star_frame --> CU (Star Frame) |

A lazy load checks the account header up front and checks fields as you read them. A checked load scans the whole account before your code runs, so its cost grows with the account. That is why the checked lookup costs so much more than the lazy one.

At 1,000 items, lazy Blubber used fewer CU than Borsh on every operation Borsh could finish, and Borsh ran out of heap on 1<!-- num:cmp.v0.n1000.summary.borsh_heap_exhausted --> more. Rewriting many strings is where Blubber is the fastest library measured: setting every string to a longer one costs about 4.5<!-- num:cmp.v0.n1000.derived.constant_fill_star_frame_ratio_rounded --> times less CU than Star Frame and about 9<!-- num:cmp.v0.n1000.derived.constant_fill_borsh_ratio_rounded --> times less than Borsh, and uses no heap. A single push or lookup costs about 550<!-- num:cmp.v0.n1000.derived.map_lookup_zeropod_extra_rounded --> to 1,050<!-- num:cmp.v0.n1000.derived.list_push_anchor_slab_extra_rounded --> CU more than the fastest library measured; see [`benchmarks/COMPARE.md`](benchmarks/COMPARE.md) and [`benchmarks/compare/GAPS.md`](benchmarks/compare/GAPS.md).

Blubber's source has no `unsafe` code. [Verus](https://github.com/verus-lang/verus) checks the byte-moving core, with all 1,707<!-- num:proof.verus.verified --> proof obligations passing, and there is no external audit yet. See [`ASSURANCE.md`](ASSURANCE.md).

## Install

Run `cargo add blubber@0.1.1 --features pinocchio`<!-- num-code:"cargo add blubber@{{num:release.blubber.version}} --features pinocchio" --> on Rust 1.89.0<!-- num:release.rust.minimum -->+. See the [guide](docs/guide.md#create-an-account) for crate features.

## Release status

`blubber`, `blubber-core` and `blubber-derive` 0.1.1<!-- num:release.blubber.version --> are on crates.io and docs.rs. See [`CHANGELOG.md`](CHANGELOG.md#011).

## What is missing

- Enums and floats.
- String map keys, because map keys are fixed-size today, such as integers and public keys.
- Rewriting every string in a list from its old value in one call, which maps already support.
- A TypeScript decoder and stock Anchor account decoding.

## Roadmap

I plan to build:

- Enums
- String map keys
- Cheaper single lookups
- Client and IDL tooling, and a TypeScript decoder
- External review

## Where to go next

- [Write an account and handler](docs/guide.md#create-an-account)
- [Size headroom and replace collections](docs/recipes.md#replace-a-collection)
- [Choose lazy or checked loads](docs/trust-model.md#choose-a-load)
- [Inspect benchmark numbers](docs/performance.md#find-the-numbers)
- [Fix a version mismatch](docs/guide.md#fix-a-version-mismatch)
- Coding agents: [`docs/agents.md`](docs/agents.md#find-the-task).

## Licence

MIT. See [`LICENSE`](LICENSE).
