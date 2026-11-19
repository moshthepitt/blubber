# Choosing an access pattern

Measure your whole handler with the schema, load policy and runtime settings
you want to deploy. Container microbenchmarks include different authentication, checksum
and commit work, so their totals need not match another caller. Whole CU is
the instruction total; a signed delta subtracts a matched idle and is not an
isolated instruction total. [CU scope](../benchmarks/CU.md) describes the fixtures.

Use ordinary point reads for a small subset. For full fixed-value map passes,
`try_values` avoids decoding keys. Batched string iteration scans all payload
bytes during setup; partial reads can cost more than ordinary iteration.
[Reader examples](guide.md#reading-lists-maps-and-strings) show both policies and error behaviour.

Use `fill` to give every list item the same value. In the retained lazy
`List<u64>` fixture at n=1000, `fill` costs 4,843 CU on v0 and 4,840 on v3.
`try_for_each_mut` with a constant setter costs 7,924 and 7,921 CU.
[Comparison receipt](../benchmarks/compare/results/receipt.json) · [Measured rows](../benchmarks/compare/results/all.csv).

Use a cursor or `try_for_each_mut` when each edit needs its own logic. Growing
every indexed value this way can repeat suffix movement and directory patching,
with quadratic total work. Use `fill_values` for a constant map replacement or
a whole-container rewrite when you can stage the replacements independently.

For a single equal-size string replacement, use a scoped `with_mut` edit. In
the 2026-11-12 point-edit receipt, the middle replacement in a lazy
1000-item string list cost 1,537 CU on v0 and 1,533 CU on v3. The callers
using `get_mut` cost 2,491 and 2,486 CU. Both callers used `Max` headroom.
[Scoped edit contract](guide.md#scoped-child-edits).

Use byte values if your program only needs bytes. `[u8; N]` stores fixed-width
bytes, and `List<u8>` stores counted bytes. Both skip UTF-8 validation. Ordinary
`Str` reads call `core::str::from_utf8` to return `&str`. A checked load also
validates the strings during its scan.
[Byte and string encodings](../SPEC.md#conventions-and-scalar-encodings).

Old-dependent rewrites require caller storage. The ordinary string-map recipe
copies each old value twice and appends `!`, records ends in `Vec<usize>`, then
borrows staged slices for `rewrite_values`. At n=1000 the retained String/ends
recipe uses 41,008 heap bytes; a fixed-width-slice variant uses 33,008 bytes.
Both exceed the default 32 KiB heap and need an explicit `RequestHeapFrame`.
These are fixture observations, not a general allocator bound. Rewrite consumes
the whole map, so inputs cannot be submitted in arbitrary chunks.

When you know the growth budget, use `Headroom::UpTo(n)` with the peak extra
encoded bytes beyond opening spare. Include directory and parent growth.
`UpTo(0)` performs no speculative opening resize. Positive `UpTo(n)` requests
at most n extra physical bytes and may grant less. In the retained prepaid raw
lease fixture at n=1000, an eight-byte growth costs 848 CU on v0 and 837 CU on
v3 with `UpTo(32)`. With `Max` it costs 863 and 852 CU.
[v0 scaling receipt](../benchmarks/CU-v0.md) · [v3 scaling receipt](../benchmarks/CU-v3.md).

`Max` probes through the invocation window. SDK-local capacity does not certify
transaction growth resources or ancestor CPI limits. `OnDemand` requires a
working allocator and retains a handle slot per growth attempt, including
failed attempts. Bound reopens/growth and all other allocations together.

Eight ordinary map groups remain tracked against their original traversal
bounds in both caller layouts. They retain byte/read/idle checks; tracking is
not a claim that they meet those bounds. Fixed maps have plain/common31
`mut_iter`, `mut_try_iter` and `cursor_set` groups (six); indexed maps have
plain/common31 `cursor_set` groups (two). The fixed bound is 120 CU/item and
`120*n+2500`; indexed cursor bounds are 205 and `205*n+2800` for plain keys,
210 and `210*n+2800` for common31 keys. The
[ordinary measurements](../benchmarks/TRAVERSAL.md#ordinary-caller-costs) show
both architectures and layouts, including all sampled totals and slopes.

The [comparison](../benchmarks/COMPARE.md) records account bytes, heap, load
policies and failed rows alongside CU. A large measurement meter allows slow
rows to finish; it is not a production transaction budget.

## Comparison by size

These retained whole-instruction observations use the same methods and data as
the README table. Each cell is checked / lazy; both architectures and all
sampled sizes are shown. The full comparison records matching idles and load
contracts.

The headline lookup is a first-probe hit for Blubber. The separate scaling
fixture measures fixed positions. For a lazy `Map<Pubkey, u64>` with 1000
plain keys, a front hit costs 2,009 CU on v0 and 2,005 CU on v3. An end hit
costs 1,930 and 1,926 CU, and a middle miss costs 1,792 and 1,788 CU.
[v0 scaling receipt](../benchmarks/CU-v0.md) · [v3 scaling receipt](../benchmarks/CU-v3.md).

<!-- compare-size:start -->

Whole CU by size, checked / lazy.

| Arch | n | List sum | Map lookup | List constant fill | Map old-dependent growth |
|---|---|---|---|---|---|
| v0 | 0 | 849 / 561 | 1,033 / 637 | 1,995 / 1,813 | 2,666 / 2,491 |
| v0 | 1 | 864 / 576 | 1,406 / 974 | 2,493 / 2,155 | 3,764 / 3,453 |
| v0 | 10 | 954 / 666 | 1,611 / 974 | 3,081 / 2,615 | 8,599 / 8,160 |
| v0 | 100 | 1,854 / 1,566 | 3,501 / 974 | 7,102 / 6,485 | 55,487 / 54,897 |
| v0 | 1000 | 10,854 / 10,566 | 22,401 / 974 | 49,177 / 45,185 | 525,376 / 521,411 |
| v3 | 0 | 846 / 558 | 1,030 / 634 | 1,986 / 1,808 | 2,656 / 2,487 |
| v3 | 1 | 861 / 573 | 1,403 / 971 | 2,482 / 2,150 | 3,752 / 3,449 |
| v3 | 10 | 951 / 663 | 1,608 / 971 | 3,099 / 2,639 | 8,587 / 8,156 |
| v3 | 100 | 1,851 / 1,563 | 3,498 / 971 | 7,480 / 6,869 | 55,475 / 54,893 |
| v3 | 1000 | 10,851 / 10,563 | 22,398 / 971 | 53,155 / 49,169 | 525,364 / 521,407 |

<!-- compare-size:end -->
