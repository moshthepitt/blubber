# Scaling measurements on SBF v0

Source SHA-256: `86ac10b35f190285bfabf829d3a7782e760c8b6e9d0d487f80a7b524e422c0c4`. See [scope and reproduction](CU.md).

Whole and baseline are separate uninstrumented instructions. Difference is whole minus baseline.
Open/load/edit/commit are calibrated instrumented brackets and cannot be added to reconstruct whole.
Mutable collections use Max/Prepaid; reads use ReadStore without resize. Funding CPIs are zero. Lengths are bytes.
Moved counts the sum of nonidentity relocation ranges, excluding encoding, zeroing and directory stores.
Ends counts directory words visited for publication, including unchanged words; it excludes count/used words.
Depth counts schema nodes, including the root and leaf. The depth-1 string is itself the envelope root.

| Fixture (layout / n / operation / position / prefix) | Baseline | Whole | Difference | Used before/after | Storage before/after | Moved | Ends | Depth | Open | Load | Edit | Commit |
|---|---:|---:|---:|---|---|---:|---:|---:|---:|---:|---:|---:|
| list_fixed_n10_load_trusted_all_plain | 223 | 223 | 0 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 34 | 9 | 14 |
| list_fixed_n10_late_read_all_plain | 223 | 437 | 214 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 34 | 222 | 14 |
| list_fixed_n10_validate_all_plain | 223 | 496 | 273 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 34 | 280 | 14 |
| list_fixed_n10_load_all_plain | 223 | 512 | 289 | 96/96 | 96/96 | 0 | 0 | 3 | 71 | 321 | 7 | 14 |
| list_fixed_n10_mutable_baseline_all_plain | 573 | 573 | 0 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 4 | 228 |
| list_fixed_n10_clear_all_plain | 573 | 2264 | 1691 | 96/16 | 96/16 | 0 | 0 | 3 | 219 | 25 | 1485 | 437 |
| list_fixed_n10_cursor_first_all_plain | 573 | 788 | 215 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 218 | 228 |
| list_fixed_n10_for_each_read_all_plain | 573 | 926 | 353 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 356 | 228 |
| list_fixed_n10_insert_front_plain | 573 | 2422 | 1849 | 96/104 | 96/104 | 80 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n10_replace_front_plain | 573 | 1927 | 1354 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n10_remove_front_plain | 573 | 1542 | 969 | 96/88 | 96/88 | 72 | 0 | 3 | 219 | 25 | 759 | 437 |
| list_fixed_n10_remove_into_front_plain | 573 | 2615 | 2042 | 96/88 | 96/88 | 72 | 0 | 3 | 219 | 25 | 1836 | 437 |
| list_fixed_n10_at_mut_front_plain | 573 | 1427 | 854 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n10_insert_bulk_front_plain | 573 | 2673 | 2100 | 96/112 | 96/112 | 80 | 0 | 3 | 219 | 25 | 2029 | 302 |
| list_fixed_n10_truncate_front_plain | 573 | 2274 | 1701 | 96/16 | 96/16 | 0 | 0 | 3 | 219 | 25 | 1495 | 437 |
| list_fixed_n10_insert_middle_plain | 573 | 2422 | 1849 | 96/104 | 96/104 | 40 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n10_replace_middle_plain | 573 | 1927 | 1354 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n10_remove_middle_plain | 573 | 1542 | 969 | 96/88 | 96/88 | 32 | 0 | 3 | 219 | 25 | 759 | 437 |
| list_fixed_n10_remove_into_middle_plain | 573 | 2615 | 2042 | 96/88 | 96/88 | 32 | 0 | 3 | 219 | 25 | 1836 | 437 |
| list_fixed_n10_at_mut_middle_plain | 573 | 1427 | 854 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n10_insert_bulk_middle_plain | 573 | 2674 | 2101 | 96/112 | 96/112 | 40 | 0 | 3 | 219 | 25 | 2030 | 302 |
| list_fixed_n10_truncate_middle_plain | 573 | 2244 | 1671 | 96/56 | 96/56 | 0 | 0 | 3 | 219 | 25 | 1465 | 437 |
| list_fixed_n10_insert_end_plain | 573 | 2226 | 1653 | 96/104 | 96/104 | 0 | 0 | 3 | 219 | 25 | 1584 | 302 |
| list_fixed_n10_replace_end_plain | 573 | 1927 | 1354 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n10_remove_end_plain | 573 | 1369 | 796 | 96/88 | 96/88 | 0 | 0 | 3 | 219 | 25 | 586 | 437 |
| list_fixed_n10_remove_into_end_plain | 573 | 2436 | 1863 | 96/88 | 96/88 | 0 | 0 | 3 | 219 | 25 | 1657 | 437 |
| list_fixed_n10_at_mut_end_plain | 573 | 1427 | 854 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n10_insert_bulk_end_plain | 573 | 2480 | 1907 | 96/112 | 96/112 | 0 | 0 | 3 | 219 | 25 | 1836 | 302 |
| list_fixed_n10_truncate_end_plain | 573 | 1907 | 1334 | 96/96 | 96/96 | 0 | 0 | 3 | 219 | 25 | 1337 | 228 |
| list_fixed_n10_push_end_plain | 573 | 1327 | 754 | 96/104 | 96/104 | 0 | 0 | 3 | 219 | 25 | 683 | 302 |
| list_fixed_n10_pop_end_plain | 573 | 2328 | 1755 | 96/88 | 96/88 | 0 | 0 | 3 | 219 | 25 | 1549 | 437 |
| list_fixed_n10_extend_end_plain | 573 | 2479 | 1906 | 96/112 | 96/112 | 0 | 0 | 3 | 219 | 25 | 1835 | 302 |
| list_indexed_n10_load_trusted_all_plain | 224 | 224 | 0 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| list_indexed_n10_late_read_all_plain | 224 | 1005 | 781 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 34 | 789 | 14 |
| list_indexed_n10_validate_all_plain | 224 | 1097 | 873 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 34 | 880 | 14 |
| list_indexed_n10_load_all_plain | 224 | 1113 | 889 | 132/132 | 132/132 | 0 | 0 | 3 | 72 | 921 | 7 | 14 |
| list_indexed_n10_mutable_baseline_all_plain | 574 | 574 | 0 | 132/132 | 132/132 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| list_indexed_n10_clear_all_plain | 574 | 2109 | 1535 | 132/16 | 132/16 | 0 | 0 | 3 | 220 | 25 | 1329 | 437 |
| list_indexed_n10_cursor_first_all_plain | 574 | 3455 | 2881 | 132/141 | 132/141 | 72 | 9 | 3 | 220 | 25 | 2810 | 302 |
| list_indexed_n10_for_each_read_all_plain | 574 | 3057 | 2483 | 132/132 | 132/132 | 0 | 0 | 3 | 220 | 25 | 2486 | 228 |
| list_indexed_n10_insert_front_plain | 574 | 3144 | 2570 | 132/153 | 132/153 | 116 | 10 | 3 | 220 | 25 | 2501 | 302 |
| list_indexed_n10_replace_front_plain | 574 | 3408 | 2834 | 132/141 | 132/141 | 72 | 9 | 3 | 220 | 25 | 2763 | 302 |
| list_indexed_n10_remove_front_plain | 574 | 3855 | 3281 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 3071 | 437 |
| list_indexed_n10_remove_into_front_plain | 574 | 3549 | 2975 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2769 | 437 |
| list_indexed_n10_at_mut_front_plain | 574 | 2863 | 2289 | 132/141 | 132/141 | 72 | 9 | 3 | 220 | 25 | 2218 | 302 |
| list_indexed_n10_insert_bulk_front_plain | 574 | 3306 | 2732 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2661 | 302 |
| list_indexed_n10_truncate_front_plain | 574 | 2113 | 1539 | 132/16 | 132/16 | 0 | 0 | 3 | 220 | 25 | 1333 | 437 |
| list_indexed_n10_insert_middle_plain | 574 | 3199 | 2625 | 132/153 | 132/153 | 96 | 10 | 3 | 220 | 25 | 2556 | 302 |
| list_indexed_n10_replace_middle_plain | 574 | 3220 | 2646 | 132/141 | 132/141 | 32 | 9 | 3 | 220 | 25 | 2575 | 302 |
| list_indexed_n10_remove_middle_plain | 574 | 3851 | 3277 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 3067 | 437 |
| list_indexed_n10_remove_into_middle_plain | 574 | 3545 | 2971 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2765 | 437 |
| list_indexed_n10_at_mut_middle_plain | 574 | 2841 | 2267 | 132/141 | 132/141 | 32 | 4 | 3 | 220 | 25 | 2196 | 302 |
| list_indexed_n10_insert_bulk_middle_plain | 574 | 3348 | 2774 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2703 | 302 |
| list_indexed_n10_truncate_middle_plain | 574 | 2981 | 2407 | 132/72 | 132/72 | 40 | 4 | 3 | 220 | 25 | 2201 | 437 |
| list_indexed_n10_insert_end_plain | 574 | 2827 | 2253 | 132/153 | 132/153 | 80 | 10 | 3 | 220 | 25 | 2184 | 302 |
| list_indexed_n10_replace_end_plain | 574 | 2787 | 2213 | 132/141 | 132/141 | 0 | 9 | 3 | 220 | 25 | 2142 | 302 |
| list_indexed_n10_remove_end_plain | 574 | 3472 | 2898 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2688 | 437 |
| list_indexed_n10_remove_into_end_plain | 574 | 3166 | 2592 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2386 | 437 |
| list_indexed_n10_at_mut_end_plain | 574 | 2489 | 1915 | 132/141 | 132/141 | 0 | 0 | 3 | 220 | 25 | 1844 | 302 |
| list_indexed_n10_insert_bulk_end_plain | 574 | 2954 | 2380 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2309 | 302 |
| list_indexed_n10_truncate_end_plain | 574 | 2407 | 1833 | 132/132 | 132/132 | 0 | 0 | 3 | 220 | 25 | 1836 | 228 |
| list_indexed_n10_push_end_plain | 574 | 2840 | 2266 | 132/153 | 132/153 | 80 | 10 | 3 | 220 | 25 | 2195 | 302 |
| list_indexed_n10_pop_end_plain | 574 | 3162 | 2588 | 132/120 | 132/120 | 72 | 8 | 3 | 220 | 25 | 2382 | 437 |
| list_indexed_n10_extend_end_plain | 574 | 2953 | 2379 | 132/174 | 132/174 | 80 | 11 | 3 | 220 | 25 | 2308 | 302 |
| map_fixed_n10_load_trusted_all_plain | 228 | 228 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n10_late_read_all_plain | 228 | 1114 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 894 | 14 |
| map_fixed_n10_validate_all_plain | 228 | 857 | 629 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 636 | 14 |
| map_fixed_n10_load_all_plain | 228 | 873 | 645 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 677 | 7 | 14 |
| map_fixed_n10_mutable_baseline_all_plain | 578 | 578 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n10_clear_all_plain | 578 | 2545 | 1967 | 416/16 | 416/16 | 0 | 0 | 3 | 223 | 25 | 1761 | 437 |
| map_fixed_n10_cursor_first_all_plain | 578 | 1077 | 499 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 502 | 228 |
| map_fixed_n10_for_each_read_all_plain | 578 | 1242 | 664 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 667 | 228 |
| map_fixed_n10_insert_front_plain | 578 | 1891 | 1313 | 416/456 | 416/456 | 400 | 0 | 3 | 223 | 25 | 1244 | 302 |
| map_fixed_n10_replace_front_plain | 578 | 3241 | 2663 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2666 | 228 |
| map_fixed_n10_remove_front_plain | 578 | 1954 | 1376 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 1170 | 437 |
| map_fixed_n10_remove_into_front_plain | 578 | 3145 | 2567 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 2361 | 437 |
| map_fixed_n10_at_mut_front_plain | 578 | 1794 | 1216 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1219 | 228 |
| map_fixed_n10_lookup_hit_front_plain | 228 | 1231 | 1003 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1011 | 14 |
| map_fixed_n10_lookup_miss_front_plain | 228 | 1083 | 855 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 865 | 14 |
| map_fixed_n10_entry_occupied_front_plain | 578 | 2699 | 2121 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2128 | 228 |
| map_fixed_n10_entry_vacant_front_plain | 578 | 4393 | 3815 | 416/456 | 416/456 | 400 | 0 | 3 | 219 | 25 | 3750 | 302 |
| map_fixed_n10_insert_middle_plain | 578 | 2357 | 1779 | 416/456 | 416/456 | 200 | 0 | 3 | 223 | 25 | 1710 | 302 |
| map_fixed_n10_replace_middle_plain | 578 | 3449 | 2871 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2874 | 228 |
| map_fixed_n10_remove_middle_plain | 578 | 2058 | 1480 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 1274 | 437 |
| map_fixed_n10_remove_into_middle_plain | 578 | 3249 | 2671 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 2465 | 437 |
| map_fixed_n10_at_mut_middle_plain | 578 | 1898 | 1320 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1323 | 228 |
| map_fixed_n10_lookup_hit_middle_plain | 228 | 832 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n10_lookup_miss_middle_plain | 228 | 966 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 748 | 14 |
| map_fixed_n10_entry_occupied_middle_plain | 578 | 2803 | 2225 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2232 | 228 |
| map_fixed_n10_entry_vacant_middle_plain | 578 | 4929 | 4351 | 416/456 | 416/456 | 200 | 0 | 3 | 219 | 25 | 4286 | 302 |
| map_fixed_n10_insert_end_plain | 578 | 1713 | 1135 | 416/456 | 416/456 | 0 | 0 | 3 | 223 | 25 | 1066 | 302 |
| map_fixed_n10_replace_end_plain | 578 | 3165 | 2587 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2590 | 228 |
| map_fixed_n10_remove_end_plain | 578 | 1743 | 1165 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 959 | 437 |
| map_fixed_n10_remove_into_end_plain | 578 | 2928 | 2350 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 2144 | 437 |
| map_fixed_n10_at_mut_end_plain | 578 | 1756 | 1178 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1181 | 228 |
| map_fixed_n10_lookup_hit_end_plain | 228 | 1114 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 894 | 14 |
| map_fixed_n10_lookup_miss_end_plain | 228 | 966 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 748 | 14 |
| map_fixed_n10_entry_occupied_end_plain | 578 | 2661 | 2083 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2090 | 228 |
| map_fixed_n10_entry_vacant_end_plain | 578 | 4159 | 3581 | 416/456 | 416/456 | 0 | 0 | 3 | 219 | 25 | 3516 | 302 |
| map_fixed_n10_load_trusted_all_common | 230 | 230 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n10_late_read_all_common | 230 | 1694 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1472 | 14 |
| map_fixed_n10_validate_all_common | 230 | 940 | 710 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 717 | 14 |
| map_fixed_n10_load_all_common | 230 | 956 | 726 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 758 | 7 | 14 |
| map_fixed_n10_mutable_baseline_all_common | 580 | 580 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n10_clear_all_common | 580 | 2547 | 1967 | 416/16 | 416/16 | 0 | 0 | 3 | 223 | 25 | 1761 | 437 |
| map_fixed_n10_cursor_first_all_common | 580 | 1079 | 499 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 502 | 228 |
| map_fixed_n10_for_each_read_all_common | 580 | 1244 | 664 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 667 | 228 |
| map_fixed_n10_insert_front_common | 580 | 1911 | 1331 | 416/456 | 416/456 | 400 | 0 | 3 | 223 | 25 | 1262 | 302 |
| map_fixed_n10_replace_front_common | 580 | 3261 | 2681 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2684 | 228 |
| map_fixed_n10_remove_front_common | 580 | 1965 | 1385 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 1179 | 437 |
| map_fixed_n10_remove_into_front_common | 580 | 3156 | 2576 | 416/376 | 416/376 | 360 | 0 | 3 | 223 | 25 | 2370 | 437 |
| map_fixed_n10_at_mut_front_common | 580 | 1805 | 1225 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1228 | 228 |
| map_fixed_n10_lookup_hit_front_common | 230 | 2100 | 1870 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1878 | 14 |
| map_fixed_n10_lookup_miss_front_common | 230 | 2241 | 2011 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 2021 | 14 |
| map_fixed_n10_entry_occupied_front_common | 580 | 2710 | 2130 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2137 | 228 |
| map_fixed_n10_entry_vacant_front_common | 580 | 4428 | 3848 | 416/456 | 416/456 | 400 | 0 | 3 | 219 | 25 | 3783 | 302 |
| map_fixed_n10_insert_middle_common | 580 | 2437 | 1857 | 416/456 | 416/456 | 200 | 0 | 3 | 223 | 25 | 1788 | 302 |
| map_fixed_n10_replace_middle_common | 580 | 3487 | 2907 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2910 | 228 |
| map_fixed_n10_remove_middle_common | 580 | 2078 | 1498 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 1292 | 437 |
| map_fixed_n10_remove_into_middle_common | 580 | 3269 | 2689 | 416/376 | 416/376 | 160 | 0 | 3 | 223 | 25 | 2483 | 437 |
| map_fixed_n10_at_mut_middle_common | 580 | 1918 | 1338 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1341 | 228 |
| map_fixed_n10_lookup_hit_middle_common | 230 | 834 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n10_lookup_miss_middle_common | 230 | 1835 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1615 | 14 |
| map_fixed_n10_entry_occupied_middle_common | 580 | 2823 | 2243 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2250 | 228 |
| map_fixed_n10_entry_vacant_middle_common | 580 | 5036 | 4456 | 416/456 | 416/456 | 200 | 0 | 3 | 219 | 25 | 4391 | 302 |
| map_fixed_n10_insert_end_common | 580 | 1724 | 1144 | 416/456 | 416/456 | 0 | 0 | 3 | 223 | 25 | 1075 | 302 |
| map_fixed_n10_replace_end_common | 580 | 3167 | 2587 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 2590 | 228 |
| map_fixed_n10_remove_end_common | 580 | 1745 | 1165 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 959 | 437 |
| map_fixed_n10_remove_into_end_common | 580 | 2930 | 2350 | 416/376 | 416/376 | 0 | 0 | 3 | 223 | 25 | 2144 | 437 |
| map_fixed_n10_at_mut_end_common | 580 | 1758 | 1178 | 416/416 | 416/416 | 0 | 0 | 3 | 223 | 25 | 1181 | 228 |
| map_fixed_n10_lookup_hit_end_common | 230 | 1694 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1472 | 14 |
| map_fixed_n10_lookup_miss_end_common | 230 | 1835 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 75 | 34 | 1615 | 14 |
| map_fixed_n10_entry_occupied_end_common | 580 | 2663 | 2083 | 416/416 | 416/416 | 0 | 0 | 3 | 219 | 25 | 2090 | 228 |
| map_fixed_n10_entry_vacant_end_common | 580 | 4182 | 3602 | 416/456 | 416/456 | 0 | 0 | 3 | 219 | 25 | 3537 | 302 |
| map_indexed_n10_load_trusted_all_plain | 226 | 226 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n10_late_read_all_plain | 226 | 1332 | 1106 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1114 | 14 |
| map_indexed_n10_validate_all_plain | 226 | 1397 | 1171 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1178 | 14 |
| map_indexed_n10_load_all_plain | 226 | 1413 | 1187 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 1219 | 7 | 14 |
| map_indexed_n10_mutable_baseline_all_plain | 576 | 576 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n10_clear_all_plain | 576 | 2826 | 2250 | 456/16 | 456/16 | 0 | 0 | 3 | 220 | 25 | 2044 | 437 |
| map_indexed_n10_cursor_first_all_plain | 576 | 4038 | 3462 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3391 | 302 |
| map_indexed_n10_for_each_read_all_plain | 576 | 3744 | 3168 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 3171 | 228 |
| map_indexed_n10_insert_front_plain | 576 | 4285 | 3709 | 456/509 | 456/509 | 440 | 11 | 3 | 220 | 25 | 3640 | 302 |
| map_indexed_n10_replace_front_plain | 576 | 3964 | 3388 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3317 | 302 |
| map_indexed_n10_remove_front_plain | 576 | 4489 | 3913 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3707 | 437 |
| map_indexed_n10_remove_into_front_plain | 576 | 4139 | 3563 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3357 | 437 |
| map_indexed_n10_at_mut_front_plain | 576 | 3391 | 2815 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 2744 | 302 |
| map_indexed_n10_lookup_hit_front_plain | 226 | 1408 | 1182 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1190 | 14 |
| map_indexed_n10_lookup_miss_front_plain | 226 | 1038 | 812 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 822 | 14 |
| map_indexed_n10_entry_occupied_front_plain | 576 | 3372 | 2796 | 456/465 | 456/465 | 72 | 10 | 3 | 216 | 25 | 2729 | 302 |
| map_indexed_n10_entry_vacant_front_plain | 576 | 5332 | 4756 | 456/509 | 456/509 | 440 | 11 | 3 | 216 | 25 | 4691 | 302 |
| map_indexed_n10_insert_middle_plain | 576 | 4852 | 4276 | 456/509 | 456/509 | 260 | 11 | 3 | 220 | 25 | 4207 | 302 |
| map_indexed_n10_replace_middle_plain | 576 | 3964 | 3388 | 456/465 | 456/465 | 32 | 10 | 3 | 220 | 25 | 3317 | 302 |
| map_indexed_n10_remove_middle_plain | 576 | 4662 | 4086 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3880 | 437 |
| map_indexed_n10_remove_into_middle_plain | 576 | 4314 | 3738 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3532 | 437 |
| map_indexed_n10_at_mut_middle_plain | 576 | 3411 | 2835 | 456/465 | 456/465 | 32 | 5 | 3 | 220 | 25 | 2764 | 302 |
| map_indexed_n10_lookup_hit_middle_plain | 226 | 1049 | 823 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 831 | 14 |
| map_indexed_n10_lookup_miss_middle_plain | 226 | 921 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 705 | 14 |
| map_indexed_n10_entry_occupied_middle_plain | 576 | 3352 | 2776 | 456/465 | 456/465 | 32 | 5 | 3 | 216 | 25 | 2709 | 302 |
| map_indexed_n10_entry_vacant_middle_plain | 576 | 5970 | 5394 | 456/509 | 456/509 | 260 | 11 | 3 | 216 | 25 | 5329 | 302 |
| map_indexed_n10_insert_end_plain | 576 | 3700 | 3124 | 456/509 | 456/509 | 80 | 11 | 3 | 220 | 25 | 3055 | 302 |
| map_indexed_n10_replace_end_plain | 576 | 3423 | 2847 | 456/465 | 456/465 | 0 | 10 | 3 | 220 | 25 | 2776 | 302 |
| map_indexed_n10_remove_end_plain | 576 | 3948 | 3372 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 3166 | 437 |
| map_indexed_n10_remove_into_end_plain | 576 | 3608 | 3032 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 2826 | 437 |
| map_indexed_n10_at_mut_end_plain | 576 | 2970 | 2394 | 456/465 | 456/465 | 0 | 1 | 3 | 220 | 25 | 2323 | 302 |
| map_indexed_n10_lookup_hit_end_plain | 226 | 1332 | 1106 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1114 | 14 |
| map_indexed_n10_lookup_miss_end_plain | 226 | 921 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 705 | 14 |
| map_indexed_n10_entry_occupied_end_plain | 576 | 2910 | 2334 | 456/465 | 456/465 | 0 | 1 | 3 | 216 | 25 | 2267 | 302 |
| map_indexed_n10_entry_vacant_end_plain | 576 | 4749 | 4173 | 456/509 | 456/509 | 80 | 11 | 3 | 216 | 25 | 4108 | 302 |
| map_indexed_n10_load_trusted_all_common | 228 | 228 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n10_late_read_all_common | 228 | 1912 | 1684 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1692 | 14 |
| map_indexed_n10_validate_all_common | 228 | 1480 | 1252 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1259 | 14 |
| map_indexed_n10_load_all_common | 228 | 1496 | 1268 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 1300 | 7 | 14 |
| map_indexed_n10_mutable_baseline_all_common | 578 | 578 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n10_clear_all_common | 578 | 2828 | 2250 | 456/16 | 456/16 | 0 | 0 | 3 | 220 | 25 | 2044 | 437 |
| map_indexed_n10_cursor_first_all_common | 578 | 4040 | 3462 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3391 | 302 |
| map_indexed_n10_for_each_read_all_common | 578 | 3827 | 3249 | 456/456 | 456/456 | 0 | 0 | 3 | 220 | 25 | 3252 | 228 |
| map_indexed_n10_insert_front_common | 578 | 4305 | 3727 | 456/509 | 456/509 | 440 | 11 | 3 | 220 | 25 | 3658 | 302 |
| map_indexed_n10_replace_front_common | 578 | 3975 | 3397 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 3326 | 302 |
| map_indexed_n10_remove_front_common | 578 | 4500 | 3922 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3716 | 437 |
| map_indexed_n10_remove_into_front_common | 578 | 4150 | 3572 | 456/412 | 456/412 | 396 | 9 | 3 | 220 | 25 | 3366 | 437 |
| map_indexed_n10_at_mut_front_common | 578 | 3402 | 2824 | 456/465 | 456/465 | 72 | 10 | 3 | 220 | 25 | 2753 | 302 |
| map_indexed_n10_lookup_hit_front_common | 228 | 2277 | 2049 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 2057 | 14 |
| map_indexed_n10_lookup_miss_front_common | 228 | 2196 | 1968 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1978 | 14 |
| map_indexed_n10_entry_occupied_front_common | 578 | 3383 | 2805 | 456/465 | 456/465 | 72 | 10 | 3 | 216 | 25 | 2738 | 302 |
| map_indexed_n10_entry_vacant_front_common | 578 | 5367 | 4789 | 456/509 | 456/509 | 440 | 11 | 3 | 216 | 25 | 4724 | 302 |
| map_indexed_n10_insert_middle_common | 578 | 4932 | 4354 | 456/509 | 456/509 | 260 | 11 | 3 | 220 | 25 | 4285 | 302 |
| map_indexed_n10_replace_middle_common | 578 | 3984 | 3406 | 456/465 | 456/465 | 32 | 10 | 3 | 220 | 25 | 3335 | 302 |
| map_indexed_n10_remove_middle_common | 578 | 4682 | 4104 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3898 | 437 |
| map_indexed_n10_remove_into_middle_common | 578 | 4334 | 3756 | 456/412 | 456/412 | 216 | 9 | 3 | 220 | 25 | 3550 | 437 |
| map_indexed_n10_at_mut_middle_common | 578 | 3431 | 2853 | 456/465 | 456/465 | 32 | 5 | 3 | 220 | 25 | 2782 | 302 |
| map_indexed_n10_lookup_hit_middle_common | 228 | 1051 | 823 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 831 | 14 |
| map_indexed_n10_lookup_miss_middle_common | 228 | 1790 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1572 | 14 |
| map_indexed_n10_entry_occupied_middle_common | 578 | 3372 | 2794 | 456/465 | 456/465 | 32 | 5 | 3 | 216 | 25 | 2727 | 302 |
| map_indexed_n10_entry_vacant_middle_common | 578 | 6077 | 5499 | 456/509 | 456/509 | 260 | 11 | 3 | 216 | 25 | 5434 | 302 |
| map_indexed_n10_insert_end_common | 578 | 3711 | 3133 | 456/509 | 456/509 | 80 | 11 | 3 | 220 | 25 | 3064 | 302 |
| map_indexed_n10_replace_end_common | 578 | 3425 | 2847 | 456/465 | 456/465 | 0 | 10 | 3 | 220 | 25 | 2776 | 302 |
| map_indexed_n10_remove_end_common | 578 | 3950 | 3372 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 3166 | 437 |
| map_indexed_n10_remove_into_end_common | 578 | 3610 | 3032 | 456/412 | 456/412 | 72 | 9 | 3 | 220 | 25 | 2826 | 437 |
| map_indexed_n10_at_mut_end_common | 578 | 2972 | 2394 | 456/465 | 456/465 | 0 | 1 | 3 | 220 | 25 | 2323 | 302 |
| map_indexed_n10_lookup_hit_end_common | 228 | 1912 | 1684 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1692 | 14 |
| map_indexed_n10_lookup_miss_end_common | 228 | 1790 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 72 | 34 | 1572 | 14 |
| map_indexed_n10_entry_occupied_end_common | 578 | 2912 | 2334 | 456/465 | 456/465 | 0 | 1 | 3 | 216 | 25 | 2267 | 302 |
| map_indexed_n10_entry_vacant_end_common | 578 | 4772 | 4194 | 456/509 | 456/509 | 80 | 11 | 3 | 216 | 25 | 4129 | 302 |
| address_fixed_n10_load_trusted_all_plain | 233 | 233 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n10_late_read_all_plain | 233 | 1119 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 894 | 14 |
| address_fixed_n10_validate_all_plain | 233 | 862 | 629 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 636 | 14 |
| address_fixed_n10_load_all_plain | 233 | 878 | 645 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 677 | 7 | 14 |
| address_fixed_n10_mutable_baseline_all_plain | 583 | 583 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n10_clear_all_plain | 583 | 2550 | 1967 | 416/16 | 416/16 | 0 | 0 | 3 | 228 | 25 | 1761 | 437 |
| address_fixed_n10_cursor_first_all_plain | 583 | 1082 | 499 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 502 | 228 |
| address_fixed_n10_for_each_read_all_plain | 583 | 1247 | 664 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 667 | 228 |
| address_fixed_n10_insert_front_plain | 583 | 1896 | 1313 | 416/456 | 416/456 | 400 | 0 | 3 | 228 | 25 | 1244 | 302 |
| address_fixed_n10_replace_front_plain | 583 | 3246 | 2663 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2666 | 228 |
| address_fixed_n10_remove_front_plain | 583 | 1959 | 1376 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 1170 | 437 |
| address_fixed_n10_remove_into_front_plain | 583 | 3150 | 2567 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 2361 | 437 |
| address_fixed_n10_at_mut_front_plain | 583 | 1799 | 1216 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1219 | 228 |
| address_fixed_n10_lookup_hit_front_plain | 233 | 1236 | 1003 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1011 | 14 |
| address_fixed_n10_lookup_miss_front_plain | 233 | 1088 | 855 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 865 | 14 |
| address_fixed_n10_entry_occupied_front_plain | 583 | 2704 | 2121 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2128 | 228 |
| address_fixed_n10_entry_vacant_front_plain | 583 | 4398 | 3815 | 416/456 | 416/456 | 400 | 0 | 3 | 224 | 25 | 3750 | 302 |
| address_fixed_n10_insert_middle_plain | 583 | 2362 | 1779 | 416/456 | 416/456 | 200 | 0 | 3 | 228 | 25 | 1710 | 302 |
| address_fixed_n10_replace_middle_plain | 583 | 3454 | 2871 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2874 | 228 |
| address_fixed_n10_remove_middle_plain | 583 | 2063 | 1480 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 1274 | 437 |
| address_fixed_n10_remove_into_middle_plain | 583 | 3254 | 2671 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 2465 | 437 |
| address_fixed_n10_at_mut_middle_plain | 583 | 1903 | 1320 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1323 | 228 |
| address_fixed_n10_lookup_hit_middle_plain | 233 | 837 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n10_lookup_miss_middle_plain | 233 | 971 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 748 | 14 |
| address_fixed_n10_entry_occupied_middle_plain | 583 | 2808 | 2225 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2232 | 228 |
| address_fixed_n10_entry_vacant_middle_plain | 583 | 4934 | 4351 | 416/456 | 416/456 | 200 | 0 | 3 | 224 | 25 | 4286 | 302 |
| address_fixed_n10_insert_end_plain | 583 | 1718 | 1135 | 416/456 | 416/456 | 0 | 0 | 3 | 228 | 25 | 1066 | 302 |
| address_fixed_n10_replace_end_plain | 583 | 3170 | 2587 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2590 | 228 |
| address_fixed_n10_remove_end_plain | 583 | 1748 | 1165 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 959 | 437 |
| address_fixed_n10_remove_into_end_plain | 583 | 2933 | 2350 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 2144 | 437 |
| address_fixed_n10_at_mut_end_plain | 583 | 1761 | 1178 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1181 | 228 |
| address_fixed_n10_lookup_hit_end_plain | 233 | 1119 | 886 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 894 | 14 |
| address_fixed_n10_lookup_miss_end_plain | 233 | 971 | 738 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 748 | 14 |
| address_fixed_n10_entry_occupied_end_plain | 583 | 2666 | 2083 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2090 | 228 |
| address_fixed_n10_entry_vacant_end_plain | 583 | 4164 | 3581 | 416/456 | 416/456 | 0 | 0 | 3 | 224 | 25 | 3516 | 302 |
| address_fixed_n10_load_trusted_all_common | 235 | 235 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n10_late_read_all_common | 235 | 1699 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1472 | 14 |
| address_fixed_n10_validate_all_common | 235 | 945 | 710 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 717 | 14 |
| address_fixed_n10_load_all_common | 235 | 961 | 726 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 758 | 7 | 14 |
| address_fixed_n10_mutable_baseline_all_common | 585 | 585 | 0 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n10_clear_all_common | 585 | 2552 | 1967 | 416/16 | 416/16 | 0 | 0 | 3 | 228 | 25 | 1761 | 437 |
| address_fixed_n10_cursor_first_all_common | 585 | 1084 | 499 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 502 | 228 |
| address_fixed_n10_for_each_read_all_common | 585 | 1249 | 664 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 667 | 228 |
| address_fixed_n10_insert_front_common | 585 | 1916 | 1331 | 416/456 | 416/456 | 400 | 0 | 3 | 228 | 25 | 1262 | 302 |
| address_fixed_n10_replace_front_common | 585 | 3266 | 2681 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2684 | 228 |
| address_fixed_n10_remove_front_common | 585 | 1970 | 1385 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 1179 | 437 |
| address_fixed_n10_remove_into_front_common | 585 | 3161 | 2576 | 416/376 | 416/376 | 360 | 0 | 3 | 228 | 25 | 2370 | 437 |
| address_fixed_n10_at_mut_front_common | 585 | 1810 | 1225 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1228 | 228 |
| address_fixed_n10_lookup_hit_front_common | 235 | 2105 | 1870 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1878 | 14 |
| address_fixed_n10_lookup_miss_front_common | 235 | 2246 | 2011 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 2021 | 14 |
| address_fixed_n10_entry_occupied_front_common | 585 | 2715 | 2130 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2137 | 228 |
| address_fixed_n10_entry_vacant_front_common | 585 | 4433 | 3848 | 416/456 | 416/456 | 400 | 0 | 3 | 224 | 25 | 3783 | 302 |
| address_fixed_n10_insert_middle_common | 585 | 2442 | 1857 | 416/456 | 416/456 | 200 | 0 | 3 | 228 | 25 | 1788 | 302 |
| address_fixed_n10_replace_middle_common | 585 | 3492 | 2907 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2910 | 228 |
| address_fixed_n10_remove_middle_common | 585 | 2083 | 1498 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 1292 | 437 |
| address_fixed_n10_remove_into_middle_common | 585 | 3274 | 2689 | 416/376 | 416/376 | 160 | 0 | 3 | 228 | 25 | 2483 | 437 |
| address_fixed_n10_at_mut_middle_common | 585 | 1923 | 1338 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1341 | 228 |
| address_fixed_n10_lookup_hit_middle_common | 235 | 839 | 604 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n10_lookup_miss_middle_common | 235 | 1840 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1615 | 14 |
| address_fixed_n10_entry_occupied_middle_common | 585 | 2828 | 2243 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2250 | 228 |
| address_fixed_n10_entry_vacant_middle_common | 585 | 5041 | 4456 | 416/456 | 416/456 | 200 | 0 | 3 | 224 | 25 | 4391 | 302 |
| address_fixed_n10_insert_end_common | 585 | 1729 | 1144 | 416/456 | 416/456 | 0 | 0 | 3 | 228 | 25 | 1075 | 302 |
| address_fixed_n10_replace_end_common | 585 | 3172 | 2587 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 2590 | 228 |
| address_fixed_n10_remove_end_common | 585 | 1750 | 1165 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 959 | 437 |
| address_fixed_n10_remove_into_end_common | 585 | 2935 | 2350 | 416/376 | 416/376 | 0 | 0 | 3 | 228 | 25 | 2144 | 437 |
| address_fixed_n10_at_mut_end_common | 585 | 1763 | 1178 | 416/416 | 416/416 | 0 | 0 | 3 | 228 | 25 | 1181 | 228 |
| address_fixed_n10_lookup_hit_end_common | 235 | 1699 | 1464 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1472 | 14 |
| address_fixed_n10_lookup_miss_end_common | 235 | 1840 | 1605 | 416/416 | 416/416 | 0 | 0 | 3 | 80 | 34 | 1615 | 14 |
| address_fixed_n10_entry_occupied_end_common | 585 | 2668 | 2083 | 416/416 | 416/416 | 0 | 0 | 3 | 224 | 25 | 2090 | 228 |
| address_fixed_n10_entry_vacant_end_common | 585 | 4187 | 3602 | 416/456 | 416/456 | 0 | 0 | 3 | 224 | 25 | 3537 | 302 |
| address_indexed_n10_load_trusted_all_plain | 234 | 234 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n10_late_read_all_plain | 234 | 1340 | 1106 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1114 | 14 |
| address_indexed_n10_validate_all_plain | 234 | 1405 | 1171 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1178 | 14 |
| address_indexed_n10_load_all_plain | 234 | 1421 | 1187 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 1219 | 7 | 14 |
| address_indexed_n10_mutable_baseline_all_plain | 584 | 584 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n10_clear_all_plain | 584 | 2834 | 2250 | 456/16 | 456/16 | 0 | 0 | 3 | 229 | 25 | 2044 | 437 |
| address_indexed_n10_cursor_first_all_plain | 584 | 4046 | 3462 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3391 | 302 |
| address_indexed_n10_for_each_read_all_plain | 584 | 3752 | 3168 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 3171 | 228 |
| address_indexed_n10_insert_front_plain | 584 | 4293 | 3709 | 456/509 | 456/509 | 440 | 11 | 3 | 229 | 25 | 3640 | 302 |
| address_indexed_n10_replace_front_plain | 584 | 3972 | 3388 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3317 | 302 |
| address_indexed_n10_remove_front_plain | 584 | 4497 | 3913 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3707 | 437 |
| address_indexed_n10_remove_into_front_plain | 584 | 4147 | 3563 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3357 | 437 |
| address_indexed_n10_at_mut_front_plain | 584 | 3399 | 2815 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 2744 | 302 |
| address_indexed_n10_lookup_hit_front_plain | 234 | 1416 | 1182 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1190 | 14 |
| address_indexed_n10_lookup_miss_front_plain | 234 | 1046 | 812 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 822 | 14 |
| address_indexed_n10_entry_occupied_front_plain | 584 | 3380 | 2796 | 456/465 | 456/465 | 72 | 10 | 3 | 225 | 25 | 2729 | 302 |
| address_indexed_n10_entry_vacant_front_plain | 584 | 5340 | 4756 | 456/509 | 456/509 | 440 | 11 | 3 | 225 | 25 | 4691 | 302 |
| address_indexed_n10_insert_middle_plain | 584 | 4860 | 4276 | 456/509 | 456/509 | 260 | 11 | 3 | 229 | 25 | 4207 | 302 |
| address_indexed_n10_replace_middle_plain | 584 | 3972 | 3388 | 456/465 | 456/465 | 32 | 10 | 3 | 229 | 25 | 3317 | 302 |
| address_indexed_n10_remove_middle_plain | 584 | 4670 | 4086 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3880 | 437 |
| address_indexed_n10_remove_into_middle_plain | 584 | 4322 | 3738 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3532 | 437 |
| address_indexed_n10_at_mut_middle_plain | 584 | 3419 | 2835 | 456/465 | 456/465 | 32 | 5 | 3 | 229 | 25 | 2764 | 302 |
| address_indexed_n10_lookup_hit_middle_plain | 234 | 1057 | 823 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 831 | 14 |
| address_indexed_n10_lookup_miss_middle_plain | 234 | 929 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 705 | 14 |
| address_indexed_n10_entry_occupied_middle_plain | 584 | 3360 | 2776 | 456/465 | 456/465 | 32 | 5 | 3 | 225 | 25 | 2709 | 302 |
| address_indexed_n10_entry_vacant_middle_plain | 584 | 5978 | 5394 | 456/509 | 456/509 | 260 | 11 | 3 | 225 | 25 | 5329 | 302 |
| address_indexed_n10_insert_end_plain | 584 | 3708 | 3124 | 456/509 | 456/509 | 80 | 11 | 3 | 229 | 25 | 3055 | 302 |
| address_indexed_n10_replace_end_plain | 584 | 3431 | 2847 | 456/465 | 456/465 | 0 | 10 | 3 | 229 | 25 | 2776 | 302 |
| address_indexed_n10_remove_end_plain | 584 | 3956 | 3372 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 3166 | 437 |
| address_indexed_n10_remove_into_end_plain | 584 | 3616 | 3032 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 2826 | 437 |
| address_indexed_n10_at_mut_end_plain | 584 | 2978 | 2394 | 456/465 | 456/465 | 0 | 1 | 3 | 229 | 25 | 2323 | 302 |
| address_indexed_n10_lookup_hit_end_plain | 234 | 1340 | 1106 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1114 | 14 |
| address_indexed_n10_lookup_miss_end_plain | 234 | 929 | 695 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 705 | 14 |
| address_indexed_n10_entry_occupied_end_plain | 584 | 2918 | 2334 | 456/465 | 456/465 | 0 | 1 | 3 | 225 | 25 | 2267 | 302 |
| address_indexed_n10_entry_vacant_end_plain | 584 | 4757 | 4173 | 456/509 | 456/509 | 80 | 11 | 3 | 225 | 25 | 4108 | 302 |
| address_indexed_n10_load_trusted_all_common | 236 | 236 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n10_late_read_all_common | 236 | 1920 | 1684 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1692 | 14 |
| address_indexed_n10_validate_all_common | 236 | 1488 | 1252 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1259 | 14 |
| address_indexed_n10_load_all_common | 236 | 1504 | 1268 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 1300 | 7 | 14 |
| address_indexed_n10_mutable_baseline_all_common | 586 | 586 | 0 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n10_clear_all_common | 586 | 2836 | 2250 | 456/16 | 456/16 | 0 | 0 | 3 | 229 | 25 | 2044 | 437 |
| address_indexed_n10_cursor_first_all_common | 586 | 4048 | 3462 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3391 | 302 |
| address_indexed_n10_for_each_read_all_common | 586 | 3835 | 3249 | 456/456 | 456/456 | 0 | 0 | 3 | 229 | 25 | 3252 | 228 |
| address_indexed_n10_insert_front_common | 586 | 4313 | 3727 | 456/509 | 456/509 | 440 | 11 | 3 | 229 | 25 | 3658 | 302 |
| address_indexed_n10_replace_front_common | 586 | 3983 | 3397 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 3326 | 302 |
| address_indexed_n10_remove_front_common | 586 | 4508 | 3922 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3716 | 437 |
| address_indexed_n10_remove_into_front_common | 586 | 4158 | 3572 | 456/412 | 456/412 | 396 | 9 | 3 | 229 | 25 | 3366 | 437 |
| address_indexed_n10_at_mut_front_common | 586 | 3410 | 2824 | 456/465 | 456/465 | 72 | 10 | 3 | 229 | 25 | 2753 | 302 |
| address_indexed_n10_lookup_hit_front_common | 236 | 2285 | 2049 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 2057 | 14 |
| address_indexed_n10_lookup_miss_front_common | 236 | 2204 | 1968 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1978 | 14 |
| address_indexed_n10_entry_occupied_front_common | 586 | 3391 | 2805 | 456/465 | 456/465 | 72 | 10 | 3 | 225 | 25 | 2738 | 302 |
| address_indexed_n10_entry_vacant_front_common | 586 | 5375 | 4789 | 456/509 | 456/509 | 440 | 11 | 3 | 225 | 25 | 4724 | 302 |
| address_indexed_n10_insert_middle_common | 586 | 4940 | 4354 | 456/509 | 456/509 | 260 | 11 | 3 | 229 | 25 | 4285 | 302 |
| address_indexed_n10_replace_middle_common | 586 | 3992 | 3406 | 456/465 | 456/465 | 32 | 10 | 3 | 229 | 25 | 3335 | 302 |
| address_indexed_n10_remove_middle_common | 586 | 4690 | 4104 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3898 | 437 |
| address_indexed_n10_remove_into_middle_common | 586 | 4342 | 3756 | 456/412 | 456/412 | 216 | 9 | 3 | 229 | 25 | 3550 | 437 |
| address_indexed_n10_at_mut_middle_common | 586 | 3439 | 2853 | 456/465 | 456/465 | 32 | 5 | 3 | 229 | 25 | 2782 | 302 |
| address_indexed_n10_lookup_hit_middle_common | 236 | 1059 | 823 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 831 | 14 |
| address_indexed_n10_lookup_miss_middle_common | 236 | 1798 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1572 | 14 |
| address_indexed_n10_entry_occupied_middle_common | 586 | 3380 | 2794 | 456/465 | 456/465 | 32 | 5 | 3 | 225 | 25 | 2727 | 302 |
| address_indexed_n10_entry_vacant_middle_common | 586 | 6085 | 5499 | 456/509 | 456/509 | 260 | 11 | 3 | 225 | 25 | 5434 | 302 |
| address_indexed_n10_insert_end_common | 586 | 3719 | 3133 | 456/509 | 456/509 | 80 | 11 | 3 | 229 | 25 | 3064 | 302 |
| address_indexed_n10_replace_end_common | 586 | 3433 | 2847 | 456/465 | 456/465 | 0 | 10 | 3 | 229 | 25 | 2776 | 302 |
| address_indexed_n10_remove_end_common | 586 | 3958 | 3372 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 3166 | 437 |
| address_indexed_n10_remove_into_end_common | 586 | 3618 | 3032 | 456/412 | 456/412 | 72 | 9 | 3 | 229 | 25 | 2826 | 437 |
| address_indexed_n10_at_mut_end_common | 586 | 2980 | 2394 | 456/465 | 456/465 | 0 | 1 | 3 | 229 | 25 | 2323 | 302 |
| address_indexed_n10_lookup_hit_end_common | 236 | 1920 | 1684 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1692 | 14 |
| address_indexed_n10_lookup_miss_end_common | 236 | 1798 | 1562 | 456/456 | 456/456 | 0 | 0 | 3 | 81 | 34 | 1572 | 14 |
| address_indexed_n10_entry_occupied_end_common | 586 | 2920 | 2334 | 456/465 | 456/465 | 0 | 1 | 3 | 225 | 25 | 2267 | 302 |
| address_indexed_n10_entry_vacant_end_common | 586 | 4780 | 4194 | 456/509 | 456/509 | 80 | 11 | 3 | 225 | 25 | 4129 | 302 |
| list_fixed_n100_load_trusted_all_plain | 223 | 223 | 0 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 34 | 9 | 14 |
| list_fixed_n100_late_read_all_plain | 223 | 437 | 214 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 34 | 222 | 14 |
| list_fixed_n100_validate_all_plain | 223 | 496 | 273 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 34 | 280 | 14 |
| list_fixed_n100_load_all_plain | 223 | 512 | 289 | 816/816 | 816/816 | 0 | 0 | 3 | 71 | 321 | 7 | 14 |
| list_fixed_n100_mutable_baseline_all_plain | 573 | 573 | 0 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 4 | 228 |
| list_fixed_n100_clear_all_plain | 573 | 2418 | 1845 | 816/16 | 816/16 | 0 | 0 | 3 | 219 | 25 | 1639 | 437 |
| list_fixed_n100_cursor_first_all_plain | 573 | 788 | 215 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 218 | 228 |
| list_fixed_n100_for_each_read_all_plain | 573 | 1916 | 1343 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1346 | 228 |
| list_fixed_n100_insert_front_plain | 573 | 2422 | 1849 | 816/824 | 816/824 | 800 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n100_replace_front_plain | 573 | 1927 | 1354 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n100_remove_front_plain | 573 | 1542 | 969 | 816/808 | 816/808 | 792 | 0 | 3 | 219 | 25 | 759 | 437 |
| list_fixed_n100_remove_into_front_plain | 573 | 2615 | 2042 | 816/808 | 816/808 | 792 | 0 | 3 | 219 | 25 | 1836 | 437 |
| list_fixed_n100_at_mut_front_plain | 573 | 1427 | 854 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n100_insert_bulk_front_plain | 573 | 2673 | 2100 | 816/832 | 816/832 | 800 | 0 | 3 | 219 | 25 | 2029 | 302 |
| list_fixed_n100_truncate_front_plain | 573 | 2428 | 1855 | 816/16 | 816/16 | 0 | 0 | 3 | 219 | 25 | 1649 | 437 |
| list_fixed_n100_insert_middle_plain | 573 | 2422 | 1849 | 816/824 | 816/824 | 400 | 0 | 3 | 219 | 25 | 1780 | 302 |
| list_fixed_n100_replace_middle_plain | 573 | 1927 | 1354 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n100_remove_middle_plain | 573 | 1542 | 969 | 816/808 | 816/808 | 392 | 0 | 3 | 219 | 25 | 759 | 437 |
| list_fixed_n100_remove_into_middle_plain | 573 | 2615 | 2042 | 816/808 | 816/808 | 392 | 0 | 3 | 219 | 25 | 1836 | 437 |
| list_fixed_n100_at_mut_middle_plain | 573 | 1427 | 854 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n100_insert_bulk_middle_plain | 573 | 2674 | 2101 | 816/832 | 816/832 | 400 | 0 | 3 | 219 | 25 | 2030 | 302 |
| list_fixed_n100_truncate_middle_plain | 573 | 2336 | 1763 | 816/416 | 816/416 | 0 | 0 | 3 | 219 | 25 | 1557 | 437 |
| list_fixed_n100_insert_end_plain | 573 | 2226 | 1653 | 816/824 | 816/824 | 0 | 0 | 3 | 219 | 25 | 1584 | 302 |
| list_fixed_n100_replace_end_plain | 573 | 1927 | 1354 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n100_remove_end_plain | 573 | 1369 | 796 | 816/808 | 816/808 | 0 | 0 | 3 | 219 | 25 | 586 | 437 |
| list_fixed_n100_remove_into_end_plain | 573 | 2436 | 1863 | 816/808 | 816/808 | 0 | 0 | 3 | 219 | 25 | 1657 | 437 |
| list_fixed_n100_at_mut_end_plain | 573 | 1427 | 854 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n100_insert_bulk_end_plain | 573 | 2480 | 1907 | 816/832 | 816/832 | 0 | 0 | 3 | 219 | 25 | 1836 | 302 |
| list_fixed_n100_truncate_end_plain | 573 | 1907 | 1334 | 816/816 | 816/816 | 0 | 0 | 3 | 219 | 25 | 1337 | 228 |
| list_fixed_n100_push_end_plain | 573 | 1327 | 754 | 816/824 | 816/824 | 0 | 0 | 3 | 219 | 25 | 683 | 302 |
| list_fixed_n100_pop_end_plain | 573 | 2328 | 1755 | 816/808 | 816/808 | 0 | 0 | 3 | 219 | 25 | 1549 | 437 |
| list_fixed_n100_extend_end_plain | 573 | 2479 | 1906 | 816/832 | 816/832 | 0 | 0 | 3 | 219 | 25 | 1835 | 302 |
| list_indexed_n100_load_trusted_all_plain | 224 | 224 | 0 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| list_indexed_n100_late_read_all_plain | 224 | 1005 | 781 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 34 | 789 | 14 |
| list_indexed_n100_validate_all_plain | 224 | 2238 | 2014 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 34 | 2021 | 14 |
| list_indexed_n100_load_all_plain | 224 | 2254 | 2030 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 72 | 2062 | 7 | 14 |
| list_indexed_n100_mutable_baseline_all_plain | 574 | 574 | 0 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| list_indexed_n100_clear_all_plain | 574 | 2357 | 1783 | 1212/16 | 1212/16 | 0 | 0 | 3 | 220 | 25 | 1577 | 437 |
| list_indexed_n100_cursor_first_all_plain | 574 | 5165 | 4591 | 1212/1221 | 1212/1221 | 792 | 99 | 3 | 220 | 25 | 4520 | 302 |
| list_indexed_n100_for_each_read_all_plain | 574 | 20967 | 20393 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 220 | 25 | 20396 | 228 |
| list_indexed_n100_insert_front_plain | 574 | 6024 | 5450 | 1212/1233 | 1212/1233 | 1196 | 100 | 3 | 220 | 25 | 5381 | 302 |
| list_indexed_n100_replace_front_plain | 574 | 7188 | 6614 | 1212/1221 | 1212/1221 | 792 | 99 | 3 | 220 | 25 | 6543 | 302 |
| list_indexed_n100_remove_front_plain | 574 | 7635 | 7061 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 6851 | 437 |
| list_indexed_n100_remove_into_front_plain | 574 | 7329 | 6755 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 6549 | 437 |
| list_indexed_n100_at_mut_front_plain | 574 | 4573 | 3999 | 1212/1221 | 1212/1221 | 792 | 99 | 3 | 220 | 25 | 3928 | 302 |
| list_indexed_n100_insert_bulk_front_plain | 574 | 6636 | 6062 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 5991 | 302 |
| list_indexed_n100_truncate_front_plain | 574 | 2361 | 1787 | 1212/16 | 1212/16 | 0 | 0 | 3 | 220 | 25 | 1581 | 437 |
| list_indexed_n100_insert_middle_plain | 574 | 4639 | 4065 | 1212/1233 | 1212/1233 | 996 | 100 | 3 | 220 | 25 | 3996 | 302 |
| list_indexed_n100_replace_middle_plain | 574 | 5110 | 4536 | 1212/1221 | 1212/1221 | 392 | 99 | 3 | 220 | 25 | 4465 | 302 |
| list_indexed_n100_remove_middle_plain | 574 | 5741 | 5167 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 4957 | 437 |
| list_indexed_n100_remove_into_middle_plain | 574 | 5435 | 4861 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 4655 | 437 |
| list_indexed_n100_at_mut_middle_plain | 574 | 3696 | 3122 | 1212/1221 | 1212/1221 | 392 | 49 | 3 | 220 | 25 | 3051 | 302 |
| list_indexed_n100_insert_bulk_middle_plain | 574 | 5013 | 4439 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 4368 | 302 |
| list_indexed_n100_truncate_middle_plain | 574 | 3102 | 2528 | 1212/612 | 1212/612 | 400 | 49 | 3 | 220 | 25 | 2322 | 437 |
| list_indexed_n100_insert_end_plain | 574 | 2827 | 2253 | 1212/1233 | 1212/1233 | 800 | 100 | 3 | 220 | 25 | 2184 | 302 |
| list_indexed_n100_replace_end_plain | 574 | 2787 | 2213 | 1212/1221 | 1212/1221 | 0 | 99 | 3 | 220 | 25 | 2142 | 302 |
| list_indexed_n100_remove_end_plain | 574 | 3472 | 2898 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 2688 | 437 |
| list_indexed_n100_remove_into_end_plain | 574 | 3166 | 2592 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 2386 | 437 |
| list_indexed_n100_at_mut_end_plain | 574 | 2489 | 1915 | 1212/1221 | 1212/1221 | 0 | 0 | 3 | 220 | 25 | 1844 | 302 |
| list_indexed_n100_insert_bulk_end_plain | 574 | 2954 | 2380 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 2309 | 302 |
| list_indexed_n100_truncate_end_plain | 574 | 2407 | 1833 | 1212/1212 | 1212/1212 | 0 | 0 | 3 | 220 | 25 | 1836 | 228 |
| list_indexed_n100_push_end_plain | 574 | 2840 | 2266 | 1212/1233 | 1212/1233 | 800 | 100 | 3 | 220 | 25 | 2195 | 302 |
| list_indexed_n100_pop_end_plain | 574 | 3162 | 2588 | 1212/1200 | 1212/1200 | 792 | 98 | 3 | 220 | 25 | 2382 | 437 |
| list_indexed_n100_extend_end_plain | 574 | 2953 | 2379 | 1212/1254 | 1212/1254 | 800 | 101 | 3 | 220 | 25 | 2308 | 302 |
| map_fixed_n100_load_trusted_all_plain | 228 | 228 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n100_late_read_all_plain | 228 | 1537 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1317 | 14 |
| map_fixed_n100_validate_all_plain | 228 | 2747 | 2519 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2526 | 14 |
| map_fixed_n100_load_all_plain | 228 | 2763 | 2535 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 2567 | 7 | 14 |
| map_fixed_n100_mutable_baseline_all_plain | 578 | 578 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n100_clear_all_plain | 578 | 3462 | 2884 | 4016/16 | 4016/16 | 0 | 0 | 3 | 223 | 25 | 2678 | 437 |
| map_fixed_n100_cursor_first_all_plain | 578 | 1077 | 499 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 502 | 228 |
| map_fixed_n100_for_each_read_all_plain | 578 | 2412 | 1834 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1837 | 228 |
| map_fixed_n100_insert_front_plain | 578 | 2287 | 1709 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 223 | 25 | 1640 | 302 |
| map_fixed_n100_replace_front_plain | 578 | 3241 | 2663 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2666 | 228 |
| map_fixed_n100_remove_front_plain | 578 | 2311 | 1733 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 1527 | 437 |
| map_fixed_n100_remove_into_front_plain | 578 | 3502 | 2924 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 2718 | 437 |
| map_fixed_n100_at_mut_front_plain | 578 | 1794 | 1216 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1219 | 228 |
| map_fixed_n100_lookup_hit_front_plain | 228 | 1630 | 1402 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1410 | 14 |
| map_fixed_n100_lookup_miss_front_plain | 228 | 1482 | 1254 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1264 | 14 |
| map_fixed_n100_entry_occupied_front_plain | 578 | 2699 | 2121 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2128 | 228 |
| map_fixed_n100_entry_vacant_front_plain | 578 | 4789 | 4211 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 219 | 25 | 4146 | 302 |
| map_fixed_n100_insert_middle_plain | 578 | 2738 | 2160 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 223 | 25 | 2091 | 302 |
| map_fixed_n100_replace_middle_plain | 578 | 3449 | 2871 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2874 | 228 |
| map_fixed_n100_remove_middle_plain | 578 | 2177 | 1599 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 1393 | 437 |
| map_fixed_n100_remove_into_middle_plain | 578 | 3368 | 2790 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 2584 | 437 |
| map_fixed_n100_at_mut_middle_plain | 578 | 1898 | 1320 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1323 | 228 |
| map_fixed_n100_lookup_hit_middle_plain | 228 | 832 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n100_lookup_miss_middle_plain | 228 | 1389 | 1161 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1171 | 14 |
| map_fixed_n100_entry_occupied_middle_plain | 578 | 2803 | 2225 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2232 | 228 |
| map_fixed_n100_entry_vacant_middle_plain | 578 | 5310 | 4732 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 219 | 25 | 4667 | 302 |
| map_fixed_n100_insert_end_plain | 578 | 1713 | 1135 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 223 | 25 | 1066 | 302 |
| map_fixed_n100_replace_end_plain | 578 | 3165 | 2587 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2590 | 228 |
| map_fixed_n100_remove_end_plain | 578 | 1743 | 1165 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 959 | 437 |
| map_fixed_n100_remove_into_end_plain | 578 | 2928 | 2350 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 2144 | 437 |
| map_fixed_n100_at_mut_end_plain | 578 | 1756 | 1178 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1181 | 228 |
| map_fixed_n100_lookup_hit_end_plain | 228 | 1537 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1317 | 14 |
| map_fixed_n100_lookup_miss_end_plain | 228 | 1530 | 1302 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 1312 | 14 |
| map_fixed_n100_entry_occupied_end_plain | 578 | 2661 | 2083 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2090 | 228 |
| map_fixed_n100_entry_vacant_end_plain | 578 | 4159 | 3581 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 219 | 25 | 3516 | 302 |
| map_fixed_n100_load_trusted_all_common | 230 | 230 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n100_late_read_all_common | 230 | 2984 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2762 | 14 |
| map_fixed_n100_validate_all_common | 230 | 3640 | 3410 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3417 | 14 |
| map_fixed_n100_load_all_common | 230 | 3656 | 3426 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 3458 | 7 | 14 |
| map_fixed_n100_mutable_baseline_all_common | 580 | 580 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n100_clear_all_common | 580 | 3464 | 2884 | 4016/16 | 4016/16 | 0 | 0 | 3 | 223 | 25 | 2678 | 437 |
| map_fixed_n100_cursor_first_all_common | 580 | 1079 | 499 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 502 | 228 |
| map_fixed_n100_for_each_read_all_common | 580 | 2414 | 1834 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1837 | 228 |
| map_fixed_n100_insert_front_common | 580 | 2307 | 1727 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 223 | 25 | 1658 | 302 |
| map_fixed_n100_replace_front_common | 580 | 3261 | 2681 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2684 | 228 |
| map_fixed_n100_remove_front_common | 580 | 2322 | 1742 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 1536 | 437 |
| map_fixed_n100_remove_into_front_common | 580 | 3513 | 2933 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 223 | 25 | 2727 | 437 |
| map_fixed_n100_at_mut_front_common | 580 | 1805 | 1225 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1228 | 228 |
| map_fixed_n100_lookup_hit_front_common | 230 | 3366 | 3136 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3144 | 14 |
| map_fixed_n100_lookup_miss_front_common | 230 | 3507 | 3277 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3287 | 14 |
| map_fixed_n100_entry_occupied_front_common | 580 | 2710 | 2130 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2137 | 228 |
| map_fixed_n100_entry_vacant_front_common | 580 | 4824 | 4244 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 219 | 25 | 4179 | 302 |
| map_fixed_n100_insert_middle_common | 580 | 2854 | 2274 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 223 | 25 | 2205 | 302 |
| map_fixed_n100_replace_middle_common | 580 | 3487 | 2907 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2910 | 228 |
| map_fixed_n100_remove_middle_common | 580 | 2197 | 1617 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 1411 | 437 |
| map_fixed_n100_remove_into_middle_common | 580 | 3388 | 2808 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 223 | 25 | 2602 | 437 |
| map_fixed_n100_at_mut_middle_common | 580 | 1918 | 1338 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1341 | 228 |
| map_fixed_n100_lookup_hit_middle_common | 230 | 834 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n100_lookup_miss_middle_common | 230 | 3125 | 2895 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2905 | 14 |
| map_fixed_n100_entry_occupied_middle_common | 580 | 2823 | 2243 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2250 | 228 |
| map_fixed_n100_entry_vacant_middle_common | 580 | 5453 | 4873 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 219 | 25 | 4808 | 302 |
| map_fixed_n100_insert_end_common | 580 | 1724 | 1144 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 223 | 25 | 1075 | 302 |
| map_fixed_n100_replace_end_common | 580 | 3167 | 2587 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 2590 | 228 |
| map_fixed_n100_remove_end_common | 580 | 1745 | 1165 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 959 | 437 |
| map_fixed_n100_remove_into_end_common | 580 | 2930 | 2350 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 223 | 25 | 2144 | 437 |
| map_fixed_n100_at_mut_end_common | 580 | 1758 | 1178 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 223 | 25 | 1181 | 228 |
| map_fixed_n100_lookup_hit_end_common | 230 | 2984 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 2762 | 14 |
| map_fixed_n100_lookup_miss_end_common | 230 | 3555 | 3325 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 75 | 34 | 3335 | 14 |
| map_fixed_n100_entry_occupied_end_common | 580 | 2663 | 2083 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 219 | 25 | 2090 | 228 |
| map_fixed_n100_entry_vacant_end_common | 580 | 4182 | 3602 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 219 | 25 | 3537 | 302 |
| map_indexed_n100_load_trusted_all_plain | 226 | 226 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n100_late_read_all_plain | 226 | 1755 | 1529 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1537 | 14 |
| map_indexed_n100_validate_all_plain | 226 | 4608 | 4382 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 4389 | 14 |
| map_indexed_n100_load_all_plain | 226 | 4624 | 4398 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 4430 | 7 | 14 |
| map_indexed_n100_mutable_baseline_all_plain | 576 | 576 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n100_clear_all_plain | 576 | 3836 | 3260 | 4416/16 | 4416/16 | 0 | 0 | 3 | 220 | 25 | 3054 | 437 |
| map_indexed_n100_cursor_first_all_plain | 576 | 6558 | 5982 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5911 | 302 |
| map_indexed_n100_for_each_read_all_plain | 576 | 26424 | 25848 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 25851 | 228 |
| map_indexed_n100_insert_front_plain | 576 | 7651 | 7075 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 220 | 25 | 7006 | 302 |
| map_indexed_n100_replace_front_plain | 576 | 6934 | 6358 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 6287 | 302 |
| map_indexed_n100_remove_front_plain | 576 | 7816 | 7240 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 7034 | 437 |
| map_indexed_n100_remove_into_front_plain | 576 | 7466 | 6890 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 6684 | 437 |
| map_indexed_n100_at_mut_front_plain | 576 | 5911 | 5335 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5264 | 302 |
| map_indexed_n100_lookup_hit_front_plain | 226 | 1807 | 1581 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1589 | 14 |
| map_indexed_n100_lookup_miss_front_plain | 226 | 1437 | 1211 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1221 | 14 |
| map_indexed_n100_entry_occupied_front_plain | 576 | 5892 | 5316 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 216 | 25 | 5249 | 302 |
| map_indexed_n100_entry_vacant_front_plain | 576 | 8698 | 8122 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 216 | 25 | 8057 | 302 |
| map_indexed_n100_insert_middle_plain | 576 | 6718 | 6142 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 220 | 25 | 6073 | 302 |
| map_indexed_n100_replace_middle_plain | 576 | 5449 | 4873 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 220 | 25 | 4802 | 302 |
| map_indexed_n100_remove_middle_plain | 576 | 6266 | 5690 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5484 | 437 |
| map_indexed_n100_remove_into_middle_plain | 576 | 5918 | 5342 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5136 | 437 |
| map_indexed_n100_at_mut_middle_plain | 576 | 4671 | 4095 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 220 | 25 | 4024 | 302 |
| map_indexed_n100_lookup_hit_middle_plain | 226 | 1049 | 823 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 831 | 14 |
| map_indexed_n100_lookup_miss_middle_plain | 226 | 1344 | 1118 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1128 | 14 |
| map_indexed_n100_entry_occupied_middle_plain | 576 | 4612 | 4036 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 216 | 25 | 3969 | 302 |
| map_indexed_n100_entry_vacant_middle_plain | 576 | 7836 | 7260 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 216 | 25 | 7195 | 302 |
| map_indexed_n100_insert_end_plain | 576 | 3700 | 3124 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 220 | 25 | 3055 | 302 |
| map_indexed_n100_replace_end_plain | 576 | 3423 | 2847 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 220 | 25 | 2776 | 302 |
| map_indexed_n100_remove_end_plain | 576 | 3948 | 3372 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 3166 | 437 |
| map_indexed_n100_remove_into_end_plain | 576 | 3608 | 3032 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 2826 | 437 |
| map_indexed_n100_at_mut_end_plain | 576 | 2970 | 2394 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 220 | 25 | 2323 | 302 |
| map_indexed_n100_lookup_hit_end_plain | 226 | 1755 | 1529 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1537 | 14 |
| map_indexed_n100_lookup_miss_end_plain | 226 | 1485 | 1259 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 1269 | 14 |
| map_indexed_n100_entry_occupied_end_plain | 576 | 2910 | 2334 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 216 | 25 | 2267 | 302 |
| map_indexed_n100_entry_vacant_end_plain | 576 | 4749 | 4173 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 216 | 25 | 4108 | 302 |
| map_indexed_n100_load_trusted_all_common | 228 | 228 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n100_late_read_all_common | 228 | 3202 | 2974 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 2982 | 14 |
| map_indexed_n100_validate_all_common | 228 | 5501 | 5273 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 5280 | 14 |
| map_indexed_n100_load_all_common | 228 | 5517 | 5289 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 5321 | 7 | 14 |
| map_indexed_n100_mutable_baseline_all_common | 578 | 578 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n100_clear_all_common | 578 | 3838 | 3260 | 4416/16 | 4416/16 | 0 | 0 | 3 | 220 | 25 | 3054 | 437 |
| map_indexed_n100_cursor_first_all_common | 578 | 6560 | 5982 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5911 | 302 |
| map_indexed_n100_for_each_read_all_common | 578 | 27317 | 26739 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 220 | 25 | 26742 | 228 |
| map_indexed_n100_insert_front_common | 578 | 7671 | 7093 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 220 | 25 | 7024 | 302 |
| map_indexed_n100_replace_front_common | 578 | 6945 | 6367 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 6296 | 302 |
| map_indexed_n100_remove_front_common | 578 | 7827 | 7249 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 7043 | 437 |
| map_indexed_n100_remove_into_front_common | 578 | 7477 | 6899 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 220 | 25 | 6693 | 437 |
| map_indexed_n100_at_mut_front_common | 578 | 5922 | 5344 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 220 | 25 | 5273 | 302 |
| map_indexed_n100_lookup_hit_front_common | 228 | 3543 | 3315 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 3323 | 14 |
| map_indexed_n100_lookup_miss_front_common | 228 | 3462 | 3234 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 3244 | 14 |
| map_indexed_n100_entry_occupied_front_common | 578 | 5903 | 5325 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 216 | 25 | 5258 | 302 |
| map_indexed_n100_entry_vacant_front_common | 578 | 8733 | 8155 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 216 | 25 | 8090 | 302 |
| map_indexed_n100_insert_middle_common | 578 | 6834 | 6256 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 220 | 25 | 6187 | 302 |
| map_indexed_n100_replace_middle_common | 578 | 5469 | 4891 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 220 | 25 | 4820 | 302 |
| map_indexed_n100_remove_middle_common | 578 | 6286 | 5708 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5502 | 437 |
| map_indexed_n100_remove_into_middle_common | 578 | 5938 | 5360 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 220 | 25 | 5154 | 437 |
| map_indexed_n100_at_mut_middle_common | 578 | 4691 | 4113 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 220 | 25 | 4042 | 302 |
| map_indexed_n100_lookup_hit_middle_common | 228 | 1051 | 823 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 831 | 14 |
| map_indexed_n100_lookup_miss_middle_common | 228 | 3080 | 2852 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 2862 | 14 |
| map_indexed_n100_entry_occupied_middle_common | 578 | 4632 | 4054 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 216 | 25 | 3987 | 302 |
| map_indexed_n100_entry_vacant_middle_common | 578 | 7979 | 7401 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 216 | 25 | 7336 | 302 |
| map_indexed_n100_insert_end_common | 578 | 3711 | 3133 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 220 | 25 | 3064 | 302 |
| map_indexed_n100_replace_end_common | 578 | 3425 | 2847 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 220 | 25 | 2776 | 302 |
| map_indexed_n100_remove_end_common | 578 | 3950 | 3372 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 3166 | 437 |
| map_indexed_n100_remove_into_end_common | 578 | 3610 | 3032 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 220 | 25 | 2826 | 437 |
| map_indexed_n100_at_mut_end_common | 578 | 2972 | 2394 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 220 | 25 | 2323 | 302 |
| map_indexed_n100_lookup_hit_end_common | 228 | 3202 | 2974 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 2982 | 14 |
| map_indexed_n100_lookup_miss_end_common | 228 | 3510 | 3282 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 72 | 34 | 3292 | 14 |
| map_indexed_n100_entry_occupied_end_common | 578 | 2912 | 2334 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 216 | 25 | 2267 | 302 |
| map_indexed_n100_entry_vacant_end_common | 578 | 4772 | 4194 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 216 | 25 | 4129 | 302 |
| address_fixed_n100_load_trusted_all_plain | 233 | 233 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n100_late_read_all_plain | 233 | 1542 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1317 | 14 |
| address_fixed_n100_validate_all_plain | 233 | 2752 | 2519 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2526 | 14 |
| address_fixed_n100_load_all_plain | 233 | 2768 | 2535 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 2567 | 7 | 14 |
| address_fixed_n100_mutable_baseline_all_plain | 583 | 583 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n100_clear_all_plain | 583 | 3467 | 2884 | 4016/16 | 4016/16 | 0 | 0 | 3 | 228 | 25 | 2678 | 437 |
| address_fixed_n100_cursor_first_all_plain | 583 | 1082 | 499 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 502 | 228 |
| address_fixed_n100_for_each_read_all_plain | 583 | 2417 | 1834 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1837 | 228 |
| address_fixed_n100_insert_front_plain | 583 | 2292 | 1709 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 228 | 25 | 1640 | 302 |
| address_fixed_n100_replace_front_plain | 583 | 3246 | 2663 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2666 | 228 |
| address_fixed_n100_remove_front_plain | 583 | 2316 | 1733 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 1527 | 437 |
| address_fixed_n100_remove_into_front_plain | 583 | 3507 | 2924 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 2718 | 437 |
| address_fixed_n100_at_mut_front_plain | 583 | 1799 | 1216 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1219 | 228 |
| address_fixed_n100_lookup_hit_front_plain | 233 | 1635 | 1402 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1410 | 14 |
| address_fixed_n100_lookup_miss_front_plain | 233 | 1487 | 1254 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1264 | 14 |
| address_fixed_n100_entry_occupied_front_plain | 583 | 2704 | 2121 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2128 | 228 |
| address_fixed_n100_entry_vacant_front_plain | 583 | 4794 | 4211 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 224 | 25 | 4146 | 302 |
| address_fixed_n100_insert_middle_plain | 583 | 2743 | 2160 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 228 | 25 | 2091 | 302 |
| address_fixed_n100_replace_middle_plain | 583 | 3454 | 2871 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2874 | 228 |
| address_fixed_n100_remove_middle_plain | 583 | 2182 | 1599 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 1393 | 437 |
| address_fixed_n100_remove_into_middle_plain | 583 | 3373 | 2790 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 2584 | 437 |
| address_fixed_n100_at_mut_middle_plain | 583 | 1903 | 1320 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1323 | 228 |
| address_fixed_n100_lookup_hit_middle_plain | 233 | 837 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n100_lookup_miss_middle_plain | 233 | 1394 | 1161 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1171 | 14 |
| address_fixed_n100_entry_occupied_middle_plain | 583 | 2808 | 2225 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2232 | 228 |
| address_fixed_n100_entry_vacant_middle_plain | 583 | 5315 | 4732 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 224 | 25 | 4667 | 302 |
| address_fixed_n100_insert_end_plain | 583 | 1718 | 1135 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 228 | 25 | 1066 | 302 |
| address_fixed_n100_replace_end_plain | 583 | 3170 | 2587 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2590 | 228 |
| address_fixed_n100_remove_end_plain | 583 | 1748 | 1165 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 959 | 437 |
| address_fixed_n100_remove_into_end_plain | 583 | 2933 | 2350 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 2144 | 437 |
| address_fixed_n100_at_mut_end_plain | 583 | 1761 | 1178 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1181 | 228 |
| address_fixed_n100_lookup_hit_end_plain | 233 | 1542 | 1309 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1317 | 14 |
| address_fixed_n100_lookup_miss_end_plain | 233 | 1535 | 1302 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 1312 | 14 |
| address_fixed_n100_entry_occupied_end_plain | 583 | 2666 | 2083 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2090 | 228 |
| address_fixed_n100_entry_vacant_end_plain | 583 | 4164 | 3581 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 224 | 25 | 3516 | 302 |
| address_fixed_n100_load_trusted_all_common | 235 | 235 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n100_late_read_all_common | 235 | 2989 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2762 | 14 |
| address_fixed_n100_validate_all_common | 235 | 3645 | 3410 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3417 | 14 |
| address_fixed_n100_load_all_common | 235 | 3661 | 3426 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 3458 | 7 | 14 |
| address_fixed_n100_mutable_baseline_all_common | 585 | 585 | 0 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n100_clear_all_common | 585 | 3469 | 2884 | 4016/16 | 4016/16 | 0 | 0 | 3 | 228 | 25 | 2678 | 437 |
| address_fixed_n100_cursor_first_all_common | 585 | 1084 | 499 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 502 | 228 |
| address_fixed_n100_for_each_read_all_common | 585 | 2419 | 1834 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1837 | 228 |
| address_fixed_n100_insert_front_common | 585 | 2312 | 1727 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 228 | 25 | 1658 | 302 |
| address_fixed_n100_replace_front_common | 585 | 3266 | 2681 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2684 | 228 |
| address_fixed_n100_remove_front_common | 585 | 2327 | 1742 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 1536 | 437 |
| address_fixed_n100_remove_into_front_common | 585 | 3518 | 2933 | 4016/3976 | 4016/3976 | 3960 | 0 | 3 | 228 | 25 | 2727 | 437 |
| address_fixed_n100_at_mut_front_common | 585 | 1810 | 1225 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1228 | 228 |
| address_fixed_n100_lookup_hit_front_common | 235 | 3371 | 3136 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3144 | 14 |
| address_fixed_n100_lookup_miss_front_common | 235 | 3512 | 3277 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3287 | 14 |
| address_fixed_n100_entry_occupied_front_common | 585 | 2715 | 2130 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2137 | 228 |
| address_fixed_n100_entry_vacant_front_common | 585 | 4829 | 4244 | 4016/4056 | 4016/4056 | 4000 | 0 | 3 | 224 | 25 | 4179 | 302 |
| address_fixed_n100_insert_middle_common | 585 | 2859 | 2274 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 228 | 25 | 2205 | 302 |
| address_fixed_n100_replace_middle_common | 585 | 3492 | 2907 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2910 | 228 |
| address_fixed_n100_remove_middle_common | 585 | 2202 | 1617 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 1411 | 437 |
| address_fixed_n100_remove_into_middle_common | 585 | 3393 | 2808 | 4016/3976 | 4016/3976 | 1960 | 0 | 3 | 228 | 25 | 2602 | 437 |
| address_fixed_n100_at_mut_middle_common | 585 | 1923 | 1338 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1341 | 228 |
| address_fixed_n100_lookup_hit_middle_common | 235 | 839 | 604 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n100_lookup_miss_middle_common | 235 | 3130 | 2895 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2905 | 14 |
| address_fixed_n100_entry_occupied_middle_common | 585 | 2828 | 2243 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2250 | 228 |
| address_fixed_n100_entry_vacant_middle_common | 585 | 5458 | 4873 | 4016/4056 | 4016/4056 | 2000 | 0 | 3 | 224 | 25 | 4808 | 302 |
| address_fixed_n100_insert_end_common | 585 | 1729 | 1144 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 228 | 25 | 1075 | 302 |
| address_fixed_n100_replace_end_common | 585 | 3172 | 2587 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 2590 | 228 |
| address_fixed_n100_remove_end_common | 585 | 1750 | 1165 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 959 | 437 |
| address_fixed_n100_remove_into_end_common | 585 | 2935 | 2350 | 4016/3976 | 4016/3976 | 0 | 0 | 3 | 228 | 25 | 2144 | 437 |
| address_fixed_n100_at_mut_end_common | 585 | 1763 | 1178 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 228 | 25 | 1181 | 228 |
| address_fixed_n100_lookup_hit_end_common | 235 | 2989 | 2754 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 2762 | 14 |
| address_fixed_n100_lookup_miss_end_common | 235 | 3560 | 3325 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 80 | 34 | 3335 | 14 |
| address_fixed_n100_entry_occupied_end_common | 585 | 2668 | 2083 | 4016/4016 | 4016/4016 | 0 | 0 | 3 | 224 | 25 | 2090 | 228 |
| address_fixed_n100_entry_vacant_end_common | 585 | 4187 | 3602 | 4016/4056 | 4016/4056 | 0 | 0 | 3 | 224 | 25 | 3537 | 302 |
| address_indexed_n100_load_trusted_all_plain | 234 | 234 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n100_late_read_all_plain | 234 | 1763 | 1529 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1537 | 14 |
| address_indexed_n100_validate_all_plain | 234 | 4616 | 4382 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 4389 | 14 |
| address_indexed_n100_load_all_plain | 234 | 4632 | 4398 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 4430 | 7 | 14 |
| address_indexed_n100_mutable_baseline_all_plain | 584 | 584 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n100_clear_all_plain | 584 | 3844 | 3260 | 4416/16 | 4416/16 | 0 | 0 | 3 | 229 | 25 | 3054 | 437 |
| address_indexed_n100_cursor_first_all_plain | 584 | 6566 | 5982 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5911 | 302 |
| address_indexed_n100_for_each_read_all_plain | 584 | 26432 | 25848 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 25851 | 228 |
| address_indexed_n100_insert_front_plain | 584 | 7659 | 7075 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 229 | 25 | 7006 | 302 |
| address_indexed_n100_replace_front_plain | 584 | 6942 | 6358 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 6287 | 302 |
| address_indexed_n100_remove_front_plain | 584 | 7824 | 7240 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 7034 | 437 |
| address_indexed_n100_remove_into_front_plain | 584 | 7474 | 6890 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 6684 | 437 |
| address_indexed_n100_at_mut_front_plain | 584 | 5919 | 5335 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5264 | 302 |
| address_indexed_n100_lookup_hit_front_plain | 234 | 1815 | 1581 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1589 | 14 |
| address_indexed_n100_lookup_miss_front_plain | 234 | 1445 | 1211 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1221 | 14 |
| address_indexed_n100_entry_occupied_front_plain | 584 | 5900 | 5316 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 225 | 25 | 5249 | 302 |
| address_indexed_n100_entry_vacant_front_plain | 584 | 8706 | 8122 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 225 | 25 | 8057 | 302 |
| address_indexed_n100_insert_middle_plain | 584 | 6726 | 6142 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 229 | 25 | 6073 | 302 |
| address_indexed_n100_replace_middle_plain | 584 | 5457 | 4873 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 229 | 25 | 4802 | 302 |
| address_indexed_n100_remove_middle_plain | 584 | 6274 | 5690 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5484 | 437 |
| address_indexed_n100_remove_into_middle_plain | 584 | 5926 | 5342 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5136 | 437 |
| address_indexed_n100_at_mut_middle_plain | 584 | 4679 | 4095 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 229 | 25 | 4024 | 302 |
| address_indexed_n100_lookup_hit_middle_plain | 234 | 1057 | 823 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 831 | 14 |
| address_indexed_n100_lookup_miss_middle_plain | 234 | 1352 | 1118 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1128 | 14 |
| address_indexed_n100_entry_occupied_middle_plain | 584 | 4620 | 4036 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 225 | 25 | 3969 | 302 |
| address_indexed_n100_entry_vacant_middle_plain | 584 | 7844 | 7260 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 225 | 25 | 7195 | 302 |
| address_indexed_n100_insert_end_plain | 584 | 3708 | 3124 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 229 | 25 | 3055 | 302 |
| address_indexed_n100_replace_end_plain | 584 | 3431 | 2847 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 229 | 25 | 2776 | 302 |
| address_indexed_n100_remove_end_plain | 584 | 3956 | 3372 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 3166 | 437 |
| address_indexed_n100_remove_into_end_plain | 584 | 3616 | 3032 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 2826 | 437 |
| address_indexed_n100_at_mut_end_plain | 584 | 2978 | 2394 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 229 | 25 | 2323 | 302 |
| address_indexed_n100_lookup_hit_end_plain | 234 | 1763 | 1529 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1537 | 14 |
| address_indexed_n100_lookup_miss_end_plain | 234 | 1493 | 1259 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 1269 | 14 |
| address_indexed_n100_entry_occupied_end_plain | 584 | 2918 | 2334 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 225 | 25 | 2267 | 302 |
| address_indexed_n100_entry_vacant_end_plain | 584 | 4757 | 4173 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 225 | 25 | 4108 | 302 |
| address_indexed_n100_load_trusted_all_common | 236 | 236 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n100_late_read_all_common | 236 | 3210 | 2974 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 2982 | 14 |
| address_indexed_n100_validate_all_common | 236 | 5509 | 5273 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 5280 | 14 |
| address_indexed_n100_load_all_common | 236 | 5525 | 5289 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 5321 | 7 | 14 |
| address_indexed_n100_mutable_baseline_all_common | 586 | 586 | 0 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n100_clear_all_common | 586 | 3846 | 3260 | 4416/16 | 4416/16 | 0 | 0 | 3 | 229 | 25 | 3054 | 437 |
| address_indexed_n100_cursor_first_all_common | 586 | 6568 | 5982 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5911 | 302 |
| address_indexed_n100_for_each_read_all_common | 586 | 27325 | 26739 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 229 | 25 | 26742 | 228 |
| address_indexed_n100_insert_front_common | 586 | 7679 | 7093 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 229 | 25 | 7024 | 302 |
| address_indexed_n100_replace_front_common | 586 | 6953 | 6367 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 6296 | 302 |
| address_indexed_n100_remove_front_common | 586 | 7835 | 7249 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 7043 | 437 |
| address_indexed_n100_remove_into_front_common | 586 | 7485 | 6899 | 4416/4372 | 4416/4372 | 4356 | 99 | 3 | 229 | 25 | 6693 | 437 |
| address_indexed_n100_at_mut_front_common | 586 | 5930 | 5344 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 229 | 25 | 5273 | 302 |
| address_indexed_n100_lookup_hit_front_common | 236 | 3551 | 3315 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 3323 | 14 |
| address_indexed_n100_lookup_miss_front_common | 236 | 3470 | 3234 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 3244 | 14 |
| address_indexed_n100_entry_occupied_front_common | 586 | 5911 | 5325 | 4416/4425 | 4416/4425 | 792 | 100 | 3 | 225 | 25 | 5258 | 302 |
| address_indexed_n100_entry_vacant_front_common | 586 | 8741 | 8155 | 4416/4469 | 4416/4469 | 4400 | 101 | 3 | 225 | 25 | 8090 | 302 |
| address_indexed_n100_insert_middle_common | 586 | 6842 | 6256 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 229 | 25 | 6187 | 302 |
| address_indexed_n100_replace_middle_common | 586 | 5477 | 4891 | 4416/4425 | 4416/4425 | 392 | 100 | 3 | 229 | 25 | 4820 | 302 |
| address_indexed_n100_remove_middle_common | 586 | 6294 | 5708 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5502 | 437 |
| address_indexed_n100_remove_into_middle_common | 586 | 5946 | 5360 | 4416/4372 | 4416/4372 | 2556 | 99 | 3 | 229 | 25 | 5154 | 437 |
| address_indexed_n100_at_mut_middle_common | 586 | 4699 | 4113 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 229 | 25 | 4042 | 302 |
| address_indexed_n100_lookup_hit_middle_common | 236 | 1059 | 823 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 831 | 14 |
| address_indexed_n100_lookup_miss_middle_common | 236 | 3088 | 2852 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 2862 | 14 |
| address_indexed_n100_entry_occupied_middle_common | 586 | 4640 | 4054 | 4416/4425 | 4416/4425 | 392 | 50 | 3 | 225 | 25 | 3987 | 302 |
| address_indexed_n100_entry_vacant_middle_common | 586 | 7987 | 7401 | 4416/4469 | 4416/4469 | 2600 | 101 | 3 | 225 | 25 | 7336 | 302 |
| address_indexed_n100_insert_end_common | 586 | 3719 | 3133 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 229 | 25 | 3064 | 302 |
| address_indexed_n100_replace_end_common | 586 | 3433 | 2847 | 4416/4425 | 4416/4425 | 0 | 100 | 3 | 229 | 25 | 2776 | 302 |
| address_indexed_n100_remove_end_common | 586 | 3958 | 3372 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 3166 | 437 |
| address_indexed_n100_remove_into_end_common | 586 | 3618 | 3032 | 4416/4372 | 4416/4372 | 792 | 99 | 3 | 229 | 25 | 2826 | 437 |
| address_indexed_n100_at_mut_end_common | 586 | 2980 | 2394 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 229 | 25 | 2323 | 302 |
| address_indexed_n100_lookup_hit_end_common | 236 | 3210 | 2974 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 2982 | 14 |
| address_indexed_n100_lookup_miss_end_common | 236 | 3518 | 3282 | 4416/4416 | 4416/4416 | 0 | 0 | 3 | 81 | 34 | 3292 | 14 |
| address_indexed_n100_entry_occupied_end_common | 586 | 2920 | 2334 | 4416/4425 | 4416/4425 | 0 | 1 | 3 | 225 | 25 | 2267 | 302 |
| address_indexed_n100_entry_vacant_end_common | 586 | 4780 | 4194 | 4416/4469 | 4416/4469 | 800 | 101 | 3 | 225 | 25 | 4129 | 302 |
| list_fixed_n1000_load_trusted_all_plain | 223 | 223 | 0 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 34 | 9 | 14 |
| list_fixed_n1000_late_read_all_plain | 223 | 437 | 214 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 34 | 222 | 14 |
| list_fixed_n1000_validate_all_plain | 223 | 496 | 273 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 34 | 280 | 14 |
| list_fixed_n1000_load_all_plain | 223 | 512 | 289 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 71 | 321 | 7 | 14 |
| list_fixed_n1000_mutable_baseline_all_plain | 573 | 573 | 0 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 4 | 228 |
| list_fixed_n1000_clear_all_plain | 573 | 4283 | 3710 | 8016/16 | 8016/16 | 0 | 0 | 3 | 219 | 25 | 3504 | 437 |
| list_fixed_n1000_cursor_first_all_plain | 573 | 788 | 215 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 218 | 228 |
| list_fixed_n1000_for_each_read_all_plain | 573 | 11816 | 11243 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 11246 | 228 |
| list_fixed_n1000_insert_front_plain | 573 | 3346 | 2773 | 8016/8024 | 8016/8024 | 8000 | 0 | 3 | 219 | 25 | 2704 | 302 |
| list_fixed_n1000_replace_front_plain | 573 | 1927 | 1354 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n1000_remove_front_plain | 573 | 2375 | 1802 | 8016/8008 | 8016/8008 | 7992 | 0 | 3 | 219 | 25 | 1592 | 437 |
| list_fixed_n1000_remove_into_front_plain | 573 | 3448 | 2875 | 8016/8008 | 8016/8008 | 7992 | 0 | 3 | 219 | 25 | 2669 | 437 |
| list_fixed_n1000_at_mut_front_plain | 573 | 1427 | 854 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n1000_insert_bulk_front_plain | 573 | 3597 | 3024 | 8016/8032 | 8016/8032 | 8000 | 0 | 3 | 219 | 25 | 2953 | 302 |
| list_fixed_n1000_truncate_front_plain | 573 | 4293 | 3720 | 8016/16 | 8016/16 | 0 | 0 | 3 | 219 | 25 | 3514 | 437 |
| list_fixed_n1000_insert_middle_plain | 573 | 2818 | 2245 | 8016/8024 | 8016/8024 | 4000 | 0 | 3 | 219 | 25 | 2176 | 302 |
| list_fixed_n1000_replace_middle_plain | 573 | 1927 | 1354 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n1000_remove_middle_plain | 573 | 1899 | 1326 | 8016/8008 | 8016/8008 | 3992 | 0 | 3 | 219 | 25 | 1116 | 437 |
| list_fixed_n1000_remove_into_middle_plain | 573 | 2972 | 2399 | 8016/8008 | 8016/8008 | 3992 | 0 | 3 | 219 | 25 | 2193 | 437 |
| list_fixed_n1000_at_mut_middle_plain | 573 | 1427 | 854 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n1000_insert_bulk_middle_plain | 573 | 3070 | 2497 | 8016/8032 | 8016/8032 | 4000 | 0 | 3 | 219 | 25 | 2426 | 302 |
| list_fixed_n1000_truncate_middle_plain | 573 | 3253 | 2680 | 8016/4016 | 8016/4016 | 0 | 0 | 3 | 219 | 25 | 2474 | 437 |
| list_fixed_n1000_insert_end_plain | 573 | 2226 | 1653 | 8016/8024 | 8016/8024 | 0 | 0 | 3 | 219 | 25 | 1584 | 302 |
| list_fixed_n1000_replace_end_plain | 573 | 1927 | 1354 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1357 | 228 |
| list_fixed_n1000_remove_end_plain | 573 | 1369 | 796 | 8016/8008 | 8016/8008 | 0 | 0 | 3 | 219 | 25 | 586 | 437 |
| list_fixed_n1000_remove_into_end_plain | 573 | 2436 | 1863 | 8016/8008 | 8016/8008 | 0 | 0 | 3 | 219 | 25 | 1657 | 437 |
| list_fixed_n1000_at_mut_end_plain | 573 | 1427 | 854 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 857 | 228 |
| list_fixed_n1000_insert_bulk_end_plain | 573 | 2480 | 1907 | 8016/8032 | 8016/8032 | 0 | 0 | 3 | 219 | 25 | 1836 | 302 |
| list_fixed_n1000_truncate_end_plain | 573 | 1907 | 1334 | 8016/8016 | 8016/8016 | 0 | 0 | 3 | 219 | 25 | 1337 | 228 |
| list_fixed_n1000_push_end_plain | 573 | 1327 | 754 | 8016/8024 | 8016/8024 | 0 | 0 | 3 | 219 | 25 | 683 | 302 |
| list_fixed_n1000_pop_end_plain | 573 | 2328 | 1755 | 8016/8008 | 8016/8008 | 0 | 0 | 3 | 219 | 25 | 1549 | 437 |
| list_fixed_n1000_extend_end_plain | 573 | 2479 | 1906 | 8016/8032 | 8016/8032 | 0 | 0 | 3 | 219 | 25 | 1835 | 302 |
| list_indexed_n1000_load_trusted_all_plain | 224 | 224 | 0 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| list_indexed_n1000_late_read_all_plain | 224 | 1005 | 781 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 34 | 789 | 14 |
| list_indexed_n1000_validate_all_plain | 224 | 15513 | 15289 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 34 | 15296 | 14 |
| list_indexed_n1000_load_all_plain | 224 | 15529 | 15305 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 72 | 15337 | 7 | 14 |
| list_indexed_n1000_mutable_baseline_all_plain | 574 | 574 | 0 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| list_indexed_n1000_clear_all_plain | 574 | 4997 | 4423 | 12012/16 | 12012/16 | 0 | 0 | 3 | 220 | 25 | 4217 | 437 |
| list_indexed_n1000_cursor_first_all_plain | 574 | 23189 | 22615 | 12012/12021 | 12012/12021 | 7992 | 999 | 3 | 220 | 25 | 22544 | 302 |
| list_indexed_n1000_for_each_read_all_plain | 574 | 200067 | 199493 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 220 | 25 | 199496 | 228 |
| list_indexed_n1000_insert_front_plain | 574 | 35748 | 35174 | 12012/12033 | 12012/12033 | 11996 | 1000 | 3 | 220 | 25 | 35105 | 302 |
| list_indexed_n1000_replace_front_plain | 574 | 45912 | 45338 | 12012/12021 | 12012/12021 | 7992 | 999 | 3 | 220 | 25 | 45267 | 302 |
| list_indexed_n1000_remove_front_plain | 574 | 46268 | 45694 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 45484 | 437 |
| list_indexed_n1000_remove_into_front_plain | 574 | 45962 | 45388 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 45182 | 437 |
| list_indexed_n1000_at_mut_front_plain | 574 | 22597 | 22023 | 12012/12021 | 12012/12021 | 7992 | 999 | 3 | 220 | 25 | 21952 | 302 |
| list_indexed_n1000_insert_bulk_front_plain | 574 | 40860 | 40286 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 40215 | 302 |
| list_indexed_n1000_truncate_front_plain | 574 | 5001 | 4427 | 12012/16 | 12012/16 | 0 | 0 | 3 | 220 | 25 | 4221 | 437 |
| list_indexed_n1000_insert_middle_plain | 574 | 19831 | 19257 | 12012/12033 | 12012/12033 | 9996 | 1000 | 3 | 220 | 25 | 19188 | 302 |
| list_indexed_n1000_replace_middle_plain | 574 | 24406 | 23832 | 12012/12021 | 12012/12021 | 3992 | 999 | 3 | 220 | 25 | 23761 | 302 |
| list_indexed_n1000_remove_middle_plain | 574 | 25355 | 24781 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 24571 | 437 |
| list_indexed_n1000_remove_into_middle_plain | 574 | 25049 | 24475 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 24269 | 437 |
| list_indexed_n1000_at_mut_middle_plain | 574 | 12642 | 12068 | 12012/12021 | 12012/12021 | 3992 | 499 | 3 | 220 | 25 | 11997 | 302 |
| list_indexed_n1000_insert_bulk_middle_plain | 574 | 22455 | 21881 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 21810 | 302 |
| list_indexed_n1000_truncate_middle_plain | 574 | 4849 | 4275 | 12012/6012 | 12012/6012 | 4000 | 499 | 3 | 220 | 25 | 4069 | 437 |
| list_indexed_n1000_insert_end_plain | 574 | 3751 | 3177 | 12012/12033 | 12012/12033 | 8000 | 1000 | 3 | 220 | 25 | 3108 | 302 |
| list_indexed_n1000_replace_end_plain | 574 | 2787 | 2213 | 12012/12021 | 12012/12021 | 0 | 999 | 3 | 220 | 25 | 2142 | 302 |
| list_indexed_n1000_remove_end_plain | 574 | 4305 | 3731 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 3521 | 437 |
| list_indexed_n1000_remove_into_end_plain | 574 | 3999 | 3425 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 3219 | 437 |
| list_indexed_n1000_at_mut_end_plain | 574 | 2489 | 1915 | 12012/12021 | 12012/12021 | 0 | 0 | 3 | 220 | 25 | 1844 | 302 |
| list_indexed_n1000_insert_bulk_end_plain | 574 | 3878 | 3304 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 3233 | 302 |
| list_indexed_n1000_truncate_end_plain | 574 | 2407 | 1833 | 12012/12012 | 12012/12012 | 0 | 0 | 3 | 220 | 25 | 1836 | 228 |
| list_indexed_n1000_push_end_plain | 574 | 3764 | 3190 | 12012/12033 | 12012/12033 | 8000 | 1000 | 3 | 220 | 25 | 3119 | 302 |
| list_indexed_n1000_pop_end_plain | 574 | 3995 | 3421 | 12012/12000 | 12012/12000 | 7992 | 998 | 3 | 220 | 25 | 3215 | 437 |
| list_indexed_n1000_extend_end_plain | 574 | 3877 | 3303 | 12012/12054 | 12012/12054 | 8000 | 1001 | 3 | 220 | 25 | 3232 | 302 |
| map_fixed_n1000_load_trusted_all_plain | 228 | 228 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n1000_late_read_all_plain | 228 | 1930 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1710 | 14 |
| map_fixed_n1000_validate_all_plain | 228 | 21647 | 21419 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 21426 | 14 |
| map_fixed_n1000_load_all_plain | 228 | 21663 | 21435 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 21467 | 7 | 14 |
| map_fixed_n1000_mutable_baseline_all_plain | 578 | 578 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n1000_clear_all_plain | 578 | 12752 | 12174 | 40016/16 | 40016/16 | 0 | 0 | 3 | 223 | 25 | 11968 | 437 |
| map_fixed_n1000_cursor_first_all_plain | 578 | 1077 | 499 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 502 | 228 |
| map_fixed_n1000_for_each_read_all_plain | 578 | 14112 | 13534 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 13537 | 228 |
| map_fixed_n1000_insert_front_plain | 578 | 7039 | 6461 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 223 | 25 | 6392 | 302 |
| map_fixed_n1000_replace_front_plain | 578 | 3241 | 2663 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2666 | 228 |
| map_fixed_n1000_remove_front_plain | 578 | 6595 | 6017 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 5811 | 437 |
| map_fixed_n1000_remove_into_front_plain | 578 | 7786 | 7208 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 7002 | 437 |
| map_fixed_n1000_at_mut_front_plain | 578 | 1794 | 1216 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1219 | 228 |
| map_fixed_n1000_lookup_hit_front_plain | 228 | 2009 | 1781 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1789 | 14 |
| map_fixed_n1000_lookup_miss_front_plain | 228 | 1861 | 1633 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1643 | 14 |
| map_fixed_n1000_entry_occupied_front_plain | 578 | 2699 | 2121 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2128 | 228 |
| map_fixed_n1000_entry_vacant_front_plain | 578 | 9541 | 8963 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 219 | 25 | 8898 | 302 |
| map_fixed_n1000_insert_middle_plain | 578 | 5363 | 4785 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 223 | 25 | 4716 | 302 |
| map_fixed_n1000_replace_middle_plain | 578 | 3449 | 2871 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2874 | 228 |
| map_fixed_n1000_remove_middle_plain | 578 | 4319 | 3741 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 3535 | 437 |
| map_fixed_n1000_remove_into_middle_plain | 578 | 5510 | 4932 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 4726 | 437 |
| map_fixed_n1000_at_mut_middle_plain | 578 | 1898 | 1320 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1323 | 228 |
| map_fixed_n1000_lookup_hit_middle_plain | 228 | 832 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n1000_lookup_miss_middle_plain | 228 | 1792 | 1564 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1574 | 14 |
| map_fixed_n1000_entry_occupied_middle_plain | 578 | 2803 | 2225 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2232 | 228 |
| map_fixed_n1000_entry_vacant_middle_plain | 578 | 7935 | 7357 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 219 | 25 | 7292 | 302 |
| map_fixed_n1000_insert_end_plain | 578 | 1713 | 1135 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 223 | 25 | 1066 | 302 |
| map_fixed_n1000_replace_end_plain | 578 | 3165 | 2587 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2590 | 228 |
| map_fixed_n1000_remove_end_plain | 578 | 1743 | 1165 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 959 | 437 |
| map_fixed_n1000_remove_into_end_plain | 578 | 2928 | 2350 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 2144 | 437 |
| map_fixed_n1000_at_mut_end_plain | 578 | 1756 | 1178 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1181 | 228 |
| map_fixed_n1000_lookup_hit_end_plain | 228 | 1930 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1710 | 14 |
| map_fixed_n1000_lookup_miss_end_plain | 228 | 1923 | 1695 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 1705 | 14 |
| map_fixed_n1000_entry_occupied_end_plain | 578 | 2661 | 2083 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2090 | 228 |
| map_fixed_n1000_entry_vacant_end_plain | 578 | 4159 | 3581 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 219 | 25 | 3516 | 302 |
| map_fixed_n1000_load_trusted_all_common | 230 | 230 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 9 | 14 |
| map_fixed_n1000_late_read_all_common | 230 | 4244 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4022 | 14 |
| map_fixed_n1000_validate_all_common | 230 | 30640 | 30410 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 30417 | 14 |
| map_fixed_n1000_load_all_common | 230 | 30656 | 30426 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 30458 | 7 | 14 |
| map_fixed_n1000_mutable_baseline_all_common | 580 | 580 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 4 | 228 |
| map_fixed_n1000_clear_all_common | 580 | 12754 | 12174 | 40016/16 | 40016/16 | 0 | 0 | 3 | 223 | 25 | 11968 | 437 |
| map_fixed_n1000_cursor_first_all_common | 580 | 1079 | 499 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 502 | 228 |
| map_fixed_n1000_for_each_read_all_common | 580 | 14114 | 13534 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 13537 | 228 |
| map_fixed_n1000_insert_front_common | 580 | 7059 | 6479 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 223 | 25 | 6410 | 302 |
| map_fixed_n1000_replace_front_common | 580 | 3261 | 2681 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2684 | 228 |
| map_fixed_n1000_remove_front_common | 580 | 6606 | 6026 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 5820 | 437 |
| map_fixed_n1000_remove_into_front_common | 580 | 7797 | 7217 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 223 | 25 | 7011 | 437 |
| map_fixed_n1000_at_mut_front_common | 580 | 1805 | 1225 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1228 | 228 |
| map_fixed_n1000_lookup_hit_front_common | 230 | 4612 | 4382 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4390 | 14 |
| map_fixed_n1000_lookup_miss_front_common | 230 | 4753 | 4523 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4533 | 14 |
| map_fixed_n1000_entry_occupied_front_common | 580 | 2710 | 2130 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2137 | 228 |
| map_fixed_n1000_entry_vacant_front_common | 580 | 9576 | 8996 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 219 | 25 | 8931 | 302 |
| map_fixed_n1000_insert_middle_common | 580 | 5515 | 4935 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 223 | 25 | 4866 | 302 |
| map_fixed_n1000_replace_middle_common | 580 | 3487 | 2907 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2910 | 228 |
| map_fixed_n1000_remove_middle_common | 580 | 4339 | 3759 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 3553 | 437 |
| map_fixed_n1000_remove_into_middle_common | 580 | 5530 | 4950 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 223 | 25 | 4744 | 437 |
| map_fixed_n1000_at_mut_middle_common | 580 | 1918 | 1338 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1341 | 228 |
| map_fixed_n1000_lookup_hit_middle_common | 230 | 834 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 612 | 14 |
| map_fixed_n1000_lookup_miss_middle_common | 230 | 4395 | 4165 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4175 | 14 |
| map_fixed_n1000_entry_occupied_middle_common | 580 | 2823 | 2243 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2250 | 228 |
| map_fixed_n1000_entry_vacant_middle_common | 580 | 8114 | 7534 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 219 | 25 | 7469 | 302 |
| map_fixed_n1000_insert_end_common | 580 | 1724 | 1144 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 223 | 25 | 1075 | 302 |
| map_fixed_n1000_replace_end_common | 580 | 3167 | 2587 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 2590 | 228 |
| map_fixed_n1000_remove_end_common | 580 | 1745 | 1165 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 959 | 437 |
| map_fixed_n1000_remove_into_end_common | 580 | 2930 | 2350 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 223 | 25 | 2144 | 437 |
| map_fixed_n1000_at_mut_end_common | 580 | 1758 | 1178 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 223 | 25 | 1181 | 228 |
| map_fixed_n1000_lookup_hit_end_common | 230 | 4244 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4022 | 14 |
| map_fixed_n1000_lookup_miss_end_common | 230 | 4815 | 4585 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 75 | 34 | 4595 | 14 |
| map_fixed_n1000_entry_occupied_end_common | 580 | 2663 | 2083 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 219 | 25 | 2090 | 228 |
| map_fixed_n1000_entry_vacant_end_common | 580 | 4182 | 3602 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 219 | 25 | 3537 | 302 |
| map_indexed_n1000_load_trusted_all_plain | 226 | 226 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n1000_late_read_all_plain | 226 | 2148 | 1922 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1930 | 14 |
| map_indexed_n1000_validate_all_plain | 226 | 38583 | 38357 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 38364 | 14 |
| map_indexed_n1000_load_all_plain | 226 | 38599 | 38373 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 38405 | 7 | 14 |
| map_indexed_n1000_mutable_baseline_all_plain | 576 | 576 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n1000_clear_all_plain | 576 | 14036 | 13460 | 44016/16 | 44016/16 | 0 | 0 | 3 | 220 | 25 | 13254 | 437 |
| map_indexed_n1000_cursor_first_all_plain | 576 | 32682 | 32106 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 32035 | 302 |
| map_indexed_n1000_for_each_read_all_plain | 576 | 253224 | 252648 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 252651 | 228 |
| map_indexed_n1000_insert_front_plain | 576 | 42499 | 41923 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 220 | 25 | 41854 | 302 |
| map_indexed_n1000_replace_front_plain | 576 | 37558 | 36982 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 36911 | 302 |
| map_indexed_n1000_remove_front_plain | 576 | 42157 | 41581 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41375 | 437 |
| map_indexed_n1000_remove_into_front_plain | 576 | 41807 | 41231 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41025 | 437 |
| map_indexed_n1000_at_mut_front_plain | 576 | 32035 | 31459 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 31388 | 302 |
| map_indexed_n1000_lookup_hit_front_plain | 226 | 2186 | 1960 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1968 | 14 |
| map_indexed_n1000_lookup_miss_front_plain | 226 | 1816 | 1590 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1600 | 14 |
| map_indexed_n1000_entry_occupied_front_plain | 576 | 32016 | 31440 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 216 | 25 | 31373 | 302 |
| map_indexed_n1000_entry_vacant_front_plain | 576 | 43546 | 42970 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 216 | 25 | 42905 | 302 |
| map_indexed_n1000_insert_middle_plain | 576 | 24721 | 24145 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 220 | 25 | 24076 | 302 |
| map_indexed_n1000_replace_middle_plain | 576 | 20695 | 20119 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 220 | 25 | 20048 | 302 |
| map_indexed_n1000_remove_middle_plain | 576 | 23734 | 23158 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22952 | 437 |
| map_indexed_n1000_remove_into_middle_plain | 576 | 23386 | 22810 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22604 | 437 |
| map_indexed_n1000_at_mut_middle_plain | 576 | 17667 | 17091 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 220 | 25 | 17020 | 302 |
| map_indexed_n1000_lookup_hit_middle_plain | 226 | 1049 | 823 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 831 | 14 |
| map_indexed_n1000_lookup_miss_middle_plain | 226 | 1747 | 1521 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1531 | 14 |
| map_indexed_n1000_entry_occupied_middle_plain | 576 | 17608 | 17032 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 216 | 25 | 16965 | 302 |
| map_indexed_n1000_entry_vacant_middle_plain | 576 | 25839 | 25263 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 216 | 25 | 25198 | 302 |
| map_indexed_n1000_insert_end_plain | 576 | 4624 | 4048 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 220 | 25 | 3979 | 302 |
| map_indexed_n1000_replace_end_plain | 576 | 3423 | 2847 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 220 | 25 | 2776 | 302 |
| map_indexed_n1000_remove_end_plain | 576 | 4781 | 4205 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3999 | 437 |
| map_indexed_n1000_remove_into_end_plain | 576 | 4441 | 3865 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3659 | 437 |
| map_indexed_n1000_at_mut_end_plain | 576 | 2970 | 2394 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 220 | 25 | 2323 | 302 |
| map_indexed_n1000_lookup_hit_end_plain | 226 | 2148 | 1922 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1930 | 14 |
| map_indexed_n1000_lookup_miss_end_plain | 226 | 1878 | 1652 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 1662 | 14 |
| map_indexed_n1000_entry_occupied_end_plain | 576 | 2910 | 2334 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 216 | 25 | 2267 | 302 |
| map_indexed_n1000_entry_vacant_end_plain | 576 | 5673 | 5097 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 216 | 25 | 5032 | 302 |
| map_indexed_n1000_load_trusted_all_common | 228 | 228 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 9 | 14 |
| map_indexed_n1000_late_read_all_common | 228 | 4462 | 4234 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4242 | 14 |
| map_indexed_n1000_validate_all_common | 228 | 47576 | 47348 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 47355 | 14 |
| map_indexed_n1000_load_all_common | 228 | 47592 | 47364 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 47396 | 7 | 14 |
| map_indexed_n1000_mutable_baseline_all_common | 578 | 578 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 4 | 228 |
| map_indexed_n1000_clear_all_common | 578 | 14038 | 13460 | 44016/16 | 44016/16 | 0 | 0 | 3 | 220 | 25 | 13254 | 437 |
| map_indexed_n1000_cursor_first_all_common | 578 | 32684 | 32106 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 32035 | 302 |
| map_indexed_n1000_for_each_read_all_common | 578 | 262217 | 261639 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 220 | 25 | 261642 | 228 |
| map_indexed_n1000_insert_front_common | 578 | 42519 | 41941 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 220 | 25 | 41872 | 302 |
| map_indexed_n1000_replace_front_common | 578 | 37569 | 36991 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 36920 | 302 |
| map_indexed_n1000_remove_front_common | 578 | 42168 | 41590 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41384 | 437 |
| map_indexed_n1000_remove_into_front_common | 578 | 41818 | 41240 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 220 | 25 | 41034 | 437 |
| map_indexed_n1000_at_mut_front_common | 578 | 32046 | 31468 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 220 | 25 | 31397 | 302 |
| map_indexed_n1000_lookup_hit_front_common | 228 | 4789 | 4561 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4569 | 14 |
| map_indexed_n1000_lookup_miss_front_common | 228 | 4708 | 4480 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4490 | 14 |
| map_indexed_n1000_entry_occupied_front_common | 578 | 32027 | 31449 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 216 | 25 | 31382 | 302 |
| map_indexed_n1000_entry_vacant_front_common | 578 | 43581 | 43003 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 216 | 25 | 42938 | 302 |
| map_indexed_n1000_insert_middle_common | 578 | 24873 | 24295 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 220 | 25 | 24226 | 302 |
| map_indexed_n1000_replace_middle_common | 578 | 20715 | 20137 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 220 | 25 | 20066 | 302 |
| map_indexed_n1000_remove_middle_common | 578 | 23754 | 23176 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22970 | 437 |
| map_indexed_n1000_remove_into_middle_common | 578 | 23406 | 22828 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 220 | 25 | 22622 | 437 |
| map_indexed_n1000_at_mut_middle_common | 578 | 17687 | 17109 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 220 | 25 | 17038 | 302 |
| map_indexed_n1000_lookup_hit_middle_common | 228 | 1051 | 823 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 831 | 14 |
| map_indexed_n1000_lookup_miss_middle_common | 228 | 4350 | 4122 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4132 | 14 |
| map_indexed_n1000_entry_occupied_middle_common | 578 | 17628 | 17050 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 216 | 25 | 16983 | 302 |
| map_indexed_n1000_entry_vacant_middle_common | 578 | 26018 | 25440 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 216 | 25 | 25375 | 302 |
| map_indexed_n1000_insert_end_common | 578 | 4635 | 4057 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 220 | 25 | 3988 | 302 |
| map_indexed_n1000_replace_end_common | 578 | 3425 | 2847 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 220 | 25 | 2776 | 302 |
| map_indexed_n1000_remove_end_common | 578 | 4783 | 4205 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3999 | 437 |
| map_indexed_n1000_remove_into_end_common | 578 | 4443 | 3865 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 220 | 25 | 3659 | 437 |
| map_indexed_n1000_at_mut_end_common | 578 | 2972 | 2394 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 220 | 25 | 2323 | 302 |
| map_indexed_n1000_lookup_hit_end_common | 228 | 4462 | 4234 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4242 | 14 |
| map_indexed_n1000_lookup_miss_end_common | 228 | 4770 | 4542 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 72 | 34 | 4552 | 14 |
| map_indexed_n1000_entry_occupied_end_common | 578 | 2912 | 2334 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 216 | 25 | 2267 | 302 |
| map_indexed_n1000_entry_vacant_end_common | 578 | 5696 | 5118 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 216 | 25 | 5053 | 302 |
| address_fixed_n1000_load_trusted_all_plain | 233 | 233 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n1000_late_read_all_plain | 233 | 1935 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1710 | 14 |
| address_fixed_n1000_validate_all_plain | 233 | 21652 | 21419 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 21426 | 14 |
| address_fixed_n1000_load_all_plain | 233 | 21668 | 21435 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 21467 | 7 | 14 |
| address_fixed_n1000_mutable_baseline_all_plain | 583 | 583 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n1000_clear_all_plain | 583 | 12757 | 12174 | 40016/16 | 40016/16 | 0 | 0 | 3 | 228 | 25 | 11968 | 437 |
| address_fixed_n1000_cursor_first_all_plain | 583 | 1082 | 499 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 502 | 228 |
| address_fixed_n1000_for_each_read_all_plain | 583 | 14117 | 13534 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 13537 | 228 |
| address_fixed_n1000_insert_front_plain | 583 | 7044 | 6461 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 228 | 25 | 6392 | 302 |
| address_fixed_n1000_replace_front_plain | 583 | 3246 | 2663 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2666 | 228 |
| address_fixed_n1000_remove_front_plain | 583 | 6600 | 6017 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 5811 | 437 |
| address_fixed_n1000_remove_into_front_plain | 583 | 7791 | 7208 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 7002 | 437 |
| address_fixed_n1000_at_mut_front_plain | 583 | 1799 | 1216 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1219 | 228 |
| address_fixed_n1000_lookup_hit_front_plain | 233 | 2014 | 1781 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1789 | 14 |
| address_fixed_n1000_lookup_miss_front_plain | 233 | 1866 | 1633 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1643 | 14 |
| address_fixed_n1000_entry_occupied_front_plain | 583 | 2704 | 2121 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2128 | 228 |
| address_fixed_n1000_entry_vacant_front_plain | 583 | 9546 | 8963 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 224 | 25 | 8898 | 302 |
| address_fixed_n1000_insert_middle_plain | 583 | 5368 | 4785 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 228 | 25 | 4716 | 302 |
| address_fixed_n1000_replace_middle_plain | 583 | 3454 | 2871 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2874 | 228 |
| address_fixed_n1000_remove_middle_plain | 583 | 4324 | 3741 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 3535 | 437 |
| address_fixed_n1000_remove_into_middle_plain | 583 | 5515 | 4932 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 4726 | 437 |
| address_fixed_n1000_at_mut_middle_plain | 583 | 1903 | 1320 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1323 | 228 |
| address_fixed_n1000_lookup_hit_middle_plain | 233 | 837 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n1000_lookup_miss_middle_plain | 233 | 1797 | 1564 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1574 | 14 |
| address_fixed_n1000_entry_occupied_middle_plain | 583 | 2808 | 2225 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2232 | 228 |
| address_fixed_n1000_entry_vacant_middle_plain | 583 | 7940 | 7357 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 224 | 25 | 7292 | 302 |
| address_fixed_n1000_insert_end_plain | 583 | 1718 | 1135 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 228 | 25 | 1066 | 302 |
| address_fixed_n1000_replace_end_plain | 583 | 3170 | 2587 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2590 | 228 |
| address_fixed_n1000_remove_end_plain | 583 | 1748 | 1165 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 959 | 437 |
| address_fixed_n1000_remove_into_end_plain | 583 | 2933 | 2350 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 2144 | 437 |
| address_fixed_n1000_at_mut_end_plain | 583 | 1761 | 1178 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1181 | 228 |
| address_fixed_n1000_lookup_hit_end_plain | 233 | 1935 | 1702 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1710 | 14 |
| address_fixed_n1000_lookup_miss_end_plain | 233 | 1928 | 1695 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 1705 | 14 |
| address_fixed_n1000_entry_occupied_end_plain | 583 | 2666 | 2083 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2090 | 228 |
| address_fixed_n1000_entry_vacant_end_plain | 583 | 4164 | 3581 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 224 | 25 | 3516 | 302 |
| address_fixed_n1000_load_trusted_all_common | 235 | 235 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 9 | 14 |
| address_fixed_n1000_late_read_all_common | 235 | 4249 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4022 | 14 |
| address_fixed_n1000_validate_all_common | 235 | 30645 | 30410 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 30417 | 14 |
| address_fixed_n1000_load_all_common | 235 | 30661 | 30426 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 30458 | 7 | 14 |
| address_fixed_n1000_mutable_baseline_all_common | 585 | 585 | 0 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 4 | 228 |
| address_fixed_n1000_clear_all_common | 585 | 12759 | 12174 | 40016/16 | 40016/16 | 0 | 0 | 3 | 228 | 25 | 11968 | 437 |
| address_fixed_n1000_cursor_first_all_common | 585 | 1084 | 499 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 502 | 228 |
| address_fixed_n1000_for_each_read_all_common | 585 | 14119 | 13534 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 13537 | 228 |
| address_fixed_n1000_insert_front_common | 585 | 7064 | 6479 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 228 | 25 | 6410 | 302 |
| address_fixed_n1000_replace_front_common | 585 | 3266 | 2681 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2684 | 228 |
| address_fixed_n1000_remove_front_common | 585 | 6611 | 6026 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 5820 | 437 |
| address_fixed_n1000_remove_into_front_common | 585 | 7802 | 7217 | 40016/39976 | 40016/39976 | 39960 | 0 | 3 | 228 | 25 | 7011 | 437 |
| address_fixed_n1000_at_mut_front_common | 585 | 1810 | 1225 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1228 | 228 |
| address_fixed_n1000_lookup_hit_front_common | 235 | 4617 | 4382 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4390 | 14 |
| address_fixed_n1000_lookup_miss_front_common | 235 | 4758 | 4523 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4533 | 14 |
| address_fixed_n1000_entry_occupied_front_common | 585 | 2715 | 2130 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2137 | 228 |
| address_fixed_n1000_entry_vacant_front_common | 585 | 9581 | 8996 | 40016/40056 | 40016/40056 | 40000 | 0 | 3 | 224 | 25 | 8931 | 302 |
| address_fixed_n1000_insert_middle_common | 585 | 5520 | 4935 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 228 | 25 | 4866 | 302 |
| address_fixed_n1000_replace_middle_common | 585 | 3492 | 2907 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2910 | 228 |
| address_fixed_n1000_remove_middle_common | 585 | 4344 | 3759 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 3553 | 437 |
| address_fixed_n1000_remove_into_middle_common | 585 | 5535 | 4950 | 40016/39976 | 40016/39976 | 19960 | 0 | 3 | 228 | 25 | 4744 | 437 |
| address_fixed_n1000_at_mut_middle_common | 585 | 1923 | 1338 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1341 | 228 |
| address_fixed_n1000_lookup_hit_middle_common | 235 | 839 | 604 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 612 | 14 |
| address_fixed_n1000_lookup_miss_middle_common | 235 | 4400 | 4165 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4175 | 14 |
| address_fixed_n1000_entry_occupied_middle_common | 585 | 2828 | 2243 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2250 | 228 |
| address_fixed_n1000_entry_vacant_middle_common | 585 | 8119 | 7534 | 40016/40056 | 40016/40056 | 20000 | 0 | 3 | 224 | 25 | 7469 | 302 |
| address_fixed_n1000_insert_end_common | 585 | 1729 | 1144 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 228 | 25 | 1075 | 302 |
| address_fixed_n1000_replace_end_common | 585 | 3172 | 2587 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 2590 | 228 |
| address_fixed_n1000_remove_end_common | 585 | 1750 | 1165 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 959 | 437 |
| address_fixed_n1000_remove_into_end_common | 585 | 2935 | 2350 | 40016/39976 | 40016/39976 | 0 | 0 | 3 | 228 | 25 | 2144 | 437 |
| address_fixed_n1000_at_mut_end_common | 585 | 1763 | 1178 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 228 | 25 | 1181 | 228 |
| address_fixed_n1000_lookup_hit_end_common | 235 | 4249 | 4014 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4022 | 14 |
| address_fixed_n1000_lookup_miss_end_common | 235 | 4820 | 4585 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 80 | 34 | 4595 | 14 |
| address_fixed_n1000_entry_occupied_end_common | 585 | 2668 | 2083 | 40016/40016 | 40016/40016 | 0 | 0 | 3 | 224 | 25 | 2090 | 228 |
| address_fixed_n1000_entry_vacant_end_common | 585 | 4187 | 3602 | 40016/40056 | 40016/40056 | 0 | 0 | 3 | 224 | 25 | 3537 | 302 |
| address_indexed_n1000_load_trusted_all_plain | 234 | 234 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n1000_late_read_all_plain | 234 | 2156 | 1922 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1930 | 14 |
| address_indexed_n1000_validate_all_plain | 234 | 38591 | 38357 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 38364 | 14 |
| address_indexed_n1000_load_all_plain | 234 | 38607 | 38373 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 38405 | 7 | 14 |
| address_indexed_n1000_mutable_baseline_all_plain | 584 | 584 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n1000_clear_all_plain | 584 | 14044 | 13460 | 44016/16 | 44016/16 | 0 | 0 | 3 | 229 | 25 | 13254 | 437 |
| address_indexed_n1000_cursor_first_all_plain | 584 | 32690 | 32106 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 32035 | 302 |
| address_indexed_n1000_for_each_read_all_plain | 584 | 253232 | 252648 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 252651 | 228 |
| address_indexed_n1000_insert_front_plain | 584 | 42507 | 41923 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 229 | 25 | 41854 | 302 |
| address_indexed_n1000_replace_front_plain | 584 | 37566 | 36982 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 36911 | 302 |
| address_indexed_n1000_remove_front_plain | 584 | 42165 | 41581 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41375 | 437 |
| address_indexed_n1000_remove_into_front_plain | 584 | 41815 | 41231 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41025 | 437 |
| address_indexed_n1000_at_mut_front_plain | 584 | 32043 | 31459 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 31388 | 302 |
| address_indexed_n1000_lookup_hit_front_plain | 234 | 2194 | 1960 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1968 | 14 |
| address_indexed_n1000_lookup_miss_front_plain | 234 | 1824 | 1590 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1600 | 14 |
| address_indexed_n1000_entry_occupied_front_plain | 584 | 32024 | 31440 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 225 | 25 | 31373 | 302 |
| address_indexed_n1000_entry_vacant_front_plain | 584 | 43554 | 42970 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 225 | 25 | 42905 | 302 |
| address_indexed_n1000_insert_middle_plain | 584 | 24729 | 24145 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 229 | 25 | 24076 | 302 |
| address_indexed_n1000_replace_middle_plain | 584 | 20703 | 20119 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 229 | 25 | 20048 | 302 |
| address_indexed_n1000_remove_middle_plain | 584 | 23742 | 23158 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22952 | 437 |
| address_indexed_n1000_remove_into_middle_plain | 584 | 23394 | 22810 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22604 | 437 |
| address_indexed_n1000_at_mut_middle_plain | 584 | 17675 | 17091 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 229 | 25 | 17020 | 302 |
| address_indexed_n1000_lookup_hit_middle_plain | 234 | 1057 | 823 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 831 | 14 |
| address_indexed_n1000_lookup_miss_middle_plain | 234 | 1755 | 1521 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1531 | 14 |
| address_indexed_n1000_entry_occupied_middle_plain | 584 | 17616 | 17032 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 225 | 25 | 16965 | 302 |
| address_indexed_n1000_entry_vacant_middle_plain | 584 | 25847 | 25263 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 225 | 25 | 25198 | 302 |
| address_indexed_n1000_insert_end_plain | 584 | 4632 | 4048 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 229 | 25 | 3979 | 302 |
| address_indexed_n1000_replace_end_plain | 584 | 3431 | 2847 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 229 | 25 | 2776 | 302 |
| address_indexed_n1000_remove_end_plain | 584 | 4789 | 4205 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3999 | 437 |
| address_indexed_n1000_remove_into_end_plain | 584 | 4449 | 3865 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3659 | 437 |
| address_indexed_n1000_at_mut_end_plain | 584 | 2978 | 2394 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 229 | 25 | 2323 | 302 |
| address_indexed_n1000_lookup_hit_end_plain | 234 | 2156 | 1922 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1930 | 14 |
| address_indexed_n1000_lookup_miss_end_plain | 234 | 1886 | 1652 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 1662 | 14 |
| address_indexed_n1000_entry_occupied_end_plain | 584 | 2918 | 2334 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 225 | 25 | 2267 | 302 |
| address_indexed_n1000_entry_vacant_end_plain | 584 | 5681 | 5097 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 225 | 25 | 5032 | 302 |
| address_indexed_n1000_load_trusted_all_common | 236 | 236 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 9 | 14 |
| address_indexed_n1000_late_read_all_common | 236 | 4470 | 4234 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4242 | 14 |
| address_indexed_n1000_validate_all_common | 236 | 47584 | 47348 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 47355 | 14 |
| address_indexed_n1000_load_all_common | 236 | 47600 | 47364 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 47396 | 7 | 14 |
| address_indexed_n1000_mutable_baseline_all_common | 586 | 586 | 0 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 4 | 228 |
| address_indexed_n1000_clear_all_common | 586 | 14046 | 13460 | 44016/16 | 44016/16 | 0 | 0 | 3 | 229 | 25 | 13254 | 437 |
| address_indexed_n1000_cursor_first_all_common | 586 | 32692 | 32106 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 32035 | 302 |
| address_indexed_n1000_for_each_read_all_common | 586 | 262225 | 261639 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 229 | 25 | 261642 | 228 |
| address_indexed_n1000_insert_front_common | 586 | 42527 | 41941 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 229 | 25 | 41872 | 302 |
| address_indexed_n1000_replace_front_common | 586 | 37577 | 36991 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 36920 | 302 |
| address_indexed_n1000_remove_front_common | 586 | 42176 | 41590 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41384 | 437 |
| address_indexed_n1000_remove_into_front_common | 586 | 41826 | 41240 | 44016/43972 | 44016/43972 | 43956 | 999 | 3 | 229 | 25 | 41034 | 437 |
| address_indexed_n1000_at_mut_front_common | 586 | 32054 | 31468 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 229 | 25 | 31397 | 302 |
| address_indexed_n1000_lookup_hit_front_common | 236 | 4797 | 4561 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4569 | 14 |
| address_indexed_n1000_lookup_miss_front_common | 236 | 4716 | 4480 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4490 | 14 |
| address_indexed_n1000_entry_occupied_front_common | 586 | 32035 | 31449 | 44016/44025 | 44016/44025 | 7992 | 1000 | 3 | 225 | 25 | 31382 | 302 |
| address_indexed_n1000_entry_vacant_front_common | 586 | 43589 | 43003 | 44016/44069 | 44016/44069 | 44000 | 1001 | 3 | 225 | 25 | 42938 | 302 |
| address_indexed_n1000_insert_middle_common | 586 | 24881 | 24295 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 229 | 25 | 24226 | 302 |
| address_indexed_n1000_replace_middle_common | 586 | 20723 | 20137 | 44016/44025 | 44016/44025 | 3992 | 1000 | 3 | 229 | 25 | 20066 | 302 |
| address_indexed_n1000_remove_middle_common | 586 | 23762 | 23176 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22970 | 437 |
| address_indexed_n1000_remove_into_middle_common | 586 | 23414 | 22828 | 44016/43972 | 44016/43972 | 25956 | 999 | 3 | 229 | 25 | 22622 | 437 |
| address_indexed_n1000_at_mut_middle_common | 586 | 17695 | 17109 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 229 | 25 | 17038 | 302 |
| address_indexed_n1000_lookup_hit_middle_common | 236 | 1059 | 823 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 831 | 14 |
| address_indexed_n1000_lookup_miss_middle_common | 236 | 4358 | 4122 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4132 | 14 |
| address_indexed_n1000_entry_occupied_middle_common | 586 | 17636 | 17050 | 44016/44025 | 44016/44025 | 3992 | 500 | 3 | 225 | 25 | 16983 | 302 |
| address_indexed_n1000_entry_vacant_middle_common | 586 | 26026 | 25440 | 44016/44069 | 44016/44069 | 26000 | 1001 | 3 | 225 | 25 | 25375 | 302 |
| address_indexed_n1000_insert_end_common | 586 | 4643 | 4057 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 229 | 25 | 3988 | 302 |
| address_indexed_n1000_replace_end_common | 586 | 3433 | 2847 | 44016/44025 | 44016/44025 | 0 | 1000 | 3 | 229 | 25 | 2776 | 302 |
| address_indexed_n1000_remove_end_common | 586 | 4791 | 4205 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3999 | 437 |
| address_indexed_n1000_remove_into_end_common | 586 | 4451 | 3865 | 44016/43972 | 44016/43972 | 7992 | 999 | 3 | 229 | 25 | 3659 | 437 |
| address_indexed_n1000_at_mut_end_common | 586 | 2980 | 2394 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 229 | 25 | 2323 | 302 |
| address_indexed_n1000_lookup_hit_end_common | 236 | 4470 | 4234 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4242 | 14 |
| address_indexed_n1000_lookup_miss_end_common | 236 | 4778 | 4542 | 44016/44016 | 44016/44016 | 0 | 0 | 3 | 81 | 34 | 4552 | 14 |
| address_indexed_n1000_entry_occupied_end_common | 586 | 2920 | 2334 | 44016/44025 | 44016/44025 | 0 | 1 | 3 | 225 | 25 | 2267 | 302 |
| address_indexed_n1000_entry_vacant_end_common | 586 | 5704 | 5118 | 44016/44069 | 44016/44069 | 8000 | 1001 | 3 | 225 | 25 | 5053 | 302 |
| depth1_mutable_baseline | 658 | 658 | 0 | 20/20 | 20/20 | 0 | 0 | 1 | 205 | 196 | 45 | 107 |
| depth1_string_set_grow | 658 | 1212 | 554 | 20/29 | 20/29 | 0 | 0 | 1 | 205 | 196 | 530 | 178 |
| depth1_string_set_shrink | 658 | 1224 | 566 | 20/13 | 20/13 | 0 | 0 | 1 | 205 | 196 | 405 | 313 |
| depth1_string_push | 658 | 1771 | 1113 | 20/37 | 20/37 | 0 | 0 | 1 | 205 | 196 | 1087 | 178 |
| depth1_string_truncate | 658 | 1724 | 1066 | 20/13 | 20/13 | 0 | 0 | 1 | 205 | 196 | 905 | 313 |
| depth1_string_clear | 658 | 1224 | 566 | 20/12 | 20/12 | 0 | 0 | 1 | 205 | 196 | 405 | 313 |
| depth4_mutable_baseline | 2997 | 2997 | 0 | 76/76 | 76/76 | 0 | 0 | 4 | 207 | 24 | 2433 | 228 |
| depth4_string_set_grow | 2997 | 4818 | 1821 | 76/85 | 76/85 | 36 | 3 | 4 | 207 | 24 | 4182 | 302 |
| depth4_string_set_shrink | 2997 | 4817 | 1820 | 76/69 | 76/69 | 36 | 3 | 4 | 207 | 24 | 4044 | 437 |
| depth4_string_push | 2997 | 5407 | 2410 | 76/93 | 76/93 | 36 | 3 | 4 | 207 | 24 | 4769 | 302 |
| depth4_string_truncate | 2997 | 5348 | 2351 | 76/69 | 76/69 | 36 | 3 | 4 | 207 | 24 | 4575 | 437 |
| depth4_string_clear | 2997 | 4817 | 1820 | 76/68 | 76/68 | 36 | 3 | 4 | 207 | 24 | 4044 | 437 |
| depth8_mutable_baseline | 10540 | 10540 | 0 | 1036/1036 | 1036/1036 | 0 | 0 | 8 | 206 | 24 | 9977 | 228 |
| depth8_string_set_grow | 10540 | 13805 | 3265 | 1036/1045 | 1036/1045 | 964 | 7 | 8 | 206 | 24 | 13170 | 302 |
| depth8_string_set_shrink | 10540 | 13804 | 3264 | 1036/1029 | 1036/1029 | 964 | 7 | 8 | 206 | 24 | 13032 | 437 |
| depth8_string_push | 10540 | 14458 | 3918 | 1036/1053 | 1036/1053 | 964 | 7 | 8 | 206 | 24 | 13821 | 302 |
| depth8_string_truncate | 10540 | 14399 | 3859 | 1036/1029 | 1036/1029 | 964 | 7 | 8 | 206 | 24 | 13627 | 437 |
| depth8_string_clear | 10540 | 13804 | 3264 | 1036/1028 | 1036/1028 | 964 | 7 | 8 | 206 | 24 | 13032 | 437 |
| depth3_option_set | 1540 | 2839 | 1299 | 21/13 | 21/13 | 0 | 0 | 3 | 207 | 986 | 1100 | 442 |
| depth3_option_some | 1540 | 3136 | 1596 | 21/30 | 21/30 | 0 | 0 | 3 | 207 | 986 | 1532 | 307 |

