# Comparison gaps

Several single-item lookups, inserts and removals cost more with Blubber than with the fastest other library. At 1,000<!-- num:compare.gaps.large_entries --> items on SBF v0, lazy Blubber leads on 3<!-- num:cmp.v0.n1000.summary.other_wins --> of 18<!-- num:cmp.v0.n1000.summary.operations --> operations and trails on 15<!-- num:cmp.v0.n1000.summary.other_losses -->. Which library is fastest depends on the operation and the run setup. I plan to make single lookups cheaper; see [`README.md#roadmap`](../../README.md#roadmap).

All rows report whole CU, the total runtime work for the instruction. The comparison checks lazy and checked Blubber at 100<!-- num:compare.gaps.small_entries --> and 1,000<!-- num:compare.gaps.large_entries --> entries against successful non-Blubber runs with the same architecture, workload, size and operation. It lists only rows strictly above 1.5<!-- num:compare.gaps.threshold -->x the fastest successful peer in each comparison group. Raw rows are in [`results/all.csv`](results/all.csv), the run receipt is in [`results/receipt.json`](results/receipt.json), and the package inventory is in [`../COMPARE.md#source-inventory`](../COMPARE.md#source-inventory).

## Load policy

A lazy load checks the account header up front and checks fields as you read them. A checked load scans the whole used encoding before returning a view or calling your edit callback. The comparison assigns each library a lazy, validated or fixed class per operation. It makes exceptions for some `u64` list operations because `u64` has no invalid bit patterns, and for full string-list reads that already check every boundary and string. See [`../COMPARE.md#load-policy-and-scope`](../COMPARE.md#load-policy-and-scope), [`SPEC.md#loads-and-writer-history`](../../SPEC.md#loads-and-writer-history) and [`ASSURANCE.md#trust-boundaries`](../../ASSURANCE.md#trust-boundaries).

## Gaps within the same load policy

A sorted map stores keys in one canonical order so lookups can search in place. Inserting one fixed value into a map of 1,000<!-- num:compare.gaps.large_entries --> items on v0 takes 5,435<!-- num:cmp.v0.n1000.map_insert.lazy --> CU with lazy Blubber and 812<!-- num:cmp.v0.n1000.map_insert.manifest --> CU with Manifest (6.7<!-- num:cmp.v0.n1000.derived.map_insert_manifest_ratio -->x). Inserting one string takes 24,875<!-- num:cmp.v0.n1000.map_insert_string.lazy --> CU with lazy Blubber and 8,003<!-- num:cmp.v0.n1000.map_insert_string.star_frame --> CU with Star Frame, which I helped build.

<!-- gaps-same-class:start -->

| Blubber row | Arch | Operation | n | Blubber CU | Best same-class CU | Ratio |
|---|---|---|---|---|---|---|
| blubber_trusted | v0 | W1 get_middle | 100 | 585 | anchor_slab 278 | 2.10x |
| blubber_trusted | v0 | W1 set_all | 100 | 1,628 | wincode 682 | 2.39x |
| blubber_trusted | v0 | W1 push | 100 | 1,403 | anchor_slab 356 | 3.94x |
| blubber_trusted | v0 | W1 remove_front | 100 | 1,297 | anchor_slab 329 | 3.94x |
| blubber_trusted | v0 | W1 get_middle | 1000 | 585 | anchor_slab 278 | 2.10x |
| blubber_trusted | v0 | W1 set_all | 1000 | 7,928 | wincode 4,282 | 1.85x |
| blubber_trusted | v0 | W1 push | 1000 | 1,403 | anchor_slab 356 | 3.94x |
| blubber_trusted | v0 | W1 remove_front | 1000 | 2,130 | anchor_slab 350 | 6.09x |
| blubber_trusted | v0 | W2 lookup | 100 | 973 | zeropod 426 | 2.28x |
| blubber_trusted | v0 | W2 insert_middle | 100 | 2,816 | manifest 755 | 3.73x |
| blubber_trusted | v0 | W2 remove | 100 | 2,128 | Star Frame (maintainer co-builder) 758 | 2.81x |
| blubber_trusted | v0 | W2 set_all | 100 | 1,749 | wincode 744 | 2.35x |
| blubber_trusted | v0 | W2 iterate_sum | 100 | 2,293 | wincode 1,374 | 1.67x |
| blubber_trusted | v0 | W2 lookup | 1000 | 973 | zeropod 426 | 2.28x |
| blubber_trusted | v0 | W2 insert_middle | 1000 | 5,435 | manifest 812 | 6.69x |
| blubber_trusted | v0 | W2 remove | 1000 | 2,734 | Star Frame (maintainer co-builder) 827 | 3.31x |
| blubber_trusted | v0 | W2 iterate_sum | 1000 | 16,693 | wincode 10,374 | 1.61x |
| blubber_trusted | v0 | W3 set_equal | 100 | 2,512 | Star Frame (maintainer co-builder) 847 | 2.97x |
| blubber_trusted | v0 | W3 grow_one | 100 | 3,591 | Star Frame (maintainer co-builder) 1,518 | 2.37x |
| blubber_trusted | v0 | W3 set_equal | 1000 | 2,512 | Star Frame (maintainer co-builder) 847 | 2.97x |
| blubber_trusted | v0 | W3 grow_one | 1000 | 8,487 | Star Frame (maintainer co-builder) 4,268 | 1.99x |
| blubber_trusted | v0 | W4 lookup | 100 | 1,258 | Star Frame (maintainer co-builder) 789 | 1.59x |
| blubber_trusted | v0 | W4 insert_middle | 100 | 6,878 | Star Frame (maintainer co-builder) 2,354 | 2.92x |
| blubber_trusted | v0 | W4 insert_middle | 1000 | 24,875 | Star Frame (maintainer co-builder) 8,003 | 3.11x |
| blubber_checked | v0 | W1 get_middle | 100 | 873 | anchor_slab 278 | 3.14x |
| blubber_checked | v0 | W1 set_all | 100 | 1,964 | wincode 682 | 2.88x |
| blubber_checked | v0 | W1 push | 100 | 1,739 | anchor_slab 356 | 4.88x |
| blubber_checked | v0 | W1 remove_front | 100 | 1,634 | anchor_slab 329 | 4.97x |
| blubber_checked | v0 | W1 get_middle | 1000 | 873 | anchor_slab 278 | 3.14x |
| blubber_checked | v0 | W1 set_all | 1000 | 8,264 | wincode 4,282 | 1.93x |
| blubber_checked | v0 | W1 push | 1000 | 1,739 | anchor_slab 356 | 4.88x |
| blubber_checked | v0 | W1 remove_front | 1000 | 2,467 | anchor_slab 350 | 7.05x |
| blubber_trusted | v3 | W1 get_middle | 100 | 582 | anchor_slab 278 | 2.09x |
| blubber_trusted | v3 | W1 set_all | 100 | 1,625 | wincode 681 | 2.39x |
| blubber_trusted | v3 | W1 push | 100 | 1,400 | anchor_slab 356 | 3.93x |
| blubber_trusted | v3 | W1 remove_front | 100 | 1,293 | anchor_slab 329 | 3.93x |
| blubber_trusted | v3 | W1 get_middle | 1000 | 582 | anchor_slab 278 | 2.09x |
| blubber_trusted | v3 | W1 set_all | 1000 | 7,925 | wincode 4,281 | 1.85x |
| blubber_trusted | v3 | W1 push | 1000 | 1,400 | anchor_slab 356 | 3.93x |
| blubber_trusted | v3 | W1 remove_front | 1000 | 2,126 | anchor_slab 350 | 6.07x |
| blubber_trusted | v3 | W2 lookup | 100 | 970 | zeropod 425 | 2.28x |
| blubber_trusted | v3 | W2 insert_middle | 100 | 2,817 | manifest 754 | 3.74x |
| blubber_trusted | v3 | W2 remove | 100 | 2,130 | Star Frame (maintainer co-builder) 758 | 2.81x |
| blubber_trusted | v3 | W2 set_all | 100 | 1,746 | wincode 743 | 2.35x |
| blubber_trusted | v3 | W2 iterate_sum | 100 | 2,290 | wincode 1,373 | 1.67x |
| blubber_trusted | v3 | W2 lookup | 1000 | 970 | zeropod 425 | 2.28x |
| blubber_trusted | v3 | W2 insert_middle | 1000 | 5,472 | manifest 811 | 6.75x |
| blubber_trusted | v3 | W2 remove | 1000 | 2,736 | Star Frame (maintainer co-builder) 827 | 3.31x |
| blubber_trusted | v3 | W2 iterate_sum | 1000 | 16,690 | wincode 10,373 | 1.61x |
| blubber_trusted | v3 | W3 set_equal | 100 | 2,500 | Star Frame (maintainer co-builder) 847 | 2.95x |
| blubber_trusted | v3 | W3 grow_one | 100 | 3,576 | Star Frame (maintainer co-builder) 1,518 | 2.36x |
| blubber_trusted | v3 | W3 set_equal | 1000 | 2,500 | Star Frame (maintainer co-builder) 847 | 2.95x |
| blubber_trusted | v3 | W3 grow_one | 1000 | 8,478 | Star Frame (maintainer co-builder) 4,268 | 1.99x |
| blubber_trusted | v3 | W4 lookup | 100 | 1,254 | Star Frame (maintainer co-builder) 789 | 1.59x |
| blubber_trusted | v3 | W4 insert_middle | 100 | 6,863 | Star Frame (maintainer co-builder) 2,354 | 2.92x |
| blubber_trusted | v3 | W4 insert_middle | 1000 | 24,904 | Star Frame (maintainer co-builder) 8,003 | 3.11x |
| blubber_checked | v3 | W1 get_middle | 100 | 870 | anchor_slab 278 | 3.13x |
| blubber_checked | v3 | W1 set_all | 100 | 1,961 | wincode 681 | 2.88x |
| blubber_checked | v3 | W1 push | 100 | 1,736 | anchor_slab 356 | 4.88x |
| blubber_checked | v3 | W1 remove_front | 100 | 1,630 | anchor_slab 329 | 4.95x |
| blubber_checked | v3 | W1 get_middle | 1000 | 870 | anchor_slab 278 | 3.13x |
| blubber_checked | v3 | W1 set_all | 1000 | 8,261 | wincode 4,281 | 1.93x |
| blubber_checked | v3 | W1 push | 1000 | 1,736 | anchor_slab 356 | 4.88x |
| blubber_checked | v3 | W1 remove_front | 1000 | 2,463 | anchor_slab 350 | 7.04x |

<!-- gaps-same-class:end -->

## Other load or storage policies

The second table lists faster peers outside the same class, such as fixed-capacity arrays or libraries with a different validation policy. They are context, not equivalent work.

<!-- gaps-other-policy:start -->

| Blubber row | Arch | Operation | n | Blubber CU | Best context CU | Ratio |
|---|---|---|---|---|---|---|
| blubber_trusted | v0 | W1 get_middle | 100 | 585 | bytemuck 286 | 2.05x |
| blubber_trusted | v0 | W1 set_all | 100 | 1,628 | bytemuck 682 | 2.39x |
| blubber_trusted | v0 | W1 push | 100 | 1,403 | bytemuck 289 | 4.85x |
| blubber_trusted | v0 | W1 remove_front | 100 | 1,297 | bytemuck 305 | 4.25x |
| blubber_trusted | v0 | W1 get_middle | 1000 | 585 | bytemuck 286 | 2.05x |
| blubber_trusted | v0 | W1 set_all | 1000 | 7,928 | bytemuck 4,282 | 1.85x |
| blubber_trusted | v0 | W1 remove_front | 1000 | 2,130 | bytemuck 326 | 6.53x |
| blubber_trusted | v0 | W2 lookup | 100 | 973 | sokoban 496 | 1.96x |
| blubber_trusted | v0 | W2 insert_middle | 100 | 2,816 | sokoban 671 | 4.20x |
| blubber_trusted | v0 | W2 remove | 100 | 2,128 | critbit 886 | 2.40x |
| blubber_trusted | v0 | W2 lookup | 1000 | 973 | sokoban 583 | 1.67x |
| blubber_trusted | v0 | W2 remove | 1000 | 2,734 | critbit 1,063 | 2.57x |
| blubber_checked | v0 | W1 get_middle | 100 | 873 | bytemuck 286 | 3.05x |
| blubber_checked | v0 | W1 set_all | 100 | 1,964 | bytemuck 682 | 2.88x |
| blubber_checked | v0 | W1 push | 100 | 1,739 | bytemuck 289 | 6.02x |
| blubber_checked | v0 | W1 remove_front | 100 | 1,634 | bytemuck 305 | 5.36x |
| blubber_checked | v0 | W1 get_middle | 1000 | 873 | bytemuck 286 | 3.05x |
| blubber_checked | v0 | W1 set_all | 1000 | 8,264 | bytemuck 4,282 | 1.93x |
| blubber_checked | v0 | W1 remove_front | 1000 | 2,467 | bytemuck 326 | 7.57x |
| blubber_checked | v0 | W2 lookup | 100 | 3,501 | zeropod 426 | 8.22x |
| blubber_checked | v0 | W2 insert_middle | 100 | 5,397 | sokoban 671 | 8.04x |
| blubber_checked | v0 | W2 remove | 100 | 4,709 | Star Frame (maintainer co-builder) 758 | 6.21x |
| blubber_checked | v0 | W2 set_all | 100 | 4,330 | wincode 744 | 5.82x |
| blubber_checked | v0 | W2 iterate_sum | 100 | 5,023 | wincode 1,374 | 3.66x |
| blubber_checked | v0 | W2 lookup | 1000 | 22,401 | zeropod 426 | 52.58x |
| blubber_checked | v0 | W2 insert_middle | 1000 | 26,916 | manifest 812 | 33.15x |
| blubber_checked | v0 | W2 remove | 1000 | 24,215 | Star Frame (maintainer co-builder) 827 | 29.28x |
| blubber_checked | v0 | W2 set_all | 1000 | 26,830 | wincode 4,344 | 6.18x |
| blubber_checked | v0 | W2 iterate_sum | 1000 | 40,123 | wincode 10,374 | 3.87x |
| blubber_checked | v0 | W3 set_equal | 100 | 4,565 | Star Frame (maintainer co-builder) 847 | 5.39x |
| blubber_checked | v0 | W3 grow_one | 100 | 6,005 | Star Frame (maintainer co-builder) 1,518 | 3.96x |
| blubber_checked | v0 | W3 set_equal | 1000 | 17,840 | Star Frame (maintainer co-builder) 847 | 21.06x |
| blubber_checked | v0 | W3 grow_one | 1000 | 28,226 | Star Frame (maintainer co-builder) 4,268 | 6.61x |
| blubber_checked | v0 | W4 lookup | 100 | 5,663 | Star Frame (maintainer co-builder) 789 | 7.18x |
| blubber_checked | v0 | W4 insert_middle | 100 | 11,637 | Star Frame (maintainer co-builder) 2,354 | 4.94x |
| blubber_checked | v0 | W4 lookup | 1000 | 39,638 | Star Frame (maintainer co-builder) 843 | 47.02x |
| blubber_checked | v0 | W4 insert_middle | 1000 | 64,059 | Star Frame (maintainer co-builder) 8,003 | 8.00x |
| blubber_trusted | v3 | W1 get_middle | 100 | 582 | bytemuck 285 | 2.04x |
| blubber_trusted | v3 | W1 set_all | 100 | 1,625 | bytemuck 681 | 2.39x |
| blubber_trusted | v3 | W1 push | 100 | 1,400 | bytemuck 288 | 4.86x |
| blubber_trusted | v3 | W1 remove_front | 100 | 1,293 | bytemuck 304 | 4.25x |
| blubber_trusted | v3 | W1 get_middle | 1000 | 582 | bytemuck 285 | 2.04x |
| blubber_trusted | v3 | W1 set_all | 1000 | 7,925 | bytemuck 4,281 | 1.85x |
| blubber_trusted | v3 | W1 remove_front | 1000 | 2,126 | bytemuck 325 | 6.54x |
| blubber_trusted | v3 | W2 lookup | 100 | 970 | sokoban 495 | 1.96x |
| blubber_trusted | v3 | W2 insert_middle | 100 | 2,817 | sokoban 670 | 4.20x |
| blubber_trusted | v3 | W2 remove | 100 | 2,130 | critbit 885 | 2.41x |
| blubber_trusted | v3 | W2 lookup | 1000 | 970 | sokoban 582 | 1.67x |
| blubber_trusted | v3 | W2 remove | 1000 | 2,736 | critbit 1,062 | 2.58x |
| blubber_checked | v3 | W1 get_middle | 100 | 870 | bytemuck 285 | 3.05x |
| blubber_checked | v3 | W1 set_all | 100 | 1,961 | bytemuck 681 | 2.88x |
| blubber_checked | v3 | W1 push | 100 | 1,736 | bytemuck 288 | 6.03x |
| blubber_checked | v3 | W1 remove_front | 100 | 1,630 | bytemuck 304 | 5.36x |
| blubber_checked | v3 | W1 get_middle | 1000 | 870 | bytemuck 285 | 3.05x |
| blubber_checked | v3 | W1 set_all | 1000 | 8,261 | bytemuck 4,281 | 1.93x |
| blubber_checked | v3 | W1 remove_front | 1000 | 2,463 | bytemuck 325 | 7.58x |
| blubber_checked | v3 | W2 lookup | 100 | 3,498 | zeropod 425 | 8.23x |
| blubber_checked | v3 | W2 insert_middle | 100 | 5,398 | sokoban 670 | 8.06x |
| blubber_checked | v3 | W2 remove | 100 | 4,711 | Star Frame (maintainer co-builder) 758 | 6.22x |
| blubber_checked | v3 | W2 set_all | 100 | 4,327 | wincode 743 | 5.82x |
| blubber_checked | v3 | W2 iterate_sum | 100 | 5,020 | wincode 1,373 | 3.66x |
| blubber_checked | v3 | W2 lookup | 1000 | 22,398 | zeropod 425 | 52.70x |
| blubber_checked | v3 | W2 insert_middle | 1000 | 26,953 | manifest 811 | 33.23x |
| blubber_checked | v3 | W2 remove | 1000 | 24,217 | Star Frame (maintainer co-builder) 827 | 29.28x |
| blubber_checked | v3 | W2 set_all | 1000 | 26,827 | wincode 4,343 | 6.18x |
| blubber_checked | v3 | W2 iterate_sum | 1000 | 40,120 | wincode 10,373 | 3.87x |
| blubber_checked | v3 | W3 set_equal | 100 | 4,551 | Star Frame (maintainer co-builder) 847 | 5.37x |
| blubber_checked | v3 | W3 grow_one | 100 | 5,991 | Star Frame (maintainer co-builder) 1,518 | 3.95x |
| blubber_checked | v3 | W3 set_equal | 1000 | 17,826 | Star Frame (maintainer co-builder) 847 | 21.05x |
| blubber_checked | v3 | W3 grow_one | 1000 | 28,218 | Star Frame (maintainer co-builder) 4,268 | 6.61x |
| blubber_checked | v3 | W4 lookup | 100 | 5,650 | Star Frame (maintainer co-builder) 789 | 7.16x |
| blubber_checked | v3 | W4 insert_middle | 100 | 11,594 | Star Frame (maintainer co-builder) 2,354 | 4.93x |
| blubber_checked | v3 | W4 lookup | 1000 | 39,625 | Star Frame (maintainer co-builder) 843 | 47.00x |
| blubber_checked | v3 | W4 insert_middle | 1000 | 64,060 | Star Frame (maintainer co-builder) 8,003 | 8.00x |

<!-- gaps-other-policy:end -->
