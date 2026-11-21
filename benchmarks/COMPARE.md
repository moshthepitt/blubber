# Benchmark comparison

## Reproduce the comparison

Raw measurements are in [`compare/results/all.csv`](compare/results/all.csv). The benchmark receipt in [`compare/results/receipt.json`](compare/results/receipt.json) records the inputs, tools, binary hashes and evidence for this run setup. Run `bash scripts/reproduce.sh cu compare` from the repository root to rebuild the whole-instruction, heap and calibration runs for both SBF architectures into the target directory's `reproduce` scratch area.

<!-- compare-summary:start -->

Blubber lazy list sum costs 10,566 CU versus 10,295 for Bytemuck; map lookup costs 974 versus 426 for Zeropod. Blubber lazy string-list growth costs 45,153 CU versus 202,917 for Star Frame, which the maintainer helped build. Old-dependent string-map growth costs 283,205 versus 878,116 for Zeropod. These are v0 whole-instruction totals for the measured methods and load policies.

<!-- compare-summary:end -->

## Method

The harness runs on Mollusk 0.16.2<!-- num:cmp.environment.mollusk -->, Agave 4.3.0<!-- num:cmp.environment.agave --> and platform-tools 1.53<!-- num:cmp.environment.platform_tools -->. It configures an instruction meter of 40,000,000<!-- num:cmp.environment.cu_meter --> CU and a 256<!-- num:cmp.environment.heap_kib --> KiB heap so large rewrites finish instead of hitting the default 32<!-- num:runtime.default.heap_kib --> KiB production limit. Each run starts with one fresh, writable program-owned account with enough prepaid rent for growth, so no rent-transfer CPI runs.

