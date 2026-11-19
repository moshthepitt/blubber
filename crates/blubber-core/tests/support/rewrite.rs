use blubber_core::raw::{resolve_account_edit, InlinePath, RewritePlan, Step};
use blubber_core::{
    validate, ErrorKind, KeyDomain, KeyLayout, Layout, Leaf, VariableField, MAX_DEPTH,
};

static TEXT: Layout = Layout::Str;
static LIST: Layout = Layout::List { element: &TEXT };
static MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Raw,
    },
    value: &TEXT,
};
static ROOT_LIST: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &LIST,
        },
        VariableField {
            declaration: 1,
            layout: &TEXT,
        },
    ],
};
static ROOT_MAP: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &MAP,
        },
        VariableField {
            declaration: 1,
            layout: &TEXT,
        },
    ],
};

// Independent serialization: neither the production planner nor its directory
// helpers contribute to this expected result.
fn collection(map: bool, lengths: &[usize], seed: u8) -> Vec<u8> {
    let mut bytes = (lengths.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for (i, &len) in lengths.iter().enumerate() {
        end += len as u32;
        if map {
            bytes.push((2 * i + 1) as u8);
        }
        if map || i + 1 < lengths.len() {
            bytes.extend_from_slice(&end.to_le_bytes());
        }
    }
    for (i, &len) in lengths.iter().enumerate() {
        bytes.extend(std::iter::repeat_n(seed + i as u8, len));
    }
    bytes
}
fn account(map: bool, lengths: &[usize], seed: u8, spare: usize) -> Vec<u8> {
    let selected = collection(map, lengths, seed);
    let mut bytes = b"rewrite8".to_vec();
    bytes.extend_from_slice(&(12 + 4 + selected.len() + 6).to_le_bytes()[..4]);
    bytes.extend_from_slice(&(selected.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&selected);
    bytes.extend_from_slice(b"suffix");
    bytes.resize(bytes.len() + spare, 0xa5);
    bytes
}
fn used(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize
}
fn path() -> InlinePath {
    let mut p = InlinePath::new();
    p.push(Step::Field(0)).unwrap();
    p
}

#[test]
fn mixed_empty_shrink_grow_slots_match_independent_full_physical_oracle() {
    for map in [false, true] {
        let layout = if map { &ROOT_MAP } else { &ROOT_LIST };
        for n in 0..=4usize {
            for geometry in 0..4usize.pow(n as u32) {
                let lengths: Vec<_> = (0..n)
                    .map(|i| (geometry / 4usize.pow(i as u32)) % 4)
                    .collect();
                let old: Vec<_> = (0..n).map(|i| (i + geometry + 1) % 4).collect();
                let mut bytes = account(map, &old, b'a', 32);
                let before = bytes.clone();
                let old_used = used(&bytes);
                let resolved =
                    resolve_account_edit(&mut bytes, *b"rewrite8", 0, layout, &path()).unwrap();
                let (span, selected) = resolved.selection();
                let plan = RewritePlan::check(
                    &resolved.inspect_resolved()[span.start as usize..span.end as usize],
                    &selected,
                    n as u32,
                    lengths.iter().sum::<usize>() as u32,
                )
                .unwrap();
                let mut write = resolved.prepare_subtree(plan.encoded_len()).unwrap();
                let mut stream = plan.begin(write.destination()).unwrap();
                for (i, &len) in lengths.iter().enumerate() {
                    {
                        stream.prepare_slot(len as u32).unwrap();
                        stream.destination()
                    }
                    .fill(b'k' + i as u8);
                    stream.advance(len).unwrap();
                }
                stream.finish().unwrap();
                write.finish();
                let new_used = used(&bytes);
                let mut expected = account(map, &lengths, b'k', 0);
                expected.resize(before.len(), 0xa5);
                // The unused capacity is framed exactly, including the old used
                // interval which is zeroed on shrink and untouched spare bytes.
                if new_used < old_used {
                    expected[new_used..old_used].fill(0);
                }
                assert_eq!(bytes, expected, "map={map} n={n} geometry={geometry}");
                validate(&bytes[12..new_used], layout, MAX_DEPTH).unwrap();
            }
        }
    }
}

#[test]
fn fixed_zero_width_and_interleaved_keys_are_exactly_framed() {
    static LEAVES: [Layout; 3] = [
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[],
        },
        Layout::Leaf(Leaf::Raw(1)),
        Layout::Leaf(Leaf::Raw(2)),
    ];
    for width in [0, 1, 2] {
        let leaf = &LEAVES[width as usize];
        let list = Layout::List { element: leaf };
        let map = Layout::Map {
            key: KeyLayout {
                width: 1,
                domain: KeyDomain::Raw,
            },
            value: leaf,
        };
        for layout in [&list, &map] {
            let is_map = std::ptr::eq(layout, &map);
            for n in 0..=4u32 {
                let mut bytes = n.to_le_bytes().to_vec();
                for i in 0..n {
                    if is_map {
                        bytes.push((i + 1) as u8);
                    }
                    bytes.extend(std::iter::repeat_n(7, width as usize));
                }
                let before = bytes.clone();
                if width == 0 {
                    assert!(RewritePlan::check(&bytes, layout, n, 0).is_err());
                    continue;
                }
                let plan = RewritePlan::check(&bytes, layout, n, n * width).unwrap();
                let mut stream = plan.begin(&mut bytes).unwrap();
                for _ in 0..n {
                    {
                        stream.prepare_slot(width).unwrap();
                        stream.destination()
                    }
                    .fill(9);
                    stream.advance(width as usize).unwrap();
                }
                stream.finish().unwrap();
                let mut expected = before;
                for i in 0..n as usize {
                    let at = 4 + i * (width as usize + usize::from(is_map)) + usize::from(is_map);
                    expected[at..at + width as usize].fill(9);
                }
                assert_eq!(bytes, expected);
                validate(&bytes, layout, MAX_DEPTH).unwrap();
            }
        }
    }
}

#[test]
fn malformed_count_keys_capacity_and_overflow_reject_before_writes() {
    let empty = [0u8; 4];
    let huge = Layout::Map {
        key: KeyLayout {
            width: u32::MAX,
            domain: KeyDomain::Raw,
        },
        value: &Layout::Str,
    };
    let plan = RewritePlan::check(&empty, &huge, 0, 0).unwrap();
    let mut empty_out = empty;
    plan.begin(&mut empty_out).unwrap().finish().unwrap();
    assert_eq!(empty_out, empty);
    let good = collection(false, &[1, 1], b'a');
    for (count, total, kind) in [
        (1, 2, ErrorKind::InvalidBoundary),
        (2, u32::MAX, ErrorKind::ArithmeticOverflow),
    ] {
        let before = good.clone();
        assert!(
            matches!(RewritePlan::check(&good,&LIST,count,total), Err(blubber_core::Error::Fault {kind:got,..}) if got==kind)
        );
        assert_eq!(good, before);
    }
    let mut bad = collection(true, &[1, 1], b'a');
    bad[9] = bad[4];
    assert!(RewritePlan::check(&bad, &MAP, 2, 2).is_err());
    let plan = RewritePlan::check(&good, &LIST, 2, 2).unwrap();
    for len in [0, 7, 9, 10, 11] {
        let mut bytes = vec![0xa5; len];
        let before = bytes.clone();
        assert!(plan.begin(&mut bytes).is_err());
        assert_eq!(bytes, before);
    }
    let mut bytes = account(false, &[1, 1], b'a', 0);
    let before = bytes.clone();
    let resolved = resolve_account_edit(&mut bytes, *b"rewrite8", 0, &ROOT_LIST, &path()).unwrap();
    assert!(resolved.prepare_subtree(100).is_err());
    assert_eq!(bytes, before);
}

#[test]
fn incomplete_or_changed_second_pass_cannot_authorize_publication() {
    for mode in 0..7 {
        let mut bytes = collection(false, &[1, 1], b'a');
        let plan = RewritePlan::check(&bytes, &LIST, 2, 2).unwrap();
        let mut stream = plan.begin(&mut bytes).unwrap();
        match mode {
            0 => {
                assert!(stream.prepare_slot(3).is_err());
            }
            1 => {
                stream.prepare_slot(1).unwrap();
                stream.destination()[0] = b'x';
                assert!(stream.advance(0).is_err());
            }
            2 => {
                stream.prepare_slot(1).unwrap();
                stream.destination()[0] = b'x';
                assert!(stream.advance(2).is_err());
            }
            3 => {
                stream.prepare_slot(1).unwrap();
                stream.destination()[0] = b'x';
                assert!(stream.prepare_slot(1).is_err());
            }
            4 => {
                {
                    stream.prepare_slot(0).unwrap();
                    stream.destination()
                };
                stream.advance(0).unwrap();
                {
                    stream.prepare_slot(0).unwrap();
                    stream.destination()
                };
                stream.advance(0).unwrap();
            }
            5 => {
                assert!(stream.advance(1).is_err());
            }
            _ => {
                stream.prepare_slot(1).unwrap();
                stream.destination()[0] = b'x';
                stream.advance(1).unwrap();
            }
        }
        assert!(stream.finish().is_err(), "mode={mode}");
    }
}

#[test]
fn nested_rewrite_patches_all_ancestors_and_moves_one_combined_outer_suffix() {
    static OUTER: Layout = Layout::List {
        element: &ROOT_LIST,
    };
    static ROOT: Layout = Layout::List { element: &OUTER };
    fn pair(first: &[u8], second: &[u8]) -> Vec<u8> {
        let mut out = 2u32.to_le_bytes().to_vec();
        out.extend_from_slice(&(first.len() as u32).to_le_bytes());
        out.extend_from_slice(first);
        out.extend_from_slice(second);
        out
    }
    for lengths in [[0, 0, 0], [0, 2, 4], [4, 4, 4], [1, 0, 1]] {
        let leaf = account(false, &[3, 0, 1], b'a', 0)[12..].to_vec();
        let updated = account(false, &lengths, b'k', 0)[12..].to_vec();
        let old_outer = pair(&leaf, &leaf);
        let new_outer = pair(&updated, &leaf);
        let old_root = pair(&old_outer, &old_outer);
        let new_root = pair(&new_outer, &old_outer);
        let old_used = 12 + old_root.len();
        let new_used = 12 + new_root.len();
        let mut bytes = b"rewrite8".to_vec();
        bytes.extend_from_slice(&(old_used as u32).to_le_bytes());
        bytes.extend_from_slice(&old_root);
        bytes.resize(bytes.len() + 32, 0xa5);
        let mut expected = bytes.clone();
        expected[..8].copy_from_slice(b"rewrite8");
        expected[8..12].copy_from_slice(&(new_used as u32).to_le_bytes());
        expected[12..new_used].copy_from_slice(&new_root);
        if new_used < old_used {
            expected[new_used..old_used].fill(0);
        }
        let mut path = InlinePath::new();
        path.push(Step::Index(0)).unwrap();
        path.push(Step::Index(0)).unwrap();
        path.push(Step::Field(0)).unwrap();
        let resolved = resolve_account_edit(&mut bytes, *b"rewrite8", 0, &ROOT, &path).unwrap();
        let (span, layout) = resolved.selection();
        let plan = RewritePlan::check(
            &resolved.inspect_resolved()[span.start as usize..span.end as usize],
            &layout,
            3,
            lengths.iter().sum::<usize>() as u32,
        )
        .unwrap();
        let mut write = resolved.prepare_subtree(plan.encoded_len()).unwrap();
        let mut stream = plan.begin(write.destination()).unwrap();
        for (i, &len) in lengths.iter().enumerate() {
            {
                stream.prepare_slot(len as u32).unwrap();
                stream.destination()
            }
            .fill(b'k' + i as u8);
            stream.advance(len).unwrap();
        }
        stream.finish().unwrap();
        write.finish();
        assert_eq!(bytes, expected);
        validate(&bytes[12..new_used], &ROOT, MAX_DEPTH).unwrap();
    }
}
