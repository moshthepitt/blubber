# Traversal release evidence

The [ordinary v0 caller](traversal-mixed-v0.csv) and
[ordinary v3 caller](traversal-mixed-v3.csv) measure the same methods as the
[alternate v0 caller](traversal-mixed-alternate-v0.csv) and
[alternate v3 caller](traversal-mixed-alternate-v3.csv). The second caller
uses a different dispatcher and cursor loop. Both use ordinary Rust loops
and closures.

Mollusk 0.16.2 runs the fixtures on SBF v0 and v3. An independent model checks
complete account bytes and return values. Each reported delta has a matching
idle measurement.

## Ordinary caller costs

The mixed lane checks 390 rows per architecture in each caller layout at
n=0/1/10/100/1000. Every byte/read/idle check remains enforced, including
tracked rows. Every performance group outside the named tracked set must meet
its original traversal limit in both layouts. Six old-value recipe groups (30 samples)
are report-only; their byte/read/idle oracles remain required.
Indexed reference/mutable iteration rows include matching read idles and fixed
per-row targets. These iteration inputs contain distinct eight-byte values. An order-sensitive
result consumes all key and value bytes; complete storage bytes remain checked.

The keyless API avoids collecting map keys. Its isolated varied 0..17 rewrite
must have both whole slopes at most 200 CU/item, every whole total at most
`200*n+3000`, and a whole total no greater than the keyed API at every n.
<!-- traversal-keyless:start -->

At n=1000, `rewrite_values` costs 127,945 / 127,935 CU for plain keys and
136,934 / 136,924 CU for common-prefix keys.

<!-- traversal-keyless:end -->

The ordinary "map grow from old values" recipe reads distinct eight-byte old
values through `try_for_each_mut`, copies each twice with `String::push_str`,
and appends `!`. A `Vec<usize>` records each end. After the callback ends, it
collects `&buf[a..b]` slices and calls `rewrite_values` once for the whole map.
The fixture measures the same staging and callback with keyed
`rewrite_values_from`, collecting keys as well. Both are report-only recipes;
every byte/read/idle oracle and bencher total remains required.

```rust
let mut buf = String::with_capacity(17 * n);
let mut ends = Vec::with_capacity(n);
root.value_mut().try_for_each_mut(|_, child| {
    let old = child.try_get()?;
    buf.push_str(old);
    buf.push_str(old);
    buf.push('!');
    ends.push(buf.len());
    Ok(())
})?;
let mut a = 0;
let values: Vec<&str> = ends.iter().map(|&b| {
    let value = &buf[a..b];
    a = b;
    value
}).collect();
root.value_mut().rewrite_values(&values)?;
```

On SBF, the keyless recipe requests `17*n + 8*n + 16*n` caller bytes. With
the pinned allocator's cursor word and alignment, heap use at
n=0/1/10/100/1000 is 0/56/424/4,112/41,008 bytes. At n=1000 callers need
`RequestHeapFrame` above the default 32 KiB heap; these measurements request
256 KiB. `rewrite_values` requires one input for every current map entry, so
callers cannot submit arbitrary chunks to fit the default heap.

The lower-heap fixed-width variant omits the ends vector and takes slices at
`17*i..17*(i+1)`. It uses 0/48/344/3,312/33,008 heap bytes at the same sizes,
so n=1000 still needs `RequestHeapFrame` above 32 KiB. The keyed recipe adds
`32*n` key bytes and collects `48*n` entry bytes: heap use is
0/120/1,064/10,512/105,008 bytes. Keyless rewrite itself needs no allocator;
these allocations belong to staging.

<!-- traversal-recipes:start -->

| Same old-value recipe, plain keys, n=1000 | Primary CU, v0 / v3 | Alternate CU, v0 / v3 | Heap bytes |
|---|---|---|---|
| String + ends, keyless | 485,745 / 485,729 | 485,776 / 485,757 | 41,008 |
| String + ends, keyed | 542,961 / 542,945 | 542,992 / 542,973 | 105,008 |
| String + fixed-width slices, keyless | 530,721 / 530,705 | 530,752 / 530,733 | 33,008 |