A CU is the Solana runtime measure of compute work. The primary metric is whole CU, the total for the entire instruction. Each operation has a matched idle control for load and close. Successful operation rows also report the signed delta. A separate diagnostic build measures dynamic heap memory as forward bump consumption, including alignment and abandoned allocations before a sentinel. OOM means a run exhausted that heap and left no final watermark. See [`benchmarks/CU.md#read-a-row`](CU.md#read-a-row) for the primary harness.

Calibration replays the primary traversal fixtures, whereas the comparison fixtures include their own dispatch and checksum work. Indexed list strings start as hexadecimal ordinals of 8<!-- num:cmp.fixture.initial_string --> bytes, and the constant fill replaces every list item with one 17<!-- num:cmp.fixture.longer_string -->-byte string. Rewriting from the old value computes a distinct replacement per item.

In the map rewrite, both [`blubber_adapter.rs`](compare/program/src/blubber_adapter.rs) and [`blubber_checked_adapter.rs`](compare/program/src/blubber_checked_adapter.rs) call `MapMut<K, Str>::rewrite_values_with`. On SBF v0 at 1,000 items, lazy Blubber uses fewer whole CU than Zeropod (283,205<!-- num:cmp.v0.n1000.map_grow_from_old.lazy --> CU versus 878,116<!-- num:cmp.v0.n1000.map_grow_from_old.zeropod --> CU), a lower operation delta after subtracting the matched idle (282,406<!-- num:cmp.v0.n1000.map_grow_from_old.lazy.delta --> CU versus 452,564<!-- num:cmp.v0.n1000.map_grow_from_old.zeropod.delta --> CU), and less heap (17,000<!-- num:cmp.v0.n1000.map_grow_from_old.lazy.heap_bytes --> bytes versus 195,912<!-- num:cmp.v0.n1000.map_grow_from_old.zeropod.heap_bytes --> bytes). Borsh hits OOM on that row. For the separate checked caller, see [`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](TRAVERSAL.md#ordinary-caller-costs).

A sorted map stores keys in a single valid ordered encoding. Middle-key fixtures derive ordered keys from even ordinals and use an absent odd key for insertion. A middle-key lookup in a nonempty fixture hits Blubber's first binary-search probe. Critbit uses key projection to map those fixture keys to a smaller integer representation. For other lookup positions and shared prefixes, see [`benchmarks/CU.md#read-a-row`](CU.md#read-a-row).

An independent host oracle checks logical results, read checksums and full expected bytes where supported. It also requires idle and failed runs to leave every byte untouched. For Sokoban, Light Zero Copy and PinaPod, the harness checks decoded logical values because inactive fixed slots differ across libraries.

## Load policy and scope

A lazy load checks the account header up front and checks fields as you read them. A checked load scans the complete used encoding before returning a view or calling your edit closure. In the tables below, Star Frame is a comparator I helped build. The Bytemuck fixture reserves all 1,000 slots up front (8,008<!-- num:cmp.v0.n100.list_sum.bytemuck.account_bytes --> bytes at 100 items, versus 816<!-- num:cmp.v0.n100.list_sum.lazy.account_bytes --> bytes for lazy Blubber) and fails when pushed past capacity.

The [`#source-inventory`](#source-inventory) table counts pre-expansion `unsafe` tokens in pinned `src/` files, excluding dependencies and expanded derives, and records the MIT licence for both Blubber rows. See [`ASSURANCE.md#trust-boundaries`](../ASSURANCE.md#trust-boundaries).

## Find losses and failed runs

Compare successful rows by whole CU. Unsupported cases have no result, and [`#failed-observations`](#failed-observations) lists recorded failures. Read each number alongside its load policy, account byte size and run status. For the loss shortlist within the same load policy, see [`benchmarks/compare/GAPS.md#gaps-within-the-same-load-policy`](compare/GAPS.md#gaps-within-the-same-load-policy).

<!-- compare-tables:start -->

## Source inventory

The source review and measurements refer to the pinned commits. This comparison includes Star Frame, which the maintainer helped build. Unsafe counts are pre-expansion `unsafe` syntax tokens in `src/`, including macro bodies and test modules, after stripping comments and literals. They exclude dependencies and expanded derives. Counts describe source, not safety.

| Library | Version | Pinned commit | Pinned date | License | Unsafe syntax |
|---|---|---|---|---|---|
| Borsh | 1.8.1 | [fe778bec428d](https://github.com/near/borsh-rs/tree/fe778bec428d5b44cd4922c896af0a7c39a863dd) | 2026-08-26 | MIT OR Apache-2.0 | 6 |
| Blubber lazy | 0.1.0 | [e906f7deca37](../README.md) | 2026-11-19 | MIT | 0 |
| Blubber checked | 0.1.0 | [e906f7deca37](../README.md) | 2026-11-19 | MIT | 0 |
| Star Frame | 0.30.0 | [6936d582a942](https://github.com/staratlasmeta/star_frame/tree/6936d582a942760b67728059be651473645ee099) | 2026-02-26 | Apache-2.0 | 164 |
| Bytemuck | 1.25.2 | [f363643e951a](https://github.com/Lokathor/bytemuck/tree/f363643e951a7ac9e4b9921de982f2b0918902e3) | 2026-07-19 | Zlib OR Apache-2.0 OR MIT | 332 |
| Sokoban | 0.3.3 | [f8ecce2c50c1](https://github.com/Ellipsis-Labs/sokoban/tree/f8ecce2c50c12e6b8731b59ecdb3a1f43769a094) | 2024-08-23 | MIT OR Apache-2.0 | 29 |
| Zeropod | 0.4.0 | [8a607caf7861](https://github.com/blueshift-gg/zeropod/tree/8a607caf7861c932786e3f7d8759a7c066671b25) | 2026-10-14 | Apache-2.0 | 173 |
| Wincode | 0.6.3 | [2d9cc7a2e974](https://github.com/anza-xyz/wincode/tree/2d9cc7a2e974d726ced31ac1ad3c645113f685f0) | 2026-10-19 | Apache-2.0 | 457 |
| PinaPod | 0.4.7 | [c1b89cc19693](https://github.com/pina-rs/pinapod/tree/c1b89cc19693f0952eb9990e23e48acbd992f2c7) | 2026-10-21 | Apache-2.0 | 84 |
| Light | 0.6.0 | [b0fe3f2ed33a](https://github.com/Lightprotocol/light-protocol/tree/b0fe3f2ed33acaf4b2ad7e025a2dccefdd0e0515) | 2026-01-20 | Apache-2.0 | 0 |
| Anchor slab | 2.0.0-rc.2 | [7d68eb325f29](https://github.com/otter-sec/anchor/tree/7d68eb325f297013a5e0903ddd698fe26218f5a3) | 2026-10-22 | Apache-2.0 | 112 |
| Critbit | 0.3.3 | [f8ecce2c50c1](https://github.com/Ellipsis-Labs/sokoban/tree/f8ecce2c50c12e6b8731b59ecdb3a1f43769a094) | 2024-08-23 | MIT OR Apache-2.0 | 29 |
| Manifest | 1.2.0 | [5f4272ec0674](https://github.com/CKS-Systems/manifest/tree/5f4272ec067459dc6ed761331319678edcd25f51) | 2026-10-22 | GPLv3 (repository LICENSE; declared lib/LICENSE absent) | 7 |
| Manifest checked | 1.2.0 | [5f4272ec0674](https://github.com/CKS-Systems/manifest/tree/5f4272ec067459dc6ed761331319678edcd25f51) | 2026-10-22 | GPLv3 (repository LICENSE; declared lib/LICENSE absent) | 7 |

## Storage and load policy

| Library | Storage | Normal load validation |
|---|---|---|
| Borsh | dynamic owned | whole decode; lengths and UTF-8; default BTreeMap does not enforce input key order |
| Blubber lazy | unsized borrowed | O(1) envelope/discriminator; caller guarantees valid program-written bytes; touched paths checked lazily |
| Blubber checked | unsized borrowed | explicit checked reads and mutable loads: complete root, strict keys and UTF-8 |
| Star Frame | unsized borrowed | fixed bit patterns and geometry; indexed children/UTF-8 checked on access; no map key-order scan |
| Bytemuck | fixed 1000 | POD size/alignment; adapter length<=1000; u64 has no invalid bit patterns |
| Sokoban | fixed 1000 | POD size/alignment; no tree-link, colour, or key-order validation |
| Zeropod | unsized borrowed | whole value lengths and UTF-8; map ordering not checked |
| Wincode | dynamic owned/borrowed | checked lengths, allocation limits, UTF-8, alignment; borrowed numeric map assumes key order |
| PinaPod | compact vector bound 1001; W3 fixed 17-byte strings | whole compact schema: lengths, allocation geometry and UTF-8 |
| Light | fixed runtime 1000 | extent/alignment and length<=capacity; u64 has no invalid bit patterns |
| Anchor slab | growable contiguous slab of fixed-size entries | owner, discriminator, header/tail extent and alignment, len<=capacity; genuine SDK exclusion; default guardrails on |
| Critbit | fixed 1000 leaves + 2000 tree nodes; projected 128-bit keys | configured POD size/alignment; short inputs can panic; links and prefixes unchecked |
| Manifest | growable arena; 56-byte slots, 32-byte caller header | caller header geometry only; native tree constructor assumes a well-formed graph |
| Manifest checked | growable arena; 56-byte slots, 32-byte caller header | upstream reachable-graph bounds/alignment, cycles, parents, colours, black height, ordering and cached max; equal keys allowed; free list remains caller trust |

## Verification, growth and integration

The upstream entries describe source review at each pinned commit. The comparison ran the measured operations. Blubber verification is covered in the assurance document. "Raw bytes" means the caller supplies account ownership, discriminators, rent and borrow lifetimes. It is not a tested native Anchor v2 integration.

| Library | Formal verification evidence | Reallocation handling | Pinocchio / Anchor v2 |
|---|---|---|---|
| Borsh | No proof inventory found in inspected crate | exact resize, encode on mutation | raw bytes / raw bytes |
| Blubber lazy | Verus kernel; bounded Kani typed checks (repository gates) | default UpTo(0); these mutable rows explicitly select Max; commit trims | native Pinocchio / Anchor v2 lazy and checked managed lifecycle (pinned SDK) |
| Blubber checked | Verus kernel; bounded Kani typed checks (repository gates) | default UpTo(0); these mutable rows explicitly select Max; commit trims | native Pinocchio / Anchor v2 lazy and checked managed lifecycle (pinned SDK) |
| Star Frame | Miri tests present; no formal proof inventory found | native SDK resizes during edits; no reserve | native older SDK / custom integration |
| Bytemuck | No formal proof inventory found in inspected crate | none; full capacity reserved | raw bytes / raw bytes |
| Sokoban | No formal proof inventory found in inspected crate | none; full capacity reserved | raw bytes / raw bytes |
| Zeropod | Kani field-move harness and Miri present; not rerun | owned replacement; exact resize before root edit | raw bytes / raw bytes |
| Wincode | Audits/fuzzing documented; no formal proof inventory found | exact resize and encode; numeric slice set in place | raw bytes / raw bytes |
| PinaPod | Kani including generated compact schemas; Miri present; not rerun | count changes resize; W3 String<17> has a fixed 18-byte slot | raw bytes / raw bytes |
| Light | No container proof inventory found in inspected crate | none; capacity fixed at initialization | raw bytes / raw bytes |
| Anchor slab | Kani capacity/length harnesses and Miri regression suites present; not rerun | resize_to_capacity(n+1) before push; truncate retains capacity; rent prepaid | native SDK / native Slab<H,T> |
| Critbit | Fuzz targets present; no formal proof inventory found | none; full capacity reserved | raw bytes / raw bytes |
| Manifest | Certora hypertree/RB specs with proof-build modeling; not rerun | exact one-slot realloc on exhaustion; free-list reuse; no shrink; rent prepaid | raw bytes / raw bytes |
| Manifest checked | same Certora evidence; upstream graph validator measured | same exact slot growth/free-list policy as native view | raw bytes / raw bytes |

## Lazy and checked columns

Blubber checked scans the whole root on reads and edits. Blubber lazy checks touched paths after account authentication. Both columns keep the same operation bodies, allocation and successful-work contract. Cells below are whole CU (signed delta); every matching idle is retained in all.csv.

### SBF v0 load policies

| Workload | Operation | n | Checked | Lazy |
|---|---|---|---|---|
| W1 | read_all | 0 | 849 (+18) | 561 (+18) |
| W1 | read_all | 1 | 864 (+33) | 576 (+33) |
| W1 | read_all | 10 | 954 (+123) | 666 (+123) |
| W1 | read_all | 100 | 1,854 (+1,023) | 1,566 (+1,023) |
| W1 | read_all | 1000 | 10,854 (+10,023) | 10,566 (+10,023) |
| W1 | get_middle | 0 | 865 (+34) | 577 (+34) |
| W1 | get_middle | 1 | 873 (+42) | 586 (+43) |
| W1 | get_middle | 10 | 873 (+42) | 586 (+43) |
| W1 | get_middle | 100 | 873 (+42) | 586 (+43) |
| W1 | get_middle | 1000 | 873 (+42) | 586 (+43) |
| W1 | set_all | 0 | 1,256 (+203) | 921 (+202) |
| W1 | set_all | 1 | 1,265 (+212) | 930 (+211) |
| W1 | set_all | 10 | 1,329 (+276) | 994 (+275) |
| W1 | set_all | 100 | 1,959 (+906) | 1,624 (+905) |
| W1 | set_all | 1000 | 8,259 (+7,206) | 7,924 (+7,205) |
| W1 | push | 0 | 1,781 (+728) | 1,446 (+727) |
| W1 | push | 1 | 1,781 (+728) | 1,446 (+727) |
| W1 | push | 10 | 1,781 (+728) | 1,446 (+727) |
| W1 | push | 100 | 1,781 (+728) | 1,446 (+727) |
| W1 | push | 1000 | 1,781 (+728) | 1,446 (+727) |
| W1 | remove_front | 0 | 1,051 (-2) | 717 (-2) |
| W1 | remove_front | 1 | 1,505 (+452) | 1,169 (+450) |
| W1 | remove_front | 10 | 1,676 (+623) | 1,340 (+621) |
| W1 | remove_front | 100 | 1,676 (+623) | 1,340 (+621) |
| W1 | remove_front | 1000 | 2,509 (+1,456) | 2,173 (+1,454) |
| W2 | lookup | 0 | 1,033 (+23) | 637 (+22) |
| W2 | lookup | 1 | 1,406 (+319) | 974 (+319) |
| W2 | lookup | 10 | 1,611 (+319) | 974 (+319) |
| W2 | lookup | 100 | 3,501 (+319) | 974 (+319) |
| W2 | lookup | 1000 | 22,401 (+319) | 974 (+319) |
| W2 | insert_middle | 0 | 2,178 (+1,015) | 1,734 (+1,015) |
| W2 | insert_middle | 1 | 2,516 (+1,312) | 2,031 (+1,312) |
| W2 | insert_middle | 10 | 3,183 (+1,774) | 2,493 (+1,774) |
| W2 | insert_middle | 100 | 5,448 (+2,149) | 2,868 (+2,149) |
| W2 | insert_middle | 1000 | 26,967 (+4,768) | 5,487 (+4,768) |
| W2 | remove | 0 | 1,768 (+605) | 1,324 (+605) |
| W2 | remove | 1 | 2,341 (+1,137) | 1,856 (+1,137) |
| W2 | remove | 10 | 2,861 (+1,452) | 2,171 (+1,452) |
| W2 | remove | 100 | 4,751 (+1,452) | 2,171 (+1,452) |
| W2 | remove | 1000 | 24,257 (+2,058) | 2,777 (+2,058) |
| W2 | set_all | 0 | 1,672 (+509) | 1,228 (+509) |
| W2 | set_all | 1 | 1,828 (+624) | 1,343 (+624) |
| W2 | set_all | 10 | 2,069 (+660) | 1,379 (+660) |
| W2 | set_all | 100 | 4,319 (+1,020) | 1,739 (+1,020) |
| W2 | set_all | 1000 | 26,819 (+4,620) | 5,339 (+4,620) |
| W2 | iterate_sum | 0 | 1,033 (+23) | 638 (+23) |
| W2 | iterate_sum | 1 | 1,146 (+59) | 710 (+55) |
| W2 | iterate_sum | 10 | 1,513 (+221) | 854 (+199) |
| W2 | iterate_sum | 100 | 5,023 (+1,841) | 2,294 (+1,639) |
| W2 | iterate_sum | 1000 | 40,123 (+18,041) | 16,694 (+16,039) |
| W3 | read_all | 0 | 1,117 (+259) | 599 (+269) |
| W3 | read_all | 1 | 1,606 (+606) | 941 (+611) |
| W3 | read_all | 10 | 4,431 (+3,204) | 3,557 (+3,227) |
| W3 | read_all | 100 | 31,312 (+28,944) | 29,477 (+29,147) |
| W3 | read_all | 1000 | 301,987 (+286,344) | 288,677 (+288,347) |
| W3 | set_equal | 0 | 1,367 (+1) | 803 (+1) |
| W3 | set_equal | 1 | 3,053 (+1,545) | 2,366 (+1,564) |
| W3 | set_equal | 10 | 3,419 (+1,684) | 2,506 (+1,704) |
| W3 | set_equal | 100 | 4,560 (+1,684) | 2,506 (+1,704) |
| W3 | set_equal | 1000 | 17,835 (+1,684) | 2,506 (+1,704) |
| W3 | grow_one | 0 | 1,367 (+1) | 803 (+1) |
| W3 | grow_one | 1 | 3,398 (+1,890) | 2,693 (+1,891) |
| W3 | grow_one | 10 | 4,050 (+2,315) | 3,187 (+2,385) |
| W3 | grow_one | 100 | 6,046 (+3,170) | 3,637 (+2,835) |
| W3 | grow_one | 1000 | 28,267 (+12,116) | 8,533 (+7,731) |
| W3 | grow_all | 0 | 1,951 (+585) | 1,625 (+823) |
| W3 | grow_all | 1 | 2,490 (+982) | 2,128 (+1,326) |
| W3 | grow_all | 10 | 3,073 (+1,338) | 2,583 (+1,781) |
| W3 | grow_all | 100 | 7,094 (+4,218) | 6,453 (+5,651) |
| W3 | grow_all | 1000 | 49,169 (+33,018) | 45,153 (+44,351) |
| W3 | grow_all_old | 0 | 2,627 (+1,261) | 2,421 (+1,619) |
| W3 | grow_all_old | 1 | 3,582 (+2,074) | 3,220 (+2,418) |
| W3 | grow_all_old | 10 | 8,162 (+6,427) | 7,672 (+6,870) |
| W3 | grow_all_old | 100 | 52,170 (+49,294) | 51,529 (+50,727) |
| W3 | grow_all_old | 1000 | 493,259 (+477,108) | 489,243 (+488,441) |
| W4 | lookup | 0 | 1,120 (+279) | 611 (+280) |
| W4 | lookup | 1 | 1,988 (+912) | 1,244 (+913) |
| W4 | lookup | 10 | 2,452 (+927) | 1,259 (+928) |
| W4 | lookup | 100 | 5,663 (+927) | 1,259 (+928) |
| W4 | lookup | 1000 | 39,638 (+927) | 1,259 (+928) |
| W4 | insert_middle | 0 | 4,236 (+2,890) | 3,582 (+2,783) |
| W4 | insert_middle | 1 | 5,235 (+3,654) | 4,323 (+3,524) |
| W4 | insert_middle | 10 | 6,562 (+4,532) | 5,183 (+4,384) |
| W4 | insert_middle | 100 | 11,678 (+6,437) | 7,043 (+6,244) |
| W4 | insert_middle | 1000 | 64,100 (+24,884) | 25,040 (+24,241) |
| W4 | grow_all_old | 0 | 2,127 (+781) | 1,672 (+873) |
| W4 | grow_all_old | 1 | 3,639 (+2,058) | 3,288 (+2,489) |
| W4 | grow_all_old | 10 | 6,380 (+4,350) | 5,901 (+5,102) |
| W4 | grow_all_old | 100 | 31,692 (+26,451) | 31,062 (+30,263) |
| W4 | grow_all_old | 1000 | 287,210 (+247,994) | 283,205 (+282,406) |

## SBF v0, n=0

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 325 (+5) | 326 (+6) | 398 (+89) | 566 (+257) | 400 (+91) |
| Blubber lazy | 561 (+18) | 577 (+34) | 921 (+202) | 1,446 (+727) | 717 (-2) |
| Blubber checked | 849 (+18) | 865 (+34) | 1,256 (+203) | 1,781 (+728) | 1,051 (-2) |
| Star Frame | 335 (+2) | 336 (+3) | 439 (+7) | 539 (+107) | 433 (+1) |
| Bytemuck | 291 (+13) | 281 (+3) | 280 (+0) | 289 (+9) | 280 (+0) |
| Zeropod | 302 (+4) | 302 (+4) | 477 (+147) | 668 (+338) | 479 (+149) |
| Wincode | 328 (+8) | 322 (+2) | 282 (+1) | 697 (+283) | 569 (+155) |
| PinaPod | 301 (+0) | 302 (+1) | 513 (+213) | 716 (+416) | 526 (+226) |
| Light | 289 (+5) | 287 (+3) | 289 (+3) | 301 (+15) | 328 (+42) |
| Anchor slab | 295 (+12) | 295 (+12) | 308 (+13) | 389 (+94) | 296 (+1) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 351 (+11) | 662 (+338) | 424 (+100) | 421 (+97) | 348 (+8) |
| Blubber lazy | 637 (+22) | 1,734 (+1,015) | 1,324 (+605) | 1,228 (+509) | 638 (+23) |
| Blubber checked | 1,033 (+23) | 2,178 (+1,015) | 1,768 (+605) | 1,672 (+509) | 1,033 (+23) |
| Star Frame | 364 (+26) | 652 (+217) | 486 (+51) | 440 (+5) | 339 (+1) |
| Sokoban | 279 (+10) | 360 (+89) | 279 (+8) | 338 (+67) | 337 (+68) |
| Zeropod | 314 (+15) | 760 (+435) | 484 (+159) | 481 (+156) | 306 (+7) |
| Wincode | 398 (+35) | 821 (+376) | 649 (+204) | 341 (+1) | 369 (+6) |
| Critbit | 280 (+12) | 401 (+131) | 280 (+10) | 336 (+66) | 332 (+64) |
| Manifest | 373 (+34) | 546 (+240) | 398 (+92) | 356 (+50) | 349 (+10) |
| Manifest checked | 375 (+34) | 548 (+240) | 400 (+92) | 358 (+50) | 351 (+10) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 325 (+3) | 390 (+86) | 388 (+84) | 389 (+85) | 390 (+86) |
| Blubber lazy | 599 (+269) | 803 (+1) | 803 (+1) | 1,625 (+823) | 2,421 (+1,619) |
| Blubber checked | 1,117 (+259) | 1,367 (+1) | 1,367 (+1) | 1,951 (+585) | 2,627 (+1,261) |
| Star Frame | 363 (+4) | 482 (+3) | 482 (+3) | 665 (+186) | 751 (+272) |
| Zeropod | 303 (+4) | 499 (+193) | 497 (+191) | 498 (+192) | 499 (+193) |
| Wincode | 394 (+10) | 584 (+138) | 582 (+136) | 583 (+137) | 584 (+138) |
| PinaPod | 310 (+3) | 762 (+461) | 762 (+461) | 756 (+455) | 764 (+463) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 345 (+11) | 744 (+421) | 413 (+90) |
| Blubber lazy | 611 (+280) | 3,582 (+2,783) | 1,672 (+873) |
| Blubber checked | 1,120 (+279) | 4,236 (+2,890) | 2,127 (+781) |
| Star Frame | 361 (+4) | 1,273 (+792) | 823 (+342) |
| Zeropod | 307 (+6) | 907 (+571) | 532 (+196) |
| Wincode | 384 (+10) | 892 (+505) | 525 (+138) |

## SBF v0, n=1

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 400 (+17) | 393 (+10) | 505 (+133) | 686 (+314) | 502 (+130) |
| Blubber lazy | 576 (+33) | 586 (+43) | 930 (+211) | 1,446 (+727) | 1,169 (+450) |
| Blubber checked | 864 (+33) | 873 (+42) | 1,265 (+212) | 1,781 (+728) | 1,505 (+452) |
| Star Frame | 358 (+25) | 343 (+10) | 446 (+14) | 539 (+107) | 546 (+114) |
| Bytemuck | 305 (+27) | 286 (+8) | 286 (+6) | 289 (+9) | 305 (+25) |
| Zeropod | 317 (+19) | 306 (+8) | 512 (+153) | 721 (+362) | 589 (+230) |
| Wincode | 342 (+22) | 325 (+5) | 286 (+5) | 749 (+307) | 636 (+194) |
| PinaPod | 313 (+12) | 307 (+6) | 586 (+286) | 767 (+467) | 636 (+336) |
| Light | 307 (+23) | 297 (+13) | 295 (+9) | 301 (+15) | 331 (+45) |
| Anchor slab | 308 (+25) | 298 (+15) | 314 (+19) | 389 (+94) | 335 (+40) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 698 (+58) | 1,140 (+516) | 891 (+267) | 853 (+229) | 682 (+42) |
| Blubber lazy | 974 (+319) | 2,031 (+1,312) | 1,856 (+1,137) | 1,343 (+624) | 710 (+55) |
| Blubber checked | 1,406 (+319) | 2,516 (+1,312) | 2,341 (+1,137) | 1,828 (+624) | 1,146 (+59) |
| Star Frame | 537 (+199) | 784 (+349) | 740 (+305) | 448 (+13) | 362 (+24) |
| Sokoban | 325 (+56) | 445 (+174) | 369 (+98) | 504 (+233) | 511 (+242) |
| Zeropod | 426 (+127) | 1,301 (+586) | 1,085 (+370) | 983 (+268) | 321 (+22) |
| Wincode | 426 (+63) | 1,047 (+488) | 931 (+372) | 348 (+8) | 384 (+21) |
| Critbit | 331 (+63) | 616 (+346) | 347 (+77) | 450 (+180) | 454 (+186) |
| Manifest | 423 (+76) | 641 (+327) | 541 (+227) | 396 (+82) | 382 (+35) |
| Manifest checked | 1,279 (+76) | 1,497 (+327) | 1,397 (+227) | 1,252 (+82) | 1,238 (+35) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 766 (+112) | 853 (+217) | 895 (+259) | 894 (+258) | 993 (+357) |
| Blubber lazy | 941 (+611) | 2,366 (+1,564) | 2,693 (+1,891) | 2,128 (+1,326) | 3,220 (+2,418) |
| Blubber checked | 1,606 (+606) | 3,053 (+1,545) | 3,398 (+1,890) | 2,490 (+982) | 3,582 (+2,074) |
| Star Frame | 664 (+305) | 847 (+368) | 1,114 (+635) | 1,039 (+560) | 1,490 (+1,011) |
| Zeropod | 559 (+122) | 1,042 (+476) | 1,142 (+576) | 1,141 (+575) | 1,240 (+674) |
| Wincode | 676 (+121) | 1,062 (+321) | 1,104 (+363) | 1,103 (+362) | 1,202 (+461) |
| PinaPod | 584 (+120) | 1,623 (+1,165) | 1,772 (+1,314) | 1,758 (+1,300) | 1,877 (+1,419) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 1,180 (+150) | 1,662 (+643) | 1,460 (+441) |
| Blubber lazy | 1,244 (+913) | 4,323 (+3,524) | 3,288 (+2,489) |
| Blubber checked | 1,988 (+912) | 5,235 (+3,654) | 3,639 (+2,058) |
| Star Frame | 678 (+321) | 1,451 (+970) | 1,689 (+1,208) |
| Zeropod | 639 (+197) | 1,913 (+941) | 1,736 (+764) |
| Wincode | 750 (+158) | 1,587 (+755) | 1,358 (+526) |

## SBF v0, n=10

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 643 (+107) | 546 (+10) | 991 (+466) | 1,135 (+610) | 957 (+432) |
| Blubber lazy | 666 (+123) | 586 (+43) | 994 (+275) | 1,446 (+727) | 1,340 (+621) |
| Blubber checked | 954 (+123) | 873 (+42) | 1,329 (+276) | 1,781 (+728) | 1,676 (+623) |
| Star Frame | 511 (+178) | 343 (+10) | 482 (+50) | 539 (+107) | 564 (+132) |
| Bytemuck | 395 (+117) | 286 (+8) | 322 (+42) | 289 (+9) | 305 (+25) |
| Zeropod | 416 (+118) | 306 (+8) | 548 (+189) | 720 (+361) | 589 (+230) |
| Wincode | 432 (+112) | 325 (+5) | 322 (+41) | 748 (+306) | 636 (+194) |
| PinaPod | 403 (+102) | 307 (+6) | 620 (+320) | 766 (+466) | 680 (+380) |
| Light | 397 (+113) | 297 (+13) | 331 (+45) | 301 (+15) | 480 (+194) |
| Anchor slab | 398 (+115) | 298 (+15) | 350 (+55) | 389 (+94) | 335 (+40) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 2,473 (+158) | 3,825 (+1,526) | 3,571 (+1,272) | 3,617 (+1,318) | 2,618 (+303) |
| Blubber lazy | 974 (+319) | 2,493 (+1,774) | 2,171 (+1,452) | 1,379 (+660) | 854 (+199) |
| Blubber checked | 1,611 (+319) | 3,183 (+1,774) | 2,861 (+1,452) | 2,069 (+660) | 1,513 (+221) |
| Star Frame | 537 (+199) | 955 (+520) | 758 (+323) | 502 (+67) | 515 (+177) |
| Sokoban | 354 (+85) | 549 (+278) | 731 (+460) | 1,281 (+1,010) | 1,351 (+1,082) |
| Zeropod | 426 (+127) | 4,236 (+1,344) | 4,021 (+1,129) | 3,997 (+1,105) | 420 (+121) |
| Wincode | 518 (+155) | 3,362 (+904) | 3,240 (+782) | 384 (+44) | 474 (+111) |
| Critbit | 709 (+441) | 983 (+713) | 710 (+440) | 1,475 (+1,205) | 1,542 (+1,274) |
| Manifest | 441 (+94) | 680 (+366) | 702 (+388) | 724 (+410) | 809 (+462) |
| Manifest checked | 7,959 (+94) | 8,198 (+366) | 8,220 (+388) | 8,242 (+410) | 8,327 (+462) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 4,276 (+1,075) | 4,147 (+964) | 4,189 (+1,006) | 4,458 (+1,275) | 5,457 (+2,274) |
| Blubber lazy | 3,557 (+3,227) | 2,506 (+1,704) | 3,187 (+2,385) | 2,583 (+1,781) | 7,672 (+6,870) |
| Blubber checked | 4,431 (+3,204) | 3,419 (+1,684) | 4,050 (+2,315) | 3,073 (+1,338) | 8,162 (+6,427) |
| Star Frame | 3,154 (+2,795) | 847 (+368) | 1,212 (+733) | 2,857 (+2,378) | 6,329 (+5,850) |
| Zeropod | 2,938 (+1,157) | 5,447 (+2,621) | 5,547 (+2,721) | 5,816 (+2,990) | 6,815 (+3,989) |
| Wincode | 2,773 (+1,084) | 4,608 (+1,491) | 4,650 (+1,533) | 4,919 (+1,802) | 5,918 (+2,801) |
| PinaPod | 2,855 (+1,101) | 7,809 (+6,061) | 7,958 (+6,210) | 8,883 (+7,135) | 9,868 (+8,120) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 5,896 (+250) | 7,738 (+2,103) | 8,731 (+3,096) |
| Blubber lazy | 1,259 (+928) | 5,183 (+4,384) | 5,901 (+5,102) |
| Blubber checked | 2,452 (+927) | 6,562 (+4,532) | 6,380 (+4,350) |
| Star Frame | 759 (+402) | 1,679 (+1,198) | 9,057 (+8,576) |
| Zeropod | 2,120 (+307) | 8,571 (+3,781) | 9,589 (+4,799) |
| Wincode | 3,795 (+278) | 8,078 (+2,458) | 9,044 (+3,424) |

## SBF v0, n=100

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 3,073 (+1,007) | 2,076 (+10) | 5,851 (+3,796) | 5,635 (+3,580) | 5,457 (+3,402) |
| Blubber lazy | 1,566 (+1,023) | 586 (+43) | 1,624 (+905) | 1,446 (+727) | 1,340 (+621) |
| Blubber checked | 1,854 (+1,023) | 873 (+42) | 1,959 (+906) | 1,781 (+728) | 1,676 (+623) |
| Star Frame | 2,041 (+1,708) | 343 (+10) | 842 (+410) | 539 (+107) | 564 (+132) |
| Bytemuck | 1,295 (+1,017) | 286 (+8) | 682 (+402) | 289 (+9) | 305 (+25) |
| Zeropod | 1,406 (+1,108) | 306 (+8) | 908 (+549) | 720 (+361) | 589 (+230) |
| Wincode | 1,332 (+1,012) | 325 (+5) | 682 (+401) | 748 (+306) | 636 (+194) |
| PinaPod | 1,303 (+1,002) | 307 (+6) | 980 (+680) | 766 (+466) | 680 (+380) |
| Light | 1,297 (+1,013) | 297 (+13) | 691 (+405) | 301 (+15) | 1,560 (+1,274) |
| Anchor slab | 1,298 (+1,015) | 298 (+15) | 710 (+415) | 389 (+94) | 335 (+40) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 19,818 (+218) | 30,758 (+11,174) | 30,136 (+10,552) | 32,145 (+12,561) | 22,636 (+3,036) |
| Blubber lazy | 974 (+319) | 2,868 (+2,149) | 2,171 (+1,452) | 1,739 (+1,020) | 2,294 (+1,639) |
| Blubber checked | 3,501 (+319) | 5,448 (+2,149) | 4,751 (+1,452) | 4,319 (+1,020) | 5,023 (+1,841) |
| Star Frame | 537 (+199) | 1,210 (+775) | 758 (+323) | 1,042 (+607) | 2,045 (+1,707) |
| Sokoban | 496 (+227) | 671 (+400) | 1,198 (+927) | 9,050 (+8,779) | 9,750 (+9,481) |
| Zeropod | 426 (+127) | 33,710 (+8,583) | 33,115 (+7,988) | 35,054 (+9,927) | 1,410 (+1,111) |
| Wincode | 575 (+212) | 48,462 (+4,170) | 48,338 (+4,046) | 744 (+404) | 1,374 (+1,011) |
| Critbit | 989 (+721) | 1,258 (+988) | 887 (+617) | 10,760 (+10,490) | 11,457 (+11,189) |
| Manifest | 519 (+172) | 755 (+441) | 922 (+608) | 4,315 (+4,001) | 5,390 (+5,043) |
| Manifest checked | 70,260 (+172) | 70,496 (+441) | 70,663 (+608) | 74,056 (+4,001) | 75,131 (+5,043) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 39,376 (+10,705) | 37,087 (+8,434) | 37,129 (+8,476) | 40,098 (+11,445) | 50,097 (+21,444) |
| Blubber lazy | 29,477 (+29,147) | 2,506 (+1,704) | 3,637 (+2,835) | 6,453 (+5,651) | 51,529 (+50,727) |
| Blubber checked | 31,312 (+28,944) | 4,560 (+1,684) | 6,046 (+3,170) | 7,094 (+4,218) | 52,170 (+49,294) |
| Star Frame | 28,174 (+27,815) | 847 (+368) | 1,518 (+1,039) | 21,037 (+20,558) | 54,839 (+54,360) |
| Zeropod | 26,608 (+11,507) | 49,187 (+23,951) | 49,287 (+24,051) | 52,256 (+27,020) | 62,255 (+37,019) |
| Wincode | 23,743 (+10,714) | 40,068 (+13,191) | 40,110 (+13,233) | 43,079 (+16,202) | 53,078 (+26,201) |
| PinaPod | 25,625 (+10,911) | 69,681 (+54,973) | 69,830 (+55,122) | 79,457 (+64,749) | 89,082 (+74,374) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 52,915 (+310) | 68,917 (+16,323) | 82,617 (+30,023) |
| Blubber lazy | 1,259 (+928) | 7,043 (+6,244) | 31,062 (+30,263) |
| Blubber checked | 5,663 (+927) | 11,678 (+6,437) | 31,692 (+26,451) |
| Star Frame | 789 (+432) | 2,354 (+1,873) | 106,465 (+105,984) |
| Zeropod | 16,700 (+1,297) | 75,279 (+31,801) | 88,991 (+45,513) |
| Wincode | 55,881 (+276) | 95,771 (+18,807) | 110,020 (+33,056) |

## SBF v0, n=1000

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 27,481 (+10,007) | 17,484 (+10) | 54,559 (+37,096) | 50,642 (+33,179) | 50,586 (+33,123) |
| Blubber lazy | 10,566 (+10,023) | 586 (+43) | 7,924 (+7,205) | 1,446 (+727) | 2,173 (+1,454) |
| Blubber checked | 10,854 (+10,023) | 873 (+42) | 8,259 (+7,206) | 1,781 (+728) | 2,509 (+1,456) |
| Star Frame | 17,341 (+17,008) | 343 (+10) | 4,442 (+4,010) | 539 (+107) | 585 (+153) |
| Bytemuck | 10,295 (+10,017) | 286 (+8) | 4,282 (+4,002) | full | 326 (+46) |
| Zeropod | 11,306 (+11,008) | 306 (+8) | 4,552 (+4,171) | 786 (+405) | 653 (+272) |
| Wincode | 10,332 (+10,012) | 325 (+5) | 4,282 (+4,001) | 814 (+350) | 700 (+236) |
| PinaPod | 10,303 (+10,002) | 307 (+6) | 4,602 (+4,302) | 832 (+532) | 744 (+444) |
| Light | 10,297 (+10,013) | 297 (+13) | 4,291 (+4,005) | full | 12,381 (+12,095) |
| Anchor slab | 10,298 (+10,015) | 298 (+15) | 4,310 (+4,015) | 389 (+94) | 356 (+61) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 188,944 (+458) | 292,449 (+103,979) | 291,444 (+102,974) | 313,381 (+124,911) | 218,822 (+30,336) |
| Blubber lazy | 974 (+319) | 5,487 (+4,768) | 2,777 (+2,058) | 5,339 (+4,620) | 16,694 (+16,039) |
| Blubber checked | 22,401 (+319) | 26,967 (+4,768) | 24,257 (+2,058) | 26,819 (+4,620) | 40,123 (+18,041) |
| Star Frame | 537 (+199) | 1,535 (+1,100) | 827 (+392) | 6,442 (+6,007) | 17,345 (+17,007) |
| Sokoban | 583 (+314) | full | 1,525 (+1,254) | 85,669 (+85,398) | 92,669 (+92,400) |
| Zeropod | 426 (+127) | 319,832 (+77,175) | 318,842 (+76,185) | 340,709 (+98,052) | 11,310 (+11,011) |
| Wincode | 632 (+269) | 708,401 (+37,144) | 708,264 (+37,007) | 4,344 (+4,004) | 10,374 (+10,011) |
| Critbit | 1,268 (+1,000) | full | 1,064 (+794) | 102,665 (+102,395) | 109,662 (+109,394) |
| Manifest | 573 (+226) | 812 (+498) | 1,162 (+848) | 40,711 (+40,397) | 51,686 (+51,339) |
| Manifest checked | 835,565 (+226) | 835,804 (+498) | 836,154 (+848) | 875,703 (+40,397) | 886,678 (+51,339) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 390,946 (+107,005) | 367,057 (+83,134) | 367,099 (+83,176) | 397,094 (+113,171) | 497,093 (+213,170) |
| Blubber lazy | 288,677 (+288,347) | 2,506 (+1,704) | 8,533 (+7,731) | 45,153 (+44,351) | 489,243 (+488,441) |
| Blubber checked | 301,987 (+286,344) | 17,835 (+1,684) | 28,267 (+12,116) | 49,169 (+33,018) | 493,259 (+477,108) |
| Star Frame | 278,374 (+278,015) | 847 (+368) | 4,268 (+3,789) | 202,917 (+202,438) | 540,019 (+539,540) |
| Zeropod | 263,308 (+115,007) | 486,587 (+237,251) | 486,687 (+237,351) | 516,682 (+267,346) | 616,681 (+367,345) |
| Wincode | 233,443 (+107,014) | 394,668 (+130,191) | 394,710 (+130,233) | 424,705 (+160,228) | 524,704 (+260,227) |
| PinaPod | 253,325 (+109,011) | 689,005 (+544,697) | 689,154 (+544,846) | 786,919 (+642,611) | 883,006 (+738,698) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 516,918 (+550) | 670,748 (+154,391) | heap exhausted |
| Blubber lazy | 1,259 (+928) | 25,040 (+24,241) | 283,205 (+282,406) |
| Blubber checked | 39,638 (+927) | 64,100 (+24,884) | 287,210 (+247,994) |
| Star Frame | 843 (+486) | 8,003 (+7,522) | 1,287,238 (+1,286,757) |
| Zeropod | 161,351 (+10,048) | 733,283 (+307,731) | 878,116 (+452,564) |
| Wincode | 783,389 (+491) | 1,181,955 (+182,659) | 1,328,841 (+329,545) |

### SBF v3 load policies

| Workload | Operation | n | Checked | Lazy |
|---|---|---|---|---|
| W1 | read_all | 0 | 846 (+18) | 558 (+18) |
| W1 | read_all | 1 | 861 (+33) | 573 (+33) |
| W1 | read_all | 10 | 951 (+123) | 663 (+123) |
| W1 | read_all | 100 | 1,851 (+1,023) | 1,563 (+1,023) |
| W1 | read_all | 1000 | 10,851 (+10,023) | 10,563 (+10,023) |
| W1 | get_middle | 0 | 862 (+34) | 574 (+34) |
| W1 | get_middle | 1 | 870 (+42) | 583 (+43) |
| W1 | get_middle | 10 | 870 (+42) | 583 (+43) |
| W1 | get_middle | 100 | 870 (+42) | 583 (+43) |
| W1 | get_middle | 1000 | 870 (+42) | 583 (+43) |
| W1 | set_all | 0 | 1,253 (+203) | 918 (+202) |
| W1 | set_all | 1 | 1,262 (+212) | 927 (+211) |
| W1 | set_all | 10 | 1,326 (+276) | 991 (+275) |
| W1 | set_all | 100 | 1,956 (+906) | 1,621 (+905) |
| W1 | set_all | 1000 | 8,256 (+7,206) | 7,921 (+7,205) |
| W1 | push | 0 | 1,778 (+728) | 1,443 (+727) |
| W1 | push | 1 | 1,778 (+728) | 1,443 (+727) |
| W1 | push | 10 | 1,778 (+728) | 1,443 (+727) |
| W1 | push | 100 | 1,778 (+728) | 1,443 (+727) |
| W1 | push | 1000 | 1,778 (+728) | 1,443 (+727) |
| W1 | remove_front | 0 | 1,048 (-2) | 714 (-2) |
| W1 | remove_front | 1 | 1,502 (+452) | 1,166 (+450) |
| W1 | remove_front | 10 | 1,672 (+622) | 1,336 (+620) |
| W1 | remove_front | 100 | 1,672 (+622) | 1,336 (+620) |
| W1 | remove_front | 1000 | 2,505 (+1,455) | 2,169 (+1,453) |
| W2 | lookup | 0 | 1,030 (+23) | 634 (+22) |
| W2 | lookup | 1 | 1,403 (+319) | 971 (+319) |
| W2 | lookup | 10 | 1,608 (+319) | 971 (+319) |
| W2 | lookup | 100 | 3,498 (+319) | 971 (+319) |
| W2 | lookup | 1000 | 22,398 (+319) | 971 (+319) |
| W2 | insert_middle | 0 | 2,168 (+1,008) | 1,724 (+1,008) |
| W2 | insert_middle | 1 | 2,515 (+1,314) | 2,030 (+1,314) |
| W2 | insert_middle | 10 | 3,179 (+1,773) | 2,489 (+1,773) |
| W2 | insert_middle | 100 | 5,446 (+2,150) | 2,866 (+2,150) |
| W2 | insert_middle | 1000 | 27,001 (+4,805) | 5,521 (+4,805) |
| W2 | remove | 0 | 1,768 (+608) | 1,324 (+608) |
| W2 | remove | 1 | 2,344 (+1,143) | 1,859 (+1,143) |
| W2 | remove | 10 | 2,863 (+1,457) | 2,173 (+1,457) |
| W2 | remove | 100 | 4,753 (+1,457) | 2,173 (+1,457) |
| W2 | remove | 1000 | 24,259 (+2,063) | 2,779 (+2,063) |
| W2 | set_all | 0 | 1,669 (+509) | 1,225 (+509) |
| W2 | set_all | 1 | 1,825 (+624) | 1,340 (+624) |
| W2 | set_all | 10 | 2,066 (+660) | 1,376 (+660) |
| W2 | set_all | 100 | 4,316 (+1,020) | 1,736 (+1,020) |
| W2 | set_all | 1000 | 26,816 (+4,620) | 5,336 (+4,620) |
| W2 | iterate_sum | 0 | 1,030 (+23) | 635 (+23) |
| W2 | iterate_sum | 1 | 1,143 (+59) | 707 (+55) |
| W2 | iterate_sum | 10 | 1,510 (+221) | 851 (+199) |
| W2 | iterate_sum | 100 | 5,020 (+1,841) | 2,291 (+1,639) |
| W2 | iterate_sum | 1000 | 40,120 (+18,041) | 16,691 (+16,039) |
| W3 | read_all | 0 | 1,108 (+259) | 596 (+269) |
| W3 | read_all | 1 | 1,595 (+606) | 938 (+611) |
| W3 | read_all | 10 | 4,420 (+3,204) | 3,554 (+3,227) |
| W3 | read_all | 100 | 31,301 (+28,944) | 29,474 (+29,147) |
| W3 | read_all | 1000 | 301,976 (+286,344) | 288,674 (+288,347) |
| W3 | set_equal | 0 | 1,358 (+1) | 800 (+1) |
| W3 | set_equal | 1 | 3,037 (+1,540) | 2,358 (+1,559) |
| W3 | set_equal | 10 | 3,405 (+1,681) | 2,500 (+1,701) |
| W3 | set_equal | 100 | 4,546 (+1,681) | 2,500 (+1,701) |
| W3 | set_equal | 1000 | 17,821 (+1,681) | 2,500 (+1,701) |
| W3 | grow_one | 0 | 1,358 (+1) | 800 (+1) |
| W3 | grow_one | 1 | 3,382 (+1,885) | 2,684 (+1,885) |
| W3 | grow_one | 10 | 4,036 (+2,312) | 3,178 (+2,379) |
| W3 | grow_one | 100 | 6,032 (+3,167) | 3,628 (+2,829) |
| W3 | grow_one | 1000 | 28,259 (+12,119) | 8,530 (+7,731) |
| W3 | grow_all | 0 | 1,942 (+585) | 1,620 (+821) |
| W3 | grow_all | 1 | 2,480 (+983) | 2,124 (+1,325) |
| W3 | grow_all | 10 | 3,093 (+1,369) | 2,609 (+1,810) |
| W3 | grow_all | 100 | 7,474 (+4,609) | 6,839 (+6,040) |
| W3 | grow_all | 1000 | 53,149 (+37,009) | 49,139 (+48,340) |
| W3 | grow_all_old | 0 | 2,618 (+1,261) | 2,416 (+1,617) |
| W3 | grow_all_old | 1 | 3,571 (+2,074) | 3,215 (+2,416) |
| W3 | grow_all_old | 10 | 8,151 (+6,427) | 7,667 (+6,868) |
| W3 | grow_all_old | 100 | 52,159 (+49,294) | 51,524 (+50,725) |
| W3 | grow_all_old | 1000 | 493,248 (+477,108) | 489,238 (+488,439) |
| W4 | lookup | 0 | 1,110 (+279) | 608 (+280) |
| W4 | lookup | 1 | 1,973 (+909) | 1,238 (+910) |
| W4 | lookup | 10 | 2,439 (+926) | 1,255 (+927) |
| W4 | lookup | 100 | 5,650 (+926) | 1,255 (+927) |
| W4 | lookup | 1000 | 39,625 (+926) | 1,255 (+927) |
| W4 | insert_middle | 0 | 4,209 (+2,873) | 3,577 (+2,781) |
| W4 | insert_middle | 1 | 5,211 (+3,642) | 4,325 (+3,529) |
| W4 | insert_middle | 10 | 6,533 (+4,515) | 5,182 (+4,386) |
| W4 | insert_middle | 100 | 11,651 (+6,422) | 7,044 (+6,248) |
| W4 | insert_middle | 1000 | 64,117 (+24,913) | 25,085 (+24,289) |
| W4 | grow_all_old | 0 | 2,117 (+781) | 1,669 (+873) |
| W4 | grow_all_old | 1 | 3,627 (+2,058) | 3,281 (+2,485) |
| W4 | grow_all_old | 10 | 6,344 (+4,326) | 5,870 (+5,074) |
| W4 | grow_all_old | 100 | 31,476 (+26,247) | 30,851 (+30,055) |
| W4 | grow_all_old | 1000 | 285,194 (+245,990) | 281,194 (+280,398) |

## SBF v3, n=0

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 324 (+5) | 325 (+6) | 397 (+89) | 565 (+257) | 399 (+91) |
| Blubber lazy | 558 (+18) | 574 (+34) | 918 (+202) | 1,443 (+727) | 714 (-2) |
| Blubber checked | 846 (+18) | 862 (+34) | 1,253 (+203) | 1,778 (+728) | 1,048 (-2) |
| Star Frame | 335 (+2) | 336 (+3) | 439 (+7) | 539 (+107) | 433 (+1) |
| Bytemuck | 290 (+13) | 280 (+3) | 279 (+0) | 288 (+9) | 279 (+0) |
| Zeropod | 301 (+4) | 301 (+4) | 476 (+147) | 667 (+338) | 478 (+149) |
| Wincode | 327 (+8) | 321 (+2) | 281 (+1) | 696 (+283) | 568 (+155) |
| PinaPod | 300 (+0) | 301 (+1) | 512 (+213) | 712 (+413) | 525 (+226) |
| Light | 288 (+5) | 286 (+3) | 288 (+3) | 300 (+15) | 327 (+42) |
| Anchor slab | 294 (+12) | 294 (+12) | 307 (+13) | 388 (+94) | 295 (+1) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 350 (+11) | 661 (+338) | 423 (+100) | 420 (+97) | 347 (+8) |
| Blubber lazy | 634 (+22) | 1,724 (+1,008) | 1,324 (+608) | 1,225 (+509) | 635 (+23) |
| Blubber checked | 1,030 (+23) | 2,168 (+1,008) | 1,768 (+608) | 1,669 (+509) | 1,030 (+23) |
| Star Frame | 364 (+26) | 652 (+217) | 486 (+51) | 440 (+5) | 339 (+1) |
| Sokoban | 278 (+10) | 359 (+89) | 278 (+8) | 337 (+67) | 336 (+68) |
| Zeropod | 313 (+15) | 759 (+435) | 483 (+159) | 480 (+156) | 305 (+7) |
| Wincode | 397 (+35) | 820 (+376) | 648 (+204) | 340 (+1) | 368 (+6) |
| Critbit | 279 (+12) | 400 (+131) | 279 (+10) | 335 (+66) | 331 (+64) |
| Manifest | 372 (+34) | 545 (+240) | 397 (+92) | 355 (+50) | 348 (+10) |
| Manifest checked | 374 (+34) | 547 (+240) | 399 (+92) | 357 (+50) | 350 (+10) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 324 (+3) | 389 (+86) | 387 (+84) | 388 (+85) | 389 (+86) |
| Blubber lazy | 596 (+269) | 800 (+1) | 800 (+1) | 1,620 (+821) | 2,416 (+1,617) |
| Blubber checked | 1,108 (+259) | 1,358 (+1) | 1,358 (+1) | 1,942 (+585) | 2,618 (+1,261) |
| Star Frame | 363 (+4) | 482 (+3) | 482 (+3) | 665 (+186) | 751 (+272) |
| Zeropod | 302 (+4) | 498 (+193) | 496 (+191) | 497 (+192) | 498 (+193) |
| Wincode | 393 (+10) | 583 (+138) | 581 (+136) | 582 (+137) | 583 (+138) |
| PinaPod | 309 (+3) | 762 (+462) | 762 (+462) | 756 (+456) | 764 (+464) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 344 (+11) | 743 (+421) | 412 (+90) |
| Blubber lazy | 608 (+280) | 3,577 (+2,781) | 1,669 (+873) |
| Blubber checked | 1,110 (+279) | 4,209 (+2,873) | 2,117 (+781) |
| Star Frame | 361 (+4) | 1,273 (+792) | 823 (+342) |
| Zeropod | 306 (+6) | 906 (+571) | 531 (+196) |
| Wincode | 383 (+10) | 891 (+505) | 524 (+138) |

## SBF v3, n=1

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 399 (+17) | 392 (+10) | 504 (+133) | 685 (+314) | 501 (+130) |
| Blubber lazy | 573 (+33) | 583 (+43) | 927 (+211) | 1,443 (+727) | 1,166 (+450) |
| Blubber checked | 861 (+33) | 870 (+42) | 1,262 (+212) | 1,778 (+728) | 1,502 (+452) |
| Star Frame | 358 (+25) | 343 (+10) | 446 (+14) | 539 (+107) | 546 (+114) |
| Bytemuck | 304 (+27) | 285 (+8) | 285 (+6) | 288 (+9) | 304 (+25) |
| Zeropod | 316 (+19) | 305 (+8) | 511 (+153) | 720 (+362) | 588 (+230) |
| Wincode | 341 (+22) | 324 (+5) | 285 (+5) | 748 (+307) | 635 (+194) |
| PinaPod | 312 (+12) | 306 (+6) | 582 (+283) | 763 (+464) | 632 (+333) |
| Light | 306 (+23) | 296 (+13) | 294 (+9) | 300 (+15) | 330 (+45) |
| Anchor slab | 307 (+25) | 297 (+15) | 313 (+19) | 388 (+94) | 334 (+40) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 697 (+58) | 1,139 (+516) | 890 (+267) | 852 (+229) | 681 (+42) |
| Blubber lazy | 971 (+319) | 2,030 (+1,314) | 1,859 (+1,143) | 1,340 (+624) | 707 (+55) |
| Blubber checked | 1,403 (+319) | 2,515 (+1,314) | 2,344 (+1,143) | 1,825 (+624) | 1,143 (+59) |
| Star Frame | 537 (+199) | 784 (+349) | 740 (+305) | 448 (+13) | 362 (+24) |
| Sokoban | 324 (+56) | 444 (+174) | 368 (+98) | 503 (+233) | 510 (+242) |
| Zeropod | 425 (+127) | 1,300 (+586) | 1,084 (+370) | 982 (+268) | 320 (+22) |
| Wincode | 425 (+63) | 1,046 (+488) | 930 (+372) | 347 (+8) | 383 (+21) |
| Critbit | 330 (+63) | 615 (+346) | 346 (+77) | 449 (+180) | 453 (+186) |
| Manifest | 422 (+76) | 640 (+327) | 540 (+227) | 395 (+82) | 381 (+35) |
| Manifest checked | 1,278 (+76) | 1,496 (+327) | 1,396 (+227) | 1,251 (+82) | 1,237 (+35) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 765 (+112) | 852 (+217) | 894 (+259) | 893 (+258) | 992 (+357) |
| Blubber lazy | 938 (+611) | 2,358 (+1,559) | 2,684 (+1,885) | 2,124 (+1,325) | 3,215 (+2,416) |
| Blubber checked | 1,595 (+606) | 3,037 (+1,540) | 3,382 (+1,885) | 2,480 (+983) | 3,571 (+2,074) |
| Star Frame | 664 (+305) | 847 (+368) | 1,114 (+635) | 1,039 (+560) | 1,490 (+1,011) |
| Zeropod | 558 (+122) | 1,041 (+476) | 1,141 (+576) | 1,140 (+575) | 1,239 (+674) |
| Wincode | 675 (+121) | 1,061 (+321) | 1,103 (+363) | 1,102 (+362) | 1,201 (+461) |
| PinaPod | 583 (+120) | 1,621 (+1,164) | 1,770 (+1,313) | 1,756 (+1,299) | 1,875 (+1,418) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 1,179 (+150) | 1,661 (+643) | 1,459 (+441) |
| Blubber lazy | 1,238 (+910) | 4,325 (+3,529) | 3,281 (+2,485) |
| Blubber checked | 1,973 (+909) | 5,211 (+3,642) | 3,627 (+2,058) |
| Star Frame | 678 (+321) | 1,451 (+970) | 1,689 (+1,208) |
| Zeropod | 638 (+197) | 1,912 (+941) | 1,735 (+764) |
| Wincode | 749 (+158) | 1,586 (+755) | 1,357 (+526) |

## SBF v3, n=10

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 642 (+107) | 545 (+10) | 990 (+466) | 1,134 (+610) | 956 (+432) |
| Blubber lazy | 663 (+123) | 583 (+43) | 991 (+275) | 1,443 (+727) | 1,336 (+620) |
| Blubber checked | 951 (+123) | 870 (+42) | 1,326 (+276) | 1,778 (+728) | 1,672 (+622) |
| Star Frame | 511 (+178) | 343 (+10) | 482 (+50) | 539 (+107) | 564 (+132) |
| Bytemuck | 394 (+117) | 285 (+8) | 321 (+42) | 288 (+9) | 304 (+25) |
| Zeropod | 415 (+118) | 305 (+8) | 547 (+189) | 719 (+361) | 588 (+230) |
| Wincode | 431 (+112) | 324 (+5) | 321 (+41) | 747 (+306) | 635 (+194) |
| PinaPod | 402 (+102) | 306 (+6) | 616 (+317) | 762 (+463) | 673 (+374) |
| Light | 396 (+113) | 296 (+13) | 330 (+45) | 300 (+15) | 479 (+194) |
| Anchor slab | 397 (+115) | 297 (+15) | 349 (+55) | 388 (+94) | 334 (+40) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 2,472 (+158) | 3,824 (+1,526) | 3,570 (+1,272) | 3,616 (+1,318) | 2,617 (+303) |
| Blubber lazy | 971 (+319) | 2,489 (+1,773) | 2,173 (+1,457) | 1,376 (+660) | 851 (+199) |
| Blubber checked | 1,608 (+319) | 3,179 (+1,773) | 2,863 (+1,457) | 2,066 (+660) | 1,510 (+221) |
| Star Frame | 537 (+199) | 955 (+520) | 758 (+323) | 502 (+67) | 515 (+177) |
| Sokoban | 353 (+85) | 548 (+278) | 730 (+460) | 1,280 (+1,010) | 1,350 (+1,082) |
| Zeropod | 425 (+127) | 4,235 (+1,344) | 4,020 (+1,129) | 3,996 (+1,105) | 419 (+121) |
| Wincode | 517 (+155) | 3,361 (+904) | 3,239 (+782) | 383 (+44) | 473 (+111) |
| Critbit | 708 (+441) | 982 (+713) | 709 (+440) | 1,474 (+1,205) | 1,541 (+1,274) |
| Manifest | 440 (+94) | 679 (+366) | 701 (+388) | 723 (+410) | 808 (+462) |
| Manifest checked | 7,958 (+94) | 8,197 (+366) | 8,219 (+388) | 8,241 (+410) | 8,326 (+462) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 4,275 (+1,075) | 4,146 (+964) | 4,188 (+1,006) | 4,457 (+1,275) | 5,456 (+2,274) |
| Blubber lazy | 3,554 (+3,227) | 2,500 (+1,701) | 3,178 (+2,379) | 2,609 (+1,810) | 7,667 (+6,868) |
| Blubber checked | 4,420 (+3,204) | 3,405 (+1,681) | 4,036 (+2,312) | 3,093 (+1,369) | 8,151 (+6,427) |
| Star Frame | 3,154 (+2,795) | 847 (+368) | 1,212 (+733) | 2,857 (+2,378) | 6,329 (+5,850) |
| Zeropod | 2,937 (+1,157) | 5,446 (+2,621) | 5,546 (+2,721) | 5,815 (+2,990) | 6,814 (+3,989) |
| Wincode | 2,772 (+1,084) | 4,607 (+1,491) | 4,649 (+1,533) | 4,918 (+1,802) | 5,917 (+2,801) |
| PinaPod | 2,854 (+1,101) | 7,807 (+6,060) | 7,956 (+6,209) | 8,881 (+7,134) | 9,866 (+8,119) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 5,895 (+250) | 7,737 (+2,103) | 8,730 (+3,096) |
| Blubber lazy | 1,255 (+927) | 5,182 (+4,386) | 5,870 (+5,074) |
| Blubber checked | 2,439 (+926) | 6,533 (+4,515) | 6,344 (+4,326) |
| Star Frame | 759 (+402) | 1,679 (+1,198) | 9,057 (+8,576) |
| Zeropod | 2,119 (+307) | 8,570 (+3,781) | 9,588 (+4,799) |
| Wincode | 3,794 (+278) | 8,077 (+2,458) | 9,043 (+3,424) |

## SBF v3, n=100

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 3,072 (+1,007) | 2,075 (+10) | 5,850 (+3,796) | 5,634 (+3,580) | 5,456 (+3,402) |
| Blubber lazy | 1,563 (+1,023) | 583 (+43) | 1,621 (+905) | 1,443 (+727) | 1,336 (+620) |
| Blubber checked | 1,851 (+1,023) | 870 (+42) | 1,956 (+906) | 1,778 (+728) | 1,672 (+622) |
| Star Frame | 2,041 (+1,708) | 343 (+10) | 842 (+410) | 539 (+107) | 564 (+132) |
| Bytemuck | 1,294 (+1,017) | 285 (+8) | 681 (+402) | 288 (+9) | 304 (+25) |
| Zeropod | 1,405 (+1,108) | 305 (+8) | 907 (+549) | 719 (+361) | 588 (+230) |
| Wincode | 1,331 (+1,012) | 324 (+5) | 681 (+401) | 747 (+306) | 635 (+194) |
| PinaPod | 1,302 (+1,002) | 306 (+6) | 976 (+677) | 762 (+463) | 673 (+374) |
| Light | 1,296 (+1,013) | 296 (+13) | 690 (+405) | 300 (+15) | 1,559 (+1,274) |
| Anchor slab | 1,297 (+1,015) | 297 (+15) | 709 (+415) | 388 (+94) | 334 (+40) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 19,814 (+218) | 30,754 (+11,174) | 30,132 (+10,552) | 32,141 (+12,561) | 22,632 (+3,036) |
| Blubber lazy | 971 (+319) | 2,866 (+2,150) | 2,173 (+1,457) | 1,736 (+1,020) | 2,291 (+1,639) |
| Blubber checked | 3,498 (+319) | 5,446 (+2,150) | 4,753 (+1,457) | 4,316 (+1,020) | 5,020 (+1,841) |
| Star Frame | 537 (+199) | 1,210 (+775) | 758 (+323) | 1,042 (+607) | 2,045 (+1,707) |
| Sokoban | 495 (+227) | 670 (+400) | 1,197 (+927) | 9,049 (+8,779) | 9,749 (+9,481) |
| Zeropod | 425 (+127) | 33,706 (+8,583) | 33,111 (+7,988) | 35,050 (+9,927) | 1,409 (+1,111) |
| Wincode | 574 (+212) | 48,461 (+4,170) | 48,337 (+4,046) | 743 (+404) | 1,373 (+1,011) |
| Critbit | 988 (+721) | 1,257 (+988) | 886 (+617) | 10,759 (+10,490) | 11,456 (+11,189) |
| Manifest | 518 (+172) | 754 (+441) | 921 (+608) | 4,314 (+4,001) | 5,389 (+5,043) |
| Manifest checked | 70,259 (+172) | 70,495 (+441) | 70,662 (+608) | 74,055 (+4,001) | 75,130 (+5,043) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 39,375 (+10,705) | 37,086 (+8,434) | 37,128 (+8,476) | 40,097 (+11,445) | 50,096 (+21,444) |
| Blubber lazy | 29,474 (+29,147) | 2,500 (+1,701) | 3,628 (+2,829) | 6,839 (+6,040) | 51,524 (+50,725) |
| Blubber checked | 31,301 (+28,944) | 4,546 (+1,681) | 6,032 (+3,167) | 7,474 (+4,609) | 52,159 (+49,294) |
| Star Frame | 28,174 (+27,815) | 847 (+368) | 1,518 (+1,039) | 21,037 (+20,558) | 54,839 (+54,360) |
| Zeropod | 26,607 (+11,507) | 49,186 (+23,951) | 49,286 (+24,051) | 52,255 (+27,020) | 62,254 (+37,019) |
| Wincode | 23,742 (+10,714) | 40,067 (+13,191) | 40,109 (+13,233) | 43,078 (+16,202) | 53,077 (+26,201) |
| PinaPod | 25,624 (+10,911) | 69,679 (+54,972) | 69,828 (+55,121) | 79,455 (+64,748) | 89,080 (+74,373) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 52,911 (+310) | 68,913 (+16,323) | 82,613 (+30,023) |
| Blubber lazy | 1,255 (+927) | 7,044 (+6,248) | 30,851 (+30,055) |
| Blubber checked | 5,650 (+926) | 11,651 (+6,422) | 31,476 (+26,247) |
| Star Frame | 789 (+432) | 2,354 (+1,873) | 106,465 (+105,984) |
| Zeropod | 16,699 (+1,297) | 75,275 (+31,801) | 88,987 (+45,513) |
| Wincode | 55,880 (+276) | 95,770 (+18,807) | 110,019 (+33,056) |

## SBF v3, n=1000

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 27,480 (+10,007) | 17,483 (+10) | 54,558 (+37,096) | 50,641 (+33,179) | 50,585 (+33,123) |
| Blubber lazy | 10,563 (+10,023) | 583 (+43) | 7,921 (+7,205) | 1,443 (+727) | 2,169 (+1,453) |
| Blubber checked | 10,851 (+10,023) | 870 (+42) | 8,256 (+7,206) | 1,778 (+728) | 2,505 (+1,455) |
| Star Frame | 17,341 (+17,008) | 343 (+10) | 4,442 (+4,010) | 539 (+107) | 585 (+153) |
| Bytemuck | 10,294 (+10,017) | 285 (+8) | 4,281 (+4,002) | full | 325 (+46) |
| Zeropod | 11,305 (+11,008) | 305 (+8) | 4,551 (+4,171) | 785 (+405) | 652 (+272) |
| Wincode | 10,331 (+10,012) | 324 (+5) | 4,281 (+4,001) | 813 (+350) | 699 (+236) |
| PinaPod | 10,302 (+10,002) | 306 (+6) | 4,598 (+4,299) | 828 (+529) | 737 (+438) |
| Light | 10,296 (+10,013) | 296 (+13) | 4,290 (+4,005) | full | 12,380 (+12,095) |
| Anchor slab | 10,297 (+10,015) | 297 (+15) | 4,309 (+4,015) | 388 (+94) | 355 (+61) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 188,940 (+458) | 292,445 (+103,979) | 291,440 (+102,974) | 313,377 (+124,911) | 218,818 (+30,336) |
| Blubber lazy | 971 (+319) | 5,521 (+4,805) | 2,779 (+2,063) | 5,336 (+4,620) | 16,691 (+16,039) |
| Blubber checked | 22,398 (+319) | 27,001 (+4,805) | 24,259 (+2,063) | 26,816 (+4,620) | 40,120 (+18,041) |
| Star Frame | 537 (+199) | 1,535 (+1,100) | 827 (+392) | 6,442 (+6,007) | 17,345 (+17,007) |
| Sokoban | 582 (+314) | full | 1,524 (+1,254) | 85,668 (+85,398) | 92,668 (+92,400) |
| Zeropod | 425 (+127) | 319,828 (+77,175) | 318,838 (+76,185) | 340,705 (+98,052) | 11,309 (+11,011) |
| Wincode | 631 (+269) | 708,400 (+37,144) | 708,263 (+37,007) | 4,343 (+4,004) | 10,373 (+10,011) |
| Critbit | 1,267 (+1,000) | full | 1,063 (+794) | 102,664 (+102,395) | 109,661 (+109,394) |
| Manifest | 572 (+226) | 811 (+498) | 1,161 (+848) | 40,710 (+40,397) | 51,685 (+51,339) |
| Manifest checked | 835,564 (+226) | 835,803 (+498) | 836,153 (+848) | 875,702 (+40,397) | 886,677 (+51,339) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 390,945 (+107,005) | 367,056 (+83,134) | 367,098 (+83,176) | 397,093 (+113,171) | 497,092 (+213,170) |
| Blubber lazy | 288,674 (+288,347) | 2,500 (+1,701) | 8,530 (+7,731) | 49,139 (+48,340) | 489,238 (+488,439) |
| Blubber checked | 301,976 (+286,344) | 17,821 (+1,681) | 28,259 (+12,119) | 53,149 (+37,009) | 493,248 (+477,108) |
| Star Frame | 278,374 (+278,015) | 847 (+368) | 4,268 (+3,789) | 202,917 (+202,438) | 540,019 (+539,540) |
| Zeropod | 263,307 (+115,007) | 486,586 (+237,251) | 486,686 (+237,351) | 516,681 (+267,346) | 616,680 (+367,345) |
| Wincode | 233,442 (+107,014) | 394,667 (+130,191) | 394,709 (+130,233) | 424,704 (+160,228) | 524,703 (+260,227) |
| PinaPod | 253,324 (+109,011) | 689,003 (+544,696) | 689,152 (+544,845) | 786,917 (+642,610) | 883,004 (+738,697) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 516,914 (+550) | 670,744 (+154,391) | heap exhausted |
| Blubber lazy | 1,255 (+927) | 25,085 (+24,289) | 281,194 (+280,398) |
| Blubber checked | 39,625 (+926) | 64,117 (+24,913) | 285,194 (+245,990) |
| Star Frame | 843 (+486) | 8,003 (+7,522) | 1,287,238 (+1,286,757) |
| Zeropod | 161,350 (+10,048) | 733,279 (+307,731) | 878,112 (+452,564) |
| Wincode | 783,388 (+491) | 1,181,954 (+182,659) | 1,328,840 (+329,545) |

## Blubber fixed-list alternatives

The main W1 set-all row uses the callback setter. Fill remains a separate measured operation with the same logical output; the table records which method is cheaper.

| Arch | n | Callback whole CU | Fill whole CU | Fill / callback |
|---|---|---|---|---|
| v0 | 100 | 1,624 | 1,243 | 0.77x |
| v0 | 1000 | 7,924 | 4,843 | 0.61x |
| v3 | 100 | 1,621 | 1,240 | 0.76x |
| v3 | 1000 | 7,921 | 4,840 | 0.61x |

## Account bytes and format limits

Initial physical account bytes, before each independent operation. Dynamic formats have no spare account-tail bytes; fixed baselines reserve 1000 slots even when empty. PinaPod W3 reserves 17 payload bytes within each active string slot. Max n is a format-size calculation for the initial 8-byte strings within a 10 MiB account, capped by the configured collection bound. It is not a measured maximum executable n: heap, CU, per-invocation growth and framework limits can bind first. The list indexed format has a separate empty case; the calculation uses the nonempty stride.

| Library | Workload | n=0 | n=1 | n=10 | n=100 | n=1000 | Max n in 10 MiB |
|---|---|---|---|---|---|---|---|
| Borsh | W1 | 4 | 12 | 84 | 804 | 8004 | 1,310,719 |
| Borsh | W2 | 4 | 44 | 404 | 4004 | 40004 | 262,143 |
| Borsh | W3 | 4 | 16 | 124 | 1204 | 12004 | 873,813 |
| Borsh | W4 | 4 | 48 | 444 | 4404 | 44004 | 238,312 |
| Blubber lazy | W1 | 16 | 24 | 96 | 816 | 8016 | 1,310,718 |
| Blubber lazy | W2 | 16 | 56 | 416 | 4016 | 40016 | 262,143 |
| Blubber lazy | W3 | 16 | 24 | 132 | 1212 | 12012 | 873,812 |
| Blubber lazy | W4 | 16 | 60 | 456 | 4416 | 44016 | 238,312 |
| Blubber checked | W1 | 16 | 24 | 96 | 816 | 8016 | 1,310,718 |
| Blubber checked | W2 | 16 | 56 | 416 | 4016 | 40016 | 262,143 |
| Blubber checked | W3 | 16 | 24 | 132 | 1212 | 12012 | 873,812 |
| Blubber checked | W4 | 16 | 60 | 456 | 4416 | 44016 | 238,312 |
| Star Frame | W1 | 4 | 12 | 84 | 804 | 8004 | 1,310,719 |
| Star Frame | W2 | 4 | 44 | 404 | 4004 | 40004 | 262,143 |
| Star Frame | W3 | 12 | 28 | 172 | 1612 | 16012 | 655,359 |
| Star Frame | W4 | 12 | 60 | 492 | 4812 | 48012 | 218,453 |
| Bytemuck | W1 | 8008 | 8008 | 8008 | 8008 | 8008 | 1,000 |
| Sokoban | W2 | 56032 | 56032 | 56032 | 56032 | 56032 | 1,000 |
| Zeropod | W1 | 4 | 12 | 84 | 804 | 8004 | 1,310,719 |
| Zeropod | W2 | 4 | 44 | 404 | 4004 | 40004 | 262,143 |
| Zeropod | W3 | 4 | 16 | 124 | 1204 | 12004 | 873,813 |
| Zeropod | W4 | 4 | 48 | 444 | 4404 | 44004 | 238,312 |
| Wincode | W1 | 8 | 16 | 88 | 808 | 8008 | 1,310,719 |
| Wincode | W2 | 8 | 48 | 408 | 4008 | 40008 | 262,143 |
| Wincode | W3 | 8 | 24 | 168 | 1608 | 16008 | 655,359 |
| Wincode | W4 | 8 | 56 | 488 | 4808 | 48008 | 218,453 |
| PinaPod | W1 | 2 | 10 | 82 | 802 | 8002 | 1,001 |
| PinaPod | W3 | 2 | 20 | 182 | 1802 | 18002 | 1,001 |
| Light | W1 | 8016 | 8016 | 8016 | 8016 | 8016 | 1,000 |
| Anchor slab | W1 | 16 | 24 | 96 | 816 | 8016 | 1,310,718 |
| Critbit | W2 | 120048 | 120048 | 120048 | 120048 | 120048 | 1,000 |
| Manifest | W2 | 32 | 88 | 592 | 5632 | 56032 | 187,245 |
| Manifest checked | W2 | 32 | 88 | 592 | 5632 | 56032 | 187,245 |

## Heap high water

Separate diagnostic ELFs report forward bump consumption, including abandoned allocations and alignment, before a final one-byte sentinel. The reported value excludes the allocator's initial eight-byte cursor and the sentinel. It is not live allocation size. Every successful row has its own heap and idle-heap values in [all.csv](compare/results/all.csv). OOM rows have no successful final watermark and remain failures.

| Arch | Library | Largest successful n=1000 heap, bytes | Operation |
|---|---|---|---|
| v0 | Borsh | 251,008 | W4 insert_middle |
| v0 | Blubber lazy | 33,000 | W3 grow_all_old |
| v0 | Blubber checked | 33,000 | W3 grow_all_old |
| v0 | Star Frame | 132,928 | W4 grow_all_old |
| v0 | Bytemuck | 0 | W1 read_all |
| v0 | Sokoban | 200 | W2 set_all |
| v0 | Zeropod | 195,912 | W4 grow_all_old |
| v0 | Wincode | 132,848 | W4 grow_all_old |
| v0 | PinaPod | 24,000 | W1 push |
| v0 | Light | 7,992 | W1 remove_front |
| v0 | Anchor slab | 0 | W1 read_all |
| v0 | Critbit | 200 | W2 set_all |
| v0 | Manifest | 0 | W2 lookup |
| v0 | Manifest checked | 27,376 | W2 lookup |
| v3 | Borsh | 251,008 | W4 insert_middle |
| v3 | Blubber lazy | 33,000 | W3 grow_all_old |
| v3 | Blubber checked | 33,000 | W3 grow_all_old |
| v3 | Star Frame | 132,928 | W4 grow_all_old |
| v3 | Bytemuck | 0 | W1 read_all |
| v3 | Sokoban | 200 | W2 set_all |
| v3 | Zeropod | 195,912 | W4 grow_all_old |
| v3 | Wincode | 132,848 | W4 grow_all_old |
| v3 | PinaPod | 24,000 | W1 push |
| v3 | Light | 7,992 | W1 remove_front |
| v3 | Anchor slab | 0 | W1 read_all |
| v3 | Critbit | 200 | W2 set_all |
| v3 | Manifest | 0 | W2 lookup |
| v3 | Manifest checked | 27,376 | W2 lookup |

## Published blubber calibration

These replay the original published mixed fixture, including its original inputs, in this comparison harness. The comparison workloads use different input values and caller code, so their totals need not equal these calibration totals. Every calibration must be within 1% of the published row.

| Arch | Published case | Published CU | Observed CU | Difference |
|---|---|---|---|---|
| v0 | list_fixed_n1000_ref_iter_plain | 10,799 | 10,798 | 0.009% |
| v0 | list_fixed_n1000_callback_equal_set_plain | 8,122 | 8,121 | 0.012% |
| v0 | map_fixed_n1000_ref_iter_plain | 91,136 | 91,135 | 0.001% |
| v0 | map_fixed_n1000_callback_equal_set_plain | 65,949 | 65,948 | 0.002% |
| v0 | list_indexed_n1000_fill_plain | 49,018 | 49,008 | 0.020% |
| v0 | map_indexed_n1000_fill_plain | 74,147 | 74,149 | 0.003% |
| v0 | map_indexed_n1000_map_grow_from_old_values_plain | 485,745 | 482,747 | 0.617% |
| v3 | list_fixed_n1000_ref_iter_plain | 10,797 | 10,794 | 0.028% |
| v3 | list_fixed_n1000_callback_equal_set_plain | 8,119 | 8,116 | 0.037% |
| v3 | map_fixed_n1000_ref_iter_plain | 91,134 | 91,131 | 0.003% |
| v3 | map_fixed_n1000_callback_equal_set_plain | 65,946 | 65,943 | 0.005% |
| v3 | list_indexed_n1000_fill_plain | 52,993 | 52,983 | 0.019% |
| v3 | map_indexed_n1000_fill_plain | 74,130 | 74,133 | 0.004% |
| v3 | map_indexed_n1000_map_grow_from_old_values_plain | 485,729 | 482,728 | 0.618% |

## Failed observations

| Arch | Library | Workload | n | Operation | Result |
|---|---|---|---|---|---|
| v0 | Borsh | W4 | 1000 | grow_all_old | heap_exhausted |
| v0 | Bytemuck | W1 | 1000 | push | capacity_exhausted |
| v0 | Sokoban | W2 | 1000 | insert_middle | capacity_exhausted |
| v0 | Light | W1 | 1000 | push | capacity_exhausted |
| v0 | Critbit | W2 | 1000 | insert_middle | capacity_exhausted |
| v3 | Borsh | W4 | 1000 | grow_all_old | heap_exhausted |
| v3 | Bytemuck | W1 | 1000 | push | capacity_exhausted |
| v3 | Sokoban | W2 | 1000 | insert_middle | capacity_exhausted |
| v3 | Light | W1 | 1000 | push | capacity_exhausted |
| v3 | Critbit | W2 | 1000 | insert_middle | capacity_exhausted |

<!-- compare-tables:end -->
