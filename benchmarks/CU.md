# Compute-unit datasets

Take the `map_fixed_n1000_lookup_hit_front_plain` row in [`benchmarks/data/scaling-v0.csv`](data/scaling-v0.csv). It probes a plain key near the front of a fixed-value map on SBF v0 and takes 2,000<!-- num:scaling.v0.n1000.map_lookup.front_hit --> CU for the whole instruction. SBF is Solana's on-chain build target, and v0 and v3 are two SBF binary architectures.

I think a measurement's scope depends on the workload, the caller and the run setup, so check the column rules below before comparing rows. For narrative summaries, see [`docs/performance.md#find-the-numbers`](../docs/performance.md#find-the-numbers). For cross-library methods, see [`benchmarks/COMPARE.md#method`](COMPARE.md#method). For caller-side map rewrites, see [`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](TRAVERSAL.md#ordinary-caller-costs).

## Choose a dataset

Each family has a v0 and a v3 CSV under `benchmarks/data/`:

- **Scaling** - [`benchmarks/data/scaling-v0.csv`](data/scaling-v0.csv) and [`benchmarks/data/scaling-v3.csv`](data/scaling-v3.csv) measure collections across size and depth. Search for a scenario name such as `map_fixed_n1000_lookup_hit_front_plain` to find its row.
- **Lease** - [`benchmarks/data/lease-v0.csv`](data/lease-v0.csv) and [`benchmarks/data/lease-v3.csv`](data/lease-v3.csv) measure headroom, account growth and rent settlement.
- **Extended** - [`benchmarks/data/extended-v0.csv`](data/extended-v0.csv) and [`benchmarks/data/extended-v3.csv`](data/extended-v3.csv) measure allocator choices, checked mutable paths, `ref_methods` and `to_owned`.
- **Runtime** - [`benchmarks/data/runtime-v0.csv`](data/runtime-v0.csv) and [`benchmarks/data/runtime-v3.csv`](data/runtime-v3.csv) measure invocation, transaction, sequence and scalar runs, including expected errors.

## Read a row

CU is the runtime work measure. Whole CU is the instruction total. Idle is a matching setup-and-finish control run, and delta subtracts idle from whole CU. Check the baseline policy before comparing deltas, because subtracting an idle with a different opening or validation policy leaves that extra work in the delta:

- Scaling CSVs pick a read, checked read or mutable idle for the operation family and require the baseline to preserve the starting bytes.
- Lease CSVs use the `same` action with matching headroom, funding, refund and reservation settings.
- Extended CSVs use a lazy idle for checked mutable rows, so their delta includes validation. The `ref_methods` and `to_owned` rows use checked idles.
- Runtime CSVs report whole CU only and have no idle or delta columns.

Diagnostic phase columns come from separately built calibrated markers. Use them to see where time goes inside an operation, because they are not uninstrumented instruction totals. In those columns, `moved_bytes` counts relocated byte ranges, and `ends` counts visited offset entries including unchanged ones.

Capacity is available storage. Used length covers the header and live payload. Spare is the physical space beyond used length, so storage and raw-lease physical byte columns include spare while used-byte columns do not. Depth counts schema nodes from root to leaf.

Lease CSVs also record opened and peak bytes, re-leases, zeroing, funding CPIs, top-ups, refunds, probe and resize counts, and allocator slot bytes. Heap is dynamic memory, and OOM means a run exhausted it. Extended CSVs record the allocator policy.

Runtime CSVs record scope, expected results, and before-and-after bytes and lamports. Tests show that account growth is capped by the 10<!-- num:runtime.account.max_mib --> MiB account limit, the 10,240<!-- num:runtime.invocation.growth_bytes -->-byte invocation limit and shared runtime limits. A local headroom request can therefore grant less room. On bank rent errors, the account columns show the state the harness returned. Profile 0<!-- num:runtime.profile.copy --> copies account data, profile 1<!-- num:runtime.profile.simd0460 --> uses SIMD-0460 copy, and profile 2<!-- num:runtime.profile.direct --> uses direct mapping, which maps the account data buffer directly into guest memory.

## Sample rows

<!-- cu-samples:start -->

| SBF | Scenario | Whole CU | Baseline CU | Difference CU |
|---|---|---|---|---|
| v0 | `list_fixed_n10_validate_all_plain` | 494 | 221 | 273 |
| v3 | `list_fixed_n10_validate_all_plain` | 490 | 217 | 273 |
| v0 | `map_fixed_n1000_lookup_hit_front_plain` | 2,000 | 225 | 1,775 |
| v3 | `map_fixed_n1000_lookup_hit_front_plain` | 1,996 | 221 | 1,775 |
| v0 | `lease_n1000_UpTo32_grow8_prepaid` | 794 | 548 | 246 |
| v3 | `lease_n1000_UpTo32_grow8_prepaid` | 794 | 549 | 245 |
| v0 | `list_fixed_n1_get_mut_front_plain_no_alloc` | 1,817 | 485 | 1,332 |
| v3 | `list_fixed_n1_get_mut_front_plain_no_alloc` | 1,813 | 481 | 1,332 |
| v0 | `fresh_windows_profile2` | 1,166 | n/a | n/a |
| v3 | `fresh_windows_profile2` | 1,162 | n/a | n/a |

<!-- cu-samples:end -->

## Reproduce

Run [`scripts/reproduce.sh`](../scripts/reproduce.sh) from the repository root. It checks the SBF toolchain first.

- `bash scripts/reproduce.sh cu` runs the uninstrumented compute-unit fixtures on v0 and v3 with a raised meter so longer scaling cases finish.
- `bash scripts/reproduce.sh sbf` runs the SBF invocation cases through the Mollusk runtime suite with the default transaction-scale meter.

A receipt records the inputs, tools, hashes and evidence for a run. The committed CU run logs are [`verification/results/performance/cu-v0.log`](../verification/results/performance/cu-v0.log) and [`verification/results/performance/cu-v3.log`](../verification/results/performance/cu-v3.log).
