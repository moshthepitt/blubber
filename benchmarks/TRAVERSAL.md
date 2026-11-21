# Traversal measurements

These measurements record the cost of reading and rewriting collections across SBF v0 and v3. SBF is the on-chain build target. The v0 and v3 targets are two binary architectures the toolchain emits. Each measurement depends on the workload, the caller and the run setup, so the tables below keep wins and losses together.

## Ordinary caller costs

Compare your handler with the mixed-caller files first: [`benchmarks/traversal-mixed-v0.csv`](traversal-mixed-v0.csv), [`benchmarks/traversal-mixed-v3.csv`](traversal-mixed-v3.csv), [`benchmarks/traversal-mixed-alternate-v0.csv`](traversal-mixed-alternate-v0.csv) and [`benchmarks/traversal-mixed-alternate-v3.csv`](traversal-mixed-alternate-v3.csv). Both the primary and alternate callers run the same library methods (`try_iter`, `try_for_each_mut`, `fill`, `rewrite_from`, `fill_values`, `rewrite_values` and `rewrite_values_from`) inside ordinary Rust loops and closures. The alternate caller changes the instruction dispatcher and cursor loop so you can see how caller shape moves the numbers.

CU is the runtime work measure. Whole CU is the whole instruction total, covering the checked load, the method work, session completion and return data. Idle is a matching control instruction that runs setup, the relevant field or cursor setup and completion without the collection pass. Delta is the difference between whole CU and idle.

Read runs fold decoded values and any copied map keys into a checksum so the compiler cannot drop the loop. See [`benchmarks/CU.md#read-a-row`](CU.md#read-a-row) for the column layout.

<!-- traversal-keyless:start -->

At n=1000, `rewrite_values` costs 124,947 / 124,937 CU for plain keys and
133,936 / 133,926 CU for common-prefix keys.

<!-- traversal-keyless:end -->

<!-- traversal-ordinary:start -->

| Ordinary operation | Primary slope, v0 / v3 | Alternate slope, v0 / v3 | Primary whole CU n=1000, v0 / v3 | Alternate whole CU n=1000, v0 / v3 | Policy |
|---|---|---|---|---|---|
| List cursor read | 13 / 13 | 20 / 20 | 13,988 / 13,983 | 20,993 / 20,991 | enforced |
| List cursor set | 7 / 7 | 15 / 15 | 7,986 / 7,981 | 15,992 / 15,990 | enforced |
| Map plain cursor read | 88 / 88 | 95 / 95 | 89,902 / 89,894 | 96,909 / 96,904 | enforced |
| Map plain cursor set | 109 / 109 | 132 / 132 | 110,905 / 110,897 | 133,926 / 133,921 | tracked, not enforced (user decision 2026-10-30) |
| Map common31 cursor read | 97 / 97 | 104 / 104 | 98,893 / 98,885 | 105,900 / 105,895 | enforced |
| Map common31 cursor set | 118 / 118 | 141 / 141 | 119,896 / 119,888 | 142,917 / 142,912 | tracked, not enforced (user decision 2026-10-29) |
| List empty callback | 24 / 24 | 24 / 24 | 25,134 / 25,129 | 25,140 / 25,138 | enforced |
| List read callback | 15 / 15 | 15 / 15 | 16,139 / 16,134 | 16,144 / 16,142 | enforced |
| List set callback | 7 / 7 | 7 / 7 | 8,132 / 8,127 | 8,138 / 8,136 | enforced |
| Map plain empty callback | 81 / 81 | 81 / 81 | 82,957 / 82,952 | 82,989 / 82,981 | enforced |
| Map plain read callback | 72 / 72 | 72 / 72 | 73,958 / 73,953 | 73,963 / 73,961 | enforced |
| Map plain set callback | 64 / 64 | 64 / 64 | 65,959 / 65,954 | 65,963 / 65,961 | enforced |
| Map common31 empty callback | 90 / 90 | 90 / 90 | 91,948 / 91,943 | 91,980 / 91,972 | enforced |
| Map common31 read callback | 81 / 81 | 81 / 81 | 82,949 / 82,944 | 82,954 / 82,952 | enforced |
| Map common31 set callback | 73 / 73 | 73 / 73 | 74,950 / 74,945 | 74,954 / 74,952 | enforced |

<!-- traversal-ordinary:end -->

<!-- traversal-tracked:start -->

| Tracked ordinary row | Primary whole CU n=1000, v0 / v3 | Alternate whole CU n=1000, v0 / v3 | Primary / alternate slope, v0 / v3 | Original limit: slope; whole CU n=1000 | Policy |
|---|---|---|---|---|---|
| Fixed map `cursor_set`, common31 | 119,896 / 119,888 | 142,917 / 142,912 | 118 / 118 / 141 / 141 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `cursor_set`, plain | 110,905 / 110,897 | 133,926 / 133,921 | 109 / 109 / 132 / 132 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-30) |
| Fixed map `mut_iter`, common31 | 140,568 / 140,563 | 140,600 / 140,592 | 139 / 139 / 139 / 139 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `mut_iter`, plain | 131,577 / 131,572 | 131,609 / 131,601 | 130 / 130 / 130 / 130 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `mut_try_iter`, common31 | 145,647 / 145,642 | 145,679 / 145,671 | 144 / 144 / 144 / 144 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `mut_try_iter`, plain | 136,656 / 136,651 | 136,688 / 136,680 | 135 / 135 / 135 / 135 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Indexed map `cursor_set`, common31 | 246,130 / 246,113 | 246,159 / 246,139 | 243.75 / 243.75 / 243.75 / 243.75 | ≤210; ≤212,800 | tracked, not enforced (user decision 2026-10-29) |
| Indexed map `cursor_set`, plain | 237,139 / 237,122 | 237,170 / 237,150 | 234.75 / 234.75 / 234.75 / 234.75 | ≤205; ≤207,800 | tracked, not enforced (user decision 2026-10-29) |