<!-- traversal-recipes:end -->

All three use the same old values and callback read pass. The keyed row also
collects copied keys; its rewrite receives those keys with the same derived
replacement slices. These replace the earlier comparison against a probe with
static replacements. Both producer tables retain all five sizes and key shapes.
The reviewer measured the String + ends variant at 516,014/515,999 CU and the
fixed-width variant at 560,991/560,976 in the larger recipe-probe dispatcher.
Whole totals depend on the surrounding caller.

Indexed-map `try_iter` read passes in that probe cost 660,767/657,767 CU at
n=1000, versus 158,109 for callback reads and 176,229 for cursor reads on both
architectures. These diagnostic passes have overhead subtracted and are not
additive phases. Use callbacks or cursors for hot indexed-map reads when the
caller has mutable access.

Fixed list iteration, cursor reads/sets and callbacks retain 30 CU/item whole
slopes and `30*n+300` common-load deltas. Enforced fixed map rows retain
120 CU/item slopes and `120*n+2500` whole totals. Indexed eight-byte rows use
[the specified per-row pairs](../SPEC.md),
with the two ordinary indexed map cursor setters tracked below. Fill retains
its `100*n+600` common delta at n>=10, 120 CU/item slopes and `120*n+2500`
total; varied rewrite retains 200 and `200*n+3000`. Both intervals and all five
totals are checked for enforced groups. Read-only rows load an immutable root. Mutable rows load and commit. Swapped idles and missing samples fail.

Every tracked sample checks complete bytes or read results, byte geometry,
the matching idle and the bencher total. The CSVs label these samples as
tracked. The receipts record the limits used by the checks.
All isolated limits remain enforced. Eight ordinary caller groups are tracked
against their original bounds, with complete byte/read/idle checks in both
layouts. Fixed-map groups retain 120 CU/item and `120*n+2500`; indexed cursor
set retains 205 and `205*n+2800` for plain keys, 210 and `210*n+2800` for common31.
Tracking does not establish that a group meets its original bound. Every other
ordinary performance group remains enforced. The measured table below names
all eight groups and shows both architectures and layouts.

Ordinary program cost depends on the surrounding caller. The tables retain
both layouts; slopes are the 100→1000 interval. The CSVs retain both intervals
and all n=0/1/10/100/1000 totals.

<!-- traversal-ordinary:start -->

| Ordinary operation | Primary slope, v0 / v3 | Alternate slope, v0 / v3 | Primary whole CU n=1000, v0 / v3 | Alternate whole CU n=1000, v0 / v3 | Policy |
|---|---|---|---|---|---|
| List cursor read | 13 / 13 | 20 / 20 | 13,978 / 13,975 | 20,982 / 20,980 | enforced |
| List cursor set | 7 / 7 | 15 / 15 | 7,976 / 7,973 | 15,981 / 15,979 | enforced |
| Map plain cursor read | 88 / 88 | 95 / 95 | 89,892 / 89,886 | 96,898 / 96,893 | enforced |
| Map plain cursor set | 109 / 109 | 132 / 132 | 110,895 / 110,889 | 133,915 / 133,910 | tracked, not enforced (user decision 2026-10-30) |
| Map common31 cursor read | 97 / 97 | 104 / 104 | 98,883 / 98,877 | 105,889 / 105,884 | enforced |
| Map common31 cursor set | 118 / 118 | 141 / 141 | 119,886 / 119,880 | 142,906 / 142,901 | tracked, not enforced (user decision 2026-10-29) |
| List empty callback | 24 / 24 | 24 / 24 | 25,124 / 25,121 | 25,129 / 25,127 | enforced |
| List read callback | 15 / 15 | 15 / 15 | 16,129 / 16,126 | 16,133 / 16,131 | enforced |
| List set callback | 7 / 7 | 7 / 7 | 8,122 / 8,119 | 8,127 / 8,125 | enforced |
| Map plain empty callback | 81 / 81 | 81 / 81 | 82,947 / 82,944 | 82,979 / 82,973 | enforced |
| Map plain read callback | 72 / 72 | 72 / 72 | 73,948 / 73,945 | 73,952 / 73,950 | enforced |
| Map plain set callback | 64 / 64 | 64 / 64 | 65,949 / 65,946 | 65,952 / 65,950 | enforced |
| Map common31 empty callback | 90 / 90 | 90 / 90 | 91,938 / 91,935 | 91,970 / 91,964 | enforced |
| Map common31 read callback | 81 / 81 | 81 / 81 | 82,939 / 82,936 | 82,943 / 82,941 | enforced |
| Map common31 set callback | 73 / 73 | 73 / 73 | 74,940 / 74,937 | 74,943 / 74,941 | enforced |

