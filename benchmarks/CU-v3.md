# Scaling measurements on SBF v3

Source SHA-256: `86ac10b35f190285bfabf829d3a7782e760c8b6e9d0d487f80a7b524e422c0c4`. See [scope and reproduction](CU.md).

Whole and baseline are separate uninstrumented instructions. Difference is whole minus baseline.
Open/load/edit/commit are calibrated instrumented brackets and cannot be added to reconstruct whole.
Mutable collections use Max/Prepaid; reads use ReadStore without resize. Funding CPIs are zero. Lengths are bytes.
Moved counts the sum of nonidentity relocation ranges, excluding encoding, zeroing and directory stores.
Ends counts directory words visited for publication, including unchanged words; it excludes count/used words.
Depth counts schema nodes, including the root and leaf. The depth-1 string is itself the envelope root.

| Fixture (layout / n / operation / position / prefix) | Baseline | Whole | Difference | Used before/after | Storage before/after | Moved | Ends | Depth | Open | Load | Edit | Commit |
|---|---:|---:|---:|---|---|---:|---:|---:|---:|---:|---:|---:|
| list_fixed_n10_load_trusted_all_plain | 219 | 219 | 0 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 34 | 9 | 14 |
| list_fixed_n10_late_read_all_plain | 219 | 433 | 214 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 34 | 222 | 14 |
| list_fixed_n10_validate_all_plain | 219 | 492 | 273 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 34 | 280 | 14 |
| list_fixed_n10_load_all_plain | 219 | 508 | 289 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 321 | 7 | 14 |
| list_fixed_n10_mutable_baseline_all_plain | 569 | 569 | 0 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 4 | 228 |
| list_fixed_n10_clear_all_plain | 569 | 2260 | 1691 | 96/16 | 96/16 | 0 | 0 | 3 | 219 | 25 | 1485 | 437 |
| list_fixed_n10_cursor_first_all_plain | 569 | 784 | 215 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 218 | 228 |
| list_fixed_n10_for_each_read_all_plain | 569 | 922 | 353 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 356 | 228 |
| list_fixed_n10_insert_front_plain | 569 | 2418 | 1849 | 96/104 | 96/104 | 80 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n10_replace_front_plain | 569 | 1923 | 1354 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n10_remove_front_plain | 569 | 1536 | 967 | 96/88 | 96/88 | 72 | 0 | 3 | 219 | 25 | 757 | 437 |
| list_fixed_n10_remove_into_front_plain | 569 | 2607 | 2038 | 96/88 | 96/88 | 72 | 0 | 3 | 219 | 25 | 1832 | 437 |
| list_fixed_n10_at_mut_front_plain | 569 | 1423 | 854 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n10_insert_bulk_front_plain | 569 | 2660 | 2091 | 96/112 | 96/112 | 80 | 0 | 3 | 219 | 25 | 2020 | 302 |
| list_fixed_n10_truncate_front_plain | 569 | 2270 | 1701 | 96/16 | 96/16 | 0 | 0 | 3 | 219 | 25 | 1495 | 437 |
| list_fixed_n10_insert_middle_plain | 569 | 2418 | 1849 | 96/104 | 96/104 | 40 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n10_replace_middle_plain | 569 | 1923 | 1354 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n10_remove_middle_plain | 569 | 1536 | 967 | 96/88 | 96/88 | 32 | 0 | 3 | 219 | 25 | 757 | 437 |
| list_fixed_n10_remove_into_middle_plain | 569 | 2607 | 2038 | 96/88 | 96/88 | 32 | 0 | 3 | 219 | 25 | 1832 | 437 |
| list_fixed_n10_at_mut_middle_plain | 569 | 1423 | 854 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n10_insert_bulk_middle_plain | 569 | 2661 | 2092 | 96/112 | 96/112 | 40 | 0 | 3 | 219 | 25 | 2021 | 302 |
| list_fixed_n10_truncate_middle_plain | 569 | 2240 | 1671 | 96/56 | 96/56 | 0 | 0 | 3 | 219 | 25 | 1465 | 437 |
| list_fixed_n10_insert_end_plain | 569 | 2222 | 1653 | 96/104 | 96/104 | 0 | 0 | 3 | 219 | 25 | 1584 | 302 |
| list_fixed_n10_replace_end_plain | 569 | 1923 | 1354 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n10_remove_end_plain | 569 | 1365 | 796 | 96/88 | 96/88 | 0 | 0 | 3 | 219 | 25 | 586 | 437 |
| list_fixed_n10_remove_into_end_plain | 569 | 2430 | 1861 | 96/88 | 96/88 | 0 | 0 | 3 | 219 | 25 | 1655 | 437 |
| list_fixed_n10_at_mut_end_plain | 569 | 1423 | 854 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n10_insert_bulk_end_plain | 569 | 2468 | 1899 | 96/112 | 96/112 | 0 | 0 | 3 | 219 | 25 | 1828 | 302 |
| list_fixed_n10_truncate_end_plain | 569 | 1903 | 1334 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1337 | 228 |
| list_fixed_n10_push_end_plain | 569 | 1323 | 754 | 96/104 | 96/104 | 0 | 0 | 3 | 219 | 25 | 683 | 302 |
| list_fixed_n10_pop_end_plain | 569 | 2326 | 1757 | 96/88 | 96/88 | 0 | 0 | 3 | 219 | 25 | 1551 | 437 |
| list_fixed_n10_extend_end_plain | 569 | 2467 | 1898 | 96/112 | 96/112 | 0 | 0 | 3 | 219 | 25 | 1827 | 302 |
| list_indexed_n10_load_trusted_all_plain | 220 | 220 | 0 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| list_indexed_n10_late_read_all_plain | 220 | 1001 | 781 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 34 | 789 | 14 |
| list_indexed_n10_validate_all_plain | 220 | 1085 | 865 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 34 | 872 | 14 |
| list_indexed_n10_load_all_plain | 220 | 1101 | 881 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 913 | 7 | 14 |
| list_indexed_n10_mutable_baseline_all_plain | 570 | 570 | 0 | 132/132 | 132/132 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| list_indexed_n10_clear_all_plain | 570 | 2106 | 1536 | 132/16 | 132/16 | 0 | 0 | 3 | 220 | 25 | 1330 | 437 |
| list_indexed_n10_cursor_first_all_plain | 570 | 3446 | 2876 | 132/141 | 132/141 | 72 | 9 | 3 | 220 | 25 | 2805 | 302 |
| list_indexed_n10_for_each_read_all_plain | 570 | 3053 | 2483 | 132/132 | 132/132 | 0 | 0 | 3 | 220 | 25 | 2486 | 228 |
| list_indexed_n10_insert_front_plain | 570 | 3131 | 2561 | 132/153 | 132/153 | 116 | 10 | 3 | 220 | 25 | 2492 | 302 |
| list_indexed_n10_replace_front_plain | 570 | 3412 | 2842 | 132/141 | 132/141 | 72 | 9 | 3 | 220 | 25 | 2771 | 302 |
| list_indexed_n10_remove_front_plain | 570 | 3858 | 3288 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 3078 | 437 |
| list_indexed_n10_remove_into_front_plain | 570 | 3548 | 2978 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2772 | 437 |
| list_indexed_n10_at_mut_front_plain | 570 | 2854 | 2284 | 132/141 | 132/141 | 72 | 9 | 3 | 220 | 25 | 2213 | 302 |
| list_indexed_n10_insert_bulk_front_plain | 570 | 3300 | 2730 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2659 | 302 |
| list_indexed_n10_truncate_front_plain | 570 | 2110 | 1540 | 132/16 | 132/16 | 0 | 0 | 3 | 220 | 25 | 1334 | 437 |
| list_indexed_n10_insert_middle_plain | 570 | 3186 | 2616 | 132/153 | 132/153 | 96 | 10 | 3 | 220 | 25 | 2547 | 302 |
| list_indexed_n10_replace_middle_plain | 570 | 3224 | 2654 | 132/141 | 132/141 | 32 | 9 | 3 | 220 | 25 | 2583 | 302 |
| list_indexed_n10_remove_middle_plain | 570 | 3854 | 3284 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 3074 | 437 |
| list_indexed_n10_remove_into_middle_plain | 570 | 3544 | 2974 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2768 | 437 |
| list_indexed_n10_at_mut_middle_plain | 570 | 2834 | 2264 | 132/141 | 132/141 | 32 | 4 | 3 | 220 | 25 | 2193 | 302 |
| list_indexed_n10_insert_bulk_middle_plain | 570 | 3344 | 2774 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2703 | 302 |
| list_indexed_n10_truncate_middle_plain | 570 | 2979 | 2409 | 132/72 | 132/72 | 40 | 4 | 3 | 220 | 25 | 2203 | 437 |
| list_indexed_n10_insert_end_plain | 570 | 2815 | 2245 | 132/153 | 132/153 | 80 | 10 | 3 | 220 | 25 | 2176 | 302 |
| list_indexed_n10_replace_end_plain | 570 | 2785 | 2215 | 132/141 | 132/141 | 0 | 9 | 3 | 220 | 25 | 2144 | 302 |
| list_indexed_n10_remove_end_plain | 570 | 3471 | 2901 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2691 | 437 |
| list_indexed_n10_remove_into_end_plain | 570 | 3161 | 2591 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2385 | 437 |
| list_indexed_n10_at_mut_end_plain | 570 | 2482 | 1912 | 132/141 | 132/141 | 0 | 0 | 3 | 220 | 25 | 1841 | 302 |
| list_indexed_n10_insert_bulk_end_plain | 570 | 2949 | 2379 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2308 | 302 |
| list_indexed_n10_truncate_end_plain | 570 | 2405 | 1835 | 132/132 | 132/132 | 0 | 0 | 3 | 220 | 25 | 1838 | 228 |
| list_indexed_n10_push_end_plain | 570 | 2827 | 2257 | 132/153 | 132/153 | 80 | 10 | 3 | 220 | 25 | 2186 | 302 |
| list_indexed_n10_pop_end_plain | 570 | 3159 | 2589 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2383 | 437 |
| list_indexed_n10_extend_end_plain | 570 | 2948 | 2378 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2307 | 302 |
| map_fixed_n10_load_trusted_all_plain | 224 | 224 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n10_late_read_all_plain | 224 | 1110 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 894 | 14 |
| map_fixed_n10_validate_all_plain | 224 | 853 | 629 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 636 | 14 |
| map_fixed_n10_load_all_plain | 224 | 869 | 645 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 677 | 7 | 14 |
| map_fixed_n10_mutable_baseline_all_plain | 574 | 574 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n10_clear_all_plain | 574 | 2538 | 1964 | 416/16 | 416/16 | 0 | 0 | 3 | 223 | 25 | 1758 | 437 |
| map_fixed_n10_cursor_first_all_plain | 574 | 1070 | 496 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 499 | 228 |
| map_fixed_n10_for_each_read_all_plain | 574 | 1235 | 661 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 664 | 228 |
| map_fixed_n10_insert_front_plain | 574 | 1877 | 1303 | 416/456 | 416/456 | 400 | 0 | 3 | 223 | 25 | 1234 | 302 |
| map_fixed_n10_replace_front_plain | 574 | 3227 | 2653 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2656 | 228 |
| map_fixed_n10_remove_front_plain | 574 | 1943 | 1369 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 1163 | 437 |
| map_fixed_n10_remove_into_front_plain | 574 | 3132 | 2558 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 2352 | 437 |
| map_fixed_n10_at_mut_front_plain | 574 | 1785 | 1211 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1214 | 228 |
| map_fixed_n10_lookup_hit_front_plain | 224 | 1227 | 1003 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1011 | 14 |
| map_fixed_n10_lookup_miss_front_plain | 224 | 1079 | 855 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 865 | 14 |
| map_fixed_n10_entry_occupied_front_plain | 574 | 2684 | 2110 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2117 | 228 |
| map_fixed_n10_entry_vacant_front_plain | 574 | 4379 | 3805 | 416/456 | 416/456 | 400 | 0 | 3 | 219 | 25 | 3740 | 302 |
| map_fixed_n10_insert_middle_plain | 574 | 2342 | 1768 | 416/456 | 416/456 | 200 | 0 | 3 | 223 | 25 | 1699 | 302 |
| map_fixed_n10_replace_middle_plain | 574 | 3429 | 2855 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2858 | 228 |
| map_fixed_n10_remove_middle_plain | 574 | 2044 | 1470 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 1264 | 437 |
| map_fixed_n10_remove_into_middle_plain | 574 | 3233 | 2659 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 2453 | 437 |
| map_fixed_n10_at_mut_middle_plain | 574 | 1886 | 1312 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1315 | 228 |
| map_fixed_n10_lookup_hit_middle_plain | 224 | 828 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n10_lookup_miss_middle_plain | 224 | 962 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 748 | 14 |
| map_fixed_n10_entry_occupied_middle_plain | 574 | 2785 | 2211 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2218 | 228 |
| map_fixed_n10_entry_vacant_middle_plain | 574 | 4915 | 4341 | 416/456 | 416/456 | 200 | 0 | 3 | 219 | 25 | 4276 | 302 |
| map_fixed_n10_insert_end_plain | 574 | 1699 | 1125 | 416/456 | 416/456 | 0 | 0 | 3 | 223 | 25 | 1056 | 302 |
| map_fixed_n10_replace_end_plain | 574 | 3155 | 2581 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2584 | 228 |
| map_fixed_n10_remove_end_plain | 574 | 1736 | 1162 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 956 | 437 |
| map_fixed_n10_remove_into_end_plain | 574 | 2919 | 2345 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 2139 | 437 |
| map_fixed_n10_at_mut_end_plain | 574 | 1749 | 1175 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1178 | 228 |
| map_fixed_n10_lookup_hit_end_plain | 224 | 1110 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 894 | 14 |
| map_fixed_n10_lookup_miss_end_plain | 224 | 962 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 748 | 14 |
| map_fixed_n10_entry_occupied_end_plain | 574 | 2648 | 2074 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2081 | 228 |
| map_fixed_n10_entry_vacant_end_plain | 574 | 4148 | 3574 | 416/456 | 416/456 | 0 | 0 | 3 | 219 | 25 | 3509 | 302 |
| map_fixed_n10_load_trusted_all_common | 226 | 226 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n10_late_read_all_common | 226 | 1690 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1472 | 14 |
| map_fixed_n10_validate_all_common | 226 | 936 | 710 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 717 | 14 |
| map_fixed_n10_load_all_common | 226 | 952 | 726 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 758 | 7 | 14 |
| map_fixed_n10_mutable_baseline_all_common | 576 | 576 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n10_clear_all_common | 576 | 2540 | 1964 | 416/16 | 416/16 | 0 | 0 | 3 | 223 | 25 | 1758 | 437 |
| map_fixed_n10_cursor_first_all_common | 576 | 1072 | 496 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 499 | 228 |
| map_fixed_n10_for_each_read_all_common | 576 | 1237 | 661 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 664 | 228 |
| map_fixed_n10_insert_front_common | 576 | 1903 | 1327 | 416/456 | 416/456 | 400 | 0 | 3 | 223 | 25 | 1258 | 302 |
| map_fixed_n10_replace_front_common | 576 | 3253 | 2677 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2680 | 228 |
| map_fixed_n10_remove_front_common | 576 | 1957 | 1381 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 1175 | 437 |
| map_fixed_n10_remove_into_front_common | 576 | 3146 | 2570 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 2364 | 437 |
| map_fixed_n10_at_mut_front_common | 576 | 1799 | 1223 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1226 | 228 |
| map_fixed_n10_lookup_hit_front_common | 226 | 2096 | 1870 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1878 | 14 |
| map_fixed_n10_lookup_miss_front_common | 226 | 2237 | 2011 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 2021 | 14 |
| map_fixed_n10_entry_occupied_front_common | 576 | 2698 | 2122 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2129 | 228 |
| map_fixed_n10_entry_vacant_front_common | 576 | 4420 | 3844 | 416/456 | 416/456 | 400 | 0 | 3 | 219 | 25 | 3779 | 302 |
| map_fixed_n10_insert_middle_common | 576 | 2428 | 1852 | 416/456 | 416/456 | 200 | 0 | 3 | 223 | 25 | 1783 | 302 |
| map_fixed_n10_replace_middle_common | 576 | 3479 | 2903 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2906 | 228 |
| map_fixed_n10_remove_middle_common | 576 | 2070 | 1494 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 1288 | 437 |
| map_fixed_n10_remove_into_middle_common | 576 | 3259 | 2683 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 2477 | 437 |
| map_fixed_n10_at_mut_middle_common | 576 | 1912 | 1336 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1339 | 228 |
| map_fixed_n10_lookup_hit_middle_common | 226 | 830 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n10_lookup_miss_middle_common | 226 | 1831 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1615 | 14 |
| map_fixed_n10_entry_occupied_middle_common | 576 | 2811 | 2235 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2242 | 228 |
| map_fixed_n10_entry_vacant_middle_common | 576 | 5028 | 4452 | 416/456 | 416/456 | 200 | 0 | 3 | 219 | 25 | 4387 | 302 |
| map_fixed_n10_insert_end_common | 576 | 1713 | 1137 | 416/456 | 416/456 | 0 | 0 | 3 | 223 | 25 | 1068 | 302 |
| map_fixed_n10_replace_end_common | 576 | 3157 | 2581 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2584 | 228 |
| map_fixed_n10_remove_end_common | 576 | 1738 | 1162 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 956 | 437 |
| map_fixed_n10_remove_into_end_common | 576 | 2921 | 2345 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 2139 | 437 |
| map_fixed_n10_at_mut_end_common | 576 | 1751 | 1175 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1178 | 228 |
| map_fixed_n10_lookup_hit_end_common | 226 | 1690 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1472 | 14 |
| map_fixed_n10_lookup_miss_end_common | 226 | 1831 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1615 | 14 |
| map_fixed_n10_entry_occupied_end_common | 576 | 2650 | 2074 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2081 | 228 |
| map_fixed_n10_entry_vacant_end_common | 576 | 4174 | 3598 | 416/456 | 416/456 | 0 | 0 | 3 | 219 | 25 | 3533 | 302 |
| map_indexed_n10_load_trusted_all_plain | 222 | 222 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n10_late_read_all_plain | 222 | 1327 | 1105 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1113 | 14 |
| map_indexed_n10_validate_all_plain | 222 | 1384 | 1162 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1169 | 14 |
| map_indexed_n10_load_all_plain | 222 | 1400 | 1178 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 1210 | 7 | 14 |
| map_indexed_n10_mutable_baseline_all_plain | 572 | 572 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n10_clear_all_plain | 572 | 2821 | 2249 | 456/16 | 456/16 | 0 | 0 | 3 | 220 | 25 | 2043 | 437 |
| map_indexed_n10_cursor_first_all_plain | 572 | 4026 | 3454 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3383 | 302 |
| map_indexed_n10_for_each_read_all_plain | 572 | 3740 | 3168 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 3171 | 228 |
| map_indexed_n10_insert_front_plain | 572 | 4272 | 3700 | 456/509 | 456/509 | 440 | 11 | 3 | 220 | 25 | 3631 | 302 |
| map_indexed_n10_replace_front_plain | 572 | 3956 | 3384 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3313 | 302 |
| map_indexed_n10_remove_front_plain | 572 | 4477 | 3905 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3699 | 437 |
| map_indexed_n10_remove_into_front_plain | 572 | 4124 | 3552 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3346 | 437 |
| map_indexed_n10_at_mut_front_plain | 572 | 3375 | 2803 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 2732 | 302 |
| map_indexed_n10_lookup_hit_front_plain | 222 | 1401 | 1179 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1187 | 14 |
| map_indexed_n10_lookup_miss_front_plain | 222 | 1034 | 812 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 822 | 14 |
| map_indexed_n10_entry_occupied_front_plain | 572 | 3359 | 2787 | 456/465 | 456/465 | 72 | 10 | 3 | 216 | 25 | 2720 | 302 |
| map_indexed_n10_entry_vacant_front_plain | 572 | 5321 | 4749 | 456/509 | 456/509 | 440 | 11 | 3 | 216 | 25 | 4684 | 302 |
| map_indexed_n10_insert_middle_plain | 572 | 4838 | 4266 | 456/509 | 456/509 | 260 | 11 | 3 | 220 | 25 | 4197 | 302 |
| map_indexed_n10_replace_middle_plain | 572 | 3953 | 3381 | 456/465 | 456/465 | 32 | 10 | 3 | 220 | 25 | 3310 | 302 |
| map_indexed_n10_remove_middle_plain | 572 | 4645 | 4073 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3867 | 437 |
| map_indexed_n10_remove_into_middle_plain | 572 | 4294 | 3722 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3516 | 437 |
| map_indexed_n10_at_mut_middle_plain | 572 | 3394 | 2822 | 456/465 | 456/465 | 32 | 5 | 3 | 220 | 25 | 2751 | 302 |
| map_indexed_n10_lookup_hit_middle_plain | 222 | 1044 | 822 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 830 | 14 |
| map_indexed_n10_lookup_miss_middle_plain | 222 | 917 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 705 | 14 |
| map_indexed_n10_entry_occupied_middle_plain | 572 | 3336 | 2764 | 456/465 | 456/465 | 32 | 5 | 3 | 216 | 25 | 2697 | 302 |
| map_indexed_n10_entry_vacant_middle_plain | 572 | 5959 | 5387 | 456/509 | 456/509 | 260 | 11 | 3 | 216 | 25 | 5322 | 302 |
| map_indexed_n10_insert_end_plain | 572 | 3690 | 3118 | 456/509 | 456/509 | 80 | 11 | 3 | 220 | 25 | 3049 | 302 |
| map_indexed_n10_replace_end_plain | 572 | 3417 | 2845 | 456/465 | 456/465 | 0 | 10 | 3 | 220 | 25 | 2774 | 302 |
| map_indexed_n10_remove_end_plain | 572 | 3940 | 3368 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 3162 | 437 |
| map_indexed_n10_remove_into_end_plain | 572 | 3597 | 3025 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 2819 | 437 |
| map_indexed_n10_at_mut_end_plain | 572 | 2958 | 2386 | 456/465 | 456/465 | 0 | 1 | 3 | 220 | 25 | 2315 | 302 |
| map_indexed_n10_lookup_hit_end_plain | 222 | 1327 | 1105 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1113 | 14 |
| map_indexed_n10_lookup_miss_end_plain | 222 | 917 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 705 | 14 |
| map_indexed_n10_entry_occupied_end_plain | 572 | 2899 | 2327 | 456/465 | 456/465 | 0 | 1 | 3 | 216 | 25 | 2260 | 302 |
| map_indexed_n10_entry_vacant_end_plain | 572 | 4741 | 4169 | 456/509 | 456/509 | 80 | 11 | 3 | 216 | 25 | 4104 | 302 |
| map_indexed_n10_load_trusted_all_common | 224 | 224 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n10_late_read_all_common | 224 | 1907 | 1683 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1691 | 14 |
| map_indexed_n10_validate_all_common | 224 | 1467 | 1243 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1250 | 14 |
| map_indexed_n10_load_all_common | 224 | 1483 | 1259 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 1291 | 7 | 14 |
| map_indexed_n10_mutable_baseline_all_common | 574 | 574 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n10_clear_all_common | 574 | 2823 | 2249 | 456/16 | 456/16 | 0 | 0 | 3 | 220 | 25 | 2043 | 437 |
| map_indexed_n10_cursor_first_all_common | 574 | 4028 | 3454 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3383 | 302 |
| map_indexed_n10_for_each_read_all_common | 574 | 3823 | 3249 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 3252 | 228 |
| map_indexed_n10_insert_front_common | 574 | 4298 | 3724 | 456/509 | 456/509 | 440 | 11 | 3 | 220 | 25 | 3655 | 302 |
| map_indexed_n10_replace_front_common | 574 | 3970 | 3396 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3325 | 302 |
| map_indexed_n10_remove_front_common | 574 | 4491 | 3917 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3711 | 437 |
| map_indexed_n10_remove_into_front_common | 574 | 4138 | 3564 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3358 | 437 |
| map_indexed_n10_at_mut_front_common | 574 | 3389 | 2815 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 2744 | 302 |
| map_indexed_n10_lookup_hit_front_common | 224 | 2270 | 2046 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 2054 | 14 |
| map_indexed_n10_lookup_miss_front_common | 224 | 2192 | 1968 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1978 | 14 |
| map_indexed_n10_entry_occupied_front_common | 574 | 3373 | 2799 | 456/465 | 456/465 | 72 | 10 | 3 | 216 | 25 | 2732 | 302 |
| map_indexed_n10_entry_vacant_front_common | 574 | 5362 | 4788 | 456/509 | 456/509 | 440 | 11 | 3 | 216 | 25 | 4723 | 302 |
| map_indexed_n10_insert_middle_common | 574 | 4924 | 4350 | 456/509 | 456/509 | 260 | 11 | 3 | 220 | 25 | 4281 | 302 |
| map_indexed_n10_replace_middle_common | 574 | 3979 | 3405 | 456/465 | 456/465 | 32 | 10 | 3 | 220 | 25 | 3334 | 302 |
| map_indexed_n10_remove_middle_common | 574 | 4671 | 4097 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3891 | 437 |
| map_indexed_n10_remove_into_middle_common | 574 | 4320 | 3746 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3540 | 437 |
| map_indexed_n10_at_mut_middle_common | 574 | 3420 | 2846 | 456/465 | 456/465 | 32 | 5 | 3 | 220 | 25 | 2775 | 302 |
| map_indexed_n10_lookup_hit_middle_common | 224 | 1046 | 822 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 830 | 14 |
| map_indexed_n10_lookup_miss_middle_common | 224 | 1786 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1572 | 14 |
| map_indexed_n10_entry_occupied_middle_common | 574 | 3362 | 2788 | 456/465 | 456/465 | 32 | 5 | 3 | 216 | 25 | 2721 | 302 |
| map_indexed_n10_entry_vacant_middle_common | 574 | 6072 | 5498 | 456/509 | 456/509 | 260 | 11 | 3 | 216 | 25 | 5433 | 302 |
| map_indexed_n10_insert_end_common | 574 | 3704 | 3130 | 456/509 | 456/509 | 80 | 11 | 3 | 220 | 25 | 3061 | 302 |
| map_indexed_n10_replace_end_common | 574 | 3419 | 2845 | 456/465 | 456/465 | 0 | 10 | 3 | 220 | 25 | 2774 | 302 |
| map_indexed_n10_remove_end_common | 574 | 3942 | 3368 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 3162 | 437 |
| map_indexed_n10_remove_into_end_common | 574 | 3599 | 3025 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 2819 | 437 |
| map_indexed_n10_at_mut_end_common | 574 | 2960 | 2386 | 456/465 | 456/465 | 0 | 1 | 3 | 220 | 25 | 2315 | 302 |
| map_indexed_n10_lookup_hit_end_common | 224 | 1907 | 1683 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1691 | 14 |
| map_indexed_n10_lookup_miss_end_common | 224 | 1786 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1572 | 14 |
| map_indexed_n10_entry_occupied_end_common | 574 | 2901 | 2327 | 456/465 | 456/465 | 0 | 1 | 3 | 216 | 25 | 2260 | 302 |
| map_indexed_n10_entry_vacant_end_common | 574 | 4767 | 4193 | 456/509 | 456/509 | 80 | 11 | 3 | 216 | 25 | 4128 | 302 |
| address_fixed_n10_load_trusted_all_plain | 229 | 229 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n10_late_read_all_plain | 229 | 1115 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 894 | 14 |
| address_fixed_n10_validate_all_plain | 229 | 858 | 629 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 636 | 14 |
| address_fixed_n10_load_all_plain | 229 | 874 | 645 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 677 | 7 | 14 |
| address_fixed_n10_mutable_baseline_all_plain | 579 | 579 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n10_clear_all_plain | 579 | 2543 | 1964 | 416/16 | 416/16 | 0 | 0 | 3 | 228 | 25 | 1758 | 437 |
| address_fixed_n10_cursor_first_all_plain | 579 | 1075 | 496 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 499 | 228 |
| address_fixed_n10_for_each_read_all_plain | 579 | 1240 | 661 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 664 | 228 |
| address_fixed_n10_insert_front_plain | 579 | 1882 | 1303 | 416/456 | 416/456 | 400 | 0 | 3 | 228 | 25 | 1234 | 302 |
| address_fixed_n10_replace_front_plain | 579 | 3232 | 2653 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2656 | 228 |
| address_fixed_n10_remove_front_plain | 579 | 1948 | 1369 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 1163 | 437 |
| address_fixed_n10_remove_into_front_plain | 579 | 3137 | 2558 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 2352 | 437 |
| address_fixed_n10_at_mut_front_plain | 579 | 1790 | 1211 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1214 | 228 |
| address_fixed_n10_lookup_hit_front_plain | 229 | 1232 | 1003 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1011 | 14 |
| address_fixed_n10_lookup_miss_front_plain | 229 | 1084 | 855 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 865 | 14 |
| address_fixed_n10_entry_occupied_front_plain | 579 | 2689 | 2110 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2117 | 228 |
| address_fixed_n10_entry_vacant_front_plain | 579 | 4384 | 3805 | 416/456 | 416/456 | 400 | 0 | 3 | 224 | 25 | 3740 | 302 |
| address_fixed_n10_insert_middle_plain | 579 | 2347 | 1768 | 416/456 | 416/456 | 200 | 0 | 3 | 228 | 25 | 1699 | 302 |
| address_fixed_n10_replace_middle_plain | 579 | 3434 | 2855 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2858 | 228 |
| address_fixed_n10_remove_middle_plain | 579 | 2049 | 1470 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 1264 | 437 |
| address_fixed_n10_remove_into_middle_plain | 579 | 3238 | 2659 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 2453 | 437 |
| address_fixed_n10_at_mut_middle_plain | 579 | 1891 | 1312 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1315 | 228 |
| address_fixed_n10_lookup_hit_middle_plain | 229 | 833 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n10_lookup_miss_middle_plain | 229 | 967 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 748 | 14 |
| address_fixed_n10_entry_occupied_middle_plain | 579 | 2790 | 2211 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2218 | 228 |
| address_fixed_n10_entry_vacant_middle_plain | 579 | 4920 | 4341 | 416/456 | 416/456 | 200 | 0 | 3 | 224 | 25 | 4276 | 302 |
| address_fixed_n10_insert_end_plain | 579 | 1704 | 1125 | 416/456 | 416/456 | 0 | 0 | 3 | 228 | 25 | 1056 | 302 |
| address_fixed_n10_replace_end_plain | 579 | 3160 | 2581 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2584 | 228 |
| address_fixed_n10_remove_end_plain | 579 | 1741 | 1162 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 956 | 437 |
| address_fixed_n10_remove_into_end_plain | 579 | 2924 | 2345 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 2139 | 437 |
| address_fixed_n10_at_mut_end_plain | 579 | 1754 | 1175 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1178 | 228 |
| address_fixed_n10_lookup_hit_end_plain | 229 | 1115 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 894 | 14 |
| address_fixed_n10_lookup_miss_end_plain | 229 | 967 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 748 | 14 |
| address_fixed_n10_entry_occupied_end_plain | 579 | 2653 | 2074 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2081 | 228 |
| address_fixed_n10_entry_vacant_end_plain | 579 | 4153 | 3574 | 416/456 | 416/456 | 0 | 0 | 3 | 224 | 25 | 3509 | 302 |
| address_fixed_n10_load_trusted_all_common | 231 | 231 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n10_late_read_all_common | 231 | 1695 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1472 | 14 |
| address_fixed_n10_validate_all_common | 231 | 941 | 710 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 717 | 14 |
| address_fixed_n10_load_all_common | 231 | 957 | 726 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 758 | 7 | 14 |
| address_fixed_n10_mutable_baseline_all_common | 581 | 581 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n10_clear_all_common | 581 | 2545 | 1964 | 416/16 | 416/16 | 0 | 0 | 3 | 228 | 25 | 1758 | 437 |
| address_fixed_n10_cursor_first_all_common | 581 | 1077 | 496 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 499 | 228 |
| address_fixed_n10_for_each_read_all_common | 581 | 1242 | 661 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 664 | 228 |
| address_fixed_n10_insert_front_common | 581 | 1908 | 1327 | 416/456 | 416/456 | 400 | 0 | 3 | 228 | 25 | 1258 | 302 |
| address_fixed_n10_replace_front_common | 581 | 3258 | 2677 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2680 | 228 |
| address_fixed_n10_remove_front_common | 581 | 1962 | 1381 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 1175 | 437 |
| address_fixed_n10_remove_into_front_common | 581 | 3151 | 2570 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 2364 | 437 |
| address_fixed_n10_at_mut_front_common | 581 | 1804 | 1223 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1226 | 228 |
| address_fixed_n10_lookup_hit_front_common | 231 | 2101 | 1870 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1878 | 14 |
| address_fixed_n10_lookup_miss_front_common | 231 | 2242 | 2011 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 2021 | 14 |
| address_fixed_n10_entry_occupied_front_common | 581 | 2703 | 2122 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2129 | 228 |
| address_fixed_n10_entry_vacant_front_common | 581 | 4425 | 3844 | 416/456 | 416/456 | 400 | 0 | 3 | 224 | 25 | 3779 | 302 |
| address_fixed_n10_insert_middle_common | 581 | 2433 | 1852 | 416/456 | 416/456 | 200 | 0 | 3 | 228 | 25 | 1783 | 302 |
| address_fixed_n10_replace_middle_common | 581 | 3484 | 2903 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2906 | 228 |
| address_fixed_n10_remove_middle_common | 581 | 2075 | 1494 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 1288 | 437 |
| address_fixed_n10_remove_into_middle_common | 581 | 3264 | 2683 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 2477 | 437 |
| address_fixed_n10_at_mut_middle_common | 581 | 1917 | 1336 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1339 | 228 |
| address_fixed_n10_lookup_hit_middle_common | 231 | 835 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n10_lookup_miss_middle_common | 231 | 1836 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1615 | 14 |
| address_fixed_n10_entry_occupied_middle_common | 581 | 2816 | 2235 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2242 | 228 |
| address_fixed_n10_entry_vacant_middle_common | 581 | 5033 | 4452 | 416/456 | 416/456 | 200 | 0 | 3 | 224 | 25 | 4387 | 302 |
| address_fixed_n10_insert_end_common | 581 | 1718 | 1137 | 416/456 | 416/456 | 0 | 0 | 3 | 228 | 25 | 1068 | 302 |
| address_fixed_n10_replace_end_common | 581 | 3162 | 2581 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2584 | 228 |
| address_fixed_n10_remove_end_common | 581 | 1743 | 1162 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 956 | 437 |
| address_fixed_n10_remove_into_end_common | 581 | 2926 | 2345 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 2139 | 437 |
| address_fixed_n10_at_mut_end_common | 581 | 1756 | 1175 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1178 | 228 |
| address_fixed_n10_lookup_hit_end_common | 231 | 1695 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1472 | 14 |
| address_fixed_n10_lookup_miss_end_common | 231 | 1836 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1615 | 14 |
| address_fixed_n10_entry_occupied_end_common | 581 | 2655 | 2074 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2081 | 228 |
| address_fixed_n10_entry_vacant_end_common | 581 | 4179 | 3598 | 416/456 | 416/456 | 0 | 0 | 3 | 224 | 25 | 3533 | 302 |
| address_indexed_n10_load_trusted_all_plain | 230 | 230 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n10_late_read_all_plain | 230 | 1335 | 1105 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1113 | 14 |
| address_indexed_n10_validate_all_plain | 230 | 1392 | 1162 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1169 | 14 |
| address_indexed_n10_load_all_plain | 230 | 1408 | 1178 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 1210 | 7 | 14 |
| address_indexed_n10_mutable_baseline_all_plain | 580 | 580 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n10_clear_all_plain | 580 | 2829 | 2249 | 456/16 | 456/16 | 0 | 0 | 3 | 229 | 25 | 2043 | 437 |
| address_indexed_n10_cursor_first_all_plain | 580 | 4034 | 3454 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3383 | 302 |
| address_indexed_n10_for_each_read_all_plain | 580 | 3748 | 3168 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 3171 | 228 |
| address_indexed_n10_insert_front_plain | 580 | 4280 | 3700 | 456/509 | 456/509 | 440 | 11 | 3 | 229 | 25 | 3631 | 302 |
| address_indexed_n10_replace_front_plain | 580 | 3964 | 3384 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3313 | 302 |
| address_indexed_n10_remove_front_plain | 580 | 4485 | 3905 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3699 | 437 |
| address_indexed_n10_remove_into_front_plain | 580 | 4132 | 3552 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3346 | 437 |
| address_indexed_n10_at_mut_front_plain | 580 | 3383 | 2803 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 2732 | 302 |
| address_indexed_n10_lookup_hit_front_plain | 230 | 1409 | 1179 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1187 | 14 |
| address_indexed_n10_lookup_miss_front_plain | 230 | 1042 | 812 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 822 | 14 |
| address_indexed_n10_entry_occupied_front_plain | 580 | 3367 | 2787 | 456/465 | 456/465 | 72 | 10 | 3 | 225 | 25 | 2720 | 302 |
| address_indexed_n10_entry_vacant_front_plain | 580 | 5329 | 4749 | 456/509 | 456/509 | 440 | 11 | 3 | 225 | 25 | 4684 | 302 |
| address_indexed_n10_insert_middle_plain | 580 | 4846 | 4266 | 456/509 | 456/509 | 260 | 11 | 3 | 229 | 25 | 4197 | 302 |
| address_indexed_n10_replace_middle_plain | 580 | 3961 | 3381 | 456/465 | 456/465 | 32 | 10 | 3 | 229 | 25 | 3310 | 302 |
| address_indexed_n10_remove_middle_plain | 580 | 4653 | 4073 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3867 | 437 |
| address_indexed_n10_remove_into_middle_plain | 580 | 4302 | 3722 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3516 | 437 |
| address_indexed_n10_at_mut_middle_plain | 580 | 3402 | 2822 | 456/465 | 456/465 | 32 | 5 | 3 | 229 | 25 | 2751 | 302 |
| address_indexed_n10_lookup_hit_middle_plain | 230 | 1052 | 822 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 830 | 14 |
| address_indexed_n10_lookup_miss_middle_plain | 230 | 925 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 705 | 14 |
| address_indexed_n10_entry_occupied_middle_plain | 580 | 3344 | 2764 | 456/465 | 456/465 | 32 | 5 | 3 | 225 | 25 | 2697 | 302 |
| address_indexed_n10_entry_vacant_middle_plain | 580 | 5967 | 5387 | 456/509 | 456/509 | 260 | 11 | 3 | 225 | 25 | 5322 | 302 |
| address_indexed_n10_insert_end_plain | 580 | 3698 | 3118 | 456/509 | 456/509 | 80 | 11 | 3 | 229 | 25 | 3049 | 302 |
| address_indexed_n10_replace_end_plain | 580 | 3425 | 2845 | 456/465 | 456/465 | 0 | 10 | 3 | 229 | 25 | 2774 | 302 |
| address_indexed_n10_remove_end_plain | 580 | 3948 | 3368 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 3162 | 437 |
| address_indexed_n10_remove_into_end_plain | 580 | 3605 | 3025 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 2819 | 437 |
| address_indexed_n10_at_mut_end_plain | 580 | 2966 | 2386 | 456/465 | 456/465 | 0 | 1 | 3 | 229 | 25 | 2315 | 302 |
| address_indexed_n10_lookup_hit_end_plain | 230 | 1335 | 1105 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1113 | 14 |
| address_indexed_n10_lookup_miss_end_plain | 230 | 925 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 705 | 14 |
| address_indexed_n10_entry_occupied_end_plain | 580 | 2907 | 2327 | 456/465 | 456/465 | 0 | 1 | 3 | 225 | 25 | 2260 | 302 |
| address_indexed_n10_entry_vacant_end_plain | 580 | 4749 | 4169 | 456/509 | 456/509 | 80 | 11 | 3 | 225 | 25 | 4104 | 302 |
| address_indexed_n10_load_trusted_all_common | 232 | 232 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n10_late_read_all_common | 232 | 1915 | 1683 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1691 | 14 |
| address_indexed_n10_validate_all_common | 232 | 1475 | 1243 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1250 | 14 |
| address_indexed_n10_load_all_common | 232 | 1491 | 1259 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 1291 | 7 | 14 |
| address_indexed_n10_mutable_baseline_all_common | 582 | 582 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n10_clear_all_common | 582 | 2831 | 2249 | 456/16 | 456/16 | 0 | 0 | 3 | 229 | 25 | 2043 | 437 |
| address_indexed_n10_cursor_first_all_common | 582 | 4036 | 3454 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3383 | 302 |
| address_indexed_n10_for_each_read_all_common | 582 | 3831 | 3249 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 3252 | 228 |
| address_indexed_n10_insert_front_common | 582 | 4306 | 3724 | 456/509 | 456/509 | 440 | 11 | 3 | 229 | 25 | 3655 | 302 |
| address_indexed_n10_replace_front_common | 582 | 3978 | 3396 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3325 | 302 |
| address_indexed_n10_remove_front_common | 582 | 4499 | 3917 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3711 | 437 |
| address_indexed_n10_remove_into_front_common | 582 | 4146 | 3564 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3358 | 437 |
| address_indexed_n10_at_mut_front_common | 582 | 3397 | 2815 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 2744 | 302 |
| address_indexed_n10_lookup_hit_front_common | 232 | 2278 | 2046 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 2054 | 14 |
| address_indexed_n10_lookup_miss_front_common | 232 | 2200 | 1968 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1978 | 14 |
| address_indexed_n10_entry_occupied_front_common | 582 | 3381 | 2799 | 456/465 | 456/465 | 72 | 10 | 3 | 225 | 25 | 2732 | 302 |
| address_indexed_n10_entry_vacant_front_common | 582 | 5370 | 4788 | 456/509 | 456/509 | 440 | 11 | 3 | 225 | 25 | 4723 | 302 |
| address_indexed_n10_insert_middle_common | 582 | 4932 | 4350 | 456/509 | 456/509 | 260 | 11 | 3 | 229 | 25 | 4281 | 302 |
| address_indexed_n10_replace_middle_common | 582 | 3987 | 3405 | 456/465 | 456/465 | 32 | 10 | 3 | 229 | 25 | 3334 | 302 |
| address_indexed_n10_remove_middle_common | 582 | 4679 | 4097 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3891 | 437 |
| address_indexed_n10_remove_into_middle_common | 582 | 4328 | 3746 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3540 | 437 |
| address_indexed_n10_at_mut_middle_common | 582 | 3428 | 2846 | 456/465 | 456/465 | 32 | 5 | 3 | 229 | 25 | 2775 | 302 |
| address_indexed_n10_lookup_hit_middle_common | 232 | 1054 | 822 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 830 | 14 |
| address_indexed_n10_lookup_miss_middle_common | 232 | 1794 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1572 | 14 |
| address_indexed_n10_entry_occupied_middle_common | 582 | 3370 | 2788 | 456/465 | 456/465 | 32 | 5 | 3 | 225 | 25 | 2721 | 302 |
| address_indexed_n10_entry_vacant_middle_common | 582 | 6080 | 5498 | 456/509 | 456/509 | 260 | 11 | 3 | 225 | 25 | 5433 | 302 |
| address_indexed_n10_insert_end_common | 582 | 3712 | 3130 | 456/509 | 456/509 | 80 | 11 | 3 | 229 | 25 | 3061 | 302 |
| address_indexed_n10_replace_end_common | 582 | 3427 | 2845 | 456/465 | 456/465 | 0 | 10 | 3 | 229 | 25 | 2774 | 302 |
| address_indexed_n10_remove_end_common | 582 | 3950 | 3368 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 3162 | 437 |
| address_indexed_n10_remove_into_end_common | 582 | 3607 | 3025 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 2819 | 437 |
| address_indexed_n10_at_mut_end_common | 582 | 2968 | 2386 | 456/465 | 456/465 | 0 | 1 | 3 | 229 | 25 | 2315 | 302 |
| address_indexed_n10_lookup_hit_end_common | 232 | 1915 | 1683 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1691 | 14 |
| address_indexed_n10_lookup_miss_end_common | 232 | 1794 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1572 | 14 |
| address_indexed_n10_entry_occupied_end_common | 582 | 2909 | 2327 | 456/465 | 456/465 | 0 | 1 | 3 | 225 | 25 | 2260 | 302 |
| address_indexed_n10_entry_vacant_end_common | 582 | 4775 | 4193 | 456/509 | 456/509 | 80 | 11 | 3 | 225 | 25 | 4128 | 302 |
| list_fixed_n100_load_trusted_all_plain | 219 | 219 | 0 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 34 | 9 | 14 |
| list_fixed_n100_late_read_all_plain | 219 | 433 | 214 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 34 | 222 | 14 |
| list_fixed_n100_validate_all_plain | 219 | 492 | 273 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 34 | 280 | 14 |
| list_fixed_n100_load_all_plain | 219 | 508 | 289 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 321 | 7 | 14 |
| list_fixed_n100_mutable_baseline_all_plain | 569 | 569 | 0 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 4 | 228 |
| list_fixed_n100_clear_all_plain | 569 | 2414 | 1845 | 816/16 | 816/16 | 0 | 0 | 3 | 219 | 25 | 1639 | 437 |
| list_fixed_n100_cursor_first_all_plain | 569 | 784 | 215 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 218 | 228 |
| list_fixed_n100_for_each_read_all_plain | 569 | 1912 | 1343 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1346 | 228 |
| list_fixed_n100_insert_front_plain | 569 | 2418 | 1849 | 816/824 | 816/824 | 800 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n100_replace_front_plain | 569 | 1923 | 1354 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n100_remove_front_plain | 569 | 1536 | 967 | 816/808 | 816/808 | 792 | 0 | 3 | 219 | 25 | 757 | 437 |
| list_fixed_n100_remove_into_front_plain | 569 | 2607 | 2038 | 816/808 | 816/808 | 792 | 0 | 3 | 219 | 25 | 1832 | 437 |
| list_fixed_n100_at_mut_front_plain | 569 | 1423 | 854 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n100_insert_bulk_front_plain | 569 | 2660 | 2091 | 816/832 | 816/832 | 800 | 0 | 3 | 219 | 25 | 2020 | 302 |
| list_fixed_n100_truncate_front_plain | 569 | 2424 | 1855 | 816/16 | 816/16 | 0 | 0 | 3 | 219 | 25 | 1649 | 437 |
| list_fixed_n100_insert_middle_plain | 569 | 2418 | 1849 | 816/824 | 816/824 | 400 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n100_replace_middle_plain | 569 | 1923 | 1354 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n100_remove_middle_plain | 569 | 1536 | 967 | 816/808 | 816/808 | 392 | 0 | 3 | 219 | 25 | 757 | 437 |
| list_fixed_n100_remove_into_middle_plain | 569 | 2607 | 2038 | 816/808 | 816/808 | 392 | 0 | 3 | 219 | 25 | 1832 | 437 |
| list_fixed_n100_at_mut_middle_plain | 569 | 1423 | 854 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n100_insert_bulk_middle_plain | 569 | 2661 | 2092 | 816/832 | 816/832 | 400 | 0 | 3 | 219 | 25 | 2021 | 302 |
| list_fixed_n100_truncate_middle_plain | 569 | 2332 | 1763 | 816/416 | 816/416 | 0 | 0 | 3 | 219 | 25 | 1557 | 437 |
| list_fixed_n100_insert_end_plain | 569 | 2222 | 1653 | 816/824 | 816/824 | 0 | 0 | 3 | 219 | 25 | 1584 | 302 |
| list_fixed_n100_replace_end_plain | 569 | 1923 | 1354 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n100_remove_end_plain | 569 | 1365 | 796 | 816/808 | 816/808 | 0 | 0 | 3 | 219 | 25 | 586 | 437 |
| list_fixed_n100_remove_into_end_plain | 569 | 2430 | 1861 | 816/808 | 816/808 | 0 | 0 | 3 | 219 | 25 | 1655 | 437 |
| list_fixed_n100_at_mut_end_plain | 569 | 1423 | 854 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n100_insert_bulk_end_plain | 569 | 2468 | 1899 | 816/832 | 816/832 | 0 | 0 | 3 | 219 | 25 | 1828 | 302 |
| list_fixed_n100_truncate_end_plain | 569 | 1903 | 1334 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1337 | 228 |
| list_fixed_n100_push_end_plain | 569 | 1323 | 754 | 816/824 | 816/824 | 0 | 0 | 3 | 219 | 25 | 683 | 302 |
| list_fixed_n100_pop_end_plain | 569 | 2326 | 1757 | 816/808 | 816/808 | 0 | 0 | 3 | 219 | 25 | 1551 | 437 |
| list_fixed_n100_extend_end_plain | 569 | 2467 | 1898 | 816/832 | 816/832 | 0 | 0 | 3 | 219 | 25 | 1827 | 302 |
| list_indexed_n100_load_trusted_all_plain | 220 | 220 | 0 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| list_indexed_n100_late_read_all_plain | 220 | 1001 | 781 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 34 | 789 | 14 |
| list_indexed_n100_validate_all_plain | 220 | 2226 | 2006 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 34 | 2013 | 14 |
| list_indexed_n100_load_all_plain | 220 | 2242 | 2022 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 2054 | 7 | 14 |
| list_indexed_n100_mutable_baseline_all_plain | 570 | 570 | 0 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| list_indexed_n100_clear_all_plain | 570 | 2354 | 1784 | 1212/16 | 1212/16 | 0 | 0 | 3 | 220 | 25 | 1578 | 437 |
| list_indexed_n100_cursor_first_all_plain | 570 | 5156 | 4586 | 1212/1221 | 1212/1221 | 792 | 99 | 3 | 220 | 25 | 4515 | 302 |
| list_indexed_n100_for_each_read_all_plain | 570 | 20963 | 20393 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 220 | 25 | 20396 | 228 |
| list_indexed_n100_insert_front_plain | 570 | 6011 | 5441 | 1212/1233 | 1212/1233 | 1196 | 100 | 3 | 220 | 25 | 5372 | 302 |
| list_indexed_n100_replace_front_plain | 570 | 7192 | 6622 | 1212/1221 | 1212/1221 | 792 | 99 | 3 | 220 | 25 | 6551 | 302 |
| list_indexed_n100_remove_front_plain | 570 | 7638 | 7068 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 6858 | 437 |
| list_indexed_n100_remove_into_front_plain | 570 | 7328 | 6758 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 6552 | 437 |
| list_indexed_n100_at_mut_front_plain | 570 | 4564 | 3994 | 1212/1221 | 1212/1221 | 792 | 99 | 3 | 220 | 25 | 3923 | 302 |
| list_indexed_n100_insert_bulk_front_plain | 570 | 6630 | 6060 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 5989 | 302 |
| list_indexed_n100_truncate_front_plain | 570 | 2358 | 1788 | 1212/16 | 1212/16 | 0 | 0 | 3 | 220 | 25 | 1582 | 437 |
| list_indexed_n100_insert_middle_plain | 570 | 4626 | 4056 | 1212/1233 | 1212/1233 | 996 | 100 | 3 | 220 | 25 | 3987 | 302 |
| list_indexed_n100_replace_middle_plain | 570 | 5114 | 4544 | 1212/1221 | 1212/1221 | 392 | 99 | 3 | 220 | 25 | 4473 | 302 |
| list_indexed_n100_remove_middle_plain | 570 | 5744 | 5174 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 4964 | 437 |
| list_indexed_n100_remove_into_middle_plain | 570 | 5434 | 4864 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 4658 | 437 |
| list_indexed_n100_at_mut_middle_plain | 570 | 3689 | 3119 | 1212/1221 | 1212/1221 | 392 | 49 | 3 | 220 | 25 | 3048 | 302 |
| list_indexed_n100_insert_bulk_middle_plain | 570 | 5009 | 4439 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 4368 | 302 |
| list_indexed_n100_truncate_middle_plain | 570 | 3100 | 2530 | 1212/612 | 1212/612 | 400 | 49 | 3 | 220 | 25 | 2324 | 437 |
| list_indexed_n100_insert_end_plain | 570 | 2815 | 2245 | 1212/1233 | 1212/1233 | 800 | 100 | 3 | 220 | 25 | 2176 | 302 |
| list_indexed_n100_replace_end_plain | 570 | 2785 | 2215 | 1212/1221 | 1212/1221 | 0 | 99 | 3 | 220 | 25 | 2144 | 302 |
| list_indexed_n100_remove_end_plain | 570 | 3471 | 2901 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 2691 | 437 |
| list_indexed_n100_remove_into_end_plain | 570 | 3161 | 2591 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 2385 | 437 |
| list_indexed_n100_at_mut_end_plain | 570 | 2482 | 1912 | 1212/1221 | 1212/1221 | 0 | 0 | 3 | 220 | 25 | 1841 | 302 |
| list_indexed_n100_insert_bulk_end_plain | 570 | 2949 | 2379 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 2308 | 302 |
| list_indexed_n100_truncate_end_plain | 570 | 2405 | 1835 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 220 | 25 | 1838 | 228 |
| list_indexed_n100_push_end_plain | 570 | 2827 | 2257 | 1212/1233 | 1212/1233 | 800 | 100 | 3 | 220 | 25 | 2186 | 302 |
| list_indexed_n100_pop_end_plain | 570 | 3159 | 2589 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 2383 | 437 |
| list_indexed_n100_extend_end_plain | 570 | 2948 | 2378 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 2307 | 302 |
| map_fixed_n100_load_trusted_all_plain | 224 | 224 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n100_late_read_all_plain | 224 | 1533 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1317 | 14 |
| map_fixed_n100_validate_all_plain | 224 | 2743 | 2519 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2526 | 14 |
| map_fixed_n100_load_all_plain | 224 | 2759 | 2535 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 2567 | 7 | 14 |
| map_fixed_n100_mutable_baseline_all_plain | 574 | 574 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n100_clear_all_plain | 574 | 3455 | 2881 | 4016/16 | 4016/16 | 0 | 0 | 3 | 223 | 25 | 2675 | 437 |
| map_fixed_n100_cursor_first_all_plain | 574 | 1070 | 496 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 499 | 228 |
| map_fixed_n100_for_each_read_all_plain | 574 | 2405 | 1831 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1834 | 228 |
| map_fixed_n100_insert_front_plain | 574 | 2279 | 1705 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 223 | 25 | 1636 | 302 |
| map_fixed_n100_replace_front_plain | 574 | 3227 | 2653 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2656 | 228 |
| map_fixed_n100_remove_front_plain | 574 | 2300 | 1726 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 1520 | 437 |
| map_fixed_n100_remove_into_front_plain | 574 | 3489 | 2915 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 2709 | 437 |
| map_fixed_n100_at_mut_front_plain | 574 | 1785 | 1211 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1214 | 228 |
| map_fixed_n100_lookup_hit_front_plain | 224 | 1626 | 1402 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1410 | 14 |
| map_fixed_n100_lookup_miss_front_plain | 224 | 1478 | 1254 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1264 | 14 |
| map_fixed_n100_entry_occupied_front_plain | 574 | 2684 | 2110 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2117 | 228 |
| map_fixed_n100_entry_vacant_front_plain | 574 | 4781 | 4207 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 219 | 25 | 4142 | 302 |
| map_fixed_n100_insert_middle_plain | 574 | 2725 | 2151 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 223 | 25 | 2082 | 302 |
| map_fixed_n100_replace_middle_plain | 574 | 3429 | 2855 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2858 | 228 |
| map_fixed_n100_remove_middle_plain | 574 | 2163 | 1589 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 1383 | 437 |
| map_fixed_n100_remove_into_middle_plain | 574 | 3352 | 2778 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 2572 | 437 |
| map_fixed_n100_at_mut_middle_plain | 574 | 1886 | 1312 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1315 | 228 |
| map_fixed_n100_lookup_hit_middle_plain | 224 | 828 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n100_lookup_miss_middle_plain | 224 | 1385 | 1161 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1171 | 14 |
| map_fixed_n100_entry_occupied_middle_plain | 574 | 2785 | 2211 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2218 | 228 |
| map_fixed_n100_entry_vacant_middle_plain | 574 | 5298 | 4724 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 219 | 25 | 4659 | 302 |
| map_fixed_n100_insert_end_plain | 574 | 1699 | 1125 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 223 | 25 | 1056 | 302 |
| map_fixed_n100_replace_end_plain | 574 | 3155 | 2581 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2584 | 228 |
| map_fixed_n100_remove_end_plain | 574 | 1736 | 1162 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 956 | 437 |
| map_fixed_n100_remove_into_end_plain | 574 | 2919 | 2345 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 2139 | 437 |
| map_fixed_n100_at_mut_end_plain | 574 | 1749 | 1175 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1178 | 228 |
| map_fixed_n100_lookup_hit_end_plain | 224 | 1533 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1317 | 14 |
| map_fixed_n100_lookup_miss_end_plain | 224 | 1526 | 1302 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1312 | 14 |
| map_fixed_n100_entry_occupied_end_plain | 574 | 2648 | 2074 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2081 | 228 |
| map_fixed_n100_entry_vacant_end_plain | 574 | 4148 | 3574 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 219 | 25 | 3509 | 302 |
| map_fixed_n100_load_trusted_all_common | 226 | 226 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n100_late_read_all_common | 226 | 2980 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2762 | 14 |
| map_fixed_n100_validate_all_common | 226 | 3636 | 3410 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3417 | 14 |
| map_fixed_n100_load_all_common | 226 | 3652 | 3426 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 3458 | 7 | 14 |
| map_fixed_n100_mutable_baseline_all_common | 576 | 576 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n100_clear_all_common | 576 | 3457 | 2881 | 4016/16 | 4016/16 | 0 | 0 | 3 | 223 | 25 | 2675 | 437 |
| map_fixed_n100_cursor_first_all_common | 576 | 1072 | 496 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 499 | 228 |
| map_fixed_n100_for_each_read_all_common | 576 | 2407 | 1831 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1834 | 228 |
| map_fixed_n100_insert_front_common | 576 | 2305 | 1729 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 223 | 25 | 1660 | 302 |
| map_fixed_n100_replace_front_common | 576 | 3253 | 2677 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2680 | 228 |
| map_fixed_n100_remove_front_common | 576 | 2314 | 1738 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 1532 | 437 |
| map_fixed_n100_remove_into_front_common | 576 | 3503 | 2927 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 2721 | 437 |
| map_fixed_n100_at_mut_front_common | 576 | 1799 | 1223 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1226 | 228 |
| map_fixed_n100_lookup_hit_front_common | 226 | 3362 | 3136 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3144 | 14 |
| map_fixed_n100_lookup_miss_front_common | 226 | 3503 | 3277 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3287 | 14 |
| map_fixed_n100_entry_occupied_front_common | 576 | 2698 | 2122 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2129 | 228 |
| map_fixed_n100_entry_vacant_front_common | 576 | 4822 | 4246 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 219 | 25 | 4181 | 302 |
| map_fixed_n100_insert_middle_common | 576 | 2847 | 2271 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 223 | 25 | 2202 | 302 |
| map_fixed_n100_replace_middle_common | 576 | 3479 | 2903 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2906 | 228 |
| map_fixed_n100_remove_middle_common | 576 | 2189 | 1613 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 1407 | 437 |
| map_fixed_n100_remove_into_middle_common | 576 | 3378 | 2802 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 2596 | 437 |
| map_fixed_n100_at_mut_middle_common | 576 | 1912 | 1336 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1339 | 228 |
| map_fixed_n100_lookup_hit_middle_common | 226 | 830 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n100_lookup_miss_middle_common | 226 | 3121 | 2895 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2905 | 14 |
| map_fixed_n100_entry_occupied_middle_common | 576 | 2811 | 2235 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2242 | 228 |
| map_fixed_n100_entry_vacant_middle_common | 576 | 5447 | 4871 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 219 | 25 | 4806 | 302 |
| map_fixed_n100_insert_end_common | 576 | 1713 | 1137 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 223 | 25 | 1068 | 302 |
| map_fixed_n100_replace_end_common | 576 | 3157 | 2581 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2584 | 228 |
| map_fixed_n100_remove_end_common | 576 | 1738 | 1162 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 956 | 437 |
| map_fixed_n100_remove_into_end_common | 576 | 2921 | 2345 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 2139 | 437 |
| map_fixed_n100_at_mut_end_common | 576 | 1751 | 1175 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1178 | 228 |
| map_fixed_n100_lookup_hit_end_common | 226 | 2980 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2762 | 14 |
| map_fixed_n100_lookup_miss_end_common | 226 | 3551 | 3325 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3335 | 14 |
| map_fixed_n100_entry_occupied_end_common | 576 | 2650 | 2074 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2081 | 228 |
| map_fixed_n100_entry_vacant_end_common | 576 | 4174 | 3598 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 219 | 25 | 3533 | 302 |
| map_indexed_n100_load_trusted_all_plain | 222 | 222 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n100_late_read_all_plain | 222 | 1750 | 1528 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1536 | 14 |
| map_indexed_n100_validate_all_plain | 222 | 4595 | 4373 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 4380 | 14 |
| map_indexed_n100_load_all_plain | 222 | 4611 | 4389 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 4421 | 7 | 14 |
| map_indexed_n100_mutable_baseline_all_plain | 572 | 572 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n100_clear_all_plain | 572 | 3831 | 3259 | 4416/16 | 4416/16 | 0 | 0 | 3 | 220 | 25 | 3053 | 437 |
| map_indexed_n100_cursor_first_all_plain | 572 | 6546 | 5974 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5903 | 302 |
| map_indexed_n100_for_each_read_all_plain | 572 | 26420 | 25848 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 25851 | 228 |
| map_indexed_n100_insert_front_plain | 572 | 7644 | 7072 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 220 | 25 | 7003 | 302 |
| map_indexed_n100_replace_front_plain | 572 | 6926 | 6354 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 6283 | 302 |
| map_indexed_n100_remove_front_plain | 572 | 7804 | 7232 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 7026 | 437 |
| map_indexed_n100_remove_into_front_plain | 572 | 7451 | 6879 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 6673 | 437 |
| map_indexed_n100_at_mut_front_plain | 572 | 5895 | 5323 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5252 | 302 |
| map_indexed_n100_lookup_hit_front_plain | 222 | 1800 | 1578 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1586 | 14 |
| map_indexed_n100_lookup_miss_front_plain | 222 | 1433 | 1211 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1221 | 14 |
| map_indexed_n100_entry_occupied_front_plain | 572 | 5879 | 5307 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 216 | 25 | 5240 | 302 |
| map_indexed_n100_entry_vacant_front_plain | 572 | 8693 | 8121 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 216 | 25 | 8056 | 302 |
| map_indexed_n100_insert_middle_plain | 572 | 6706 | 6134 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 220 | 25 | 6065 | 302 |
| map_indexed_n100_replace_middle_plain | 572 | 5438 | 4866 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 220 | 25 | 4795 | 302 |
| map_indexed_n100_remove_middle_plain | 572 | 6249 | 5677 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5471 | 437 |
| map_indexed_n100_remove_into_middle_plain | 572 | 5898 | 5326 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5120 | 437 |
| map_indexed_n100_at_mut_middle_plain | 572 | 4654 | 4082 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 220 | 25 | 4011 | 302 |
| map_indexed_n100_lookup_hit_middle_plain | 222 | 1044 | 822 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 830 | 14 |
| map_indexed_n100_lookup_miss_middle_plain | 222 | 1340 | 1118 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1128 | 14 |
| map_indexed_n100_entry_occupied_middle_plain | 572 | 4596 | 4024 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 216 | 25 | 3957 | 302 |
| map_indexed_n100_entry_vacant_middle_plain | 572 | 7827 | 7255 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 216 | 25 | 7190 | 302 |
| map_indexed_n100_insert_end_plain | 572 | 3690 | 3118 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 220 | 25 | 3049 | 302 |
| map_indexed_n100_replace_end_plain | 572 | 3417 | 2845 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 220 | 25 | 2774 | 302 |
| map_indexed_n100_remove_end_plain | 572 | 3940 | 3368 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 3162 | 437 |
| map_indexed_n100_remove_into_end_plain | 572 | 3597 | 3025 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 2819 | 437 |
| map_indexed_n100_at_mut_end_plain | 572 | 2958 | 2386 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 220 | 25 | 2315 | 302 |
| map_indexed_n100_lookup_hit_end_plain | 222 | 1750 | 1528 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1536 | 14 |
| map_indexed_n100_lookup_miss_end_plain | 222 | 1481 | 1259 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1269 | 14 |
| map_indexed_n100_entry_occupied_end_plain | 572 | 2899 | 2327 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 216 | 25 | 2260 | 302 |
| map_indexed_n100_entry_vacant_end_plain | 572 | 4741 | 4169 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 216 | 25 | 4104 | 302 |
| map_indexed_n100_load_trusted_all_common | 224 | 224 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n100_late_read_all_common | 224 | 3197 | 2973 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 2981 | 14 |
| map_indexed_n100_validate_all_common | 224 | 5488 | 5264 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 5271 | 14 |
| map_indexed_n100_load_all_common | 224 | 5504 | 5280 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 5312 | 7 | 14 |
| map_indexed_n100_mutable_baseline_all_common | 574 | 574 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n100_clear_all_common | 574 | 3833 | 3259 | 4416/16 | 4416/16 | 0 | 0 | 3 | 220 | 25 | 3053 | 437 |
| map_indexed_n100_cursor_first_all_common | 574 | 6548 | 5974 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5903 | 302 |
| map_indexed_n100_for_each_read_all_common | 574 | 27313 | 26739 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 26742 | 228 |
| map_indexed_n100_insert_front_common | 574 | 7670 | 7096 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 220 | 25 | 7027 | 302 |
| map_indexed_n100_replace_front_common | 574 | 6940 | 6366 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 6295 | 302 |
| map_indexed_n100_remove_front_common | 574 | 7818 | 7244 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 7038 | 437 |
| map_indexed_n100_remove_into_front_common | 574 | 7465 | 6891 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 6685 | 437 |
| map_indexed_n100_at_mut_front_common | 574 | 5909 | 5335 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5264 | 302 |
| map_indexed_n100_lookup_hit_front_common | 224 | 3536 | 3312 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 3320 | 14 |
| map_indexed_n100_lookup_miss_front_common | 224 | 3458 | 3234 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 3244 | 14 |
| map_indexed_n100_entry_occupied_front_common | 574 | 5893 | 5319 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 216 | 25 | 5252 | 302 |
| map_indexed_n100_entry_vacant_front_common | 574 | 8734 | 8160 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 216 | 25 | 8095 | 302 |
| map_indexed_n100_insert_middle_common | 574 | 6828 | 6254 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 220 | 25 | 6185 | 302 |
| map_indexed_n100_replace_middle_common | 574 | 5464 | 4890 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 220 | 25 | 4819 | 302 |
| map_indexed_n100_remove_middle_common | 574 | 6275 | 5701 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5495 | 437 |
| map_indexed_n100_remove_into_middle_common | 574 | 5924 | 5350 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5144 | 437 |
| map_indexed_n100_at_mut_middle_common | 574 | 4680 | 4106 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 220 | 25 | 4035 | 302 |
| map_indexed_n100_lookup_hit_middle_common | 224 | 1046 | 822 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 830 | 14 |
| map_indexed_n100_lookup_miss_middle_common | 224 | 3076 | 2852 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 2862 | 14 |
| map_indexed_n100_entry_occupied_middle_common | 574 | 4622 | 4048 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 216 | 25 | 3981 | 302 |
| map_indexed_n100_entry_vacant_middle_common | 574 | 7976 | 7402 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 216 | 25 | 7337 | 302 |
| map_indexed_n100_insert_end_common | 574 | 3704 | 3130 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 220 | 25 | 3061 | 302 |
| map_indexed_n100_replace_end_common | 574 | 3419 | 2845 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 220 | 25 | 2774 | 302 |
| map_indexed_n100_remove_end_common | 574 | 3942 | 3368 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 3162 | 437 |
| map_indexed_n100_remove_into_end_common | 574 | 3599 | 3025 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 2819 | 437 |
| map_indexed_n100_at_mut_end_common | 574 | 2960 | 2386 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 220 | 25 | 2315 | 302 |
| map_indexed_n100_lookup_hit_end_common | 224 | 3197 | 2973 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 2981 | 14 |
| map_indexed_n100_lookup_miss_end_common | 224 | 3506 | 3282 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 3292 | 14 |
| map_indexed_n100_entry_occupied_end_common | 574 | 2901 | 2327 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 216 | 25 | 2260 | 302 |
| map_indexed_n100_entry_vacant_end_common | 574 | 4767 | 4193 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 216 | 25 | 4128 | 302 |
| address_fixed_n100_load_trusted_all_plain | 229 | 229 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n100_late_read_all_plain | 229 | 1538 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1317 | 14 |
| address_fixed_n100_validate_all_plain | 229 | 2748 | 2519 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2526 | 14 |
| address_fixed_n100_load_all_plain | 229 | 2764 | 2535 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 2567 | 7 | 14 |
| address_fixed_n100_mutable_baseline_all_plain | 579 | 579 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n100_clear_all_plain | 579 | 3460 | 2881 | 4016/16 | 4016/16 | 0 | 0 | 3 | 228 | 25 | 2675 | 437 |
| address_fixed_n100_cursor_first_all_plain | 579 | 1075 | 496 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 499 | 228 |
| address_fixed_n100_for_each_read_all_plain | 579 | 2410 | 1831 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1834 | 228 |
| address_fixed_n100_insert_front_plain | 579 | 2284 | 1705 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 228 | 25 | 1636 | 302 |
| address_fixed_n100_replace_front_plain | 579 | 3232 | 2653 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2656 | 228 |
| address_fixed_n100_remove_front_plain | 579 | 2305 | 1726 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 1520 | 437 |
| address_fixed_n100_remove_into_front_plain | 579 | 3494 | 2915 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 2709 | 437 |
| address_fixed_n100_at_mut_front_plain | 579 | 1790 | 1211 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1214 | 228 |
| address_fixed_n100_lookup_hit_front_plain | 229 | 1631 | 1402 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1410 | 14 |
| address_fixed_n100_lookup_miss_front_plain | 229 | 1483 | 1254 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1264 | 14 |
| address_fixed_n100_entry_occupied_front_plain | 579 | 2689 | 2110 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2117 | 228 |
| address_fixed_n100_entry_vacant_front_plain | 579 | 4786 | 4207 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 224 | 25 | 4142 | 302 |
| address_fixed_n100_insert_middle_plain | 579 | 2730 | 2151 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 228 | 25 | 2082 | 302 |
| address_fixed_n100_replace_middle_plain | 579 | 3434 | 2855 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2858 | 228 |
| address_fixed_n100_remove_middle_plain | 579 | 2168 | 1589 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 1383 | 437 |
| address_fixed_n100_remove_into_middle_plain | 579 | 3357 | 2778 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 2572 | 437 |
| address_fixed_n100_at_mut_middle_plain | 579 | 1891 | 1312 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1315 | 228 |
| address_fixed_n100_lookup_hit_middle_plain | 229 | 833 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n100_lookup_miss_middle_plain | 229 | 1390 | 1161 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1171 | 14 |
| address_fixed_n100_entry_occupied_middle_plain | 579 | 2790 | 2211 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2218 | 228 |
| address_fixed_n100_entry_vacant_middle_plain | 579 | 5303 | 4724 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 224 | 25 | 4659 | 302 |
| address_fixed_n100_insert_end_plain | 579 | 1704 | 1125 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 228 | 25 | 1056 | 302 |
| address_fixed_n100_replace_end_plain | 579 | 3160 | 2581 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2584 | 228 |
| address_fixed_n100_remove_end_plain | 579 | 1741 | 1162 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 956 | 437 |
| address_fixed_n100_remove_into_end_plain | 579 | 2924 | 2345 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 2139 | 437 |
| address_fixed_n100_at_mut_end_plain | 579 | 1754 | 1175 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1178 | 228 |
| address_fixed_n100_lookup_hit_end_plain | 229 | 1538 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1317 | 14 |
| address_fixed_n100_lookup_miss_end_plain | 229 | 1531 | 1302 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1312 | 14 |
| address_fixed_n100_entry_occupied_end_plain | 579 | 2653 | 2074 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2081 | 228 |
| address_fixed_n100_entry_vacant_end_plain | 579 | 4153 | 3574 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 224 | 25 | 3509 | 302 |
| address_fixed_n100_load_trusted_all_common | 231 | 231 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n100_late_read_all_common | 231 | 2985 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2762 | 14 |
| address_fixed_n100_validate_all_common | 231 | 3641 | 3410 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3417 | 14 |
| address_fixed_n100_load_all_common | 231 | 3657 | 3426 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 3458 | 7 | 14 |
| address_fixed_n100_mutable_baseline_all_common | 581 | 581 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n100_clear_all_common | 581 | 3462 | 2881 | 4016/16 | 4016/16 | 0 | 0 | 3 | 228 | 25 | 2675 | 437 |
| address_fixed_n100_cursor_first_all_common | 581 | 1077 | 496 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 499 | 228 |
| address_fixed_n100_for_each_read_all_common | 581 | 2412 | 1831 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1834 | 228 |
| address_fixed_n100_insert_front_common | 581 | 2310 | 1729 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 228 | 25 | 1660 | 302 |
| address_fixed_n100_replace_front_common | 581 | 3258 | 2677 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2680 | 228 |
| address_fixed_n100_remove_front_common | 581 | 2319 | 1738 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 1532 | 437 |
| address_fixed_n100_remove_into_front_common | 581 | 3508 | 2927 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 2721 | 437 |
| address_fixed_n100_at_mut_front_common | 581 | 1804 | 1223 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1226 | 228 |
| address_fixed_n100_lookup_hit_front_common | 231 | 3367 | 3136 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3144 | 14 |
| address_fixed_n100_lookup_miss_front_common | 231 | 3508 | 3277 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3287 | 14 |
| address_fixed_n100_entry_occupied_front_common | 581 | 2703 | 2122 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2129 | 228 |
| address_fixed_n100_entry_vacant_front_common | 581 | 4827 | 4246 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 224 | 25 | 4181 | 302 |
| address_fixed_n100_insert_middle_common | 581 | 2852 | 2271 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 228 | 25 | 2202 | 302 |
| address_fixed_n100_replace_middle_common | 581 | 3484 | 2903 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2906 | 228 |
| address_fixed_n100_remove_middle_common | 581 | 2194 | 1613 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 1407 | 437 |
| address_fixed_n100_remove_into_middle_common | 581 | 3383 | 2802 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 2596 | 437 |
| address_fixed_n100_at_mut_middle_common | 581 | 1917 | 1336 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1339 | 228 |
| address_fixed_n100_lookup_hit_middle_common | 231 | 835 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n100_lookup_miss_middle_common | 231 | 3126 | 2895 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2905 | 14 |
| address_fixed_n100_entry_occupied_middle_common | 581 | 2816 | 2235 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2242 | 228 |
| address_fixed_n100_entry_vacant_middle_common | 581 | 5452 | 4871 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 224 | 25 | 4806 | 302 |
| address_fixed_n100_insert_end_common | 581 | 1718 | 1137 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 228 | 25 | 1068 | 302 |
| address_fixed_n100_replace_end_common | 581 | 3162 | 2581 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2584 | 228 |
| address_fixed_n100_remove_end_common | 581 | 1743 | 1162 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 956 | 437 |
| address_fixed_n100_remove_into_end_common | 581 | 2926 | 2345 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 2139 | 437 |
| address_fixed_n100_at_mut_end_common | 581 | 1756 | 1175 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1178 | 228 |
| address_fixed_n100_lookup_hit_end_common | 231 | 2985 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2762 | 14 |
| address_fixed_n100_lookup_miss_end_common | 231 | 3556 | 3325 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3335 | 14 |
| address_fixed_n100_entry_occupied_end_common | 581 | 2655 | 2074 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2081 | 228 |
| address_fixed_n100_entry_vacant_end_common | 581 | 4179 | 3598 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 224 | 25 | 3533 | 302 |
| address_indexed_n100_load_trusted_all_plain | 230 | 230 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n100_late_read_all_plain | 230 | 1758 | 1528 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1536 | 14 |
| address_indexed_n100_validate_all_plain | 230 | 4603 | 4373 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 4380 | 14 |
| address_indexed_n100_load_all_plain | 230 | 4619 | 4389 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 4421 | 7 | 14 |
| address_indexed_n100_mutable_baseline_all_plain | 580 | 580 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n100_clear_all_plain | 580 | 3839 | 3259 | 4416/16 | 4416/16 | 0 | 0 | 3 | 229 | 25 | 3053 | 437 |
| address_indexed_n100_cursor_first_all_plain | 580 | 6554 | 5974 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5903 | 302 |
| address_indexed_n100_for_each_read_all_plain | 580 | 26428 | 25848 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 25851 | 228 |
| address_indexed_n100_insert_front_plain | 580 | 7652 | 7072 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 229 | 25 | 7003 | 302 |
| address_indexed_n100_replace_front_plain | 580 | 6934 | 6354 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 6283 | 302 |
| address_indexed_n100_remove_front_plain | 580 | 7812 | 7232 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 7026 | 437 |
| address_indexed_n100_remove_into_front_plain | 580 | 7459 | 6879 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 6673 | 437 |
| address_indexed_n100_at_mut_front_plain | 580 | 5903 | 5323 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5252 | 302 |
| address_indexed_n100_lookup_hit_front_plain | 230 | 1808 | 1578 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1586 | 14 |
| address_indexed_n100_lookup_miss_front_plain | 230 | 1441 | 1211 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1221 | 14 |
| address_indexed_n100_entry_occupied_front_plain | 580 | 5887 | 5307 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 225 | 25 | 5240 | 302 |
| address_indexed_n100_entry_vacant_front_plain | 580 | 8701 | 8121 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 225 | 25 | 8056 | 302 |
| address_indexed_n100_insert_middle_plain | 580 | 6714 | 6134 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 229 | 25 | 6065 | 302 |
| address_indexed_n100_replace_middle_plain | 580 | 5446 | 4866 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 229 | 25 | 4795 | 302 |
| address_indexed_n100_remove_middle_plain | 580 | 6257 | 5677 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5471 | 437 |
| address_indexed_n100_remove_into_middle_plain | 580 | 5906 | 5326 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5120 | 437 |
| address_indexed_n100_at_mut_middle_plain | 580 | 4662 | 4082 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 229 | 25 | 4011 | 302 |
| address_indexed_n100_lookup_hit_middle_plain | 230 | 1052 | 822 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 830 | 14 |
| address_indexed_n100_lookup_miss_middle_plain | 230 | 1348 | 1118 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1128 | 14 |
| address_indexed_n100_entry_occupied_middle_plain | 580 | 4604 | 4024 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 225 | 25 | 3957 | 302 |
| address_indexed_n100_entry_vacant_middle_plain | 580 | 7835 | 7255 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 225 | 25 | 7190 | 302 |
| address_indexed_n100_insert_end_plain | 580 | 3698 | 3118 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 229 | 25 | 3049 | 302 |
| address_indexed_n100_replace_end_plain | 580 | 3425 | 2845 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 229 | 25 | 2774 | 302 |
| address_indexed_n100_remove_end_plain | 580 | 3948 | 3368 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 3162 | 437 |
| address_indexed_n100_remove_into_end_plain | 580 | 3605 | 3025 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 2819 | 437 |
| address_indexed_n100_at_mut_end_plain | 580 | 2966 | 2386 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 229 | 25 | 2315 | 302 |
| address_indexed_n100_lookup_hit_end_plain | 230 | 1758 | 1528 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1536 | 14 |
| address_indexed_n100_lookup_miss_end_plain | 230 | 1489 | 1259 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1269 | 14 |
| address_indexed_n100_entry_occupied_end_plain | 580 | 2907 | 2327 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 225 | 25 | 2260 | 302 |
| address_indexed_n100_entry_vacant_end_plain | 580 | 4749 | 4169 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 225 | 25 | 4104 | 302 |
| address_indexed_n100_load_trusted_all_common | 232 | 232 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n100_late_read_all_common | 232 | 3205 | 2973 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 2981 | 14 |
| address_indexed_n100_validate_all_common | 232 | 5496 | 5264 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 5271 | 14 |
| address_indexed_n100_load_all_common | 232 | 5512 | 5280 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 5312 | 7 | 14 |
| address_indexed_n100_mutable_baseline_all_common | 582 | 582 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n100_clear_all_common | 582 | 3841 | 3259 | 4416/16 | 4416/16 | 0 | 0 | 3 | 229 | 25 | 3053 | 437 |
| address_indexed_n100_cursor_first_all_common | 582 | 6556 | 5974 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5903 | 302 |
| address_indexed_n100_for_each_read_all_common | 582 | 27321 | 26739 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 26742 | 228 |
| address_indexed_n100_insert_front_common | 582 | 7678 | 7096 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 229 | 25 | 7027 | 302 |
| address_indexed_n100_replace_front_common | 582 | 6948 | 6366 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 6295 | 302 |
| address_indexed_n100_remove_front_common | 582 | 7826 | 7244 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 7038 | 437 |
| address_indexed_n100_remove_into_front_common | 582 | 7473 | 6891 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 6685 | 437 |
| address_indexed_n100_at_mut_front_common | 582 | 5917 | 5335 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5264 | 302 |
| address_indexed_n100_lookup_hit_front_common | 232 | 3544 | 3312 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 3320 | 14 |
| address_indexed_n100_lookup_miss_front_common | 232 | 3466 | 3234 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 3244 | 14 |
| address_indexed_n100_entry_occupied_front_common | 582 | 5901 | 5319 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 225 | 25 | 5252 | 302 |
| address_indexed_n100_entry_vacant_front_common | 582 | 8742 | 8160 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 225 | 25 | 8095 | 302 |
| address_indexed_n100_insert_middle_common | 582 | 6836 | 6254 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 229 | 25 | 6185 | 302 |
| address_indexed_n100_replace_middle_common | 582 | 5472 | 4890 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 229 | 25 | 4819 | 302 |
| address_indexed_n100_remove_middle_common | 582 | 6283 | 5701 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5495 | 437 |
| address_indexed_n100_remove_into_middle_common | 582 | 5932 | 5350 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5144 | 437 |
| address_indexed_n100_at_mut_middle_common | 582 | 4688 | 4106 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 229 | 25 | 4035 | 302 |
| address_indexed_n100_lookup_hit_middle_common | 232 | 1054 | 822 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 830 | 14 |
| address_indexed_n100_lookup_miss_middle_common | 232 | 3084 | 2852 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 2862 | 14 |
| address_indexed_n100_entry_occupied_middle_common | 582 | 4630 | 4048 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 225 | 25 | 3981 | 302 |
| address_indexed_n100_entry_vacant_middle_common | 582 | 7984 | 7402 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 225 | 25 | 7337 | 302 |
| address_indexed_n100_insert_end_common | 582 | 3712 | 3130 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 229 | 25 | 3061 | 302 |
| address_indexed_n100_replace_end_common | 582 | 3427 | 2845 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 229 | 25 | 2774 | 302 |
| address_indexed_n100_remove_end_common | 582 | 3950 | 3368 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 3162 | 437 |
| address_indexed_n100_remove_into_end_common | 582 | 3607 | 3025 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 2819 | 437 |
| address_indexed_n100_at_mut_end_common | 582 | 2968 | 2386 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 229 | 25 | 2315 | 302 |
| address_indexed_n100_lookup_hit_end_common | 232 | 3205 | 2973 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 2981 | 14 |
| address_indexed_n100_lookup_miss_end_common | 232 | 3514 | 3282 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 3292 | 14 |
| address_indexed_n100_entry_occupied_end_common | 582 | 2909 | 2327 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 225 | 25 | 2260 | 302 |
| address_indexed_n100_entry_vacant_end_common | 582 | 4775 | 4193 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 225 | 25 | 4128 | 302 |
| list_fixed_n1000_load_trusted_all_plain | 219 | 219 | 0 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 34 | 9 | 14 |
| list_fixed_n1000_late_read_all_plain | 219 | 433 | 214 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 34 | 222 | 14 |
| list_fixed_n1000_validate_all_plain | 219 | 492 | 273 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 34 | 280 | 14 |
| list_fixed_n1000_load_all_plain | 219 | 508 | 289 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 321 | 7 | 14 |
| list_fixed_n1000_mutable_baseline_all_plain | 569 | 569 | 0 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 4 | 228 |
| list_fixed_n1000_clear_all_plain | 569 | 4279 | 3710 | 8016/16 | 8016/16 | 0 | 0 | 3 | 219 | 25 | 3504 | 437 |
| list_fixed_n1000_cursor_first_all_plain | 569 | 784 | 215 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 218 | 228 |
| list_fixed_n1000_for_each_read_all_plain | 569 | 11812 | 11243 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 11246 | 228 |
| list_fixed_n1000_insert_front_plain | 569 | 3356 | 2787 | 8016/8024 | 8016/8024 | 8000 | 0 | 3 | 219 | 25 | 2718 | 302 |
| list_fixed_n1000_replace_front_plain | 569 | 1923 | 1354 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n1000_remove_front_plain | 569 | 2369 | 1800 | 8016/8008 | 8016/8008 | 7992 | 0 | 3 | 219 | 25 | 1590 | 437 |
| list_fixed_n1000_remove_into_front_plain | 569 | 3440 | 2871 | 8016/8008 | 8016/8008 | 7992 | 0 | 3 | 219 | 25 | 2665 | 437 |
| list_fixed_n1000_at_mut_front_plain | 569 | 1423 | 854 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n1000_insert_bulk_front_plain | 569 | 3598 | 3029 | 8016/8032 | 8016/8032 | 8000 | 0 | 3 | 219 | 25 | 2958 | 302 |
| list_fixed_n1000_truncate_front_plain | 569 | 4289 | 3720 | 8016/16 | 8016/16 | 0 | 0 | 3 | 219 | 25 | 3514 | 437 |
| list_fixed_n1000_insert_middle_plain | 569 | 2820 | 2251 | 8016/8024 | 8016/8024 | 4000 | 0 | 3 | 219 | 25 | 2182 | 302 |
| list_fixed_n1000_replace_middle_plain | 569 | 1923 | 1354 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n1000_remove_middle_plain | 569 | 1893 | 1324 | 8016/8008 | 8016/8008 | 3992 | 0 | 3 | 219 | 25 | 1114 | 437 |
| list_fixed_n1000_remove_into_middle_plain | 569 | 2964 | 2395 | 8016/8008 | 8016/8008 | 3992 | 0 | 3 | 219 | 25 | 2189 | 437 |
| list_fixed_n1000_at_mut_middle_plain | 569 | 1423 | 854 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n1000_insert_bulk_middle_plain | 569 | 3063 | 2494 | 8016/8032 | 8016/8032 | 4000 | 0 | 3 | 219 | 25 | 2423 | 302 |
| list_fixed_n1000_truncate_middle_plain | 569 | 3249 | 2680 | 8016/4016 | 8016/4016 | 0 | 0 | 3 | 219 | 25 | 2474 | 437 |
| list_fixed_n1000_insert_end_plain | 569 | 2222 | 1653 | 8016/8024 | 8016/8024 | 0 | 0 | 3 | 219 | 25 | 1584 | 302 |
| list_fixed_n1000_replace_end_plain | 569 | 1923 | 1354 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n1000_remove_end_plain | 569 | 1365 | 796 | 8016/8008 | 8016/8008 | 0 | 0 | 3 | 219 | 25 | 586 | 437 |
| list_fixed_n1000_remove_into_end_plain | 569 | 2430 | 1861 | 8016/8008 | 8016/8008 | 0 | 0 | 3 | 219 | 25 | 1655 | 437 |
| list_fixed_n1000_at_mut_end_plain | 569 | 1423 | 854 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n1000_insert_bulk_end_plain | 569 | 2468 | 1899 | 8016/8032 | 8016/8032 | 0 | 0 | 3 | 219 | 25 | 1828 | 302 |
| list_fixed_n1000_truncate_end_plain | 569 | 1903 | 1334 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1337 | 228 |
| list_fixed_n1000_push_end_plain | 569 | 1323 | 754 | 8016/8024 | 8016/8024 | 0 | 0 | 3 | 219 | 25 | 683 | 302 |
| list_fixed_n1000_pop_end_plain | 569 | 2326 | 1757 | 8016/8008 | 8016/8008 | 0 | 0 | 3 | 219 | 25 | 1551 | 437 |
| list_fixed_n1000_extend_end_plain | 569 | 2467 | 1898 | 8016/8032 | 8016/8032 | 0 | 0 | 3 | 219 | 25 | 1827 | 302 |
| list_indexed_n1000_load_trusted_all_plain | 220 | 220 | 0 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| list_indexed_n1000_late_read_all_plain | 220 | 1001 | 781 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 34 | 789 | 14 |
| list_indexed_n1000_validate_all_plain | 220 | 15501 | 15281 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 34 | 15288 | 14 |
| list_indexed_n1000_load_all_plain | 220 | 15517 | 15297 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 15329 | 7 | 14 |
| list_indexed_n1000_mutable_baseline_all_plain | 570 | 570 | 0 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| list_indexed_n1000_clear_all_plain | 570 | 4994 | 4424 | 12012/16 | 12012/16 | 0 | 0 | 3 | 220 | 25 | 4218 | 437 |
| list_indexed_n1000_cursor_first_all_plain | 570 | 23194 | 22624 | 12012/12021 | 12012/12021 | 7992 | 999 | 3 | 220 | 25 | 22553 | 302 |
| list_indexed_n1000_for_each_read_all_plain | 570 | 200063 | 199493 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 220 | 25 | 199496 | 228 |
| list_indexed_n1000_insert_front_plain | 570 | 35749 | 35179 | 12012/12033 | 12012/12033 | 11996 | 1000 | 3 | 220 | 25 | 35110 | 302 |
| list_indexed_n1000_replace_front_plain | 570 | 45930 | 45360 | 12012/12021 | 12012/12021 | 7992 | 999 | 3 | 220 | 25 | 45289 | 302 |
| list_indexed_n1000_remove_front_plain | 570 | 46271 | 45701 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 45491 | 437 |
| list_indexed_n1000_remove_into_front_plain | 570 | 45961 | 45391 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 45185 | 437 |
| list_indexed_n1000_at_mut_front_plain | 570 | 22602 | 22032 | 12012/12021 | 12012/12021 | 7992 | 999 | 3 | 220 | 25 | 21961 | 302 |
| list_indexed_n1000_insert_bulk_front_plain | 570 | 40868 | 40298 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 40227 | 302 |
| list_indexed_n1000_truncate_front_plain | 570 | 4998 | 4428 | 12012/16 | 12012/16 | 0 | 0 | 3 | 220 | 25 | 4222 | 437 |
| list_indexed_n1000_insert_middle_plain | 570 | 19830 | 19260 | 12012/12033 | 12012/12033 | 9996 | 1000 | 3 | 220 | 25 | 19191 | 302 |
| list_indexed_n1000_replace_middle_plain | 570 | 24416 | 23846 | 12012/12021 | 12012/12021 | 3992 | 999 | 3 | 220 | 25 | 23775 | 302 |
| list_indexed_n1000_remove_middle_plain | 570 | 25358 | 24788 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 24578 | 437 |
| list_indexed_n1000_remove_into_middle_plain | 570 | 25048 | 24478 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 24272 | 437 |
| list_indexed_n1000_at_mut_middle_plain | 570 | 12641 | 12071 | 12012/12021 | 12012/12021 | 3992 | 499 | 3 | 220 | 25 | 12000 | 302 |
| list_indexed_n1000_insert_bulk_middle_plain | 570 | 22463 | 21893 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 21822 | 302 |
| list_indexed_n1000_truncate_middle_plain | 570 | 4847 | 4277 | 12012/6012 | 12012/6012 | 4000 | 499 | 3 | 220 | 25 | 4071 | 437 |
| list_indexed_n1000_insert_end_plain | 570 | 3753 | 3183 | 12012/12033 | 12012/12033 | 8000 | 1000 | 3 | 220 | 25 | 3114 | 302 |
| list_indexed_n1000_replace_end_plain | 570 | 2785 | 2215 | 12012/12021 | 12012/12021 | 0 | 999 | 3 | 220 | 25 | 2144 | 302 |
| list_indexed_n1000_remove_end_plain | 570 | 4304 | 3734 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 3524 | 437 |
| list_indexed_n1000_remove_into_end_plain | 570 | 3994 | 3424 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 3218 | 437 |
| list_indexed_n1000_at_mut_end_plain | 570 | 2482 | 1912 | 12012/12021 | 12012/12021 | 0 | 0 | 3 | 220 | 25 | 1841 | 302 |
| list_indexed_n1000_insert_bulk_end_plain | 570 | 3887 | 3317 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 3246 | 302 |
| list_indexed_n1000_truncate_end_plain | 570 | 2405 | 1835 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 220 | 25 | 1838 | 228 |
| list_indexed_n1000_push_end_plain | 570 | 3765 | 3195 | 12012/12033 | 12012/12033 | 8000 | 1000 | 3 | 220 | 25 | 3124 | 302 |
| list_indexed_n1000_pop_end_plain | 570 | 3992 | 3422 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 3216 | 437 |
| list_indexed_n1000_extend_end_plain | 570 | 3886 | 3316 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 3245 | 302 |
| map_fixed_n1000_load_trusted_all_plain | 224 | 224 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n1000_late_read_all_plain | 224 | 1926 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1710 | 14 |
| map_fixed_n1000_validate_all_plain | 224 | 21643 | 21419 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 21426 | 14 |
| map_fixed_n1000_load_all_plain | 224 | 21659 | 21435 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 21467 | 7 | 14 |
| map_fixed_n1000_mutable_baseline_all_plain | 574 | 574 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n1000_clear_all_plain | 574 | 12745 | 12171 | 40016/16 | 40016/16 | 0 | 0 | 3 | 223 | 25 | 11965 | 437 |
| map_fixed_n1000_cursor_first_all_plain | 574 | 1070 | 496 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 499 | 228 |
| map_fixed_n1000_for_each_read_all_plain | 574 | 14105 | 13531 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 13534 | 228 |
| map_fixed_n1000_insert_front_plain | 574 | 7103 | 6529 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 223 | 25 | 6460 | 302 |
| map_fixed_n1000_replace_front_plain | 574 | 3227 | 2653 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2656 | 228 |
| map_fixed_n1000_remove_front_plain | 574 | 6584 | 6010 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 5804 | 437 |
| map_fixed_n1000_remove_into_front_plain | 574 | 7773 | 7199 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 6993 | 437 |
| map_fixed_n1000_at_mut_front_plain | 574 | 1785 | 1211 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1214 | 228 |
| map_fixed_n1000_lookup_hit_front_plain | 224 | 2005 | 1781 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1789 | 14 |
| map_fixed_n1000_lookup_miss_front_plain | 224 | 1857 | 1633 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1643 | 14 |
| map_fixed_n1000_entry_occupied_front_plain | 574 | 2684 | 2110 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2117 | 228 |
| map_fixed_n1000_entry_vacant_front_plain | 574 | 9605 | 9031 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 219 | 25 | 8966 | 302 |
| map_fixed_n1000_insert_middle_plain | 574 | 5386 | 4812 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 223 | 25 | 4743 | 302 |
| map_fixed_n1000_replace_middle_plain | 574 | 3429 | 2855 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2858 | 228 |
| map_fixed_n1000_remove_middle_plain | 574 | 4305 | 3731 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 3525 | 437 |
| map_fixed_n1000_remove_into_middle_plain | 574 | 5494 | 4920 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 4714 | 437 |
| map_fixed_n1000_at_mut_middle_plain | 574 | 1886 | 1312 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1315 | 228 |
| map_fixed_n1000_lookup_hit_middle_plain | 224 | 828 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n1000_lookup_miss_middle_plain | 224 | 1788 | 1564 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1574 | 14 |
| map_fixed_n1000_entry_occupied_middle_plain | 574 | 2785 | 2211 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2218 | 228 |
| map_fixed_n1000_entry_vacant_middle_plain | 574 | 7959 | 7385 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 219 | 25 | 7320 | 302 |
| map_fixed_n1000_insert_end_plain | 574 | 1699 | 1125 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 223 | 25 | 1056 | 302 |
| map_fixed_n1000_replace_end_plain | 574 | 3155 | 2581 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2584 | 228 |
| map_fixed_n1000_remove_end_plain | 574 | 1736 | 1162 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 956 | 437 |
| map_fixed_n1000_remove_into_end_plain | 574 | 2919 | 2345 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 2139 | 437 |
| map_fixed_n1000_at_mut_end_plain | 574 | 1749 | 1175 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1178 | 228 |
| map_fixed_n1000_lookup_hit_end_plain | 224 | 1926 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1710 | 14 |
| map_fixed_n1000_lookup_miss_end_plain | 224 | 1919 | 1695 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1705 | 14 |
| map_fixed_n1000_entry_occupied_end_plain | 574 | 2648 | 2074 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2081 | 228 |
| map_fixed_n1000_entry_vacant_end_plain | 574 | 4148 | 3574 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 219 | 25 | 3509 | 302 |
| map_fixed_n1000_load_trusted_all_common | 226 | 226 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n1000_late_read_all_common | 226 | 4240 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4022 | 14 |
| map_fixed_n1000_validate_all_common | 226 | 30636 | 30410 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 30417 | 14 |
| map_fixed_n1000_load_all_common | 226 | 30652 | 30426 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 30458 | 7 | 14 |
| map_fixed_n1000_mutable_baseline_all_common | 576 | 576 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n1000_clear_all_common | 576 | 12747 | 12171 | 40016/16 | 40016/16 | 0 | 0 | 3 | 223 | 25 | 11965 | 437 |
| map_fixed_n1000_cursor_first_all_common | 576 | 1072 | 496 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 499 | 228 |
| map_fixed_n1000_for_each_read_all_common | 576 | 14107 | 13531 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 13534 | 228 |
| map_fixed_n1000_insert_front_common | 576 | 7129 | 6553 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 223 | 25 | 6484 | 302 |
| map_fixed_n1000_replace_front_common | 576 | 3253 | 2677 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2680 | 228 |
| map_fixed_n1000_remove_front_common | 576 | 6598 | 6022 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 5816 | 437 |
| map_fixed_n1000_remove_into_front_common | 576 | 7787 | 7211 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 7005 | 437 |
| map_fixed_n1000_at_mut_front_common | 576 | 1799 | 1223 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1226 | 228 |
| map_fixed_n1000_lookup_hit_front_common | 226 | 4608 | 4382 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4390 | 14 |
| map_fixed_n1000_lookup_miss_front_common | 226 | 4749 | 4523 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4533 | 14 |
| map_fixed_n1000_entry_occupied_front_common | 576 | 2698 | 2122 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2129 | 228 |
| map_fixed_n1000_entry_vacant_front_common | 576 | 9646 | 9070 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 219 | 25 | 9005 | 302 |
| map_fixed_n1000_insert_middle_common | 576 | 5544 | 4968 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 223 | 25 | 4899 | 302 |
| map_fixed_n1000_replace_middle_common | 576 | 3479 | 2903 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2906 | 228 |
| map_fixed_n1000_remove_middle_common | 576 | 4331 | 3755 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 3549 | 437 |
| map_fixed_n1000_remove_into_middle_common | 576 | 5520 | 4944 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 4738 | 437 |
| map_fixed_n1000_at_mut_middle_common | 576 | 1912 | 1336 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1339 | 228 |
| map_fixed_n1000_lookup_hit_middle_common | 226 | 830 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n1000_lookup_miss_middle_common | 226 | 4391 | 4165 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4175 | 14 |
| map_fixed_n1000_entry_occupied_middle_common | 576 | 2811 | 2235 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2242 | 228 |
| map_fixed_n1000_entry_vacant_middle_common | 576 | 8144 | 7568 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 219 | 25 | 7503 | 302 |
| map_fixed_n1000_insert_end_common | 576 | 1713 | 1137 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 223 | 25 | 1068 | 302 |
| map_fixed_n1000_replace_end_common | 576 | 3157 | 2581 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2584 | 228 |
| map_fixed_n1000_remove_end_common | 576 | 1738 | 1162 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 956 | 437 |
| map_fixed_n1000_remove_into_end_common | 576 | 2921 | 2345 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 2139 | 437 |
| map_fixed_n1000_at_mut_end_common | 576 | 1751 | 1175 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1178 | 228 |
| map_fixed_n1000_lookup_hit_end_common | 226 | 4240 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4022 | 14 |
| map_fixed_n1000_lookup_miss_end_common | 226 | 4811 | 4585 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4595 | 14 |
| map_fixed_n1000_entry_occupied_end_common | 576 | 2650 | 2074 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2081 | 228 |
| map_fixed_n1000_entry_vacant_end_common | 576 | 4174 | 3598 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 219 | 25 | 3533 | 302 |
| map_indexed_n1000_load_trusted_all_plain | 222 | 222 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n1000_late_read_all_plain | 222 | 2143 | 1921 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1929 | 14 |
| map_indexed_n1000_validate_all_plain | 222 | 38570 | 38348 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 38355 | 14 |
| map_indexed_n1000_load_all_plain | 222 | 38586 | 38364 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 38396 | 7 | 14 |
| map_indexed_n1000_mutable_baseline_all_plain | 572 | 572 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n1000_clear_all_plain | 572 | 14031 | 13459 | 44016/16 | 44016/16 | 0 | 0 | 3 | 220 | 25 | 13253 | 437 |
| map_indexed_n1000_cursor_first_all_plain | 572 | 32684 | 32112 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 32041 | 302 |
| map_indexed_n1000_for_each_read_all_plain | 572 | 253220 | 252648 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 252651 | 228 |
| map_indexed_n1000_insert_front_plain | 572 | 42570 | 41998 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 220 | 25 | 41929 | 302 |
| map_indexed_n1000_replace_front_plain | 572 | 37564 | 36992 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 36921 | 302 |
| map_indexed_n1000_remove_front_plain | 572 | 42145 | 41573 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41367 | 437 |
| map_indexed_n1000_remove_into_front_plain | 572 | 41792 | 41220 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41014 | 437 |
| map_indexed_n1000_at_mut_front_plain | 572 | 32033 | 31461 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 31390 | 302 |
| map_indexed_n1000_lookup_hit_front_plain | 222 | 2179 | 1957 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1965 | 14 |
| map_indexed_n1000_lookup_miss_front_plain | 222 | 1812 | 1590 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1600 | 14 |
| map_indexed_n1000_entry_occupied_front_plain | 572 | 32017 | 31445 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 216 | 25 | 31378 | 302 |
| map_indexed_n1000_entry_vacant_front_plain | 572 | 43619 | 43047 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 216 | 25 | 42982 | 302 |
| map_indexed_n1000_insert_middle_plain | 572 | 24753 | 24181 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 220 | 25 | 24112 | 302 |
| map_indexed_n1000_replace_middle_plain | 572 | 20690 | 20118 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 220 | 25 | 20047 | 302 |
| map_indexed_n1000_remove_middle_plain | 572 | 23717 | 23145 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22939 | 437 |
| map_indexed_n1000_remove_into_middle_plain | 572 | 23366 | 22794 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22588 | 437 |
| map_indexed_n1000_at_mut_middle_plain | 572 | 17656 | 17084 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 220 | 25 | 17013 | 302 |
| map_indexed_n1000_lookup_hit_middle_plain | 222 | 1044 | 822 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 830 | 14 |
| map_indexed_n1000_lookup_miss_middle_plain | 222 | 1743 | 1521 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1531 | 14 |
| map_indexed_n1000_entry_occupied_middle_plain | 572 | 17598 | 17026 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 216 | 25 | 16959 | 302 |
| map_indexed_n1000_entry_vacant_middle_plain | 572 | 25874 | 25302 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 216 | 25 | 25237 | 302 |
| map_indexed_n1000_insert_end_plain | 572 | 4628 | 4056 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 220 | 25 | 3987 | 302 |
| map_indexed_n1000_replace_end_plain | 572 | 3417 | 2845 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 220 | 25 | 2774 | 302 |
| map_indexed_n1000_remove_end_plain | 572 | 4773 | 4201 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3995 | 437 |
| map_indexed_n1000_remove_into_end_plain | 572 | 4430 | 3858 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3652 | 437 |
| map_indexed_n1000_at_mut_end_plain | 572 | 2958 | 2386 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 220 | 25 | 2315 | 302 |
| map_indexed_n1000_lookup_hit_end_plain | 222 | 2143 | 1921 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1929 | 14 |
| map_indexed_n1000_lookup_miss_end_plain | 222 | 1874 | 1652 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1662 | 14 |
| map_indexed_n1000_entry_occupied_end_plain | 572 | 2899 | 2327 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 216 | 25 | 2260 | 302 |
| map_indexed_n1000_entry_vacant_end_plain | 572 | 5679 | 5107 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 216 | 25 | 5042 | 302 |
| map_indexed_n1000_load_trusted_all_common | 224 | 224 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n1000_late_read_all_common | 224 | 4457 | 4233 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4241 | 14 |
| map_indexed_n1000_validate_all_common | 224 | 47563 | 47339 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 47346 | 14 |
| map_indexed_n1000_load_all_common | 224 | 47579 | 47355 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 47387 | 7 | 14 |
| map_indexed_n1000_mutable_baseline_all_common | 574 | 574 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n1000_clear_all_common | 574 | 14033 | 13459 | 44016/16 | 44016/16 | 0 | 0 | 3 | 220 | 25 | 13253 | 437 |
| map_indexed_n1000_cursor_first_all_common | 574 | 32686 | 32112 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 32041 | 302 |
| map_indexed_n1000_for_each_read_all_common | 574 | 262213 | 261639 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 261642 | 228 |
| map_indexed_n1000_insert_front_common | 574 | 42596 | 42022 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 220 | 25 | 41953 | 302 |
| map_indexed_n1000_replace_front_common | 574 | 37578 | 37004 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 36933 | 302 |
| map_indexed_n1000_remove_front_common | 574 | 42159 | 41585 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41379 | 437 |
| map_indexed_n1000_remove_into_front_common | 574 | 41806 | 41232 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41026 | 437 |
| map_indexed_n1000_at_mut_front_common | 574 | 32047 | 31473 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 31402 | 302 |
| map_indexed_n1000_lookup_hit_front_common | 224 | 4782 | 4558 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4566 | 14 |
| map_indexed_n1000_lookup_miss_front_common | 224 | 4704 | 4480 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4490 | 14 |
| map_indexed_n1000_entry_occupied_front_common | 574 | 32031 | 31457 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 216 | 25 | 31390 | 302 |
| map_indexed_n1000_entry_vacant_front_common | 574 | 43660 | 43086 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 216 | 25 | 43021 | 302 |
| map_indexed_n1000_insert_middle_common | 574 | 24911 | 24337 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 220 | 25 | 24268 | 302 |
| map_indexed_n1000_replace_middle_common | 574 | 20716 | 20142 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 220 | 25 | 20071 | 302 |
| map_indexed_n1000_remove_middle_common | 574 | 23743 | 23169 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22963 | 437 |
| map_indexed_n1000_remove_into_middle_common | 574 | 23392 | 22818 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22612 | 437 |
| map_indexed_n1000_at_mut_middle_common | 574 | 17682 | 17108 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 220 | 25 | 17037 | 302 |
| map_indexed_n1000_lookup_hit_middle_common | 224 | 1046 | 822 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 830 | 14 |
| map_indexed_n1000_lookup_miss_middle_common | 224 | 4346 | 4122 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4132 | 14 |
| map_indexed_n1000_entry_occupied_middle_common | 574 | 17624 | 17050 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 216 | 25 | 16983 | 302 |
| map_indexed_n1000_entry_vacant_middle_common | 574 | 26059 | 25485 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 216 | 25 | 25420 | 302 |
| map_indexed_n1000_insert_end_common | 574 | 4642 | 4068 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 220 | 25 | 3999 | 302 |
| map_indexed_n1000_replace_end_common | 574 | 3419 | 2845 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 220 | 25 | 2774 | 302 |
| map_indexed_n1000_remove_end_common | 574 | 4775 | 4201 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3995 | 437 |
| map_indexed_n1000_remove_into_end_common | 574 | 4432 | 3858 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3652 | 437 |
| map_indexed_n1000_at_mut_end_common | 574 | 2960 | 2386 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 220 | 25 | 2315 | 302 |
| map_indexed_n1000_lookup_hit_end_common | 224 | 4457 | 4233 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4241 | 14 |
| map_indexed_n1000_lookup_miss_end_common | 224 | 4766 | 4542 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4552 | 14 |
| map_indexed_n1000_entry_occupied_end_common | 574 | 2901 | 2327 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 216 | 25 | 2260 | 302 |
| map_indexed_n1000_entry_vacant_end_common | 574 | 5705 | 5131 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 216 | 25 | 5066 | 302 |
| address_fixed_n1000_load_trusted_all_plain | 229 | 229 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n1000_late_read_all_plain | 229 | 1931 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1710 | 14 |
| address_fixed_n1000_validate_all_plain | 229 | 21648 | 21419 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 21426 | 14 |
| address_fixed_n1000_load_all_plain | 229 | 21664 | 21435 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 21467 | 7 | 14 |
| address_fixed_n1000_mutable_baseline_all_plain | 579 | 579 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n1000_clear_all_plain | 579 | 12750 | 12171 | 40016/16 | 40016/16 | 0 | 0 | 3 | 228 | 25 | 11965 | 437 |
| address_fixed_n1000_cursor_first_all_plain | 579 | 1075 | 496 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 499 | 228 |
| address_fixed_n1000_for_each_read_all_plain | 579 | 14110 | 13531 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 13534 | 228 |
| address_fixed_n1000_insert_front_plain | 579 | 7108 | 6529 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 228 | 25 | 6460 | 302 |
| address_fixed_n1000_replace_front_plain | 579 | 3232 | 2653 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2656 | 228 |
| address_fixed_n1000_remove_front_plain | 579 | 6589 | 6010 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 5804 | 437 |
| address_fixed_n1000_remove_into_front_plain | 579 | 7778 | 7199 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 6993 | 437 |
| address_fixed_n1000_at_mut_front_plain | 579 | 1790 | 1211 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1214 | 228 |
| address_fixed_n1000_lookup_hit_front_plain | 229 | 2010 | 1781 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1789 | 14 |
| address_fixed_n1000_lookup_miss_front_plain | 229 | 1862 | 1633 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1643 | 14 |
| address_fixed_n1000_entry_occupied_front_plain | 579 | 2689 | 2110 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2117 | 228 |
| address_fixed_n1000_entry_vacant_front_plain | 579 | 9610 | 9031 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 224 | 25 | 8966 | 302 |
| address_fixed_n1000_insert_middle_plain | 579 | 5391 | 4812 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 228 | 25 | 4743 | 302 |
| address_fixed_n1000_replace_middle_plain | 579 | 3434 | 2855 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2858 | 228 |
| address_fixed_n1000_remove_middle_plain | 579 | 4310 | 3731 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 3525 | 437 |
| address_fixed_n1000_remove_into_middle_plain | 579 | 5499 | 4920 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 4714 | 437 |
| address_fixed_n1000_at_mut_middle_plain | 579 | 1891 | 1312 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1315 | 228 |
| address_fixed_n1000_lookup_hit_middle_plain | 229 | 833 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n1000_lookup_miss_middle_plain | 229 | 1793 | 1564 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1574 | 14 |
| address_fixed_n1000_entry_occupied_middle_plain | 579 | 2790 | 2211 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2218 | 228 |
| address_fixed_n1000_entry_vacant_middle_plain | 579 | 7964 | 7385 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 224 | 25 | 7320 | 302 |
| address_fixed_n1000_insert_end_plain | 579 | 1704 | 1125 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 228 | 25 | 1056 | 302 |
| address_fixed_n1000_replace_end_plain | 579 | 3160 | 2581 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2584 | 228 |
| address_fixed_n1000_remove_end_plain | 579 | 1741 | 1162 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 956 | 437 |
| address_fixed_n1000_remove_into_end_plain | 579 | 2924 | 2345 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 2139 | 437 |
| address_fixed_n1000_at_mut_end_plain | 579 | 1754 | 1175 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1178 | 228 |
| address_fixed_n1000_lookup_hit_end_plain | 229 | 1931 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1710 | 14 |
| address_fixed_n1000_lookup_miss_end_plain | 229 | 1924 | 1695 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1705 | 14 |
| address_fixed_n1000_entry_occupied_end_plain | 579 | 2653 | 2074 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2081 | 228 |
| address_fixed_n1000_entry_vacant_end_plain | 579 | 4153 | 3574 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 224 | 25 | 3509 | 302 |
| address_fixed_n1000_load_trusted_all_common | 231 | 231 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n1000_late_read_all_common | 231 | 4245 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4022 | 14 |
| address_fixed_n1000_validate_all_common | 231 | 30641 | 30410 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 30417 | 14 |
| address_fixed_n1000_load_all_common | 231 | 30657 | 30426 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 30458 | 7 | 14 |
| address_fixed_n1000_mutable_baseline_all_common | 581 | 581 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n1000_clear_all_common | 581 | 12752 | 12171 | 40016/16 | 40016/16 | 0 | 0 | 3 | 228 | 25 | 11965 | 437 |
| address_fixed_n1000_cursor_first_all_common | 581 | 1077 | 496 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 499 | 228 |
| address_fixed_n1000_for_each_read_all_common | 581 | 14112 | 13531 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 13534 | 228 |
| address_fixed_n1000_insert_front_common | 581 | 7134 | 6553 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 228 | 25 | 6484 | 302 |
| address_fixed_n1000_replace_front_common | 581 | 3258 | 2677 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2680 | 228 |
| address_fixed_n1000_remove_front_common | 581 | 6603 | 6022 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 5816 | 437 |
| address_fixed_n1000_remove_into_front_common | 581 | 7792 | 7211 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 7005 | 437 |
| address_fixed_n1000_at_mut_front_common | 581 | 1804 | 1223 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1226 | 228 |
| address_fixed_n1000_lookup_hit_front_common | 231 | 4613 | 4382 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4390 | 14 |
| address_fixed_n1000_lookup_miss_front_common | 231 | 4754 | 4523 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4533 | 14 |
| address_fixed_n1000_entry_occupied_front_common | 581 | 2703 | 2122 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2129 | 228 |
| address_fixed_n1000_entry_vacant_front_common | 581 | 9651 | 9070 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 224 | 25 | 9005 | 302 |
| address_fixed_n1000_insert_middle_common | 581 | 5549 | 4968 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 228 | 25 | 4899 | 302 |
| address_fixed_n1000_replace_middle_common | 581 | 3484 | 2903 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2906 | 228 |
| address_fixed_n1000_remove_middle_common | 581 | 4336 | 3755 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 3549 | 437 |
| address_fixed_n1000_remove_into_middle_common | 581 | 5525 | 4944 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 4738 | 437 |
| address_fixed_n1000_at_mut_middle_common | 581 | 1917 | 1336 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1339 | 228 |
| address_fixed_n1000_lookup_hit_middle_common | 231 | 835 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n1000_lookup_miss_middle_common | 231 | 4396 | 4165 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4175 | 14 |
| address_fixed_n1000_entry_occupied_middle_common | 581 | 2816 | 2235 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2242 | 228 |
| address_fixed_n1000_entry_vacant_middle_common | 581 | 8149 | 7568 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 224 | 25 | 7503 | 302 |
| address_fixed_n1000_insert_end_common | 581 | 1718 | 1137 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 228 | 25 | 1068 | 302 |
| address_fixed_n1000_replace_end_common | 581 | 3162 | 2581 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2584 | 228 |
| address_fixed_n1000_remove_end_common | 581 | 1743 | 1162 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 956 | 437 |
| address_fixed_n1000_remove_into_end_common | 581 | 2926 | 2345 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 2139 | 437 |
| address_fixed_n1000_at_mut_end_common | 581 | 1756 | 1175 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1178 | 228 |
| address_fixed_n1000_lookup_hit_end_common | 231 | 4245 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4022 | 14 |
| address_fixed_n1000_lookup_miss_end_common | 231 | 4816 | 4585 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4595 | 14 |
| address_fixed_n1000_entry_occupied_end_common | 581 | 2655 | 2074 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2081 | 228 |
| address_fixed_n1000_entry_vacant_end_common | 581 | 4179 | 3598 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 224 | 25 | 3533 | 302 |
| address_indexed_n1000_load_trusted_all_plain | 230 | 230 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n1000_late_read_all_plain | 230 | 2151 | 1921 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1929 | 14 |
| address_indexed_n1000_validate_all_plain | 230 | 38578 | 38348 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 38355 | 14 |
| address_indexed_n1000_load_all_plain | 230 | 38594 | 38364 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 38396 | 7 | 14 |
| address_indexed_n1000_mutable_baseline_all_plain | 580 | 580 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n1000_clear_all_plain | 580 | 14039 | 13459 | 44016/16 | 44016/16 | 0 | 0 | 3 | 229 | 25 | 13253 | 437 |
| address_indexed_n1000_cursor_first_all_plain | 580 | 32692 | 32112 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 32041 | 302 |
| address_indexed_n1000_for_each_read_all_plain | 580 | 253228 | 252648 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 252651 | 228 |
| address_indexed_n1000_insert_front_plain | 580 | 42578 | 41998 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 229 | 25 | 41929 | 302 |
| address_indexed_n1000_replace_front_plain | 580 | 37572 | 36992 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 36921 | 302 |
| address_indexed_n1000_remove_front_plain | 580 | 42153 | 41573 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41367 | 437 |
| address_indexed_n1000_remove_into_front_plain | 580 | 41800 | 41220 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41014 | 437 |
| address_indexed_n1000_at_mut_front_plain | 580 | 32041 | 31461 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 31390 | 302 |
| address_indexed_n1000_lookup_hit_front_plain | 230 | 2187 | 1957 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1965 | 14 |
| address_indexed_n1000_lookup_miss_front_plain | 230 | 1820 | 1590 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1600 | 14 |
| address_indexed_n1000_entry_occupied_front_plain | 580 | 32025 | 31445 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 225 | 25 | 31378 | 302 |
| address_indexed_n1000_entry_vacant_front_plain | 580 | 43627 | 43047 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 225 | 25 | 42982 | 302 |
| address_indexed_n1000_insert_middle_plain | 580 | 24761 | 24181 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 229 | 25 | 24112 | 302 |
| address_indexed_n1000_replace_middle_plain | 580 | 20698 | 20118 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 229 | 25 | 20047 | 302 |
| address_indexed_n1000_remove_middle_plain | 580 | 23725 | 23145 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22939 | 437 |
| address_indexed_n1000_remove_into_middle_plain | 580 | 23374 | 22794 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22588 | 437 |
| address_indexed_n1000_at_mut_middle_plain | 580 | 17664 | 17084 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 229 | 25 | 17013 | 302 |
| address_indexed_n1000_lookup_hit_middle_plain | 230 | 1052 | 822 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 830 | 14 |
| address_indexed_n1000_lookup_miss_middle_plain | 230 | 1751 | 1521 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1531 | 14 |
| address_indexed_n1000_entry_occupied_middle_plain | 580 | 17606 | 17026 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 225 | 25 | 16959 | 302 |
| address_indexed_n1000_entry_vacant_middle_plain | 580 | 25882 | 25302 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 225 | 25 | 25237 | 302 |
| address_indexed_n1000_insert_end_plain | 580 | 4636 | 4056 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 229 | 25 | 3987 | 302 |
| address_indexed_n1000_replace_end_plain | 580 | 3425 | 2845 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 229 | 25 | 2774 | 302 |
| address_indexed_n1000_remove_end_plain | 580 | 4781 | 4201 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3995 | 437 |
| address_indexed_n1000_remove_into_end_plain | 580 | 4438 | 3858 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3652 | 437 |
| address_indexed_n1000_at_mut_end_plain | 580 | 2966 | 2386 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 229 | 25 | 2315 | 302 |
| address_indexed_n1000_lookup_hit_end_plain | 230 | 2151 | 1921 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1929 | 14 |
| address_indexed_n1000_lookup_miss_end_plain | 230 | 1882 | 1652 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1662 | 14 |
| address_indexed_n1000_entry_occupied_end_plain | 580 | 2907 | 2327 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 225 | 25 | 2260 | 302 |
| address_indexed_n1000_entry_vacant_end_plain | 580 | 5687 | 5107 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 225 | 25 | 5042 | 302 |
| address_indexed_n1000_load_trusted_all_common | 232 | 232 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n1000_late_read_all_common | 232 | 4465 | 4233 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4241 | 14 |
| address_indexed_n1000_validate_all_common | 232 | 47571 | 47339 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 47346 | 14 |
| address_indexed_n1000_load_all_common | 232 | 47587 | 47355 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 47387 | 7 | 14 |
| address_indexed_n1000_mutable_baseline_all_common | 582 | 582 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n1000_clear_all_common | 582 | 14041 | 13459 | 44016/16 | 44016/16 | 0 | 0 | 3 | 229 | 25 | 13253 | 437 |
| address_indexed_n1000_cursor_first_all_common | 582 | 32694 | 32112 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 32041 | 302 |
| address_indexed_n1000_for_each_read_all_common | 582 | 262221 | 261639 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 261642 | 228 |
| address_indexed_n1000_insert_front_common | 582 | 42604 | 42022 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 229 | 25 | 41953 | 302 |
| address_indexed_n1000_replace_front_common | 582 | 37586 | 37004 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 36933 | 302 |
| address_indexed_n1000_remove_front_common | 582 | 42167 | 41585 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41379 | 437 |
| address_indexed_n1000_remove_into_front_common | 582 | 41814 | 41232 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41026 | 437 |
| address_indexed_n1000_at_mut_front_common | 582 | 32055 | 31473 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 31402 | 302 |
| address_indexed_n1000_lookup_hit_front_common | 232 | 4790 | 4558 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4566 | 14 |
| address_indexed_n1000_lookup_miss_front_common | 232 | 4712 | 4480 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4490 | 14 |
| address_indexed_n1000_entry_occupied_front_common | 582 | 32039 | 31457 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 225 | 25 | 31390 | 302 |
| address_indexed_n1000_entry_vacant_front_common | 582 | 43668 | 43086 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 225 | 25 | 43021 | 302 |
| address_indexed_n1000_insert_middle_common | 582 | 24919 | 24337 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 229 | 25 | 24268 | 302 |
| address_indexed_n1000_replace_middle_common | 582 | 20724 | 20142 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 229 | 25 | 20071 | 302 |
| address_indexed_n1000_remove_middle_common | 582 | 23751 | 23169 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22963 | 437 |
| address_indexed_n1000_remove_into_middle_common | 582 | 23400 | 22818 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22612 | 437 |
| address_indexed_n1000_at_mut_middle_common | 582 | 17690 | 17108 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 229 | 25 | 17037 | 302 |
| address_indexed_n1000_lookup_hit_middle_common | 232 | 1054 | 822 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 830 | 14 |
| address_indexed_n1000_lookup_miss_middle_common | 232 | 4354 | 4122 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4132 | 14 |
| address_indexed_n1000_entry_occupied_middle_common | 582 | 17632 | 17050 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 225 | 25 | 16983 | 302 |
| address_indexed_n1000_entry_vacant_middle_common | 582 | 26067 | 25485 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 225 | 25 | 25420 | 302 |
| address_indexed_n1000_insert_end_common | 582 | 4650 | 4068 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 229 | 25 | 3999 | 302 |
| address_indexed_n1000_replace_end_common | 582 | 3427 | 2845 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 229 | 25 | 2774 | 302 |
| address_indexed_n1000_remove_end_common | 582 | 4783 | 4201 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3995 | 437 |
| address_indexed_n1000_remove_into_end_common | 582 | 4440 | 3858 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3652 | 437 |
| address_indexed_n1000_at_mut_end_common | 582 | 2968 | 2386 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 229 | 25 | 2315 | 302 |
| address_indexed_n1000_lookup_hit_end_common | 232 | 4465 | 4233 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4241 | 14 |
| address_indexed_n1000_lookup_miss_end_common | 232 | 4774 | 4542 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4552 | 14 |
| address_indexed_n1000_entry_occupied_end_common | 582 | 2909 | 2327 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 225 | 25 | 2260 | 302 |
| address_indexed_n1000_entry_vacant_end_common | 582 | 5713 | 5131 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 225 | 25 | 5066 | 302 |
| depth1_mutable_baseline | 654 | 654 | 0 | 20/20 | 20/20 | 0 | 0 | 1 | 205 | 196 | 45 | 107 |
| depth1_string_set_grow | 654 | 1208 | 554 | 20/29 | 20/29 | 0 | 0 | 1 | 205 | 196 | 530 | 178 |
| depth1_string_set_shrink | 654 | 1220 | 566 | 20/13 | 20/13 | 0 | 0 | 1 | 205 | 196 | 405 | 313 |
| depth1_string_push | 654 | 1757 | 1103 | 20/37 | 20/37 | 0 | 0 | 1 | 205 | 196 | 1077 | 178 |
| depth1_string_truncate | 654 | 1715 | 1061 | 20/13 | 20/13 | 0 | 0 | 1 | 205 | 196 | 900 | 313 |
| depth1_string_clear | 654 | 1220 | 566 | 20/12 | 20/12 | 0 | 0 | 1 | 205 | 196 | 405 | 313 |
| depth4_mutable_baseline | 2986 | 2986 | 0 | 76/76 | 76/76 | 0 | 0 | 4 | 207 | 24 | 2426 | 228 |
| depth4_string_set_grow | 2986 | 4801 | 1815 | 76/85 | 76/85 | 36 | 3 | 4 | 207 | 24 | 4169 | 302 |
| depth4_string_set_shrink | 2986 | 4798 | 1812 | 76/69 | 76/69 | 36 | 3 | 4 | 207 | 24 | 4029 | 437 |
| depth4_string_push | 2986 | 5384 | 2398 | 76/93 | 76/93 | 36 | 3 | 4 | 207 | 24 | 4750 | 302 |
| depth4_string_truncate | 2986 | 5325 | 2339 | 76/69 | 76/69 | 36 | 3 | 4 | 207 | 24 | 4556 | 437 |
| depth4_string_clear | 2986 | 4798 | 1812 | 76/68 | 76/68 | 36 | 3 | 4 | 207 | 24 | 4029 | 437 |
| depth8_mutable_baseline | 10479 | 10479 | 0 | 1036/1036 | 1036/1036 | 0 | 0 | 8 | 206 | 24 | 9920 | 228 |
| depth8_string_set_grow | 10479 | 13726 | 3247 | 1036/1045 | 1036/1045 | 964 | 7 | 8 | 206 | 24 | 13095 | 302 |
| depth8_string_set_shrink | 10479 | 13723 | 3244 | 1036/1029 | 1036/1029 | 964 | 7 | 8 | 206 | 24 | 12955 | 437 |
| depth8_string_push | 10479 | 14373 | 3894 | 1036/1053 | 1036/1053 | 964 | 7 | 8 | 206 | 24 | 13740 | 302 |
| depth8_string_truncate | 10479 | 14314 | 3835 | 1036/1029 | 1036/1029 | 964 | 7 | 8 | 206 | 24 | 13546 | 437 |
| depth8_string_clear | 10479 | 13723 | 3244 | 1036/1028 | 1036/1028 | 964 | 7 | 8 | 206 | 24 | 12955 | 437 |
| depth3_option_set | 1518 | 2814 | 1296 | 21/13 | 21/13 | 0 | 0 | 3 | 207 | 968 | 1097 | 442 |
| depth3_option_some | 1518 | 3114 | 1596 | 21/30 | 21/30 | 0 | 0 | 3 | 207 | 968 | 1532 | 307 |