<!-- traversal-tracked:end -->

The rows above are named fixed-map cursor and mutable iteration passes, plus indexed-map cursor passes, where the cost missed the original goal. Their original targets stay beside the numbers so the gap is visible. Missing a CU target does not relax correctness: the independent model checks and matched idle checks still run on every exception row.

## Staging and heap

A bulk fill repeats one replacement across a collection. A rewrite supplies a replacement for every item. When a rewrite depends on the old value, the caller stages temporary values before changing the account. Heap is dynamic memory, and OOM means a run exhausted it.

In the primary checked v0 plain-key caller at 1,000 entries, the manual recipe that stages replacements in one `String` and an ends table (`try_for_each_mut` with `rewrite_values`) uses 41,008<!-- num:traversal.v0.n1000.map_rewrite.string_ends.heap_bytes --> bytes of diagnostic heap. The manual fixed-width-slice recipe in that same caller uses 33,008<!-- num:traversal.v0.n1000.map_rewrite.fixed_width.heap_bytes --> bytes. Both diagnostic watermarks exceed the default 32<!-- num:runtime.default.heap_kib --> KiB SDK heap.

`MapMut<K, Str>::rewrite_values_with` writes replacements straight into one packed output buffer and allocates an ends table only when string lengths differ. In the checked traversal caller in `programs/cu-fixture/src/mixed_operations.rs`, on SBF v0 at 1,000 items with plain keys, `rewrite_values_with` takes 255,155<!-- num:traversal.v0.n1000.map_rewrite.with.whole --> CU and 17,008<!-- num:traversal.v0.n1000.map_rewrite.with.heap_bytes --> bytes of heap, while manual batching takes 288,129<!-- num:traversal.v0.n1000.map_rewrite.batched.whole --> CU and 33,008<!-- num:traversal.v0.n1000.map_rewrite.batched.heap_bytes --> bytes of heap, saving 32,974<!-- num:traversal.v0.n1000.map_rewrite.transform_saving --> CU. The comparison caller in `benchmarks/compare/program/src/blubber_adapter.rs` also uses `rewrite_values_with`; see [`benchmarks/COMPARE.md`](COMPARE.md). For handler code, see [`docs/recipes.md#replace-a-collection`](../docs/recipes.md#replace-a-collection) and [`docs/performance.md#replace-many-values`](../docs/performance.md#replace-many-values).

<!-- traversal-recipes:start -->

| Same old-value recipe, plain keys, n=1000 | Primary CU, v0 / v3 | Alternate CU, v0 / v3 | Heap bytes |
|---|---|---|---|
| String + ends, keyless | 482,708 / 482,689 | 482,739 / 482,717 | 41,008 |
| String + ends, keyed | 542,932 / 542,913 | 542,963 / 542,941 | 105,008 |
| String + fixed-width slices, keyless | 527,684 / 527,665 | 527,715 / 527,693 | 33,008 |
| Batched reads and staged UTF-8 | 288,129 / 288,118 | 288,160 / 288,146 | 33,008 |
| Library transform | 255,155 / 250,151 | 255,186 / 250,179 | 17,008 |

<!-- traversal-recipes:end -->

<!-- traversal-manual:start -->

| Primary fixed-list operation | n=10, v0/v3 CU/item | n=100, v0/v3 CU/item | n=1000, v0/v3 CU/item |
|---|---|---|---|
| Manual get_mut + set | 956.900 / 956.300 | 872.390 / 872.330 | 863.939 / 863.933 |
| Scoped callback set | 120.200 / 119.700 | 18.320 / 18.270 | 8.132 / 8.127 |
| Cursor set | 105.600 / 105.100 | 16.860 / 16.810 | 7.986 / 7.981 |

The manual loop has a constant 863 CU/item slope on both intervals and
architectures.

<!-- traversal-manual:end -->

## Isolated diagnostics

The isolated files [`benchmarks/traversal-v0.csv`](traversal-v0.csv) and [`benchmarks/traversal-v3.csv`](traversal-v3.csv), along with the extended sweeps in [`benchmarks/traversal-extended-v0.csv`](traversal-extended-v0.csv) and [`benchmarks/traversal-extended-v3.csv`](traversal-extended-v3.csv), include isolated fixtures, with phase columns populated by separate diagnostic builds. Those builds insert markers to show where a pass spends work. Use them to inspect phase splits, not as ordinary handler costs or production limits.

## Check the evidence

An independent owned model supplies expected bytes and values. The harness checks account bytes and return data against its oracles. Each benchmark receipt records the inputs, tools, ELF hashes, model checks and baseline files for the run. Inspect [`verification/results/traversal/`](../verification/results/traversal/), including [`passes-v0.log`](../verification/results/traversal/passes-v0.log) and [`passes-v3.log`](../verification/results/traversal/passes-v3.log), and see [`ASSURANCE.md#trust-boundaries`](../ASSURANCE.md#trust-boundaries) for what the checks cover.
