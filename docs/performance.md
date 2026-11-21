# Performance guide

## Measure the whole handler

In the comparison fixture on SBF v0, looking up the middle key in a 1,000-entry fixed-value map takes 974<!-- num:cmp.v0.n1000.map_lookup.lazy --> CU with a lazy load and 22,401<!-- num:cmp.v0.n1000.map_lookup.checked --> CU with a checked load. That middle key hits Blubber's first binary-search probe. In the separate scaling fixture, a front hit takes 2,000<!-- num:scaling.v0.n1000.map_lookup.front_hit --> CU, a middle miss takes 1,783<!-- num:scaling.v0.n1000.map_lookup.middle_miss --> CU, an end hit takes 1,921<!-- num:scaling.v0.n1000.map_lookup.end_hit --> CU, and a front hit where keys share a common prefix takes 4,603<!-- num:scaling.v0.n1000.map_lookup.common_prefix_front_hit --> CU.

Whole CU is the total cost of the instruction, including setup, account loading and any commit. Idle is a control run that sets up and finishes without the operation. Delta is whole CU minus idle. An idle baseline can already include a checked scan or an account resize, depending on the dataset.

In my view, a benchmark row only holds for its own caller and setup, so measure the whole instruction in your handler rather than copying a delta from a table. Match your target SBF binary architecture (v0 or v3), collection size, key position and prefix, load policy, growth budget and allocator. For choosing between a lazy load and a checked load, see [`docs/trust-model.md#choose-a-load`](trust-model.md#choose-a-load).

## Read one item or many

Fallible reads return a Rust error when they detect invalid account bytes. `try_get` reports absence when a lookup finds no item, while `try_iter` yields a `Result` per item.

Call `try_get` when your handler needs only a few items, because scanning the whole collection pays for entries you never use. Several single-item lookups, inserts and removals still cost more CU in Blubber than in the fastest other library in the comparison; see [`benchmarks/compare/GAPS.md#gaps-within-the-same-load-policy`](../benchmarks/compare/GAPS.md#gaps-within-the-same-load-policy).

When you need every item in a map whose values implement `Fixed` and do not care about the keys, call `try_values`. It reads the values without decoding the keys.

For a `List<Str>` or `Map<K, Str>`, `try_iter_batched` scans the string payload during setup and checks item ends as it yields values. Use `try_iter_batched` when you walk the whole collection. Stick to `try_get` or `try_iter` when you only inspect a small subset or stop early.

## Replace many values

Growing strings one item at a time in an indexed list or map can move the following bytes and patch the offset directory on every step. Across N growths, byte movement can be quadratic. Bulk fill and rewrite methods avoid that repeated shifting.

Fill repeats one replacement across every slot. On a 1,000-item `List<u64>` on SBF v0, replacing every element with a constant takes 4,843<!-- num:cmp.v0.n1000.list_set.fill.lazy --> whole CU with `fill`, versus 7,924<!-- num:cmp.v0.n1000.list_set.callback.lazy --> whole CU with `try_for_each_mut`. Use `fill` on a list or `fill_values` on a map when every item gets the same value.

Rewrite supplies all replacements in one pass. When each new string is built from the previous string in that slot, staging temporary values on the heap before `rewrite_from` or `rewrite_values` can blow the default 32<!-- num:runtime.default.heap_kib --> KiB SDK bump heap. In the 1,000-entry checked map caller on SBF v0, staging `String` data plus end offsets uses 41,008<!-- num:traversal.v0.n1000.map_rewrite.string_ends.heap_bytes --> bytes of heap, and staging fixed-width slices uses 33,008<!-- num:traversal.v0.n1000.map_rewrite.fixed_width.heap_bytes --> bytes. Both exceed the default heap and need a larger heap to run; see [`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](../benchmarks/TRAVERSAL.md#ordinary-caller-costs).

With `alloc`, call `rewrite_values_with` on a `MapMut<K, Str>` to build each replacement from the previous string within an explicit `output_capacity` byte bound. In the checked traversal caller on SBF v0 at 1,000 items ([`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](../benchmarks/TRAVERSAL.md#ordinary-caller-costs)), `rewrite_values_with` takes 255,155<!-- num:traversal.v0.n1000.map_rewrite.with.whole --> CU and 17,008<!-- num:traversal.v0.n1000.map_rewrite.with.heap_bytes --> bytes of heap, compared with 288,129<!-- num:traversal.v0.n1000.map_rewrite.batched.whole --> CU and 33,008<!-- num:traversal.v0.n1000.map_rewrite.batched.heap_bytes --> bytes for manual batching (32,974<!-- num:traversal.v0.n1000.map_rewrite.transform_saving --> CU less). For working code, see [`docs/recipes.md#replace-a-collection`](recipes.md#replace-a-collection). The contract is in [`SPEC.md#walks-fill-and-rewrite`](../SPEC.md#walks-fill-and-rewrite).

## Budget growth and memory

Headroom is the extra physical space an edit session asks for. Prefer `Headroom::UpTo(bytes)` when you know the peak extra bytes your edit needs. `Headroom::Max` probes the local growth budget, and `Headroom::OnDemand` resizes during edits and allocates on growth attempts. On Solana, the runtime caps an account at 10<!-- num:runtime.account.max_mib --> MiB and caps its growth at 10,240<!-- num:runtime.invocation.growth_bytes --> bytes per invocation. There is also a separate shared transaction growth budget.

The ordinary demo program runs without a heap allocator. If your handler uses `OnDemand`, owned helpers or manual staging, budget that heap alongside whatever else your instruction allocates.

An SBF call frame is limited to 4,096<!-- num:runtime.sbf.stack_frame_bytes --> bytes of stack space. Run `bash scripts/sbf-stack.sh MANIFEST PACKAGE ARCH OUTPUT_DIR LOG` to force a final-crate rebuild and reject stack-frame warnings that `cargo` otherwise lets pass.

## Find the numbers

Each benchmark page answers a different question:

- [`benchmarks/CU.md`](../benchmarks/CU.md) and [`#read-a-row`](../benchmarks/CU.md#read-a-row) explain the scaling fixtures, memory profiles (including direct mapping) and CSV columns.
- [`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](../benchmarks/TRAVERSAL.md#ordinary-caller-costs) records caller and staging heap high-water marks.
- [`benchmarks/COMPARE.md#method`](../benchmarks/COMPARE.md#method) covers the cross-library harness, its 256<!-- num:cmp.environment.heap_kib --> KiB configured heap and the benchmark receipt that records the compiler and ELF hashes.
- [`benchmarks/compare/GAPS.md#gaps-within-the-same-load-policy`](../benchmarks/compare/GAPS.md#gaps-within-the-same-load-policy) lists large whole-instruction CU gaps under comparable load policies.