<!-- traversal-ordinary:end -->

The tracked groups have slope and whole-instruction bounds. All
8 groups below remain present in each layout; no other group is exempt.

<!-- traversal-tracked:start -->

| Tracked ordinary row | Primary whole CU n=1000, v0 / v3 | Alternate whole CU n=1000, v0 / v3 | Primary / alternate slope, v0 / v3 | Original limit: slope; whole CU n=1000 | Policy |
|---|---|---|---|---|---|
| Fixed map `cursor_set`, common31 | 119,886 / 119,880 | 142,906 / 142,901 | 118 / 118 / 141 / 141 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `cursor_set`, plain | 110,895 / 110,889 | 133,915 / 133,910 | 109 / 109 / 132 / 132 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-30) |
| Fixed map `mut_iter`, common31 | 140,558 / 140,555 | 140,590 / 140,584 | 139 / 139 / 139 / 139 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `mut_iter`, plain | 131,567 / 131,564 | 131,599 / 131,593 | 130 / 130 / 130 / 130 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `mut_try_iter`, common31 | 145,637 / 145,634 | 145,669 / 145,663 | 144 / 144 / 144 / 144 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Fixed map `mut_try_iter`, plain | 136,646 / 136,643 | 136,678 / 136,672 | 135 / 135 / 135 / 135 | ≤120; ≤122,500 | tracked, not enforced (user decision 2026-10-29) |
| Indexed map `cursor_set`, common31 | 246,117 / 246,102 | 246,146 / 246,128 | 243.75 / 243.75 / 243.75 / 243.75 | ≤210; ≤212,800 | tracked, not enforced (user decision 2026-10-29) |
| Indexed map `cursor_set`, plain | 237,126 / 237,111 | 237,157 / 237,139 | 234.75 / 234.75 / 234.75 / 234.75 | ≤205; ≤207,800 | tracked, not enforced (user decision 2026-10-29) |

<!-- traversal-tracked:end -->

The manual row calls `root.value_mut().get_mut(i)?.unwrap().set(123)?`
for every fixed-list ordinal. These whole CU/item values include a validated
load, commit and return data, with no baseline subtraction.

<!-- traversal-manual:start -->

| Primary fixed-list operation | n=10, v0/v3 CU/item | n=100, v0/v3 CU/item | n=1000, v0/v3 CU/item |
|---|---|---|---|
| Manual get_mut + set | 955.900 / 955.500 | 872.290 / 872.250 | 863.929 / 863.925 |
| Scoped callback set | 119.200 / 118.900 | 18.220 / 18.190 | 8.122 / 8.119 |
| Cursor set | 104.600 / 104.300 | 16.760 / 16.730 | 7.976 / 7.973 |

The manual loop has a constant 863 CU/item slope on both intervals and
architectures.

<!-- traversal-manual:end -->

Per-call selection work does not grow with n for this fixed
list; whole CU/item falls as its fixed setup cost is shared across more calls.
Use a cursor or `try_for_each_mut` for per-item edits. Map `get_mut` also
repeats lookup and selection. Use `fill` to replace every item with the same
value. [Performance guide](../docs/performance.md).

