# Benchmark comparison

## Reproduce the comparison

Raw measurements are in [`compare/results/all.csv`](compare/results/all.csv). The benchmark receipt in [`compare/results/receipt.json`](compare/results/receipt.json) records the inputs, tools, binary hashes and evidence for this run setup. Run `bash scripts/reproduce.sh cu compare` from the repository root to rebuild the whole-instruction, heap and calibration runs for both SBF architectures into the target directory's `reproduce` scratch area.

<!-- compare-summary:start -->

Blubber lazy list sum costs 10,565 CU versus 10,295 for Bytemuck; map lookup costs 973 versus 426 for Zeropod. Blubber lazy string-list growth costs 45,110 CU versus 202,917 for Star Frame, which the maintainer helped build. Old-dependent string-map growth costs 283,164 versus 878,116 for Zeropod. These are v0 whole-instruction totals for the measured methods and load policies.

<!-- compare-summary:end -->

## Method

The harness runs on Mollusk 0.16.2<!-- num:cmp.environment.mollusk -->, Agave 4.3.0<!-- num:cmp.environment.agave --> and platform-tools 1.53<!-- num:cmp.environment.platform_tools -->. It configures an instruction meter of 40,000,000<!-- num:cmp.environment.cu_meter --> CU and a 256<!-- num:cmp.environment.heap_kib --> KiB heap so large rewrites finish instead of hitting the default 32<!-- num:runtime.default.heap_kib --> KiB production limit. Each run starts with one fresh, writable program-owned account with enough prepaid rent for growth, so no rent-transfer CPI runs.