| Raw lease (n / mode / action / funding) | Baseline | Whole | Difference | Used | Physical before/after | Opened | Peak | Re-leases | Zeroed | CPIs | TopUp | Refund | Open | Load | Work | Commit | Open probes | Work resizes | Commit resizes | Slot bytes |
|---|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lease_n10_Max_same_prepaid | 579 | 579 | 0 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n10_Max_same_topup | 635 | 635 | 0 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n10_Max_same_refund | 903 | 903 | 0 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 99998468800 | 211 | 0 | 145 | 378 | 1 | 0 | 1 | 0 |
| lease_n10_Max_same_retain32 | 2164 | 2164 | 0 | 92 | 92/124 | 10332 | 10332 | 0 | 10240 | 1 | 222720 | 0 | 268 | 0 | 294 | 1434 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_prepaid | 579 | 863 | 284 | 100 | 92/100 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 294 | 197 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_topup | 635 | 2168 | 1533 | 100 | 92/100 | 10332 | 10332 | 0 | 10240 | 1 | 55680 | 0 | 268 | 0 | 305 | 1435 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_refund | 903 | 973 | 70 | 100 | 92/100 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 99998413120 | 211 | 0 | 365 | 236 | 1 | 0 | 1 | 0 |
| lease_n10_Max_grow8_retain32 | 2164 | 2250 | 86 | 100 | 92/132 | 10332 | 10332 | 0 | 10240 | 1 | 278400 | 0 | 268 | 0 | 388 | 1434 | 1 | 0 | 1 | 0 |
| lease_n10_Max_fifty_pushes_prepaid | 579 | 5141 | 4562 | 492 | 92/492 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 4567 | 197 | 1 | 0 | 1 | 0 |
| lease_n10_Max_after_grow501_prepaid | 579 | 2058 | 1479 | 593 | 92/593 | 10332 | 10332 | 0 | 9739 | 0 | 0 | 0 | 1111 | 0 | 76 | 125 | 15 | 0 | 1 | 0 |
| lease_n10_Max_after_shrink37_prepaid | 579 | 1119 | 540 | 55 | 92/55 | 10295 | 10295 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 76 | 123 | 1 | 0 | 1 | 0 |
| lease_n10_Max_two_cycles_prepaid | 579 | 872 | 293 | 92 | 92/92 | 10332 | 10332 | 0 | 20480 | 0 | 0 | 0 | 211 | 0 | 76 | 416 | 1 | 0 | 3 | 0 |
| lease_n10_Max_readonly_prepaid | 579 | 218 | -361 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 40 | 0 | 4 | 6 | 0 | 0 | 0 | 0 |
| lease_n10_Max_shrink64_prepaid | 579 | 831 | 252 | 64 | 92/64 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 121 | 332 | 1 | 0 | 1 | 0 |
| lease_n10_Max_shrink64_topup | 635 | 950 | 315 | 64 | 92/64 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 121 | 395 | 1 | 0 | 1 | 0 |
| lease_n10_Max_shrink64_refund | 903 | 941 | 38 | 64 | 92/64 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 99998663680 | 211 | 0 | 192 | 371 | 1 | 0 | 1 | 0 |
| lease_n10_Max_shrink64_retain32 | 2164 | 2209 | 45 | 64 | 92/96 | 10332 | 10332 | 0 | 10240 | 1 | 27840 | 0 | 268 | 0 | 341 | 1434 | 1 | 0 | 1 | 0 |
| lease_n10_Max_exact_window_prepaid | 579 | 834 | 255 | 10332 | 92/10332 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 295 | 161 | 1 | 0 | 0 | 0 |
| lease_n10_Max_failed_growth_prepaid | 579 | 629 | 50 | 92 | 92/92 | 10332 | 10332 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 125 | 125 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo0_same_prepaid | 467 | 467 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 74 | 89 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_same_topup | 523 | 523 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 192 | 0 | 74 | 89 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_same_refund | 791 | 791 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 99998468800 | 135 | 0 | 145 | 342 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_after_grow501_prepaid | 467 | 1046 | 579 | 593 | 92/593 | 593 | 593 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 89 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_after_shrink37_prepaid | 467 | 1009 | 542 | 55 | 92/55 | 55 | 55 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 89 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_two_cycles_prepaid | 467 | 649 | 182 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 269 | 0 | 0 | 0 | 0 |
| lease_n10_UpTo0_shrink64_prepaid | 467 | 755 | 288 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 121 | 332 | 0 | 0 | 1 | 0 |
| lease_n10_UpTo0_shrink64_topup | 523 | 874 | 351 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 192 | 0 | 121 | 395 | 0 | 0 | 1 | 0 |
| lease_n10_UpTo0_shrink64_refund | 791 | 865 | 74 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 99998663680 | 135 | 0 | 192 | 371 | 0 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_prepaid | 564 | 564 | 0 | 92 | 92/92 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_topup | 620 | 620 | 0 | 92 | 92/92 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_refund | 888 | 888 | 0 | 92 | 92/92 | 124 | 124 | 0 | 32 | 0 | 0 | 99998468800 | 196 | 0 | 145 | 378 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_same_retain32 | 2113 | 2113 | 0 | 92 | 92/124 | 124 | 124 | 0 | 32 | 1 | 222720 | 0 | 253 | 0 | 294 | 1398 | 1 | 0 | 0 | 0 |
| lease_n10_UpTo32_grow8_prepaid | 564 | 848 | 284 | 100 | 92/100 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 294 | 197 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_grow8_topup | 620 | 2153 | 1533 | 100 | 92/100 | 124 | 124 | 0 | 32 | 1 | 55680 | 0 | 253 | 0 | 305 | 1435 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_grow8_refund | 888 | 958 | 70 | 100 | 92/100 | 124 | 124 | 0 | 32 | 0 | 0 | 99998413120 | 196 | 0 | 365 | 236 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_after_grow501_prepaid | 564 | 1143 | 579 | 593 | 92/593 | 625 | 625 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 76 | 125 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_after_shrink37_prepaid | 564 | 1102 | 538 | 55 | 92/55 | 87 | 87 | 0 | 32 | 0 | 0 | 0 | 194 | 0 | 76 | 123 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_two_cycles_prepaid | 564 | 843 | 279 | 92 | 92/92 | 124 | 124 | 0 | 64 | 0 | 0 | 0 | 196 | 0 | 76 | 402 | 1 | 0 | 3 | 0 |
| lease_n10_UpTo32_shrink64_prepaid | 564 | 816 | 252 | 64 | 92/64 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 121 | 332 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_shrink64_topup | 620 | 935 | 315 | 64 | 92/64 | 124 | 124 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 121 | 395 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_shrink64_refund | 888 | 926 | 38 | 64 | 92/64 | 124 | 124 | 0 | 32 | 0 | 0 | 99998663680 | 196 | 0 | 192 | 371 | 1 | 0 | 1 | 0 |
| lease_n10_UpTo32_shrink64_retain32 | 2113 | 2194 | 81 | 64 | 92/96 | 124 | 124 | 0 | 32 | 1 | 27840 | 0 | 253 | 0 | 341 | 1434 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_prepaid | 579 | 579 | 0 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_topup | 635 | 635 | 0 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_refund | 903 | 903 | 0 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 99993457600 | 211 | 0 | 145 | 378 | 1 | 0 | 1 | 0 |
| lease_n100_Max_same_retain32 | 2167 | 2167 | 0 | 812 | 812/844 | 11052 | 11052 | 0 | 10240 | 1 | 222720 | 0 | 268 | 0 | 294 | 1437 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_prepaid | 579 | 863 | 284 | 820 | 812/820 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 294 | 197 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_topup | 635 | 2171 | 1536 | 820 | 812/820 | 11052 | 11052 | 0 | 10240 | 1 | 55680 | 0 | 268 | 0 | 305 | 1438 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_refund | 903 | 973 | 70 | 820 | 812/820 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 99993401920 | 211 | 0 | 365 | 236 | 1 | 0 | 1 | 0 |
| lease_n100_Max_grow8_retain32 | 2167 | 2253 | 86 | 820 | 812/852 | 11052 | 11052 | 0 | 10240 | 1 | 278400 | 0 | 268 | 0 | 388 | 1437 | 1 | 0 | 1 | 0 |
| lease_n100_Max_fifty_pushes_prepaid | 579 | 5141 | 4562 | 1212 | 812/1212 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 4567 | 197 | 1 | 0 | 1 | 0 |
| lease_n100_Max_after_grow501_prepaid | 579 | 2058 | 1479 | 1313 | 812/1313 | 11052 | 11052 | 0 | 9739 | 0 | 0 | 0 | 1111 | 0 | 76 | 125 | 15 | 0 | 1 | 0 |
| lease_n100_Max_after_shrink37_prepaid | 579 | 1119 | 540 | 775 | 812/775 | 11015 | 11015 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 76 | 123 | 1 | 0 | 1 | 0 |
| lease_n100_Max_two_cycles_prepaid | 579 | 872 | 293 | 812 | 812/812 | 11052 | 11052 | 0 | 20480 | 0 | 0 | 0 | 211 | 0 | 76 | 416 | 1 | 0 | 3 | 0 |
| lease_n100_Max_readonly_prepaid | 579 | 218 | -361 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 40 | 0 | 4 | 6 | 0 | 0 | 0 | 0 |
| lease_n100_Max_shrink64_prepaid | 579 | 831 | 252 | 64 | 812/64 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 121 | 332 | 1 | 0 | 1 | 0 |
| lease_n100_Max_shrink64_topup | 635 | 950 | 315 | 64 | 812/64 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 121 | 395 | 1 | 0 | 1 | 0 |
| lease_n100_Max_shrink64_refund | 903 | 941 | 38 | 64 | 812/64 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 99998663680 | 211 | 0 | 192 | 371 | 1 | 0 | 1 | 0 |
| lease_n100_Max_shrink64_retain32 | 2167 | 992 | -1175 | 64 | 812/96 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 164 | 394 | 1 | 0 | 1 | 0 |
| lease_n100_Max_exact_window_prepaid | 579 | 834 | 255 | 11052 | 812/11052 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 295 | 161 | 1 | 0 | 0 | 0 |
| lease_n100_Max_failed_growth_prepaid | 579 | 629 | 50 | 812 | 812/812 | 11052 | 11052 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 125 | 125 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo0_same_prepaid | 467 | 467 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 74 | 89 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_same_topup | 523 | 523 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 192 | 0 | 74 | 89 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_same_refund | 791 | 791 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 99993457600 | 135 | 0 | 145 | 342 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_after_grow501_prepaid | 467 | 1046 | 579 | 1313 | 812/1313 | 1313 | 1313 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 89 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_after_shrink37_prepaid | 467 | 1009 | 542 | 775 | 812/775 | 775 | 775 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 89 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_two_cycles_prepaid | 467 | 649 | 182 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 269 | 0 | 0 | 0 | 0 |
| lease_n100_UpTo0_shrink64_prepaid | 467 | 755 | 288 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 121 | 332 | 0 | 0 | 1 | 0 |
| lease_n100_UpTo0_shrink64_topup | 523 | 874 | 351 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 192 | 0 | 121 | 395 | 0 | 0 | 1 | 0 |
| lease_n100_UpTo0_shrink64_refund | 791 | 865 | 74 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 99998663680 | 135 | 0 | 192 | 371 | 0 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_prepaid | 564 | 564 | 0 | 812 | 812/812 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_topup | 620 | 620 | 0 | 812 | 812/812 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_refund | 888 | 888 | 0 | 812 | 812/812 | 844 | 844 | 0 | 32 | 0 | 0 | 99993457600 | 196 | 0 | 145 | 378 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_same_retain32 | 2116 | 2116 | 0 | 812 | 812/844 | 844 | 844 | 0 | 32 | 1 | 222720 | 0 | 253 | 0 | 294 | 1401 | 1 | 0 | 0 | 0 |
| lease_n100_UpTo32_grow8_prepaid | 564 | 848 | 284 | 820 | 812/820 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 294 | 197 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_grow8_topup | 620 | 2156 | 1536 | 820 | 812/820 | 844 | 844 | 0 | 32 | 1 | 55680 | 0 | 253 | 0 | 305 | 1438 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_grow8_refund | 888 | 958 | 70 | 820 | 812/820 | 844 | 844 | 0 | 32 | 0 | 0 | 99993401920 | 196 | 0 | 365 | 236 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_after_grow501_prepaid | 564 | 1143 | 579 | 1313 | 812/1313 | 1345 | 1345 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 76 | 125 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_after_shrink37_prepaid | 564 | 1102 | 538 | 775 | 812/775 | 807 | 807 | 0 | 32 | 0 | 0 | 0 | 194 | 0 | 76 | 123 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_two_cycles_prepaid | 564 | 843 | 279 | 812 | 812/812 | 844 | 844 | 0 | 64 | 0 | 0 | 0 | 196 | 0 | 76 | 402 | 1 | 0 | 3 | 0 |
| lease_n100_UpTo32_shrink64_prepaid | 564 | 816 | 252 | 64 | 812/64 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 121 | 332 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_shrink64_topup | 620 | 935 | 315 | 64 | 812/64 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 121 | 395 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_shrink64_refund | 888 | 926 | 38 | 64 | 812/64 | 844 | 844 | 0 | 32 | 0 | 0 | 99998663680 | 196 | 0 | 192 | 371 | 1 | 0 | 1 | 0 |
| lease_n100_UpTo32_shrink64_retain32 | 2116 | 977 | -1139 | 64 | 812/96 | 844 | 844 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 164 | 394 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_prepaid | 579 | 579 | 0 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_topup | 635 | 635 | 0 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_refund | 903 | 903 | 0 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 99943345600 | 211 | 0 | 145 | 378 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_same_retain32 | 2196 | 2196 | 0 | 8012 | 8012/8044 | 18252 | 18252 | 0 | 10240 | 1 | 222720 | 0 | 268 | 0 | 294 | 1466 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_prepaid | 579 | 863 | 284 | 8020 | 8012/8020 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 294 | 197 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_topup | 635 | 2200 | 1565 | 8020 | 8012/8020 | 18252 | 18252 | 0 | 10240 | 1 | 55680 | 0 | 268 | 0 | 305 | 1467 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_refund | 903 | 973 | 70 | 8020 | 8012/8020 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 99943289920 | 211 | 0 | 365 | 236 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_grow8_retain32 | 2196 | 2282 | 86 | 8020 | 8012/8052 | 18252 | 18252 | 0 | 10240 | 1 | 278400 | 0 | 268 | 0 | 388 | 1466 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_fifty_pushes_prepaid | 579 | 5141 | 4562 | 8412 | 8012/8412 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 4567 | 197 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_after_grow501_prepaid | 579 | 2058 | 1479 | 8513 | 8012/8513 | 18252 | 18252 | 0 | 9739 | 0 | 0 | 0 | 1111 | 0 | 76 | 125 | 15 | 0 | 1 | 0 |
| lease_n1000_Max_after_shrink37_prepaid | 579 | 1119 | 540 | 7975 | 8012/7975 | 18215 | 18215 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 76 | 123 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_two_cycles_prepaid | 579 | 872 | 293 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 20480 | 0 | 0 | 0 | 211 | 0 | 76 | 416 | 1 | 0 | 3 | 0 |
| lease_n1000_Max_readonly_prepaid | 579 | 218 | -361 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 40 | 0 | 4 | 6 | 0 | 0 | 0 | 0 |
| lease_n1000_Max_shrink64_prepaid | 579 | 831 | 252 | 64 | 8012/64 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 121 | 332 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_shrink64_topup | 635 | 950 | 315 | 64 | 8012/64 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 121 | 395 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_shrink64_refund | 903 | 941 | 38 | 64 | 8012/64 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 99998663680 | 211 | 0 | 192 | 371 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_shrink64_retain32 | 2196 | 992 | -1204 | 64 | 8012/96 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 268 | 0 | 164 | 394 | 1 | 0 | 1 | 0 |
| lease_n1000_Max_exact_window_prepaid | 579 | 834 | 255 | 18252 | 8012/18252 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 295 | 161 | 1 | 0 | 0 | 0 |
| lease_n1000_Max_failed_growth_prepaid | 579 | 629 | 50 | 8012 | 8012/8012 | 18252 | 18252 | 0 | 10240 | 0 | 0 | 0 | 211 | 0 | 125 | 125 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo0_same_prepaid | 467 | 467 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 74 | 89 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_same_topup | 523 | 523 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 192 | 0 | 74 | 89 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_same_refund | 791 | 791 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99943345600 | 135 | 0 | 145 | 342 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_after_grow501_prepaid | 467 | 1046 | 579 | 8513 | 8012/8513 | 8513 | 8513 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 89 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_after_shrink37_prepaid | 467 | 1009 | 542 | 7975 | 8012/7975 | 7975 | 7975 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 89 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_two_cycles_prepaid | 467 | 649 | 182 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 76 | 269 | 0 | 0 | 0 | 0 |
| lease_n1000_UpTo0_shrink64_prepaid | 467 | 755 | 288 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 135 | 0 | 121 | 332 | 0 | 0 | 1 | 0 |
| lease_n1000_UpTo0_shrink64_topup | 523 | 874 | 351 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 192 | 0 | 121 | 395 | 0 | 0 | 1 | 0 |
| lease_n1000_UpTo0_shrink64_refund | 791 | 865 | 74 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99998663680 | 135 | 0 | 192 | 371 | 0 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_prepaid | 564 | 564 | 0 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_topup | 620 | 620 | 0 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 74 | 125 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_refund | 888 | 888 | 0 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 32 | 0 | 0 | 99943345600 | 196 | 0 | 145 | 378 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_same_retain32 | 2145 | 2145 | 0 | 8012 | 8012/8044 | 8044 | 8044 | 0 | 32 | 1 | 222720 | 0 | 253 | 0 | 294 | 1430 | 1 | 0 | 0 | 0 |
| lease_n1000_UpTo32_grow8_prepaid | 564 | 848 | 284 | 8020 | 8012/8020 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 294 | 197 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_grow8_topup | 620 | 2185 | 1565 | 8020 | 8012/8020 | 8044 | 8044 | 0 | 32 | 1 | 55680 | 0 | 253 | 0 | 305 | 1467 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_grow8_refund | 888 | 958 | 70 | 8020 | 8012/8020 | 8044 | 8044 | 0 | 32 | 0 | 0 | 99943289920 | 196 | 0 | 365 | 236 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_after_grow501_prepaid | 564 | 1143 | 579 | 8513 | 8012/8513 | 8545 | 8545 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 76 | 125 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_after_shrink37_prepaid | 564 | 1102 | 538 | 7975 | 8012/7975 | 8007 | 8007 | 0 | 32 | 0 | 0 | 0 | 194 | 0 | 76 | 123 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_two_cycles_prepaid | 564 | 843 | 279 | 8012 | 8012/8012 | 8044 | 8044 | 0 | 64 | 0 | 0 | 0 | 196 | 0 | 76 | 402 | 1 | 0 | 3 | 0 |
| lease_n1000_UpTo32_shrink64_prepaid | 564 | 816 | 252 | 64 | 8012/64 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 196 | 0 | 121 | 332 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_shrink64_topup | 620 | 935 | 315 | 64 | 8012/64 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 121 | 395 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_shrink64_refund | 888 | 926 | 38 | 64 | 8012/64 | 8044 | 8044 | 0 | 32 | 0 | 0 | 99998663680 | 196 | 0 | 192 | 371 | 1 | 0 | 1 | 0 |
| lease_n1000_UpTo32_shrink64_retain32 | 2145 | 977 | -1168 | 64 | 8012/96 | 8044 | 8044 | 0 | 32 | 0 | 0 | 0 | 253 | 0 | 164 | 394 | 1 | 0 | 1 | 0 |
| lease_n10_OnDemand_same_prepaid | 413 | 413 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 76 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_same_topup | 467 | 467 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 76 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_same_refund | 721 | 721 | 0 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 99998468800 | 88 | 0 | 131 | 341 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_same_retain32 | 2173 | 2173 | 0 | 92 | 92/124 | 92 | 156 | 1 | 64 | 1 | 222720 | 0 | 145 | 0 | 437 | 1433 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_prepaid | 413 | 859 | 446 | 100 | 92/100 | 92 | 123 | 1 | 31 | 0 | 0 | 0 | 88 | 0 | 422 | 196 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_topup | 467 | 2162 | 1695 | 100 | 92/100 | 92 | 123 | 1 | 31 | 1 | 55680 | 0 | 145 | 0 | 433 | 1434 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_refund | 721 | 953 | 232 | 100 | 92/100 | 92 | 123 | 1 | 31 | 0 | 0 | 99998413120 | 88 | 0 | 477 | 235 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_grow8_retain32 | 2173 | 2383 | 210 | 100 | 92/132 | 92 | 162 | 2 | 70 | 1 | 278400 | 0 | 145 | 0 | 655 | 1433 | 0 | 2 | 1 | 16 |
| lease_n10_OnDemand_fifty_pushes_prepaid | 413 | 6194 | 5781 | 492 | 92/492 | 92 | 494 | 7 | 402 | 0 | 0 | 0 | 88 | 0 | 5752 | 196 | 0 | 7 | 1 | 56 |
| lease_n10_OnDemand_after_grow501_prepaid | 413 | 995 | 582 | 593 | 92/593 | 593 | 593 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_after_shrink37_prepaid | 413 | 954 | 541 | 55 | 92/55 | 55 | 55 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 88 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_two_cycles_prepaid | 413 | 544 | 131 | 92 | 92/92 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 217 | 0 | 0 | 0 | 0 |
| lease_n10_OnDemand_shrink64_prepaid | 413 | 704 | 291 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 125 | 331 | 0 | 0 | 1 | 0 |
| lease_n10_OnDemand_shrink64_topup | 467 | 821 | 354 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 125 | 394 | 0 | 0 | 1 | 0 |
| lease_n10_OnDemand_shrink64_refund | 721 | 798 | 77 | 64 | 92/64 | 92 | 92 | 0 | 0 | 0 | 0 | 99998663680 | 88 | 0 | 180 | 370 | 0 | 0 | 1 | 0 |
| lease_n10_OnDemand_shrink64_retain32 | 2173 | 2219 | 46 | 64 | 92/96 | 92 | 119 | 1 | 27 | 1 | 27840 | 0 | 145 | 0 | 484 | 1433 | 0 | 1 | 1 | 8 |
| lease_n10_OnDemand_exact_window_prepaid | 413 | 890 | 477 | 10332 | 92/10332 | 92 | 10332 | 1 | 10240 | 0 | 0 | 0 | 88 | 0 | 482 | 160 | 0 | 2 | 0 | 8 |
| lease_n10_OnDemand_failed_growth_prepaid | 413 | 747 | 334 | 92 | 92/92 | 92 | 92 | 1 | 0 | 0 | 0 | 0 | 88 | 0 | 411 | 88 | 0 | 2 | 0 | 8 |
| lease_n100_OnDemand_same_prepaid | 413 | 413 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 76 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_same_topup | 467 | 467 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 76 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_same_refund | 721 | 721 | 0 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 99993457600 | 88 | 0 | 131 | 341 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_same_retain32 | 2174 | 2174 | 0 | 812 | 812/844 | 812 | 1047 | 1 | 235 | 1 | 222720 | 0 | 145 | 0 | 435 | 1436 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_prepaid | 413 | 859 | 446 | 820 | 812/820 | 812 | 1023 | 1 | 211 | 0 | 0 | 0 | 88 | 0 | 422 | 196 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_topup | 467 | 2165 | 1698 | 820 | 812/820 | 812 | 1023 | 1 | 211 | 1 | 55680 | 0 | 145 | 0 | 433 | 1437 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_refund | 721 | 953 | 232 | 820 | 812/820 | 812 | 1023 | 1 | 211 | 0 | 0 | 99993401920 | 88 | 0 | 477 | 235 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_grow8_retain32 | 2174 | 2268 | 94 | 820 | 812/852 | 812 | 1023 | 1 | 211 | 1 | 278400 | 0 | 145 | 0 | 537 | 1436 | 0 | 1 | 1 | 8 |
| lease_n100_OnDemand_fifty_pushes_prepaid | 413 | 5599 | 5186 | 1212 | 812/1212 | 812 | 1283 | 2 | 471 | 0 | 0 | 0 | 88 | 0 | 5157 | 196 | 0 | 2 | 1 | 16 |
| lease_n100_OnDemand_after_grow501_prepaid | 413 | 995 | 582 | 1313 | 812/1313 | 1313 | 1313 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_after_shrink37_prepaid | 413 | 954 | 541 | 775 | 812/775 | 775 | 775 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 88 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_two_cycles_prepaid | 413 | 544 | 131 | 812 | 812/812 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 217 | 0 | 0 | 0 | 0 |
| lease_n100_OnDemand_shrink64_prepaid | 413 | 704 | 291 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 125 | 331 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_shrink64_topup | 467 | 821 | 354 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 125 | 394 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_shrink64_refund | 721 | 798 | 77 | 64 | 812/64 | 812 | 812 | 0 | 0 | 0 | 0 | 99998663680 | 88 | 0 | 180 | 370 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_shrink64_retain32 | 2174 | 882 | -1292 | 64 | 812/96 | 812 | 812 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 187 | 393 | 0 | 0 | 1 | 0 |
| lease_n100_OnDemand_exact_window_prepaid | 413 | 890 | 477 | 11052 | 812/11052 | 812 | 11052 | 1 | 10240 | 0 | 0 | 0 | 88 | 0 | 482 | 160 | 0 | 2 | 0 | 8 |
| lease_n100_OnDemand_failed_growth_prepaid | 413 | 747 | 334 | 812 | 812/812 | 812 | 812 | 1 | 0 | 0 | 0 | 0 | 88 | 0 | 411 | 88 | 0 | 2 | 0 | 8 |
| lease_n1000_OnDemand_same_prepaid | 413 | 413 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 76 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_same_topup | 467 | 467 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 76 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_same_refund | 721 | 721 | 0 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99943345600 | 88 | 0 | 131 | 341 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_same_retain32 | 2203 | 2203 | 0 | 8012 | 8012/8044 | 8012 | 10047 | 1 | 2035 | 1 | 222720 | 0 | 145 | 0 | 435 | 1465 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_prepaid | 413 | 859 | 446 | 8020 | 8012/8020 | 8012 | 10023 | 1 | 2011 | 0 | 0 | 0 | 88 | 0 | 422 | 196 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_topup | 467 | 2194 | 1727 | 8020 | 8012/8020 | 8012 | 10023 | 1 | 2011 | 1 | 55680 | 0 | 145 | 0 | 433 | 1466 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_refund | 721 | 953 | 232 | 8020 | 8012/8020 | 8012 | 10023 | 1 | 2011 | 0 | 0 | 99943289920 | 88 | 0 | 477 | 235 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_grow8_retain32 | 2203 | 2297 | 94 | 8020 | 8012/8052 | 8012 | 10023 | 1 | 2011 | 1 | 278400 | 0 | 145 | 0 | 537 | 1465 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_fifty_pushes_prepaid | 413 | 5480 | 5067 | 8412 | 8012/8412 | 8012 | 10023 | 1 | 2011 | 0 | 0 | 0 | 88 | 0 | 5038 | 196 | 0 | 1 | 1 | 8 |
| lease_n1000_OnDemand_after_grow501_prepaid | 413 | 995 | 582 | 8513 | 8012/8513 | 8513 | 8513 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_after_shrink37_prepaid | 413 | 954 | 541 | 7975 | 8012/7975 | 7975 | 7975 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 88 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_two_cycles_prepaid | 413 | 544 | 131 | 8012 | 8012/8012 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 78 | 217 | 0 | 0 | 0 | 0 |
| lease_n1000_OnDemand_shrink64_prepaid | 413 | 704 | 291 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 88 | 0 | 125 | 331 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_shrink64_topup | 467 | 821 | 354 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 125 | 394 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_shrink64_refund | 721 | 798 | 77 | 64 | 8012/64 | 8012 | 8012 | 0 | 0 | 0 | 0 | 99998663680 | 88 | 0 | 180 | 370 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_shrink64_retain32 | 2203 | 882 | -1321 | 64 | 8012/96 | 8012 | 8012 | 0 | 0 | 0 | 0 | 0 | 145 | 0 | 187 | 393 | 0 | 0 | 1 | 0 |
| lease_n1000_OnDemand_exact_window_prepaid | 413 | 890 | 477 | 18252 | 8012/18252 | 8012 | 18252 | 1 | 10240 | 0 | 0 | 0 | 88 | 0 | 482 | 160 | 0 | 2 | 0 | 8 |
| lease_n1000_OnDemand_failed_growth_prepaid | 413 | 747 | 334 | 8012 | 8012/8012 | 8012 | 8012 | 1 | 0 | 0 | 0 | 0 | 88 | 0 | 411 | 88 | 0 | 2 | 0 | 8 |
