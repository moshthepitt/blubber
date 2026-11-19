# Compute units

Acceptance fixtures run on Mollusk 0.16.2 / Agave 4.3.0, SBF platform-tools
v1.53 (Rust 1.89.0-dev, LLVM 20.1.7) and host Rust 1.97.1. Release builds use
overflow checks, fat LTO and one codegen unit. Unless a table says otherwise,
the meter is 1,400,000 CU and heap is 32 KiB. Rent uses the harness's live sysvar.
These are harness observations rather than cluster measurements.

Typed rows include opening, load, one edit and commit. Headroom defaults to
`UpTo(0)`; growing fixtures select explicit budgets. State is prefunded, so
there is no funding CPI unless the row says TopUp. Checked rows scan the root;
ordinary authenticated runtime loads use lazy touched checks. Raw lease rows
exercise explicit sizing/funding/refund policies independently of typed editing.

The summary uses the basic measurements retained on 2026-11-18. The receipts
record the funding and headroom used by each fixture.
[v0 receipt](../verification/results/performance/cu-v0.log) · [v3 receipt](../verification/results/performance/cu-v3.log).

<!-- basic-cu:start -->

| Whole instruction | SBF v0 CU | SBF v3 CU |
|---|---|---|
| Scalar set | 625 | 623 |
| Eight-byte string, same size | 531 | 529 |
| Checked scalar set | 975 | 973 |
| Checked eight-byte string, same size | 724 | 722 |
| String growth, eight to 32 bytes | 845 | 843 |
| String shrink, eight to one byte | 823 | 821 |
| List<u64>, n=100, push | 1,174 | 1,172 |
| Map<Pubkey,u64>, n=100, append | 1,589 | 1,585 |
| Map<Pubkey,u64>, n=100, middle | 2,602 | 2,601 |
| Map<Pubkey,u64>, n=100, front | 2,161 | 2,167 |
| Map<Pubkey,u64>, n=100, common-prefix append | 1,600 | 1,596 |
| Patch100, borrow and both directory passes | 1,845 | 1,939 |
| Complete tour | 140,368 | 139,947 |
| Raw lease, prepaid unchanged | 785 | 777 |
| Raw lease, top-up grow 200 bytes | 2,329 | 2,321 |
| Raw lease, refund shrink to 64 bytes | 1,079 | 1,069 |
| Raw lease, retain 32 bytes | 2,369 | 2,361 |

<!-- basic-cu:end -->

Opening `UpTo(0)` performs no speculative resize. Positive budgets can use the
invocation growth window and may grant less than requested. SDK-local capacity
does not certify transaction growth resources or ancestor CPI growth limits. Commit
trims and settles rent against actual physical length. Failed final shrink is
diagnostic in the receipt; funding/settlement errors return errors. Drop makes
a best-effort trim without rent settlement.

The 2026-11-11 scaling measurements retain whole and baseline CU separately.
The extended runtime measurements from the same run cover collection and option
cases, sequences and invocation behaviour.
[Scaling v0](CU-v0.md) · [Scaling v3](CU-v3.md) · [Runtime v0](CU-v0-runtime.md) · [Runtime v3](CU-v3-runtime.md).

Some scaling rows use a 20,000,000-CU exploration meter and exceed the default transaction
budget. Their presence does not show that the workload fits a deployable
transaction. Calibrated phase markers belong to separate diagnostic ELFs and
never determine whole-instruction acceptance.

[Ordinary traversal measurements](TRAVERSAL.md) retain both caller layouts,
n=0/1/10/100/1000, matching idles, whole slopes and eight explicitly tracked
map groups. [Comparison tables](COMPARE.md) use their own account shape,
40,000,000-CU meter and 256 KiB heap. [The performance guide](../docs/performance.md)
explains API choices and staging costs. Never compare totals without their
load, caller, allocation and runtime contracts.

Run `bash scripts/reproduce.sh cu` for fresh v0 and v3 measurements with
byte and result checks. The [assurance document](../ASSURANCE.md) records
the runtime assumptions.

## Full validation above a lazy load

These differences come from the 2026-11-11 scaling measurements.
[Scaling v0](CU-v0.md) · [Scaling v3](CU-v3.md).

<!-- validation v0 -->
Full validation above lazy read baseline, SBF v0, in CU. Every fixture checks complete bytes against the independent model.

| Layout / key prefix | n=10 | n=100 | n=1,000 |
|---|---:|---:|---:|
| list_fixed / plain | 273 | 273 | 273 |
| list_indexed / plain | 873 | 2014 | 15289 |
| map_fixed / plain | 629 | 2519 | 21419 |
| map_fixed / common | 710 | 3410 | 30410 |
| address_fixed / plain | 629 | 2519 | 21419 |
| address_fixed / common | 710 | 3410 | 30410 |
| map_indexed / plain | 1171 | 4382 | 38357 |
| map_indexed / common | 1252 | 5273 | 47348 |
| address_indexed / plain | 1171 | 4382 | 38357 |
| address_indexed / common | 1252 | 5273 | 47348 |
<!-- /validation v0 -->

<!-- validation v3 -->
Full validation above lazy read baseline, SBF v3, in CU. Every fixture checks complete bytes against the independent model.

| Layout / key prefix | n=10 | n=100 | n=1,000 |
|---|---:|---:|---:|
| list_fixed / plain | 273 | 273 | 273 |
| list_indexed / plain | 865 | 2006 | 15281 |
| map_fixed / plain | 629 | 2519 | 21419 |
| map_fixed / common | 710 | 3410 | 30410 |
| address_fixed / plain | 629 | 2519 | 21419 |
| address_fixed / common | 710 | 3410 | 30410 |
| map_indexed / plain | 1162 | 4373 | 38348 |
| map_indexed / common | 1243 | 5264 | 47339 |
| address_indexed / plain | 1162 | 4373 | 38348 |
| address_indexed / common | 1243 | 5264 | 47339 |
<!-- /validation v3 -->