A CU is the Solana runtime measure of compute work. The primary metric is whole CU, the total for the entire instruction. Each operation has a matched idle control for load and close. Successful operation rows also report the signed delta. A separate diagnostic build measures dynamic heap memory as forward bump consumption, including alignment and abandoned allocations before a sentinel. OOM means a run exhausted that heap and left no final watermark. See [`benchmarks/CU.md#read-a-row`](CU.md#read-a-row) for the primary harness.

Calibration replays the primary traversal fixtures, whereas the comparison fixtures include their own dispatch and checksum work. Indexed list strings start as hexadecimal ordinals of 8<!-- num:cmp.fixture.initial_string --> bytes, and the constant fill replaces every list item with one 17<!-- num:cmp.fixture.longer_string -->-byte string. Rewriting from the old value computes a distinct replacement per item.

In the map rewrite, both [`blubber_adapter.rs`](compare/program/src/blubber_adapter.rs) and [`blubber_checked_adapter.rs`](compare/program/src/blubber_checked_adapter.rs) call `MapMut<K, Str>::rewrite_values_with`. On SBF v0 at 1,000 items, lazy Blubber uses fewer whole CU than Zeropod (283,164<!-- num:cmp.v0.n1000.map_grow_from_old.lazy --> CU versus 878,116<!-- num:cmp.v0.n1000.map_grow_from_old.zeropod --> CU), a lower operation delta after subtracting the matched idle (282,361<!-- num:cmp.v0.n1000.map_grow_from_old.lazy.delta --> CU versus 452,564<!-- num:cmp.v0.n1000.map_grow_from_old.zeropod.delta --> CU), and less heap (17,000<!-- num:cmp.v0.n1000.map_grow_from_old.lazy.heap_bytes --> bytes versus 195,912<!-- num:cmp.v0.n1000.map_grow_from_old.zeropod.heap_bytes --> bytes). Borsh hits OOM on that row. For the separate checked caller, see [`benchmarks/TRAVERSAL.md#ordinary-caller-costs`](TRAVERSAL.md#ordinary-caller-costs).

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
| Blubber lazy | 0.1.1 | [4a1b9af7e77a](../README.md) | 2026-11-24 | MIT | 0 |
| Blubber checked | 0.1.1 | [4a1b9af7e77a](../README.md) | 2026-11-24 | MIT | 0 |
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
| W1 | read_all | 0 | 849 (+18) | 560 (+18) |
| W1 | read_all | 1 | 864 (+33) | 575 (+33) |
| W1 | read_all | 10 | 954 (+123) | 665 (+123) |
| W1 | read_all | 100 | 1,854 (+1,023) | 1,565 (+1,023) |
| W1 | read_all | 1000 | 10,854 (+10,023) | 10,565 (+10,023) |
| W1 | get_middle | 0 | 865 (+34) | 576 (+34) |
| W1 | get_middle | 1 | 873 (+42) | 585 (+43) |
| W1 | get_middle | 10 | 873 (+42) | 585 (+43) |
| W1 | get_middle | 100 | 873 (+42) | 585 (+43) |
| W1 | get_middle | 1000 | 873 (+42) | 585 (+43) |
| W1 | set_all | 0 | 1,261 (+203) | 925 (+202) |
| W1 | set_all | 1 | 1,270 (+212) | 934 (+211) |
| W1 | set_all | 10 | 1,334 (+276) | 998 (+275) |
| W1 | set_all | 100 | 1,964 (+906) | 1,628 (+905) |
| W1 | set_all | 1000 | 8,264 (+7,206) | 7,928 (+7,205) |
| W1 | push | 0 | 1,739 (+681) | 1,403 (+680) |
| W1 | push | 1 | 1,739 (+681) | 1,403 (+680) |
| W1 | push | 10 | 1,739 (+681) | 1,403 (+680) |
| W1 | push | 100 | 1,739 (+681) | 1,403 (+680) |
| W1 | push | 1000 | 1,739 (+681) | 1,403 (+680) |
| W1 | remove_front | 0 | 1,056 (-2) | 721 (-2) |
| W1 | remove_front | 1 | 1,463 (+405) | 1,126 (+403) |
| W1 | remove_front | 10 | 1,634 (+576) | 1,297 (+574) |
| W1 | remove_front | 100 | 1,634 (+576) | 1,297 (+574) |
| W1 | remove_front | 1000 | 2,467 (+1,409) | 2,130 (+1,407) |
| W2 | lookup | 0 | 1,033 (+23) | 636 (+22) |
| W2 | lookup | 1 | 1,406 (+319) | 973 (+319) |
| W2 | lookup | 10 | 1,611 (+319) | 973 (+319) |
| W2 | lookup | 100 | 3,501 (+319) | 973 (+319) |
| W2 | lookup | 1000 | 22,401 (+319) | 973 (+319) |
| W2 | insert_middle | 0 | 2,127 (+959) | 1,682 (+959) |
| W2 | insert_middle | 1 | 2,465 (+1,256) | 1,979 (+1,256) |
| W2 | insert_middle | 10 | 3,132 (+1,718) | 2,441 (+1,718) |
| W2 | insert_middle | 100 | 5,397 (+2,093) | 2,816 (+2,093) |
| W2 | insert_middle | 1000 | 26,916 (+4,712) | 5,435 (+4,712) |
| W2 | remove | 0 | 1,773 (+605) | 1,328 (+605) |
| W2 | remove | 1 | 2,299 (+1,090) | 1,813 (+1,090) |
| W2 | remove | 10 | 2,819 (+1,405) | 2,128 (+1,405) |
| W2 | remove | 100 | 4,709 (+1,405) | 2,128 (+1,405) |
| W2 | remove | 1000 | 24,215 (+2,011) | 2,734 (+2,011) |
| W2 | set_all | 0 | 1,679 (+511) | 1,234 (+511) |
| W2 | set_all | 1 | 1,839 (+630) | 1,353 (+630) |
| W2 | set_all | 10 | 2,080 (+666) | 1,389 (+666) |
| W2 | set_all | 100 | 4,330 (+1,026) | 1,749 (+1,026) |
| W2 | set_all | 1000 | 26,830 (+4,626) | 5,349 (+4,626) |
| W2 | iterate_sum | 0 | 1,033 (+23) | 637 (+23) |
| W2 | iterate_sum | 1 | 1,146 (+59) | 709 (+55) |
| W2 | iterate_sum | 10 | 1,513 (+221) | 853 (+199) |
| W2 | iterate_sum | 100 | 5,023 (+1,841) | 2,293 (+1,639) |
| W2 | iterate_sum | 1000 | 40,123 (+18,041) | 16,693 (+16,039) |
| W3 | read_all | 0 | 1,117 (+259) | 598 (+269) |
| W3 | read_all | 1 | 1,606 (+606) | 940 (+611) |
| W3 | read_all | 10 | 4,431 (+3,204) | 3,556 (+3,227) |
| W3 | read_all | 100 | 31,312 (+28,944) | 29,476 (+29,147) |
| W3 | read_all | 1000 | 301,987 (+286,344) | 288,676 (+288,347) |
| W3 | set_equal | 0 | 1,372 (+1) | 807 (+1) |
| W3 | set_equal | 1 | 3,058 (+1,545) | 2,372 (+1,566) |
| W3 | set_equal | 10 | 3,424 (+1,684) | 2,512 (+1,706) |
| W3 | set_equal | 100 | 4,565 (+1,684) | 2,512 (+1,706) |
| W3 | set_equal | 1000 | 17,840 (+1,684) | 2,512 (+1,706) |
| W3 | grow_one | 0 | 1,372 (+1) | 807 (+1) |
| W3 | grow_one | 1 | 3,357 (+1,844) | 2,647 (+1,841) |
| W3 | grow_one | 10 | 4,009 (+2,269) | 3,141 (+2,335) |
| W3 | grow_one | 100 | 6,005 (+3,124) | 3,591 (+2,785) |
| W3 | grow_one | 1000 | 28,226 (+12,070) | 8,487 (+7,681) |
| W3 | grow_all | 0 | 1,955 (+584) | 1,629 (+823) |
| W3 | grow_all | 1 | 2,447 (+934) | 2,084 (+1,278) |
| W3 | grow_all | 10 | 3,031 (+1,291) | 2,540 (+1,734) |
| W3 | grow_all | 100 | 7,052 (+4,171) | 6,410 (+5,604) |
| W3 | grow_all | 1000 | 49,127 (+32,971) | 45,110 (+44,304) |
| W3 | grow_all_old | 0 | 2,633 (+1,262) | 2,426 (+1,620) |
| W3 | grow_all_old | 1 | 3,537 (+2,024) | 3,174 (+2,368) |
| W3 | grow_all_old | 10 | 8,117 (+6,377) | 7,626 (+6,820) |
| W3 | grow_all_old | 100 | 52,125 (+49,244) | 51,483 (+50,677) |
| W3 | grow_all_old | 1000 | 493,214 (+477,058) | 489,197 (+488,391) |
| W4 | lookup | 0 | 1,120 (+279) | 610 (+280) |
| W4 | lookup | 1 | 1,988 (+912) | 1,243 (+913) |
| W4 | lookup | 10 | 2,452 (+927) | 1,258 (+928) |
| W4 | lookup | 100 | 5,663 (+927) | 1,258 (+928) |
| W4 | lookup | 1000 | 39,638 (+927) | 1,258 (+928) |
| W4 | insert_middle | 0 | 4,195 (+2,844) | 3,413 (+2,610) |
| W4 | insert_middle | 1 | 5,194 (+3,608) | 4,158 (+3,355) |
| W4 | insert_middle | 10 | 6,521 (+4,486) | 5,018 (+4,215) |
| W4 | insert_middle | 100 | 11,637 (+6,391) | 6,878 (+6,075) |
| W4 | insert_middle | 1000 | 64,059 (+24,838) | 24,875 (+24,072) |
| W4 | grow_all_old | 0 | 2,130 (+779) | 1,674 (+871) |
| W4 | grow_all_old | 1 | 3,599 (+2,013) | 3,247 (+2,444) |
| W4 | grow_all_old | 10 | 6,340 (+4,305) | 5,860 (+5,057) |
| W4 | grow_all_old | 100 | 31,652 (+26,406) | 31,021 (+30,218) |
| W4 | grow_all_old | 1000 | 287,170 (+247,949) | 283,164 (+282,361) |

## SBF v0, n=0

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 325 (+5) | 326 (+6) | 398 (+89) | 566 (+257) | 400 (+91) |
| Blubber lazy | 560 (+18) | 576 (+34) | 925 (+202) | 1,403 (+680) | 721 (-2) |
| Blubber checked | 849 (+18) | 865 (+34) | 1,261 (+203) | 1,739 (+681) | 1,056 (-2) |
| Star Frame | 335 (+2) | 336 (+3) | 439 (+7) | 539 (+107) | 433 (+1) |
| Bytemuck | 291 (+13) | 281 (+3) | 280 (+0) | 289 (+9) | 280 (+0) |
| Zeropod | 302 (+4) | 302 (+4) | 477 (+147) | 668 (+338) | 479 (+149) |
| Wincode | 328 (+8) | 322 (+2) | 282 (+1) | 697 (+283) | 569 (+155) |
| PinaPod | 301 (+0) | 302 (+1) | 513 (+213) | 716 (+416) | 526 (+226) |
| Light | 289 (+5) | 287 (+3) | 289 (+3) | 301 (+15) | 328 (+42) |
| Anchor slab | 274 (+10) | 273 (+9) | 288 (+14) | 356 (+82) | 281 (+7) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 351 (+11) | 662 (+338) | 424 (+100) | 421 (+97) | 348 (+8) |
| Blubber lazy | 636 (+22) | 1,682 (+959) | 1,328 (+605) | 1,234 (+511) | 637 (+23) |
| Blubber checked | 1,033 (+23) | 2,127 (+959) | 1,773 (+605) | 1,679 (+511) | 1,033 (+23) |
| Star Frame | 364 (+26) | 652 (+217) | 486 (+51) | 440 (+5) | 339 (+1) |
| Sokoban | 279 (+10) | 360 (+89) | 279 (+8) | 338 (+67) | 337 (+68) |
| Zeropod | 314 (+15) | 760 (+435) | 484 (+159) | 481 (+156) | 306 (+7) |
| Wincode | 398 (+35) | 821 (+376) | 649 (+204) | 341 (+1) | 369 (+6) |
| Critbit | 279 (+12) | 400 (+131) | 279 (+10) | 335 (+66) | 331 (+64) |
| Manifest | 373 (+34) | 546 (+240) | 398 (+92) | 356 (+50) | 349 (+10) |
| Manifest checked | 375 (+34) | 548 (+240) | 400 (+92) | 358 (+50) | 351 (+10) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 325 (+3) | 390 (+86) | 388 (+84) | 389 (+85) | 390 (+86) |
| Blubber lazy | 598 (+269) | 807 (+1) | 807 (+1) | 1,629 (+823) | 2,426 (+1,620) |
| Blubber checked | 1,117 (+259) | 1,372 (+1) | 1,372 (+1) | 1,955 (+584) | 2,633 (+1,262) |
| Star Frame | 363 (+4) | 482 (+3) | 482 (+3) | 665 (+186) | 751 (+272) |
| Zeropod | 303 (+4) | 499 (+193) | 497 (+191) | 498 (+192) | 499 (+193) |
| Wincode | 394 (+10) | 584 (+138) | 582 (+136) | 583 (+137) | 584 (+138) |
| PinaPod | 310 (+3) | 762 (+461) | 762 (+461) | 756 (+455) | 764 (+463) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 345 (+11) | 744 (+421) | 413 (+90) |
| Blubber lazy | 610 (+280) | 3,413 (+2,610) | 1,674 (+871) |
| Blubber checked | 1,120 (+279) | 4,195 (+2,844) | 2,130 (+779) |
| Star Frame | 361 (+4) | 1,273 (+792) | 823 (+342) |
| Zeropod | 307 (+6) | 907 (+571) | 532 (+196) |
| Wincode | 384 (+10) | 892 (+505) | 525 (+138) |

## SBF v0, n=1

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 400 (+17) | 393 (+10) | 505 (+133) | 686 (+314) | 502 (+130) |
| Blubber lazy | 575 (+33) | 585 (+43) | 934 (+211) | 1,403 (+680) | 1,126 (+403) |
| Blubber checked | 864 (+33) | 873 (+42) | 1,270 (+212) | 1,739 (+681) | 1,463 (+405) |
| Star Frame | 358 (+25) | 343 (+10) | 446 (+14) | 539 (+107) | 546 (+114) |
| Bytemuck | 305 (+27) | 286 (+8) | 286 (+6) | 289 (+9) | 305 (+25) |
| Zeropod | 317 (+19) | 306 (+8) | 512 (+153) | 721 (+362) | 589 (+230) |
| Wincode | 342 (+22) | 325 (+5) | 286 (+5) | 749 (+307) | 636 (+194) |
| PinaPod | 313 (+12) | 307 (+6) | 586 (+286) | 767 (+467) | 636 (+336) |
| Light | 307 (+23) | 297 (+13) | 295 (+9) | 301 (+15) | 331 (+45) |
| Anchor slab | 287 (+23) | 278 (+14) | 297 (+23) | 356 (+82) | 310 (+36) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 698 (+58) | 1,140 (+516) | 891 (+267) | 853 (+229) | 682 (+42) |
| Blubber lazy | 973 (+319) | 1,979 (+1,256) | 1,813 (+1,090) | 1,353 (+630) | 709 (+55) |
| Blubber checked | 1,406 (+319) | 2,465 (+1,256) | 2,299 (+1,090) | 1,839 (+630) | 1,146 (+59) |
| Star Frame | 537 (+199) | 784 (+349) | 740 (+305) | 448 (+13) | 362 (+24) |
| Sokoban | 325 (+56) | 445 (+174) | 369 (+98) | 504 (+233) | 511 (+242) |
| Zeropod | 426 (+127) | 1,301 (+586) | 1,085 (+370) | 983 (+268) | 321 (+22) |
| Wincode | 426 (+63) | 1,047 (+488) | 931 (+372) | 348 (+8) | 384 (+21) |
| Critbit | 330 (+63) | 615 (+346) | 346 (+77) | 449 (+180) | 453 (+186) |
| Manifest | 423 (+76) | 641 (+327) | 541 (+227) | 396 (+82) | 382 (+35) |
| Manifest checked | 1,279 (+76) | 1,497 (+327) | 1,397 (+227) | 1,252 (+82) | 1,238 (+35) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 766 (+112) | 853 (+217) | 895 (+259) | 894 (+258) | 993 (+357) |
| Blubber lazy | 940 (+611) | 2,372 (+1,566) | 2,647 (+1,841) | 2,084 (+1,278) | 3,174 (+2,368) |
| Blubber checked | 1,606 (+606) | 3,058 (+1,545) | 3,357 (+1,844) | 2,447 (+934) | 3,537 (+2,024) |
| Star Frame | 664 (+305) | 847 (+368) | 1,114 (+635) | 1,039 (+560) | 1,490 (+1,011) |
| Zeropod | 559 (+122) | 1,042 (+476) | 1,142 (+576) | 1,141 (+575) | 1,240 (+674) |
| Wincode | 676 (+121) | 1,062 (+321) | 1,104 (+363) | 1,103 (+362) | 1,202 (+461) |
| PinaPod | 584 (+120) | 1,623 (+1,165) | 1,772 (+1,314) | 1,758 (+1,300) | 1,877 (+1,419) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 1,180 (+150) | 1,662 (+643) | 1,460 (+441) |
| Blubber lazy | 1,243 (+913) | 4,158 (+3,355) | 3,247 (+2,444) |
| Blubber checked | 1,988 (+912) | 5,194 (+3,608) | 3,599 (+2,013) |
| Star Frame | 678 (+321) | 1,451 (+970) | 1,689 (+1,208) |
| Zeropod | 639 (+197) | 1,913 (+941) | 1,736 (+764) |
| Wincode | 750 (+158) | 1,587 (+755) | 1,358 (+526) |

## SBF v0, n=10

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 643 (+107) | 546 (+10) | 991 (+466) | 1,135 (+610) | 957 (+432) |
| Blubber lazy | 665 (+123) | 585 (+43) | 998 (+275) | 1,403 (+680) | 1,297 (+574) |
| Blubber checked | 954 (+123) | 873 (+42) | 1,334 (+276) | 1,739 (+681) | 1,634 (+576) |
| Star Frame | 511 (+178) | 343 (+10) | 482 (+50) | 539 (+107) | 564 (+132) |
| Bytemuck | 395 (+117) | 286 (+8) | 322 (+42) | 289 (+9) | 305 (+25) |
| Zeropod | 416 (+118) | 306 (+8) | 548 (+189) | 720 (+361) | 589 (+230) |
| Wincode | 432 (+112) | 325 (+5) | 322 (+41) | 748 (+306) | 636 (+194) |
| PinaPod | 403 (+102) | 307 (+6) | 620 (+320) | 766 (+466) | 680 (+380) |
| Light | 397 (+113) | 297 (+13) | 331 (+45) | 301 (+15) | 480 (+194) |
| Anchor slab | 377 (+113) | 278 (+14) | 333 (+59) | 356 (+82) | 329 (+55) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 2,473 (+158) | 3,825 (+1,526) | 3,571 (+1,272) | 3,617 (+1,318) | 2,618 (+303) |
| Blubber lazy | 973 (+319) | 2,441 (+1,718) | 2,128 (+1,405) | 1,389 (+666) | 853 (+199) |
| Blubber checked | 1,611 (+319) | 3,132 (+1,718) | 2,819 (+1,405) | 2,080 (+666) | 1,513 (+221) |
| Star Frame | 537 (+199) | 955 (+520) | 758 (+323) | 502 (+67) | 515 (+177) |
| Sokoban | 354 (+85) | 549 (+278) | 731 (+460) | 1,281 (+1,010) | 1,351 (+1,082) |
| Zeropod | 426 (+127) | 4,236 (+1,344) | 4,021 (+1,129) | 3,997 (+1,105) | 420 (+121) |
| Wincode | 518 (+155) | 3,362 (+904) | 3,240 (+782) | 384 (+44) | 474 (+111) |
| Critbit | 708 (+441) | 982 (+713) | 709 (+440) | 1,474 (+1,205) | 1,541 (+1,274) |
| Manifest | 441 (+94) | 680 (+366) | 702 (+388) | 724 (+410) | 809 (+462) |
| Manifest checked | 7,959 (+94) | 8,198 (+366) | 8,220 (+388) | 8,242 (+410) | 8,327 (+462) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 4,276 (+1,075) | 4,147 (+964) | 4,189 (+1,006) | 4,458 (+1,275) | 5,457 (+2,274) |
| Blubber lazy | 3,556 (+3,227) | 2,512 (+1,706) | 3,141 (+2,335) | 2,540 (+1,734) | 7,626 (+6,820) |
| Blubber checked | 4,431 (+3,204) | 3,424 (+1,684) | 4,009 (+2,269) | 3,031 (+1,291) | 8,117 (+6,377) |
| Star Frame | 3,154 (+2,795) | 847 (+368) | 1,212 (+733) | 2,857 (+2,378) | 6,329 (+5,850) |
| Zeropod | 2,938 (+1,157) | 5,447 (+2,621) | 5,547 (+2,721) | 5,816 (+2,990) | 6,815 (+3,989) |
| Wincode | 2,773 (+1,084) | 4,608 (+1,491) | 4,650 (+1,533) | 4,919 (+1,802) | 5,918 (+2,801) |
| PinaPod | 2,855 (+1,101) | 7,809 (+6,061) | 7,958 (+6,210) | 8,883 (+7,135) | 9,868 (+8,120) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 5,896 (+250) | 7,738 (+2,103) | 8,731 (+3,096) |
| Blubber lazy | 1,258 (+928) | 5,018 (+4,215) | 5,860 (+5,057) |
| Blubber checked | 2,452 (+927) | 6,521 (+4,486) | 6,340 (+4,305) |
| Star Frame | 759 (+402) | 1,679 (+1,198) | 9,057 (+8,576) |
| Zeropod | 2,120 (+307) | 8,571 (+3,781) | 9,589 (+4,799) |
| Wincode | 3,795 (+278) | 8,078 (+2,458) | 9,044 (+3,424) |

## SBF v0, n=100

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 3,073 (+1,007) | 2,076 (+10) | 5,851 (+3,796) | 5,635 (+3,580) | 5,457 (+3,402) |
| Blubber lazy | 1,565 (+1,023) | 585 (+43) | 1,628 (+905) | 1,403 (+680) | 1,297 (+574) |
| Blubber checked | 1,854 (+1,023) | 873 (+42) | 1,964 (+906) | 1,739 (+681) | 1,634 (+576) |
| Star Frame | 2,041 (+1,708) | 343 (+10) | 842 (+410) | 539 (+107) | 564 (+132) |
| Bytemuck | 1,295 (+1,017) | 286 (+8) | 682 (+402) | 289 (+9) | 305 (+25) |
| Zeropod | 1,406 (+1,108) | 306 (+8) | 908 (+549) | 720 (+361) | 589 (+230) |
| Wincode | 1,332 (+1,012) | 325 (+5) | 682 (+401) | 748 (+306) | 636 (+194) |
| PinaPod | 1,303 (+1,002) | 307 (+6) | 980 (+680) | 766 (+466) | 680 (+380) |
| Light | 1,297 (+1,013) | 297 (+13) | 691 (+405) | 301 (+15) | 1,560 (+1,274) |
| Anchor slab | 1,277 (+1,013) | 278 (+14) | 693 (+419) | 356 (+82) | 329 (+55) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 19,818 (+218) | 30,758 (+11,174) | 30,136 (+10,552) | 32,145 (+12,561) | 22,636 (+3,036) |
| Blubber lazy | 973 (+319) | 2,816 (+2,093) | 2,128 (+1,405) | 1,749 (+1,026) | 2,293 (+1,639) |
| Blubber checked | 3,501 (+319) | 5,397 (+2,093) | 4,709 (+1,405) | 4,330 (+1,026) | 5,023 (+1,841) |
| Star Frame | 537 (+199) | 1,210 (+775) | 758 (+323) | 1,042 (+607) | 2,045 (+1,707) |
| Sokoban | 496 (+227) | 671 (+400) | 1,198 (+927) | 9,050 (+8,779) | 9,750 (+9,481) |
| Zeropod | 426 (+127) | 33,710 (+8,583) | 33,115 (+7,988) | 35,054 (+9,927) | 1,410 (+1,111) |
| Wincode | 575 (+212) | 48,462 (+4,170) | 48,338 (+4,046) | 744 (+404) | 1,374 (+1,011) |
| Critbit | 988 (+721) | 1,257 (+988) | 886 (+617) | 10,759 (+10,490) | 11,456 (+11,189) |
| Manifest | 519 (+172) | 755 (+441) | 922 (+608) | 4,315 (+4,001) | 5,390 (+5,043) |
| Manifest checked | 70,260 (+172) | 70,496 (+441) | 70,663 (+608) | 74,056 (+4,001) | 75,131 (+5,043) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 39,376 (+10,705) | 37,087 (+8,434) | 37,129 (+8,476) | 40,098 (+11,445) | 50,097 (+21,444) |
| Blubber lazy | 29,476 (+29,147) | 2,512 (+1,706) | 3,591 (+2,785) | 6,410 (+5,604) | 51,483 (+50,677) |
| Blubber checked | 31,312 (+28,944) | 4,565 (+1,684) | 6,005 (+3,124) | 7,052 (+4,171) | 52,125 (+49,244) |
| Star Frame | 28,174 (+27,815) | 847 (+368) | 1,518 (+1,039) | 21,037 (+20,558) | 54,839 (+54,360) |
| Zeropod | 26,608 (+11,507) | 49,187 (+23,951) | 49,287 (+24,051) | 52,256 (+27,020) | 62,255 (+37,019) |
| Wincode | 23,743 (+10,714) | 40,068 (+13,191) | 40,110 (+13,233) | 43,079 (+16,202) | 53,078 (+26,201) |
| PinaPod | 25,625 (+10,911) | 69,681 (+54,973) | 69,830 (+55,122) | 79,457 (+64,749) | 89,082 (+74,374) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 52,915 (+310) | 68,917 (+16,323) | 82,617 (+30,023) |
| Blubber lazy | 1,258 (+928) | 6,878 (+6,075) | 31,021 (+30,218) |
| Blubber checked | 5,663 (+927) | 11,637 (+6,391) | 31,652 (+26,406) |
| Star Frame | 789 (+432) | 2,354 (+1,873) | 106,465 (+105,984) |
| Zeropod | 16,700 (+1,297) | 75,279 (+31,801) | 88,991 (+45,513) |
| Wincode | 55,881 (+276) | 95,771 (+18,807) | 110,020 (+33,056) |

## SBF v0, n=1000

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 27,481 (+10,007) | 17,484 (+10) | 54,559 (+37,096) | 50,642 (+33,179) | 50,586 (+33,123) |
| Blubber lazy | 10,565 (+10,023) | 585 (+43) | 7,928 (+7,205) | 1,403 (+680) | 2,130 (+1,407) |
| Blubber checked | 10,854 (+10,023) | 873 (+42) | 8,264 (+7,206) | 1,739 (+681) | 2,467 (+1,409) |
| Star Frame | 17,341 (+17,008) | 343 (+10) | 4,442 (+4,010) | 539 (+107) | 585 (+153) |
| Bytemuck | 10,295 (+10,017) | 286 (+8) | 4,282 (+4,002) | full | 326 (+46) |
| Zeropod | 11,306 (+11,008) | 306 (+8) | 4,552 (+4,171) | 786 (+405) | 653 (+272) |
| Wincode | 10,332 (+10,012) | 325 (+5) | 4,282 (+4,001) | 814 (+350) | 700 (+236) |
| PinaPod | 10,303 (+10,002) | 307 (+6) | 4,602 (+4,302) | 832 (+532) | 744 (+444) |
| Light | 10,297 (+10,013) | 297 (+13) | 4,291 (+4,005) | full | 12,381 (+12,095) |
| Anchor slab | 10,277 (+10,013) | 278 (+14) | 4,293 (+4,019) | 356 (+82) | 350 (+76) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 188,944 (+458) | 292,449 (+103,979) | 291,444 (+102,974) | 313,381 (+124,911) | 218,822 (+30,336) |
| Blubber lazy | 973 (+319) | 5,435 (+4,712) | 2,734 (+2,011) | 5,349 (+4,626) | 16,693 (+16,039) |
| Blubber checked | 22,401 (+319) | 26,916 (+4,712) | 24,215 (+2,011) | 26,830 (+4,626) | 40,123 (+18,041) |
| Star Frame | 537 (+199) | 1,535 (+1,100) | 827 (+392) | 6,442 (+6,007) | 17,345 (+17,007) |
| Sokoban | 583 (+314) | full | 1,525 (+1,254) | 85,669 (+85,398) | 92,669 (+92,400) |
| Zeropod | 426 (+127) | 319,832 (+77,175) | 318,842 (+76,185) | 340,709 (+98,052) | 11,310 (+11,011) |
| Wincode | 632 (+269) | 708,401 (+37,144) | 708,264 (+37,007) | 4,344 (+4,004) | 10,374 (+10,011) |
| Critbit | 1,267 (+1,000) | full | 1,063 (+794) | 102,664 (+102,395) | 109,661 (+109,394) |
| Manifest | 573 (+226) | 812 (+498) | 1,162 (+848) | 40,711 (+40,397) | 51,686 (+51,339) |
| Manifest checked | 835,565 (+226) | 835,804 (+498) | 836,154 (+848) | 875,703 (+40,397) | 886,678 (+51,339) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 390,946 (+107,005) | 367,057 (+83,134) | 367,099 (+83,176) | 397,094 (+113,171) | 497,093 (+213,170) |
| Blubber lazy | 288,676 (+288,347) | 2,512 (+1,706) | 8,487 (+7,681) | 45,110 (+44,304) | 489,197 (+488,391) |
| Blubber checked | 301,987 (+286,344) | 17,840 (+1,684) | 28,226 (+12,070) | 49,127 (+32,971) | 493,214 (+477,058) |
| Star Frame | 278,374 (+278,015) | 847 (+368) | 4,268 (+3,789) | 202,917 (+202,438) | 540,019 (+539,540) |
| Zeropod | 263,308 (+115,007) | 486,587 (+237,251) | 486,687 (+237,351) | 516,682 (+267,346) | 616,681 (+367,345) |
| Wincode | 233,443 (+107,014) | 394,668 (+130,191) | 394,710 (+130,233) | 424,705 (+160,228) | 524,704 (+260,227) |
| PinaPod | 253,325 (+109,011) | 689,005 (+544,697) | 689,154 (+544,846) | 786,919 (+642,611) | 883,006 (+738,698) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 516,918 (+550) | 670,748 (+154,391) | heap exhausted |
| Blubber lazy | 1,258 (+928) | 24,875 (+24,072) | 283,164 (+282,361) |
| Blubber checked | 39,638 (+927) | 64,059 (+24,838) | 287,170 (+247,949) |
| Star Frame | 843 (+486) | 8,003 (+7,522) | 1,287,238 (+1,286,757) |
| Zeropod | 161,351 (+10,048) | 733,283 (+307,731) | 878,116 (+452,564) |
| Wincode | 783,389 (+491) | 1,181,955 (+182,659) | 1,328,841 (+329,545) |

### SBF v3 load policies

| Workload | Operation | n | Checked | Lazy |
|---|---|---|---|---|
| W1 | read_all | 0 | 846 (+18) | 557 (+18) |
| W1 | read_all | 1 | 861 (+33) | 572 (+33) |
| W1 | read_all | 10 | 951 (+123) | 662 (+123) |
| W1 | read_all | 100 | 1,851 (+1,023) | 1,562 (+1,023) |
| W1 | read_all | 1000 | 10,851 (+10,023) | 10,562 (+10,023) |
| W1 | get_middle | 0 | 862 (+34) | 573 (+34) |
| W1 | get_middle | 1 | 870 (+42) | 582 (+43) |
| W1 | get_middle | 10 | 870 (+42) | 582 (+43) |
| W1 | get_middle | 100 | 870 (+42) | 582 (+43) |
| W1 | get_middle | 1000 | 870 (+42) | 582 (+43) |
| W1 | set_all | 0 | 1,258 (+203) | 922 (+202) |
| W1 | set_all | 1 | 1,267 (+212) | 931 (+211) |
| W1 | set_all | 10 | 1,331 (+276) | 995 (+275) |
| W1 | set_all | 100 | 1,961 (+906) | 1,625 (+905) |
| W1 | set_all | 1000 | 8,261 (+7,206) | 7,925 (+7,205) |
| W1 | push | 0 | 1,736 (+681) | 1,400 (+680) |
| W1 | push | 1 | 1,736 (+681) | 1,400 (+680) |
| W1 | push | 10 | 1,736 (+681) | 1,400 (+680) |
| W1 | push | 100 | 1,736 (+681) | 1,400 (+680) |
| W1 | push | 1000 | 1,736 (+681) | 1,400 (+680) |
| W1 | remove_front | 0 | 1,053 (-2) | 718 (-2) |
| W1 | remove_front | 1 | 1,460 (+405) | 1,123 (+403) |
| W1 | remove_front | 10 | 1,630 (+575) | 1,293 (+573) |
| W1 | remove_front | 100 | 1,630 (+575) | 1,293 (+573) |
| W1 | remove_front | 1000 | 2,463 (+1,408) | 2,126 (+1,406) |
| W2 | lookup | 0 | 1,030 (+23) | 633 (+22) |
| W2 | lookup | 1 | 1,403 (+319) | 970 (+319) |
| W2 | lookup | 10 | 1,608 (+319) | 970 (+319) |
| W2 | lookup | 100 | 3,498 (+319) | 970 (+319) |
| W2 | lookup | 1000 | 22,398 (+319) | 970 (+319) |
| W2 | insert_middle | 0 | 2,120 (+955) | 1,675 (+955) |
| W2 | insert_middle | 1 | 2,467 (+1,261) | 1,981 (+1,261) |
| W2 | insert_middle | 10 | 3,131 (+1,720) | 2,440 (+1,720) |
| W2 | insert_middle | 100 | 5,398 (+2,097) | 2,817 (+2,097) |
| W2 | insert_middle | 1000 | 26,953 (+4,752) | 5,472 (+4,752) |
| W2 | remove | 0 | 1,773 (+608) | 1,328 (+608) |
| W2 | remove | 1 | 2,302 (+1,096) | 1,816 (+1,096) |
| W2 | remove | 10 | 2,821 (+1,410) | 2,130 (+1,410) |
| W2 | remove | 100 | 4,711 (+1,410) | 2,130 (+1,410) |
| W2 | remove | 1000 | 24,217 (+2,016) | 2,736 (+2,016) |
| W2 | set_all | 0 | 1,676 (+511) | 1,231 (+511) |
| W2 | set_all | 1 | 1,836 (+630) | 1,350 (+630) |
| W2 | set_all | 10 | 2,077 (+666) | 1,386 (+666) |
| W2 | set_all | 100 | 4,327 (+1,026) | 1,746 (+1,026) |
| W2 | set_all | 1000 | 26,827 (+4,626) | 5,346 (+4,626) |
| W2 | iterate_sum | 0 | 1,030 (+23) | 634 (+23) |
| W2 | iterate_sum | 1 | 1,143 (+59) | 706 (+55) |
| W2 | iterate_sum | 10 | 1,510 (+221) | 850 (+199) |
| W2 | iterate_sum | 100 | 5,020 (+1,841) | 2,290 (+1,639) |
| W2 | iterate_sum | 1000 | 40,120 (+18,041) | 16,690 (+16,039) |
| W3 | read_all | 0 | 1,108 (+259) | 595 (+269) |
| W3 | read_all | 1 | 1,595 (+606) | 937 (+611) |
| W3 | read_all | 10 | 4,420 (+3,204) | 3,553 (+3,227) |
| W3 | read_all | 100 | 31,301 (+28,944) | 29,473 (+29,147) |
| W3 | read_all | 1000 | 301,976 (+286,344) | 288,673 (+288,347) |
| W3 | set_equal | 0 | 1,363 (+1) | 804 (+1) |
| W3 | set_equal | 1 | 3,042 (+1,540) | 2,358 (+1,555) |
| W3 | set_equal | 10 | 3,410 (+1,681) | 2,500 (+1,697) |
| W3 | set_equal | 100 | 4,551 (+1,681) | 2,500 (+1,697) |
| W3 | set_equal | 1000 | 17,826 (+1,681) | 2,500 (+1,697) |
| W3 | grow_one | 0 | 1,363 (+1) | 804 (+1) |
| W3 | grow_one | 1 | 3,341 (+1,839) | 2,632 (+1,829) |
| W3 | grow_one | 10 | 3,995 (+2,266) | 3,126 (+2,323) |
| W3 | grow_one | 100 | 5,991 (+3,121) | 3,576 (+2,773) |
| W3 | grow_one | 1000 | 28,218 (+12,073) | 8,478 (+7,675) |
| W3 | grow_all | 0 | 1,946 (+584) | 1,624 (+821) |
| W3 | grow_all | 1 | 2,437 (+935) | 2,080 (+1,277) |
| W3 | grow_all | 10 | 3,051 (+1,322) | 2,566 (+1,763) |
| W3 | grow_all | 100 | 7,432 (+4,562) | 6,796 (+5,993) |
| W3 | grow_all | 1000 | 53,107 (+36,962) | 49,096 (+48,293) |
| W3 | grow_all_old | 0 | 2,624 (+1,262) | 2,421 (+1,618) |
| W3 | grow_all_old | 1 | 3,526 (+2,024) | 3,169 (+2,366) |
| W3 | grow_all_old | 10 | 8,106 (+6,377) | 7,621 (+6,818) |
| W3 | grow_all_old | 100 | 52,114 (+49,244) | 51,478 (+50,675) |
| W3 | grow_all_old | 1000 | 493,203 (+477,058) | 489,192 (+488,389) |
| W4 | lookup | 0 | 1,110 (+279) | 607 (+280) |
| W4 | lookup | 1 | 1,973 (+909) | 1,237 (+910) |
| W4 | lookup | 10 | 2,439 (+926) | 1,254 (+927) |
| W4 | lookup | 100 | 5,650 (+926) | 1,254 (+927) |
| W4 | lookup | 1000 | 39,625 (+926) | 1,254 (+927) |
| W4 | insert_middle | 0 | 4,152 (+2,811) | 3,396 (+2,596) |
| W4 | insert_middle | 1 | 5,154 (+3,580) | 4,144 (+3,344) |
| W4 | insert_middle | 10 | 6,476 (+4,453) | 5,001 (+4,201) |
| W4 | insert_middle | 100 | 11,594 (+6,360) | 6,863 (+6,063) |
| W4 | insert_middle | 1000 | 64,060 (+24,851) | 24,904 (+24,104) |
| W4 | grow_all_old | 0 | 2,120 (+779) | 1,671 (+871) |
| W4 | grow_all_old | 1 | 3,587 (+2,013) | 3,240 (+2,440) |
| W4 | grow_all_old | 10 | 6,304 (+4,281) | 5,829 (+5,029) |
| W4 | grow_all_old | 100 | 31,436 (+26,202) | 30,810 (+30,010) |
| W4 | grow_all_old | 1000 | 285,154 (+245,945) | 281,153 (+280,353) |

## SBF v3, n=0

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 324 (+5) | 325 (+6) | 397 (+89) | 565 (+257) | 399 (+91) |
| Blubber lazy | 557 (+18) | 573 (+34) | 922 (+202) | 1,400 (+680) | 718 (-2) |
| Blubber checked | 846 (+18) | 862 (+34) | 1,258 (+203) | 1,736 (+681) | 1,053 (-2) |
| Star Frame | 335 (+2) | 336 (+3) | 439 (+7) | 539 (+107) | 433 (+1) |
| Bytemuck | 290 (+13) | 280 (+3) | 279 (+0) | 288 (+9) | 279 (+0) |
| Zeropod | 301 (+4) | 301 (+4) | 476 (+147) | 667 (+338) | 478 (+149) |
| Wincode | 327 (+8) | 321 (+2) | 281 (+1) | 696 (+283) | 568 (+155) |
| PinaPod | 300 (+0) | 301 (+1) | 512 (+213) | 712 (+413) | 525 (+226) |
| Light | 288 (+5) | 286 (+3) | 288 (+3) | 300 (+15) | 327 (+42) |
| Anchor slab | 274 (+10) | 273 (+9) | 288 (+14) | 356 (+82) | 281 (+7) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 350 (+11) | 661 (+338) | 423 (+100) | 420 (+97) | 347 (+8) |
| Blubber lazy | 633 (+22) | 1,675 (+955) | 1,328 (+608) | 1,231 (+511) | 634 (+23) |
| Blubber checked | 1,030 (+23) | 2,120 (+955) | 1,773 (+608) | 1,676 (+511) | 1,030 (+23) |
| Star Frame | 364 (+26) | 652 (+217) | 486 (+51) | 440 (+5) | 339 (+1) |
| Sokoban | 278 (+10) | 359 (+89) | 278 (+8) | 337 (+67) | 336 (+68) |
| Zeropod | 313 (+15) | 759 (+435) | 483 (+159) | 480 (+156) | 305 (+7) |
| Wincode | 397 (+35) | 820 (+376) | 648 (+204) | 340 (+1) | 368 (+6) |
| Critbit | 278 (+12) | 399 (+131) | 278 (+10) | 334 (+66) | 330 (+64) |
| Manifest | 372 (+34) | 545 (+240) | 397 (+92) | 355 (+50) | 348 (+10) |
| Manifest checked | 374 (+34) | 547 (+240) | 399 (+92) | 357 (+50) | 350 (+10) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 324 (+3) | 389 (+86) | 387 (+84) | 388 (+85) | 389 (+86) |
| Blubber lazy | 595 (+269) | 804 (+1) | 804 (+1) | 1,624 (+821) | 2,421 (+1,618) |
| Blubber checked | 1,108 (+259) | 1,363 (+1) | 1,363 (+1) | 1,946 (+584) | 2,624 (+1,262) |
| Star Frame | 363 (+4) | 482 (+3) | 482 (+3) | 665 (+186) | 751 (+272) |
| Zeropod | 302 (+4) | 498 (+193) | 496 (+191) | 497 (+192) | 498 (+193) |
| Wincode | 393 (+10) | 583 (+138) | 581 (+136) | 582 (+137) | 583 (+138) |
| PinaPod | 309 (+3) | 762 (+462) | 762 (+462) | 756 (+456) | 764 (+464) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 344 (+11) | 743 (+421) | 412 (+90) |
| Blubber lazy | 607 (+280) | 3,396 (+2,596) | 1,671 (+871) |
| Blubber checked | 1,110 (+279) | 4,152 (+2,811) | 2,120 (+779) |
| Star Frame | 361 (+4) | 1,273 (+792) | 823 (+342) |
| Zeropod | 306 (+6) | 906 (+571) | 531 (+196) |
| Wincode | 383 (+10) | 891 (+505) | 524 (+138) |

## SBF v3, n=1

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 399 (+17) | 392 (+10) | 504 (+133) | 685 (+314) | 501 (+130) |
| Blubber lazy | 572 (+33) | 582 (+43) | 931 (+211) | 1,400 (+680) | 1,123 (+403) |
| Blubber checked | 861 (+33) | 870 (+42) | 1,267 (+212) | 1,736 (+681) | 1,460 (+405) |
| Star Frame | 358 (+25) | 343 (+10) | 446 (+14) | 539 (+107) | 546 (+114) |
| Bytemuck | 304 (+27) | 285 (+8) | 285 (+6) | 288 (+9) | 304 (+25) |
| Zeropod | 316 (+19) | 305 (+8) | 511 (+153) | 720 (+362) | 588 (+230) |
| Wincode | 341 (+22) | 324 (+5) | 285 (+5) | 748 (+307) | 635 (+194) |
| PinaPod | 312 (+12) | 306 (+6) | 582 (+283) | 763 (+464) | 632 (+333) |
| Light | 306 (+23) | 296 (+13) | 294 (+9) | 300 (+15) | 330 (+45) |
| Anchor slab | 287 (+23) | 278 (+14) | 297 (+23) | 356 (+82) | 310 (+36) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 697 (+58) | 1,139 (+516) | 890 (+267) | 852 (+229) | 681 (+42) |
| Blubber lazy | 970 (+319) | 1,981 (+1,261) | 1,816 (+1,096) | 1,350 (+630) | 706 (+55) |
| Blubber checked | 1,403 (+319) | 2,467 (+1,261) | 2,302 (+1,096) | 1,836 (+630) | 1,143 (+59) |
| Star Frame | 537 (+199) | 784 (+349) | 740 (+305) | 448 (+13) | 362 (+24) |
| Sokoban | 324 (+56) | 444 (+174) | 368 (+98) | 503 (+233) | 510 (+242) |
| Zeropod | 425 (+127) | 1,300 (+586) | 1,084 (+370) | 982 (+268) | 320 (+22) |
| Wincode | 425 (+63) | 1,046 (+488) | 930 (+372) | 347 (+8) | 383 (+21) |
| Critbit | 329 (+63) | 614 (+346) | 345 (+77) | 448 (+180) | 452 (+186) |
| Manifest | 422 (+76) | 640 (+327) | 540 (+227) | 395 (+82) | 381 (+35) |
| Manifest checked | 1,278 (+76) | 1,496 (+327) | 1,396 (+227) | 1,251 (+82) | 1,237 (+35) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 765 (+112) | 852 (+217) | 894 (+259) | 893 (+258) | 992 (+357) |
| Blubber lazy | 937 (+611) | 2,358 (+1,555) | 2,632 (+1,829) | 2,080 (+1,277) | 3,169 (+2,366) |
| Blubber checked | 1,595 (+606) | 3,042 (+1,540) | 3,341 (+1,839) | 2,437 (+935) | 3,526 (+2,024) |
| Star Frame | 664 (+305) | 847 (+368) | 1,114 (+635) | 1,039 (+560) | 1,490 (+1,011) |
| Zeropod | 558 (+122) | 1,041 (+476) | 1,141 (+576) | 1,140 (+575) | 1,239 (+674) |
| Wincode | 675 (+121) | 1,061 (+321) | 1,103 (+363) | 1,102 (+362) | 1,201 (+461) |
| PinaPod | 583 (+120) | 1,621 (+1,164) | 1,770 (+1,313) | 1,756 (+1,299) | 1,875 (+1,418) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 1,179 (+150) | 1,661 (+643) | 1,459 (+441) |
| Blubber lazy | 1,237 (+910) | 4,144 (+3,344) | 3,240 (+2,440) |
| Blubber checked | 1,973 (+909) | 5,154 (+3,580) | 3,587 (+2,013) |
| Star Frame | 678 (+321) | 1,451 (+970) | 1,689 (+1,208) |
| Zeropod | 638 (+197) | 1,912 (+941) | 1,735 (+764) |
| Wincode | 749 (+158) | 1,586 (+755) | 1,357 (+526) |

## SBF v3, n=10

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 642 (+107) | 545 (+10) | 990 (+466) | 1,134 (+610) | 956 (+432) |
| Blubber lazy | 662 (+123) | 582 (+43) | 995 (+275) | 1,400 (+680) | 1,293 (+573) |
| Blubber checked | 951 (+123) | 870 (+42) | 1,331 (+276) | 1,736 (+681) | 1,630 (+575) |
| Star Frame | 511 (+178) | 343 (+10) | 482 (+50) | 539 (+107) | 564 (+132) |
| Bytemuck | 394 (+117) | 285 (+8) | 321 (+42) | 288 (+9) | 304 (+25) |
| Zeropod | 415 (+118) | 305 (+8) | 547 (+189) | 719 (+361) | 588 (+230) |
| Wincode | 431 (+112) | 324 (+5) | 321 (+41) | 747 (+306) | 635 (+194) |
| PinaPod | 402 (+102) | 306 (+6) | 616 (+317) | 762 (+463) | 673 (+374) |
| Light | 396 (+113) | 296 (+13) | 330 (+45) | 300 (+15) | 479 (+194) |
| Anchor slab | 377 (+113) | 278 (+14) | 333 (+59) | 356 (+82) | 329 (+55) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 2,472 (+158) | 3,824 (+1,526) | 3,570 (+1,272) | 3,616 (+1,318) | 2,617 (+303) |
| Blubber lazy | 970 (+319) | 2,440 (+1,720) | 2,130 (+1,410) | 1,386 (+666) | 850 (+199) |
| Blubber checked | 1,608 (+319) | 3,131 (+1,720) | 2,821 (+1,410) | 2,077 (+666) | 1,510 (+221) |
| Star Frame | 537 (+199) | 955 (+520) | 758 (+323) | 502 (+67) | 515 (+177) |
| Sokoban | 353 (+85) | 548 (+278) | 730 (+460) | 1,280 (+1,010) | 1,350 (+1,082) |
| Zeropod | 425 (+127) | 4,235 (+1,344) | 4,020 (+1,129) | 3,996 (+1,105) | 419 (+121) |
| Wincode | 517 (+155) | 3,361 (+904) | 3,239 (+782) | 383 (+44) | 473 (+111) |
| Critbit | 707 (+441) | 981 (+713) | 708 (+440) | 1,473 (+1,205) | 1,540 (+1,274) |
| Manifest | 440 (+94) | 679 (+366) | 701 (+388) | 723 (+410) | 808 (+462) |
| Manifest checked | 7,958 (+94) | 8,197 (+366) | 8,219 (+388) | 8,241 (+410) | 8,326 (+462) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 4,275 (+1,075) | 4,146 (+964) | 4,188 (+1,006) | 4,457 (+1,275) | 5,456 (+2,274) |
| Blubber lazy | 3,553 (+3,227) | 2,500 (+1,697) | 3,126 (+2,323) | 2,566 (+1,763) | 7,621 (+6,818) |
| Blubber checked | 4,420 (+3,204) | 3,410 (+1,681) | 3,995 (+2,266) | 3,051 (+1,322) | 8,106 (+6,377) |
| Star Frame | 3,154 (+2,795) | 847 (+368) | 1,212 (+733) | 2,857 (+2,378) | 6,329 (+5,850) |
| Zeropod | 2,937 (+1,157) | 5,446 (+2,621) | 5,546 (+2,721) | 5,815 (+2,990) | 6,814 (+3,989) |
| Wincode | 2,772 (+1,084) | 4,607 (+1,491) | 4,649 (+1,533) | 4,918 (+1,802) | 5,917 (+2,801) |
| PinaPod | 2,854 (+1,101) | 7,807 (+6,060) | 7,956 (+6,209) | 8,881 (+7,134) | 9,866 (+8,119) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 5,895 (+250) | 7,737 (+2,103) | 8,730 (+3,096) |
| Blubber lazy | 1,254 (+927) | 5,001 (+4,201) | 5,829 (+5,029) |
| Blubber checked | 2,439 (+926) | 6,476 (+4,453) | 6,304 (+4,281) |
| Star Frame | 759 (+402) | 1,679 (+1,198) | 9,057 (+8,576) |
| Zeropod | 2,119 (+307) | 8,570 (+3,781) | 9,588 (+4,799) |
| Wincode | 3,794 (+278) | 8,077 (+2,458) | 9,043 (+3,424) |

## SBF v3, n=100

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 3,072 (+1,007) | 2,075 (+10) | 5,850 (+3,796) | 5,634 (+3,580) | 5,456 (+3,402) |
| Blubber lazy | 1,562 (+1,023) | 582 (+43) | 1,625 (+905) | 1,400 (+680) | 1,293 (+573) |
| Blubber checked | 1,851 (+1,023) | 870 (+42) | 1,961 (+906) | 1,736 (+681) | 1,630 (+575) |
| Star Frame | 2,041 (+1,708) | 343 (+10) | 842 (+410) | 539 (+107) | 564 (+132) |
| Bytemuck | 1,294 (+1,017) | 285 (+8) | 681 (+402) | 288 (+9) | 304 (+25) |
| Zeropod | 1,405 (+1,108) | 305 (+8) | 907 (+549) | 719 (+361) | 588 (+230) |
| Wincode | 1,331 (+1,012) | 324 (+5) | 681 (+401) | 747 (+306) | 635 (+194) |
| PinaPod | 1,302 (+1,002) | 306 (+6) | 976 (+677) | 762 (+463) | 673 (+374) |
| Light | 1,296 (+1,013) | 296 (+13) | 690 (+405) | 300 (+15) | 1,559 (+1,274) |
| Anchor slab | 1,277 (+1,013) | 278 (+14) | 693 (+419) | 356 (+82) | 329 (+55) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 19,814 (+218) | 30,754 (+11,174) | 30,132 (+10,552) | 32,141 (+12,561) | 22,632 (+3,036) |
| Blubber lazy | 970 (+319) | 2,817 (+2,097) | 2,130 (+1,410) | 1,746 (+1,026) | 2,290 (+1,639) |
| Blubber checked | 3,498 (+319) | 5,398 (+2,097) | 4,711 (+1,410) | 4,327 (+1,026) | 5,020 (+1,841) |
| Star Frame | 537 (+199) | 1,210 (+775) | 758 (+323) | 1,042 (+607) | 2,045 (+1,707) |
| Sokoban | 495 (+227) | 670 (+400) | 1,197 (+927) | 9,049 (+8,779) | 9,749 (+9,481) |
| Zeropod | 425 (+127) | 33,706 (+8,583) | 33,111 (+7,988) | 35,050 (+9,927) | 1,409 (+1,111) |
| Wincode | 574 (+212) | 48,461 (+4,170) | 48,337 (+4,046) | 743 (+404) | 1,373 (+1,011) |
| Critbit | 987 (+721) | 1,256 (+988) | 885 (+617) | 10,758 (+10,490) | 11,455 (+11,189) |
| Manifest | 518 (+172) | 754 (+441) | 921 (+608) | 4,314 (+4,001) | 5,389 (+5,043) |
| Manifest checked | 70,259 (+172) | 70,495 (+441) | 70,662 (+608) | 74,055 (+4,001) | 75,130 (+5,043) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 39,375 (+10,705) | 37,086 (+8,434) | 37,128 (+8,476) | 40,097 (+11,445) | 50,096 (+21,444) |
| Blubber lazy | 29,473 (+29,147) | 2,500 (+1,697) | 3,576 (+2,773) | 6,796 (+5,993) | 51,478 (+50,675) |
| Blubber checked | 31,301 (+28,944) | 4,551 (+1,681) | 5,991 (+3,121) | 7,432 (+4,562) | 52,114 (+49,244) |
| Star Frame | 28,174 (+27,815) | 847 (+368) | 1,518 (+1,039) | 21,037 (+20,558) | 54,839 (+54,360) |
| Zeropod | 26,607 (+11,507) | 49,186 (+23,951) | 49,286 (+24,051) | 52,255 (+27,020) | 62,254 (+37,019) |
| Wincode | 23,742 (+10,714) | 40,067 (+13,191) | 40,109 (+13,233) | 43,078 (+16,202) | 53,077 (+26,201) |
| PinaPod | 25,624 (+10,911) | 69,679 (+54,972) | 69,828 (+55,121) | 79,455 (+64,748) | 89,080 (+74,373) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 52,911 (+310) | 68,913 (+16,323) | 82,613 (+30,023) |
| Blubber lazy | 1,254 (+927) | 6,863 (+6,063) | 30,810 (+30,010) |
| Blubber checked | 5,650 (+926) | 11,594 (+6,360) | 31,436 (+26,202) |
| Star Frame | 789 (+432) | 2,354 (+1,873) | 106,465 (+105,984) |
| Zeropod | 16,699 (+1,297) | 75,275 (+31,801) | 88,987 (+45,513) |
| Wincode | 55,880 (+276) | 95,770 (+18,807) | 110,019 (+33,056) |

## SBF v3, n=1000

Cells are whole CU (signed operation delta from the matching load-only idle). "Full" is a failed capacity-limited operation.

### W1

| Library | read_all | get_middle | set_all | push | remove_front |
|---|---|---|---|---|---|
| Borsh | 27,480 (+10,007) | 17,483 (+10) | 54,558 (+37,096) | 50,641 (+33,179) | 50,585 (+33,123) |
| Blubber lazy | 10,562 (+10,023) | 582 (+43) | 7,925 (+7,205) | 1,400 (+680) | 2,126 (+1,406) |
| Blubber checked | 10,851 (+10,023) | 870 (+42) | 8,261 (+7,206) | 1,736 (+681) | 2,463 (+1,408) |
| Star Frame | 17,341 (+17,008) | 343 (+10) | 4,442 (+4,010) | 539 (+107) | 585 (+153) |
| Bytemuck | 10,294 (+10,017) | 285 (+8) | 4,281 (+4,002) | full | 325 (+46) |
| Zeropod | 11,305 (+11,008) | 305 (+8) | 4,551 (+4,171) | 785 (+405) | 652 (+272) |
| Wincode | 10,331 (+10,012) | 324 (+5) | 4,281 (+4,001) | 813 (+350) | 699 (+236) |
| PinaPod | 10,302 (+10,002) | 306 (+6) | 4,598 (+4,299) | 828 (+529) | 737 (+438) |
| Light | 10,296 (+10,013) | 296 (+13) | 4,290 (+4,005) | full | 12,380 (+12,095) |
| Anchor slab | 10,277 (+10,013) | 278 (+14) | 4,293 (+4,019) | 356 (+82) | 350 (+76) |

### W2

| Library | lookup | insert_middle | remove | set_all | iterate_sum |
|---|---|---|---|---|---|
| Borsh | 188,940 (+458) | 292,445 (+103,979) | 291,440 (+102,974) | 313,377 (+124,911) | 218,818 (+30,336) |
| Blubber lazy | 970 (+319) | 5,472 (+4,752) | 2,736 (+2,016) | 5,346 (+4,626) | 16,690 (+16,039) |
| Blubber checked | 22,398 (+319) | 26,953 (+4,752) | 24,217 (+2,016) | 26,827 (+4,626) | 40,120 (+18,041) |
| Star Frame | 537 (+199) | 1,535 (+1,100) | 827 (+392) | 6,442 (+6,007) | 17,345 (+17,007) |
| Sokoban | 582 (+314) | full | 1,524 (+1,254) | 85,668 (+85,398) | 92,668 (+92,400) |
| Zeropod | 425 (+127) | 319,828 (+77,175) | 318,838 (+76,185) | 340,705 (+98,052) | 11,309 (+11,011) |
| Wincode | 631 (+269) | 708,400 (+37,144) | 708,263 (+37,007) | 4,343 (+4,004) | 10,373 (+10,011) |
| Critbit | 1,266 (+1,000) | full | 1,062 (+794) | 102,663 (+102,395) | 109,660 (+109,394) |
| Manifest | 572 (+226) | 811 (+498) | 1,161 (+848) | 40,710 (+40,397) | 51,685 (+51,339) |
| Manifest checked | 835,564 (+226) | 835,803 (+498) | 836,153 (+848) | 875,702 (+40,397) | 886,677 (+51,339) |

### W3

| Library | read_all | set_equal | grow_one | grow_all | grow_all_old |
|---|---|---|---|---|---|
| Borsh | 390,945 (+107,005) | 367,056 (+83,134) | 367,098 (+83,176) | 397,093 (+113,171) | 497,092 (+213,170) |
| Blubber lazy | 288,673 (+288,347) | 2,500 (+1,697) | 8,478 (+7,675) | 49,096 (+48,293) | 489,192 (+488,389) |
| Blubber checked | 301,976 (+286,344) | 17,826 (+1,681) | 28,218 (+12,073) | 53,107 (+36,962) | 493,203 (+477,058) |
| Star Frame | 278,374 (+278,015) | 847 (+368) | 4,268 (+3,789) | 202,917 (+202,438) | 540,019 (+539,540) |
| Zeropod | 263,307 (+115,007) | 486,586 (+237,251) | 486,686 (+237,351) | 516,681 (+267,346) | 616,680 (+367,345) |
| Wincode | 233,442 (+107,014) | 394,667 (+130,191) | 394,709 (+130,233) | 424,704 (+160,228) | 524,703 (+260,227) |
| PinaPod | 253,324 (+109,011) | 689,003 (+544,696) | 689,152 (+544,845) | 786,917 (+642,610) | 883,004 (+738,697) |

### W4

| Library | lookup | insert_middle | grow_all_old |
|---|---|---|---|
| Borsh | 516,914 (+550) | 670,744 (+154,391) | heap exhausted |
| Blubber lazy | 1,254 (+927) | 24,904 (+24,104) | 281,153 (+280,353) |
| Blubber checked | 39,625 (+926) | 64,060 (+24,851) | 285,154 (+245,945) |
| Star Frame | 843 (+486) | 8,003 (+7,522) | 1,287,238 (+1,286,757) |
| Zeropod | 161,350 (+10,048) | 733,279 (+307,731) | 878,112 (+452,564) |
| Wincode | 783,388 (+491) | 1,181,954 (+182,659) | 1,328,840 (+329,545) |

## Blubber fixed-list alternatives

The main W1 set-all row uses the callback setter. Fill remains a separate measured operation with the same logical output; the table records which method is cheaper.

| Arch | n | Callback whole CU | Fill whole CU | Fill / callback |
|---|---|---|---|---|
| v0 | 100 | 1,628 | 1,247 | 0.77x |
| v0 | 1000 | 7,928 | 4,847 | 0.61x |
| v3 | 100 | 1,625 | 1,244 | 0.77x |
| v3 | 1000 | 7,925 | 4,844 | 0.61x |

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
| v0 | list_fixed_n1000_ref_iter_plain | 10,798 | 10,798 | 0.000% |
| v0 | list_fixed_n1000_callback_equal_set_plain | 8,132 | 8,132 | 0.000% |
| v0 | map_fixed_n1000_ref_iter_plain | 91,135 | 91,135 | 0.000% |
| v0 | map_fixed_n1000_callback_equal_set_plain | 65,959 | 65,959 | 0.000% |
| v0 | list_indexed_n1000_fill_plain | 48,972 | 48,972 | 0.000% |
| v0 | map_indexed_n1000_fill_plain | 74,113 | 74,113 | 0.000% |
| v0 | map_indexed_n1000_map_grow_from_old_values_plain | 482,708 | 482,708 | 0.000% |
| v3 | list_fixed_n1000_ref_iter_plain | 10,794 | 10,794 | 0.000% |
| v3 | list_fixed_n1000_callback_equal_set_plain | 8,127 | 8,127 | 0.000% |
| v3 | map_fixed_n1000_ref_iter_plain | 91,131 | 91,131 | 0.000% |
| v3 | map_fixed_n1000_callback_equal_set_plain | 65,954 | 65,954 | 0.000% |
| v3 | list_indexed_n1000_fill_plain | 52,947 | 52,947 | 0.000% |
| v3 | map_indexed_n1000_fill_plain | 74,096 | 74,096 | 0.000% |
| v3 | map_indexed_n1000_map_grow_from_old_values_plain | 482,689 | 482,689 | 0.000% |

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