## Isolated diagnostics

[The v0 table](traversal-v0.csv) and [v3 table](traversal-v3.csv) cover fixed/indexed
lists and Pubkey maps at n=0/1/10/100/1000. Every method has its own operation code.
`get_lookup` means list ordinal selection or map key lookup. Both infallible and
fallible methods are separate. Immutable and mutable handles have separate rows;
`to_owned` exists on immutable collection readers, with selected-object and root
copies measured separately. Each copies once. Fixed cursor and callback rows here use
fixture-only isolation and are labelled diagnostics; use the mixed caller for
ordinary cursor and callback costs. The
bundled rows in CU.md retain independent regression checks.

Whole CU includes storage, a validated load, method work, commit where mutable,
and eight-byte return-data publication. The common load performs the same work
without collection projection. The matched load adds the same projection and
iterator/cursor construction as the operation; root copies and standalone
measurements need no projection. Both idles publish the same eight-byte result
shape; zero is its idle value. Tables retain both differences, both per-item differences, and both
10→100 and 100→1000 whole-instruction slopes. Empty rows leave per-item differences
blank. Setup remains in whole CU; subtraction cannot establish an acceptance pass.
Lazy read rows are explicitly named and use their own lazy idle.

Walk checksums include all decoded values; immutable/mutable iteration and cursor
reads also include all 32 bytes of each copied map key. Owned-copy checksums cover
the complete copied collection. The host constructs logical values, expected
account bytes and observable results independently through `blubber-model`; it
never imports the production encoding helper. Cursor-next and
empty callback rows make no leaf-read claim. Map setters transport copied keys
through an opaque black-box use and preserve complete key bytes in the host oracle.
Long-prefix maps have adjacent keys sharing 31 bytes; n=1000 requires more than
one final-byte bucket, so the entire map cannot share 31 bytes.

Equal replacements are eight bytes; growth is 8→17. Supplied rewrite inputs
cycle through lengths 0..17 in the exact-count API fixtures. Old-dependent callbacks read each old
value once before choosing a replacement. These loops may exceed a transaction
meter. Scaling observations use an explicit 40,000,000 CU budget. The separate tour fixtures run at their default meter.

All traversal observations request a 256 KiB heap so that n=1000 owned copies can
complete. Owned-copy and old-value recipe rows allocate; this heap request is separate from the
32 KiB acceptance cohort. `measure_str`, `measure_bytes` and
`measure_derived` run n independent top-level measurements. List<u8> includes its
list count; the derived record contains a u64 plus eight strings of lengths 0..7.
Its measurement visits those eight children on every call. These rows price
recursive work; an outer ends workspace cannot remove that work. The default
scratch-free API is shipped. Optional workspace APIs are omitted: the measured
Str scratch prototype was slower, and these recursive microfixtures do not
establish a complete List<u8>/derived rewrite benefit.

The main table has 1,215 current rows per architecture.

Fixed-list gates cover fixed-list `cursor_get`, `cursor_set`, `empty_callback`, `callback_get`
and `callback_equal_set` at n=10/100/1000: each whole slope is at most 30 CU/item,
common-load delta is at most `30*n+300`, and the common load has zero slope.
Matched-load deltas remain reported separately. Ten source-identical public
representation cases run in separate SBF artifacts on each architecture.

Fixed-map gates cover fixed-map iteration, cursor reads/sets and scoped reads/sets at both
whole slopes of at most 120 CU/item and totals of at most `120*n+2500`, including
n=0/1 setup, ordinary keys and adjacent keys with a 31-byte common prefix. The
empty callback is checked too. Opaque cursor-next transport is measured separately.


[Compact receipts](../verification/results/traversal/) bind source, tool,
lockfile, independent oracles, frozen baselines, whole/diagnostic ELF hashes and
raw logs. Calibrated markers live only in the diagnostic ELF and never determine
whole-instruction acceptance. For standalone codec measurements, the open bracket
also contains a validated load; the measurement itself has its own operation
bracket.

