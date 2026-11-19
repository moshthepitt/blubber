# Comparison gaps

Derived from the retained [comparison receipt](results/receipt.json) and [observations](results/all.csv). Load policy is assigned from source review at the pinned commits. Whole instruction CU is the ranking metric. The tables list retained n=100/1000 rows above 1.5x.

## Load policy

`blubber_checked` verifies the complete active root on load. `blubber_trusted` loads authenticated program-written bytes lazily and checks touched paths. Both run the same operation bodies and byte/checksum oracles. The [specification](../../SPEC.md#loads-and-writer-history) defines the writer-history requirement. [Assurance](../../ASSURANCE.md) states verification limits.

Classification is per operation. A complete string scan validates visited boundaries and UTF-8 even from a lazy load. Full decoding need not reject noncanonical key order. Fixed-capacity storage has a different space policy. The [source inventory](../COMPARE.md#source-inventory) records each measured loader and storage contract. These costs do not establish identical invalid-input behaviour.

## Gaps within the same load policy

| Blubber row | Arch | Operation | n | Blubber CU | Best same-class CU | Ratio |
|---|---|---|---|---|---|---|
| blubber_trusted | v0 | W1 get_middle | 100 | 586 | anchor_slab 298 | 1.97x |
| blubber_trusted | v0 | W1 set_all | 100 | 1,624 | wincode 682 | 2.38x |
| blubber_trusted | v0 | W1 push | 100 | 1,446 | anchor_slab 389 | 3.72x |
| blubber_trusted | v0 | W1 remove_front | 100 | 1,340 | anchor_slab 335 | 4.00x |
| blubber_trusted | v0 | W1 get_middle | 1000 | 586 | anchor_slab 298 | 1.97x |
| blubber_trusted | v0 | W1 set_all | 1000 | 7,924 | wincode 4,282 | 1.85x |
| blubber_trusted | v0 | W1 push | 1000 | 1,446 | anchor_slab 389 | 3.72x |
| blubber_trusted | v0 | W1 remove_front | 1000 | 2,173 | anchor_slab 356 | 6.10x |
| blubber_trusted | v0 | W2 lookup | 100 | 974 | zeropod 426 | 2.29x |
| blubber_trusted | v0 | W2 insert_middle | 100 | 2,868 | manifest 755 | 3.80x |
| blubber_trusted | v0 | W2 remove | 100 | 2,171 | star_frame 758 | 2.86x |
| blubber_trusted | v0 | W2 set_all | 100 | 1,788 | wincode 744 | 2.40x |
| blubber_trusted | v0 | W2 iterate_sum | 100 | 2,294 | wincode 1,374 | 1.67x |
| blubber_trusted | v0 | W2 lookup | 1000 | 974 | zeropod 426 | 2.29x |
| blubber_trusted | v0 | W2 insert_middle | 1000 | 5,487 | manifest 812 | 6.76x |
| blubber_trusted | v0 | W2 remove | 1000 | 2,777 | star_frame 827 | 3.36x |
| blubber_trusted | v0 | W2 iterate_sum | 1000 | 16,694 | wincode 10,374 | 1.61x |
| blubber_trusted | v0 | W3 set_equal | 100 | 2,506 | star_frame 847 | 2.96x |
| blubber_trusted | v0 | W3 grow_one | 100 | 3,637 | star_frame 1,518 | 2.40x |
| blubber_trusted | v0 | W3 set_equal | 1000 | 2,506 | star_frame 847 | 2.96x |
| blubber_trusted | v0 | W3 grow_one | 1000 | 8,533 | star_frame 4,268 | 2.00x |
| blubber_trusted | v0 | W4 lookup | 100 | 1,259 | star_frame 789 | 1.60x |
| blubber_trusted | v0 | W4 insert_middle | 100 | 7,046 | star_frame 2,354 | 2.99x |
| blubber_trusted | v0 | W4 insert_middle | 1000 | 25,043 | star_frame 8,003 | 3.13x |
| blubber_checked | v0 | W1 get_middle | 100 | 873 | anchor_slab 298 | 2.93x |
| blubber_checked | v0 | W1 set_all | 100 | 1,959 | wincode 682 | 2.87x |
| blubber_checked | v0 | W1 push | 100 | 1,781 | anchor_slab 389 | 4.58x |
| blubber_checked | v0 | W1 remove_front | 100 | 1,676 | anchor_slab 335 | 5.00x |
| blubber_checked | v0 | W1 get_middle | 1000 | 873 | anchor_slab 298 | 2.93x |
| blubber_checked | v0 | W1 set_all | 1000 | 8,259 | wincode 4,282 | 1.93x |
| blubber_checked | v0 | W1 push | 1000 | 1,781 | anchor_slab 389 | 4.58x |
| blubber_checked | v0 | W1 remove_front | 1000 | 2,509 | anchor_slab 356 | 7.05x |
| blubber_trusted | v3 | W1 get_middle | 100 | 583 | anchor_slab 297 | 1.96x |
| blubber_trusted | v3 | W1 set_all | 100 | 1,621 | wincode 681 | 2.38x |
| blubber_trusted | v3 | W1 push | 100 | 1,443 | anchor_slab 388 | 3.72x |
| blubber_trusted | v3 | W1 remove_front | 100 | 1,336 | anchor_slab 334 | 4.00x |
| blubber_trusted | v3 | W1 get_middle | 1000 | 583 | anchor_slab 297 | 1.96x |
| blubber_trusted | v3 | W1 set_all | 1000 | 7,921 | wincode 4,281 | 1.85x |
| blubber_trusted | v3 | W1 push | 1000 | 1,443 | anchor_slab 388 | 3.72x |
| blubber_trusted | v3 | W1 remove_front | 1000 | 2,169 | anchor_slab 355 | 6.11x |
| blubber_trusted | v3 | W2 lookup | 100 | 971 | zeropod 425 | 2.28x |
| blubber_trusted | v3 | W2 insert_middle | 100 | 2,866 | manifest 754 | 3.80x |
| blubber_trusted | v3 | W2 remove | 100 | 2,173 | star_frame 758 | 2.87x |
| blubber_trusted | v3 | W2 set_all | 100 | 1,785 | wincode 743 | 2.40x |
| blubber_trusted | v3 | W2 iterate_sum | 100 | 2,291 | wincode 1,373 | 1.67x |
| blubber_trusted | v3 | W2 lookup | 1000 | 971 | zeropod 425 | 2.28x |
| blubber_trusted | v3 | W2 insert_middle | 1000 | 5,521 | manifest 811 | 6.81x |
| blubber_trusted | v3 | W2 remove | 1000 | 2,779 | star_frame 827 | 3.36x |
| blubber_trusted | v3 | W2 iterate_sum | 1000 | 16,691 | wincode 10,373 | 1.61x |
| blubber_trusted | v3 | W3 set_equal | 100 | 2,500 | star_frame 847 | 2.95x |
| blubber_trusted | v3 | W3 grow_one | 100 | 3,628 | star_frame 1,518 | 2.39x |
| blubber_trusted | v3 | W3 set_equal | 1000 | 2,500 | star_frame 847 | 2.95x |
| blubber_trusted | v3 | W3 grow_one | 1000 | 8,530 | star_frame 4,268 | 2.00x |
| blubber_trusted | v3 | W4 lookup | 100 | 1,255 | star_frame 789 | 1.59x |
| blubber_trusted | v3 | W4 insert_middle | 100 | 7,047 | star_frame 2,354 | 2.99x |
| blubber_trusted | v3 | W4 insert_middle | 1000 | 25,088 | star_frame 8,003 | 3.13x |
| blubber_checked | v3 | W1 get_middle | 100 | 870 | anchor_slab 297 | 2.93x |
| blubber_checked | v3 | W1 set_all | 100 | 1,956 | wincode 681 | 2.87x |
| blubber_checked | v3 | W1 push | 100 | 1,778 | anchor_slab 388 | 4.58x |
| blubber_checked | v3 | W1 remove_front | 100 | 1,672 | anchor_slab 334 | 5.01x |
| blubber_checked | v3 | W1 get_middle | 1000 | 870 | anchor_slab 297 | 2.93x |
| blubber_checked | v3 | W1 set_all | 1000 | 8,256 | wincode 4,281 | 1.93x |
| blubber_checked | v3 | W1 push | 1000 | 1,778 | anchor_slab 388 | 4.58x |
| blubber_checked | v3 | W1 remove_front | 1000 | 2,505 | anchor_slab 355 | 7.06x |

## Other load or storage policies

These ratios compare different load or capacity rules and serve as context. Fixed fixtures reserve their configured entries; other formats can retain per-value space. They do not establish a performance gap for equivalent work.

| Blubber row | Arch | Operation | n | Blubber CU | Best context CU | Ratio |
|---|---|---|---|---|---|---|
| blubber_trusted | v0 | W1 get_middle | 100 | 586 | bytemuck 286 | 2.05x |
| blubber_trusted | v0 | W1 set_all | 100 | 1,624 | bytemuck 682 | 2.38x |
| blubber_trusted | v0 | W1 push | 100 | 1,446 | bytemuck 289 | 5.00x |
| blubber_trusted | v0 | W1 remove_front | 100 | 1,340 | bytemuck 305 | 4.39x |
| blubber_trusted | v0 | W1 get_middle | 1000 | 586 | bytemuck 286 | 2.05x |
| blubber_trusted | v0 | W1 set_all | 1000 | 7,924 | bytemuck 4,282 | 1.85x |
| blubber_trusted | v0 | W1 remove_front | 1000 | 2,173 | bytemuck 326 | 6.67x |
| blubber_trusted | v0 | W2 lookup | 100 | 974 | sokoban 496 | 1.96x |
| blubber_trusted | v0 | W2 insert_middle | 100 | 2,868 | sokoban 671 | 4.27x |
| blubber_trusted | v0 | W2 remove | 100 | 2,171 | critbit 887 | 2.45x |
| blubber_trusted | v0 | W2 lookup | 1000 | 974 | sokoban 583 | 1.67x |
| blubber_trusted | v0 | W2 remove | 1000 | 2,777 | critbit 1,064 | 2.61x |
| blubber_checked | v0 | W1 get_middle | 100 | 873 | bytemuck 286 | 3.05x |
| blubber_checked | v0 | W1 set_all | 100 | 1,959 | bytemuck 682 | 2.87x |
| blubber_checked | v0 | W1 push | 100 | 1,781 | bytemuck 289 | 6.16x |
| blubber_checked | v0 | W1 remove_front | 100 | 1,676 | bytemuck 305 | 5.50x |
| blubber_checked | v0 | W1 get_middle | 1000 | 873 | bytemuck 286 | 3.05x |
| blubber_checked | v0 | W1 set_all | 1000 | 8,259 | bytemuck 4,282 | 1.93x |
| blubber_checked | v0 | W1 remove_front | 1000 | 2,509 | bytemuck 326 | 7.70x |
| blubber_checked | v0 | W2 lookup | 100 | 3,501 | zeropod 426 | 8.22x |
| blubber_checked | v0 | W2 insert_middle | 100 | 5,448 | sokoban 671 | 8.12x |
| blubber_checked | v0 | W2 remove | 100 | 4,751 | star_frame 758 | 6.27x |
| blubber_checked | v0 | W2 set_all | 100 | 4,316 | wincode 744 | 5.80x |
| blubber_checked | v0 | W2 iterate_sum | 100 | 5,023 | wincode 1,374 | 3.66x |
| blubber_checked | v0 | W2 lookup | 1000 | 22,401 | zeropod 426 | 52.58x |
| blubber_checked | v0 | W2 insert_middle | 1000 | 26,967 | manifest 812 | 33.21x |
| blubber_checked | v0 | W2 remove | 1000 | 24,257 | star_frame 827 | 29.33x |
| blubber_checked | v0 | W2 set_all | 1000 | 26,816 | wincode 4,344 | 6.17x |
| blubber_checked | v0 | W2 iterate_sum | 1000 | 40,123 | wincode 10,374 | 3.87x |
| blubber_checked | v0 | W3 set_equal | 100 | 4,560 | star_frame 847 | 5.38x |
| blubber_checked | v0 | W3 grow_one | 100 | 6,046 | star_frame 1,518 | 3.98x |
| blubber_checked | v0 | W3 set_equal | 1000 | 17,835 | star_frame 847 | 21.06x |
| blubber_checked | v0 | W3 grow_one | 1000 | 28,267 | star_frame 4,268 | 6.62x |
| blubber_checked | v0 | W4 lookup | 100 | 5,663 | star_frame 789 | 7.18x |
| blubber_checked | v0 | W4 insert_middle | 100 | 11,681 | star_frame 2,354 | 4.96x |
| blubber_checked | v0 | W4 lookup | 1000 | 39,638 | star_frame 843 | 47.02x |
| blubber_checked | v0 | W4 insert_middle | 1000 | 64,103 | star_frame 8,003 | 8.01x |
| blubber_trusted | v3 | W1 get_middle | 100 | 583 | bytemuck 285 | 2.05x |
| blubber_trusted | v3 | W1 set_all | 100 | 1,621 | bytemuck 681 | 2.38x |
| blubber_trusted | v3 | W1 push | 100 | 1,443 | bytemuck 288 | 5.01x |
| blubber_trusted | v3 | W1 remove_front | 100 | 1,336 | bytemuck 304 | 4.39x |
| blubber_trusted | v3 | W1 get_middle | 1000 | 583 | bytemuck 285 | 2.05x |
| blubber_trusted | v3 | W1 set_all | 1000 | 7,921 | bytemuck 4,281 | 1.85x |
| blubber_trusted | v3 | W1 remove_front | 1000 | 2,169 | bytemuck 325 | 6.67x |
| blubber_trusted | v3 | W2 lookup | 100 | 971 | sokoban 495 | 1.96x |
| blubber_trusted | v3 | W2 insert_middle | 100 | 2,866 | sokoban 670 | 4.28x |
| blubber_trusted | v3 | W2 remove | 100 | 2,173 | critbit 886 | 2.45x |
| blubber_trusted | v3 | W2 lookup | 1000 | 971 | sokoban 582 | 1.67x |
| blubber_trusted | v3 | W2 remove | 1000 | 2,779 | critbit 1,063 | 2.61x |
| blubber_checked | v3 | W1 get_middle | 100 | 870 | bytemuck 285 | 3.05x |
| blubber_checked | v3 | W1 set_all | 100 | 1,956 | bytemuck 681 | 2.87x |
| blubber_checked | v3 | W1 push | 100 | 1,778 | bytemuck 288 | 6.17x |
| blubber_checked | v3 | W1 remove_front | 100 | 1,672 | bytemuck 304 | 5.50x |
| blubber_checked | v3 | W1 get_middle | 1000 | 870 | bytemuck 285 | 3.05x |
| blubber_checked | v3 | W1 set_all | 1000 | 8,256 | bytemuck 4,281 | 1.93x |
| blubber_checked | v3 | W1 remove_front | 1000 | 2,505 | bytemuck 325 | 7.71x |
| blubber_checked | v3 | W2 lookup | 100 | 3,498 | zeropod 425 | 8.23x |
| blubber_checked | v3 | W2 insert_middle | 100 | 5,446 | sokoban 670 | 8.13x |
| blubber_checked | v3 | W2 remove | 100 | 4,753 | star_frame 758 | 6.27x |
| blubber_checked | v3 | W2 set_all | 100 | 4,313 | wincode 743 | 5.80x |
| blubber_checked | v3 | W2 iterate_sum | 100 | 5,020 | wincode 1,373 | 3.66x |
| blubber_checked | v3 | W2 lookup | 1000 | 22,398 | zeropod 425 | 52.70x |
| blubber_checked | v3 | W2 insert_middle | 1000 | 27,001 | manifest 811 | 33.29x |
| blubber_checked | v3 | W2 remove | 1000 | 24,259 | star_frame 827 | 29.33x |
| blubber_checked | v3 | W2 set_all | 1000 | 26,813 | wincode 4,343 | 6.17x |
| blubber_checked | v3 | W2 iterate_sum | 1000 | 40,120 | wincode 10,373 | 3.87x |
| blubber_checked | v3 | W3 set_equal | 100 | 4,546 | star_frame 847 | 5.37x |
| blubber_checked | v3 | W3 grow_one | 100 | 6,032 | star_frame 1,518 | 3.97x |
| blubber_checked | v3 | W3 set_equal | 1000 | 17,821 | star_frame 847 | 21.04x |
| blubber_checked | v3 | W3 grow_one | 1000 | 28,259 | star_frame 4,268 | 6.62x |
| blubber_checked | v3 | W4 lookup | 100 | 5,650 | star_frame 789 | 7.16x |
| blubber_checked | v3 | W4 insert_middle | 100 | 11,654 | star_frame 2,354 | 4.95x |
| blubber_checked | v3 | W4 lookup | 1000 | 39,625 | star_frame 843 | 47.00x |
| blubber_checked | v3 | W4 insert_middle | 1000 | 64,120 | star_frame 8,003 | 8.01x |