| Raw lease (n / mode / action / funding) | Baseline | Whole | Difference | Used | Physical before/after | Opened | Peak | Re-leases | Zeroed | CPIs | TopUp | Refund | Open | Load | Work | Commit | Open probes | Work resizes | Commit resizes | Slot bytes |
|---|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lease_n10_Max_same_prepaid | 569 | 569 | 0 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n10_Max_same_topup | 623 | 623 | 0 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n10_Max_same_refund | 893 | 893 | 0 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 99998468800 | 211 | 0 | 142 | 377 | 1 | 0 | 1 | 0 |
| lease_n10_Max_same_retain32 | 2152 | 2152 | 0 | 92 | 92/124 | 10332 | 10332 | 0 | 10240 | 1 | 222720 | 0 | 268 | 0 | 291 | 1433 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_prepaid | 569 | 852 | 283 | 100 | 92/100 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 291 | 196 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_topup | 623 | 2155 | 1532 | 100 | 92/100 | 10332 | 10332 | 0 | 10240 | 1 | 55680 | 0 | 268 | 0 | 302 | 1434 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_refund | 893 | 962 | 69 | 100 | 92/100 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 99998413120 | 211 | 0 | 362 | 235 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_retain32 | 2152 | 2237 | 85 | 100 | 92/132 | 10332 | 10332 | 0 | 10240 | 1 | 278400 | 0 | 268 | 0 | 385 | 1433 | 1 | 0 | 1 | 0 |
| lease_n10_Max_fifty_pushes_prepaid | 569 | 5132 | 4563 | 492 | 92/492 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 4564 | 196 | 1 | 0 | 1 | 0 |
| lease_n10_Max_after_grow501_prepaid | 569 | 2048 | 1479 | 593 | 92/593 | 10332 | 10332 | 0 | 9739 | 0 | 0 | 0 | 1111 | 0 | 73 | 124 | 15 | 0 | 1 | 0 |
| lease_n10_Max_after_shrink37_prepaid | 569 | 1110 | 541 | 55 | 92/55 | 10295 | 10295 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 73 | 122 | 1 | 0 | 1 | 0 |
| lease_n10_Max_two_cycles_prepaid | 569 | 860 | 291 | 92 | 92/92 | 10332 | 10332 | 0 | 20480 | 0 | 0 | 0 | 211 | 0 | 73 | 414 | 1 | 0 | 3 | 0 |
| lease_n10_Max_readonly_prepaid | 569 | 210 | -359 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 41 | 0 | 4 | 6 | 0 | 0 | 0 | 0 |
| lease_n10_Max_shrink64_prepaid | 569 | 822 | 253 | 64 | 92/64 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 118 | 331 | 1 | 0 | 1 | 0 |
| lease_n10_Max_shrink64_topup | 623 | 939 | 316 | 64 | 92/64 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 118 | 394 | 1 | 0 | 1 | 0 |
| lease_n10_Max_shrink64_refund | 893 | 932 | 39 | 64 | 92/64 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 99998663680 | 211 | 0 | 189 | 370 | 1 | 0 | 1 | 0 |
| lease_n10_Max_shrink64_retain32 | 2152 | 2198 | 46 | 64 | 92/96 | 10332 | 10332 | 0 | 10240 | 1 | 27840 | 0 | 268 | 0 | 338 | 1433 | 1 | 0 | 1 | 0 |
| lease_n10_Max_exact_window_prepaid | 569 | 825 | 256 | 10332 | 92/10332 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 292 | 160 | 1 | 0 | 0 | 0 |
| lease_n10_Max_failed_growth_prepaid | 569 | 619 | 50 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 122 | 124 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo0_same_prepaid | 457 | 457 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 71 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_same_topup | 511 | 511 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 194 | 0 | 71 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_same_refund | 781 | 781 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 99998468800 | 137 | 0 | 142 | 341 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_after_grow501_prepaid | 457 | 1036 | 579 | 593 | 92/593 | 593 | 593 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_after_shrink37_prepaid | 457 | 1000 | 543 | 55 | 92/55 | 55 | 55 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_two_cycles_prepaid | 457 | 637 | 180 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 267 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_shrink64_prepaid | 457 | 746 | 289 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 118 | 331 | 0 | 0 | 1 | 0 |
| lease_n10_UpTo0_shrink64_topup | 511 | 863 | 352 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 194 | 0 | 118 | 394 | 0 | 0 | 1 | 0 |
| lease_n10_UpTo0_shrink64_refund | 781 | 856 | 75 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 99998663680 | 137 | 0 | 189 | 370 | 0 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_prepaid | 554 | 554 | 0 | 92 | 92/92 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_topup | 608 | 608 | 0 | 92 | 92/92 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_refund | 878 | 878 | 0 | 92 | 92/92 | 124 | 124 | 0 | 32 | 0 | 0 | 99998468800 | 198 | 0 | 142 | 377 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_retain32 | 2101 | 2101 | 0 | 92 | 92/124 | 124 | 124 | 0 | 32 | 1 | 222720 | 0 | 255 | 0 | 291 | 1397 | 1 | 0 | 0 | 0 |
| lease_n10_UpTo32_grow8_prepaid | 554 | 837 | 283 | 100 | 92/100 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 291 | 196 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_grow8_topup | 608 | 2140 | 1532 | 100 | 92/100 | 124 | 124 | 0 | 32 | 1 | 55680 | 0 | 255 | 0 | 302 | 1434 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_grow8_refund | 878 | 947 | 69 | 100 | 92/100 | 124 | 124 | 0 | 32 | 0 | 0 | 99998413120 | 198 | 0 | 362 | 235 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_after_grow501_prepaid | 554 | 1133 | 579 | 593 | 92/593 | 625 | 625 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 73 | 124 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_after_shrink37_prepaid | 554 | 1093 | 539 | 55 | 92/55 | 87 | 87 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 73 | 122 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_two_cycles_prepaid | 554 | 831 | 277 | 92 | 92/92 | 124 | 124 | 0 | 64 | 0 | 0 | 0 | 198 | 0 | 73 | 400 | 1 | 0 | 3 | 0 |
| lease_n10_UpTo32_shrink64_prepaid | 554 | 807 | 253 | 64 | 92/64 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 118 | 331 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_shrink64_topup | 608 | 924 | 316 | 64 | 92/64 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 118 | 394 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_shrink64_refund | 878 | 917 | 39 | 64 | 92/64 | 124 | 124 | 0 | 32 | 0 | 0 | 99998663680 | 198 | 0 | 189 | 370 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_shrink64_retain32 | 2101 | 2183 | 82 | 64 | 92/96 | 124 | 124 | 0 | 32 | 1 | 27840 | 0 | 255 | 0 | 338 | 1433 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_prepaid | 569 | 569 | 0 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_topup | 623 | 623 | 0 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_refund | 893 | 893 | 0 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 99993457600 | 211 | 0 | 142 | 377 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_retain32 | 2155 | 2155 | 0 | 812 | 812/844 | 11052 | 11052 | 0 | 10240 | 1 | 222720 | 0 | 268 | 0 | 291 | 1436 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_prepaid | 569 | 852 | 283 | 820 | 812/820 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 291 | 196 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_topup | 623 | 2158 | 1535 | 820 | 812/820 | 11052 | 11052 | 0 | 10240 | 1 | 55680 | 0 | 268 | 0 | 302 | 1437 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_refund | 893 | 962 | 69 | 820 | 812/820 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 99993401920 | 211 | 0 | 362 | 235 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_retain32 | 2155 | 2240 | 85 | 820 | 812/852 | 11052 | 11052 | 0 | 10240 | 1 | 278400 | 0 | 268 | 0 | 385 | 1436 | 1 | 0 | 1 | 0 |
| lease_n100_Max_fifty_pushes_prepaid | 569 | 5132 | 4563 | 1212 | 812/1212 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 4564 | 196 | 1 | 0 | 1 | 0 |
| lease_n100_Max_after_grow501_prepaid | 569 | 2048 | 1479 | 1313 | 812/1313 | 11052 | 11052 | 0 | 9739 | 0 | 0 | 0 | 1111 | 0 | 73 | 124 | 15 | 0 | 1 | 0 |
| lease_n100_Max_after_shrink37_prepaid | 569 | 1110 | 541 | 775 | 812/775 | 11015 | 11015 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 73 | 122 | 1 | 0 | 1 | 0 |
| lease_n100_Max_two_cycles_prepaid | 569 | 860 | 291 | 812 | 812/812 | 11052 | 11052 | 0 | 20480 | 0 | 0 | 0 | 211 | 0 | 73 | 414 | 1 | 0 | 3 | 0 |
| lease_n100_Max_readonly_prepaid | 569 | 210 | -359 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 41 | 0 | 4 | 6 | 0 | 0 | 0 | 0 |
| lease_n100_Max_shrink64_prepaid | 569 | 822 | 253 | 64 | 812/64 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 118 | 331 | 1 | 0 | 1 | 0 |
| lease_n100_Max_shrink64_topup | 623 | 939 | 316 | 64 | 812/64 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 118 | 394 | 1 | 0 | 1 | 0 |
| lease_n100_Max_shrink64_refund | 893 | 932 | 39 | 64 | 812/64 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 99998663680 | 211 | 0 | 189 | 370 | 1 | 0 | 1 | 0 |
| lease_n100_Max_shrink64_retain32 | 2155 | 981 | -1174 | 64 | 812/96 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 161 | 393 | 1 | 0 | 1 | 0 |
| lease_n100_Max_exact_window_prepaid | 569 | 825 | 256 | 11052 | 812/11052 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 292 | 160 | 1 | 0 | 0 | 0 |
| lease_n100_Max_failed_growth_prepaid | 569 | 619 | 50 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 122 | 124 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo0_same_prepaid | 457 | 457 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 71 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_same_topup | 511 | 511 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 194 | 0 | 71 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_same_refund | 781 | 781 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 99993457600 | 137 | 0 | 142 | 341 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_after_grow501_prepaid | 457 | 1036 | 579 | 1313 | 812/1313 | 1313 | 1313 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_after_shrink37_prepaid | 457 | 1000 | 543 | 775 | 812/775 | 775 | 775 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_two_cycles_prepaid | 457 | 637 | 180 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 267 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_shrink64_prepaid | 457 | 746 | 289 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 118 | 331 | 0 | 0 | 1 | 0 |
| lease_n100_UpTo0_shrink64_topup | 511 | 863 | 352 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 194 | 0 | 118 | 394 | 0 | 0 | 1 | 0 |
| lease_n100_UpTo0_shrink64_refund | 781 | 856 | 75 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 99998663680 | 137 | 0 | 189 | 370 | 0 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_prepaid | 554 | 554 | 0 | 812 | 812/812 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_topup | 608 | 608 | 0 | 812 | 812/812 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_refund | 878 | 878 | 0 | 812 | 812/812 | 844 | 844 | 0 | 32 | 0 | 0 | 99993457600 | 198 | 0 | 142 | 377 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_retain32 | 2104 | 2104 | 0 | 812 | 812/844 | 844 | 844 | 0 | 32 | 1 | 222720 | 0 | 255 | 0 | 291 | 1400 | 1 | 0 | 0 | 0 |
| lease_n100_UpTo32_grow8_prepaid | 554 | 837 | 283 | 820 | 812/820 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 291 | 196 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_grow8_topup | 608 | 2143 | 1535 | 820 | 812/820 | 844 | 844 | 0 | 32 | 1 | 55680 | 0 | 255 | 0 | 302 | 1437 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_grow8_refund | 878 | 947 | 69 | 820 | 812/820 | 844 | 844 | 0 | 32 | 0 | 0 | 99993401920 | 198 | 0 | 362 | 235 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_after_grow501_prepaid | 554 | 1133 | 579 | 1313 | 812/1313 | 1345 | 1345 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 73 | 124 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_after_shrink37_prepaid | 554 | 1093 | 539 | 775 | 812/775 | 807 | 807 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 73 | 122 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_two_cycles_prepaid | 554 | 831 | 277 | 812 | 812/812 | 844 | 844 | 0 | 64 | 0 | 0 | 0 | 198 | 0 | 73 | 400 | 1 | 0 | 3 | 0 |
| lease_n100_UpTo32_shrink64_prepaid | 554 | 807 | 253 | 64 | 812/64 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 118 | 331 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_shrink64_topup | 608 | 924 | 316 | 64 | 812/64 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 118 | 394 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_shrink64_refund | 878 | 917 | 39 | 64 | 812/64 | 844 | 844 | 0 | 32 | 0 | 0 | 99998663680 | 198 | 0 | 189 | 370 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_shrink64_retain32 | 2104 | 966 | -1138 | 64 | 812/96 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 161 | 393 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_prepaid | 569 | 569 | 0 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_topup | 623 | 623 | 0 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_refund | 893 | 893 | 0 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 99943345600 | 211 | 0 | 142 | 377 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_retain32 | 2184 | 2184 | 0 | 8012 | 8012/8044 | 18252 | 18252 | 0 | 10240 | 1 | 222720 | 0 | 268 | 0 | 291 | 1465 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_prepaid | 569 | 852 | 283 | 8020 | 8012/8020 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 291 | 196 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_topup | 623 | 2187 | 1564 | 8020 | 8012/8020 | 18252 | 18252 | 0 | 10240 | 1 | 55680 | 0 | 268 | 0 | 302 | 1466 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_refund | 893 | 962 | 69 | 8020 | 8012/8020 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 99943289920 | 211 | 0 | 362 | 235 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_retain32 | 2184 | 2269 | 85 | 8020 | 8012/8052 | 18252 | 18252 | 0 | 10240 | 1 | 278400 | 0 | 268 | 0 | 385 | 1465 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_fifty_pushes_prepaid | 569 | 5132 | 4563 | 8412 | 8012/8412 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 4564 | 196 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_after_grow501_prepaid | 569 | 2048 | 1479 | 8513 | 8012/8513 | 18252 | 18252 | 0 | 9739 | 0 | 0 | 0 | 1111 | 0 | 73 | 124 | 15 | 0 | 1 | 0 |
| lease_n1000_Max_after_shrink37_prepaid | 569 | 1110 | 541 | 7975 | 8012/7975 | 18215 | 18215 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 73 | 122 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_two_cycles_prepaid | 569 | 860 | 291 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 20480 | 0 | 0 | 0 | 211 | 0 | 73 | 414 | 1 | 0 | 3 | 0 |
| lease_n1000_Max_readonly_prepaid | 569 | 210 | -359 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 41 | 0 | 4 | 6 | 0 | 0 | 0 | 0 |
| lease_n1000_Max_shrink64_prepaid | 569 | 822 | 253 | 64 | 8012/64 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 118 | 331 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_shrink64_topup | 623 | 939 | 316 | 64 | 8012/64 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 118 | 394 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_shrink64_refund | 893 | 932 | 39 | 64 | 8012/64 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 99998663680 | 211 | 0 | 189 | 370 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_shrink64_retain32 | 2184 | 981 | -1203 | 64 | 8012/96 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 161 | 393 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_exact_window_prepaid | 569 | 825 | 256 | 18252 | 8012/18252 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 292 | 160 | 1 | 0 | 0 | 0 |
| lease_n1000_Max_failed_growth_prepaid | 569 | 619 | 50 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 122 | 124 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo0_same_prepaid | 457 | 457 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 71 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_same_topup | 511 | 511 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 194 | 0 | 71 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_same_refund | 781 | 781 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99943345600 | 137 | 0 | 142 | 341 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_after_grow501_prepaid | 457 | 1036 | 579 | 8513 | 8012/8513 | 8513 | 8513 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_after_shrink37_prepaid | 457 | 1000 | 543 | 7975 | 8012/7975 | 7975 | 7975 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_two_cycles_prepaid | 457 | 637 | 180 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 73 | 267 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_shrink64_prepaid | 457 | 746 | 289 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 137 | 0 | 118 | 331 | 0 | 0 | 1 | 0 |
| lease_n1000_UpTo0_shrink64_topup | 511 | 863 | 352 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 194 | 0 | 118 | 394 | 0 | 0 | 1 | 0 |
| lease_n1000_UpTo0_shrink64_refund | 781 | 856 | 75 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99998663680 | 137 | 0 | 189 | 370 | 0 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_prepaid | 554 | 554 | 0 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_topup | 608 | 608 | 0 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 71 | 124 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_refund | 878 | 878 | 0 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 32 | 0 | 0 | 99943345600 | 198 | 0 | 142 | 377 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_retain32 | 2133 | 2133 | 0 | 8012 | 8012/8044 | 8044 | 8044 | 0 | 32 | 1 | 222720 | 0 | 255 | 0 | 291 | 1429 | 1 | 0 | 0 | 0 |
| lease_n1000_UpTo32_grow8_prepaid | 554 | 837 | 283 | 8020 | 8012/8020 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 291 | 196 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_grow8_topup | 608 | 2172 | 1564 | 8020 | 8012/8020 | 8044 | 8044 | 0 | 32 | 1 | 55680 | 0 | 255 | 0 | 302 | 1466 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_grow8_refund | 878 | 947 | 69 | 8020 | 8012/8020 | 8044 | 8044 | 0 | 32 | 0 | 0 | 99943289920 | 198 | 0 | 362 | 235 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_after_grow501_prepaid | 554 | 1133 | 579 | 8513 | 8012/8513 | 8545 | 8545 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 73 | 124 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_after_shrink37_prepaid | 554 | 1093 | 539 | 7975 | 8012/7975 | 8007 | 8007 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 73 | 122 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_two_cycles_prepaid | 554 | 831 | 277 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 64 | 0 | 0 | 0 | 198 | 0 | 73 | 400 | 1 | 0 | 3 | 0 |
| lease_n1000_UpTo32_shrink64_prepaid | 554 | 807 | 253 | 64 | 8012/64 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 198 | 0 | 118 | 331 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_shrink64_topup | 608 | 924 | 316 | 64 | 8012/64 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 118 | 394 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_shrink64_refund | 878 | 917 | 39 | 64 | 8012/64 | 8044 | 8044 | 0 | 32 | 0 | 0 | 99998663680 | 198 | 0 | 189 | 370 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_shrink64_retain32 | 2133 | 966 | -1167 | 64 | 8012/96 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 255 | 0 | 161 | 393 | 1 | 0 | 1 | 0 |
| lease_n10_OnDemand_same_prepaid | 411 | 411 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_same_topup | 465 | 465 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_same_refund | 719 | 719 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 99998468800 | 89 | 0 | 128 | 341 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_same_retain32 | 2171 | 2171 | 0 | 92 | 92/124 | 92 | 156 | 1 | 64 | 1 | 222720 | 0 | 146 | 0 | 434 | 1433 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_prepaid | 411 | 857 | 446 | 100 | 92/100 | 92 | 123 | 1 | 31 | 0 | 0 | 0 | 89 | 0 | 419 | 196 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_topup | 465 | 2160 | 1695 | 100 | 92/100 | 92 | 123 | 1 | 31 | 1 | 55680 | 0 | 146 | 0 | 430 | 1434 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_refund | 719 | 951 | 232 | 100 | 92/100 | 92 | 123 | 1 | 31 | 0 | 0 | 99998413120 | 89 | 0 | 474 | 235 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_retain32 | 2171 | 2381 | 210 | 100 | 92/132 | 92 | 162 | 2 | 70 | 1 | 278400 | 0 | 146 | 0 | 652 | 1433 | 0 | 2 | 1 | 16 |
| lease_n10_OnDemand_fifty_pushes_prepaid | 411 | 6192 | 5781 | 492 | 92/492 | 92 | 494 | 7 | 402 | 0 | 0 | 0 | 89 | 0 | 5749 | 196 | 0 | 7 | 1 | 56 |
| lease_n10_OnDemand_after_grow501_prepaid | 411 | 992 | 581 | 593 | 92/593 | 593 | 593 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_after_shrink37_prepaid | 411 | 951 | 540 | 55 | 92/55 | 55 | 55 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_two_cycles_prepaid | 411 | 543 | 132 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 219 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_shrink64_prepaid | 411 | 702 | 291 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 122 | 331 | 0 | 0 | 1 | 0 |
| lease_n10_OnDemand_shrink64_topup | 465 | 819 | 354 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 122 | 394 | 0 | 0 | 1 | 0 |
| lease_n10_OnDemand_shrink64_refund | 719 | 796 | 77 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 99998663680 | 89 | 0 | 177 | 370 | 0 | 0 | 1 | 0 |
| lease_n10_OnDemand_shrink64_retain32 | 2171 | 2217 | 46 | 64 | 92/96 | 92 | 119 | 1 | 27 | 1 | 27840 | 0 | 146 | 0 | 481 | 1433 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_exact_window_prepaid | 411 | 888 | 477 | 10332 | 92/10332 | 92 | 10332 | 1 | 10240 | 0 | 0 | 0 | 89 | 0 | 479 | 160 | 0 | 2 | 0 | 8 |
| lease_n10_OnDemand_failed_growth_prepaid | 411 | 745 | 334 | 92 | 92/92 | 92 | 92 | 1 | 0 | 0 | 0 | 0 | 89 | 0 | 408 | 88 | 0 | 2 | 0 | 8 |
| lease_n100_OnDemand_same_prepaid | 411 | 411 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_same_topup | 465 | 465 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_same_refund | 719 | 719 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 99993457600 | 89 | 0 | 128 | 341 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_same_retain32 | 2172 | 2172 | 0 | 812 | 812/844 | 812 | 1047 | 1 | 235 | 1 | 222720 | 0 | 146 | 0 | 432 | 1436 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_prepaid | 411 | 857 | 446 | 820 | 812/820 | 812 | 1023 | 1 | 211 | 0 | 0 | 0 | 89 | 0 | 419 | 196 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_topup | 465 | 2163 | 1698 | 820 | 812/820 | 812 | 1023 | 1 | 211 | 1 | 55680 | 0 | 146 | 0 | 430 | 1437 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_refund | 719 | 951 | 232 | 820 | 812/820 | 812 | 1023 | 1 | 211 | 0 | 0 | 99993401920 | 89 | 0 | 474 | 235 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_retain32 | 2172 | 2266 | 94 | 820 | 812/852 | 812 | 1023 | 1 | 211 | 1 | 278400 | 0 | 146 | 0 | 534 | 1436 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_fifty_pushes_prepaid | 411 | 5597 | 5186 | 1212 | 812/1212 | 812 | 1283 | 2 | 471 | 0 | 0 | 0 | 89 | 0 | 5154 | 196 | 0 | 2 | 1 | 16 |
| lease_n100_OnDemand_after_grow501_prepaid | 411 | 992 | 581 | 1313 | 812/1313 | 1313 | 1313 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_after_shrink37_prepaid | 411 | 951 | 540 | 775 | 812/775 | 775 | 775 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_two_cycles_prepaid | 411 | 543 | 132 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 219 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_shrink64_prepaid | 411 | 702 | 291 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 122 | 331 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_shrink64_topup | 465 | 819 | 354 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 122 | 394 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_shrink64_refund | 719 | 796 | 77 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 99998663680 | 89 | 0 | 177 | 370 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_shrink64_retain32 | 2172 | 880 | -1292 | 64 | 812/96 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 184 | 393 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_exact_window_prepaid | 411 | 888 | 477 | 11052 | 812/11052 | 812 | 11052 | 1 | 10240 | 0 | 0 | 0 | 89 | 0 | 479 | 160 | 0 | 2 | 0 | 8 |
| lease_n100_OnDemand_failed_growth_prepaid | 411 | 745 | 334 | 812 | 812/812 | 812 | 812 | 1 | 0 | 0 | 0 | 0 | 89 | 0 | 408 | 88 | 0 | 2 | 0 | 8 |
| lease_n1000_OnDemand_same_prepaid | 411 | 411 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_same_topup | 465 | 465 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 73 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_same_refund | 719 | 719 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99943345600 | 89 | 0 | 128 | 341 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_same_retain32 | 2201 | 2201 | 0 | 8012 | 8012/8044 | 8012 | 10047 | 1 | 2035 | 1 | 222720 | 0 | 146 | 0 | 432 | 1465 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_prepaid | 411 | 857 | 446 | 8020 | 8012/8020 | 8012 | 10023 | 1 | 2011 | 0 | 0 | 0 | 89 | 0 | 419 | 196 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_topup | 465 | 2192 | 1727 | 8020 | 8012/8020 | 8012 | 10023 | 1 | 2011 | 1 | 55680 | 0 | 146 | 0 | 430 | 1466 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_refund | 719 | 951 | 232 | 8020 | 8012/8020 | 8012 | 10023 | 1 | 2011 | 0 | 0 | 99943289920 | 89 | 0 | 474 | 235 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_retain32 | 2201 | 2295 | 94 | 8020 | 8012/8052 | 8012 | 10023 | 1 | 2011 | 1 | 278400 | 0 | 146 | 0 | 534 | 1465 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_fifty_pushes_prepaid | 411 | 5478 | 5067 | 8412 | 8012/8412 | 8012 | 10023 | 1 | 2011 | 0 | 0 | 0 | 89 | 0 | 5035 | 196 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_after_grow501_prepaid | 411 | 992 | 581 | 8513 | 8012/8513 | 8513 | 8513 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_after_shrink37_prepaid | 411 | 951 | 540 | 7975 | 8012/7975 | 7975 | 7975 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_two_cycles_prepaid | 411 | 543 | 132 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 75 | 219 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_shrink64_prepaid | 411 | 702 | 291 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 89 | 0 | 122 | 331 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_shrink64_topup | 465 | 819 | 354 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 122 | 394 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_shrink64_refund | 719 | 796 | 77 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99998663680 | 89 | 0 | 177 | 370 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_shrink64_retain32 | 2201 | 880 | -1321 | 64 | 8012/96 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 146 | 0 | 184 | 393 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_exact_window_prepaid | 411 | 888 | 477 | 18252 | 8012/18252 | 8012 | 18252 | 1 | 10240 | 0 | 0 | 0 | 89 | 0 | 479 | 160 | 0 | 2 | 0 | 8 |
| lease_n1000_OnDemand_failed_growth_prepaid | 411 | 745 | 334 | 8012 | 8012/8012 | 8012 | 8012 | 1 | 0 | 0 | 0 | 0 | 89 | 0 | 408 | 88 | 0 | 2 | 0 | 8 |