## Shipped scope and counters

The implementation retains `Edit` ownership and public mutable types. Fixed regions, immutable
chunks, generated slots, copied map keys and indexed coordinates avoid repeated
path resolution. Immutable and mutable fixed-list iteration also obey the
`30*n+300` common delta at n=0/1 and both 30 CU/item slopes. Generated
get/set/empty-callback rows use the same bounds, including setup. Indexed
list rows retain 200 CU/item and `200*n+2500`; the eight indexed map rows use
the [specified per-row budgets](../SPEC.md), recorded with both
slopes and all five totals in the [v0](../verification/results/traversal/passes-v0.log)
and [v3](../verification/results/traversal/passes-v3.log) receipts.

The [extended v0 table](traversal-extended-v0.csv) and
[extended v3 table](traversal-extended-v3.csv) add 115 rows per architecture:
45 generated-slot/representation, 15 rewrite-kernel, 40 typed fill/rewrite and
15 shared-caller rows. Together with each 1,215-row current main table, these retain
every shipped fixture, n=0/1 setup, both idle deltas and both whole slopes.
Diagnostic brackets are populated only for instrumented suites. Their totals
are diagnostic. `kernel_rewrite` measures the low-level stream;
`fill` and `rewrite` measure public methods with supplied map keys included;
`rewrite_values` measures the keyless map API.
The shared caller executes all five APIs from one frame; its combined total
covers all five APIs together. All builds reject frames above 4,096 bytes.

| API | Top-level measure calls | Encodes and end stores | Backend work |
|---|---|---|---|
| `ListMut::fill`, `MapMut::fill_values` | Once for a nonempty collection; zero when empty | n encodes; at most n ends | Reserve, outer suffix move and ancestor/envelope publication each at most once |
| `ListMut::rewrite_from`, `MapMut::rewrite_values_from` | At most 2n across preflight and replay | n encodes; at most n ends | Same bounds; count rejection precedes measurement, each key is checked before its value, and all preflight finishes before reserve |
| `MapMut::rewrite_values` | At most 2n across preflight and replay | n encodes; n indexed ends (zero fixed ends) | Shared list preflight and map stride; count rejection precedes measurement; all preflight finishes before reserve |
| Low-level rewrite fixture | 2n | n encodes; list max(n−1,0) ends or map n ends | Fixture needs no reserve or outer suffix move; publication once when nonempty |

Counts describe top-level codec calls. Recursive measurements remain codec work:
Str has no child measurements, eight-byte List<u8> visits eight u8 children, and
MeasuredRecord visits one u64, one List and eight Str children per input.
`list_edits` counter assertions cover the public methods; `rewrite_kernel`
checks nested suffix/ancestor and backend failures. Fill's growing 8→17 common
delta is at most `100*n+600` at n>=10, both whole slopes at most 120 and each
total at most `120*n+2500`. Varied 0..17 rewrites have slopes at most 200 and
totals at most `200*n+3000`. The low-level rewrite common delta remains at most
`100*n+600` at every sampled size. Keyless map rewrite also stays at or below keyed rewrite at every sampled n.
Empty calls preserve the source.

Count, key and measurement errors occur before the backend write phase. Reserve
may relocate storage; failures there carry the backend's documented state.
Second-pass codec error, wrong width or unwind leaves sticky poison, so an edit
cannot commit partial encoding. Inputs must replay deterministically. Maps
preserve count and encoded keys; these APIs do not insert or remove entries.

Growing callbacks and manual cursors use packed single-edit movement. They
retain callback-once partial progress and sticky poison; movement/directory work
can remain quadratic. Fill/rewrite limits do not apply to arbitrary callbacks.
No amortized-growth theorem is claimed.

The [assurance document](../ASSURANCE.md) gives the tool versions and
reproduction commands.
