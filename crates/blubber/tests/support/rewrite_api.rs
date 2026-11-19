//! Exact-count operations, independent bytes and invalid-input codec controls.
#![forbid(unsafe_code)]

use blubber::raw::Resize;
use blubber::Resize as _;
use blubber::{ErrorKind, List, Map, ReadStorage, Schema, SliceStore, Str};

#[blubber::schema(account)]
struct Batch {
    values: List<Str>,
    entries: Map<i16, Str>,
    tail: Str,
}

fn oracle(values: &[&str], entries: &[(i16, &str)]) -> Vec<u8> {
    use blubber_model::{IntWidth, Key, KeySchema, Schema as S, Value as V};
    let schema = S::Struct(vec![
        S::List(Box::new(S::Str)),
        S::Map(KeySchema::Signed(IntWidth::W2), Box::new(S::Str)),
        S::Str,
    ]);
    let value = V::Struct(vec![
        V::List(values.iter().map(|s| V::Str((*s).into())).collect()),
        V::Map(
            entries
                .iter()
                .map(|(k, v)| (Key::Signed(i128::from(*k)), V::Str((*v).into())))
                .collect(),
        ),
        V::Str("suffix 🦀".into()),
    ]);
    let used = 12 + schema.encode(&value).unwrap().len();
    schema
        .encode_account(Batch::DISCRIMINATOR, &value, used)
        .unwrap()
}

#[test]
fn mixed_rewrites_and_fills_match_independent_nested_account_bytes() {
    // Counts 0..4, all ordinal shrink/grow/empty positions, retained outer
    // suffix and ancestor ends; Miri runs this bounded deterministic history.
    for n in 0..=4 {
        let mut bytes = [0; 256];
        bytes[12..].fill(0xa5);
        let mut store = SliceStore::new(&mut bytes);
        let old = ["original"; 4];
        let pairs = [
            (-7, "original"),
            (-1, "original"),
            (0, "original"),
            (9, "original"),
        ];
        Batch::initialize(
            &mut store,
            BatchInit {
                values: &old[..n],
                entries: &pairs[..n],
                tail: "suffix 🦀",
            },
        )
        .unwrap();
        let mut values = vec!["original"; n];
        let mut entries = pairs[..n].to_vec();
        for round in 0..12 {
            let frame = store.bytes().to_vec();
            let old_used = blubber_core::read_u32(&frame, 8).unwrap() as usize;
            let inputs: Vec<_> = (0..n)
                .map(|i| ["", "é", "abcdefghijklmnopq", "x"][(i + round) % 4])
                .collect();
            let map_inputs: Vec<_> = entries
                .iter()
                .enumerate()
                .map(|(i, (key, _))| (*key, inputs[i]))
                .collect();
            let mut root = Batch::load_mut(&mut store).unwrap();
            if round % 3 == 0 {
                root.values_mut().fill("replacement grows").unwrap();
                root.entries_mut().fill_values("replacement grows").unwrap();
                values.fill("replacement grows");
                for (_, value) in &mut entries {
                    *value = "replacement grows";
                }
            } else {
                root.values_mut().rewrite_from(&inputs).unwrap();
                if round % 2 == 0 {
                    root.entries_mut().rewrite_values(&inputs).unwrap();
                } else {
                    root.entries_mut().rewrite_values_from(&map_inputs).unwrap();
                }
                values = inputs;
                entries = map_inputs;
            }
            root.validate().unwrap();
            let used = root.used_len() as usize;
            assert_eq!(root.tail(), "suffix 🦀");
            let _ = root;
            assert_eq!(&store.bytes()[..used], oracle(&values, &entries));
            for (i, byte) in store.bytes().iter().enumerate().skip(used) {
                assert_eq!(*byte, if i < old_used { 0 } else { frame[i] });
            }
        }
    }
}

#[test]
fn count_key_and_capacity_errors_preserve_full_buffer() {
    let mut bytes = [0; 128];
    bytes[12..].fill(0xa5);
    let mut store = SliceStore::new(&mut bytes);
    Batch::initialize(
        &mut store,
        BatchInit {
            values: &["a", "b"],
            entries: &[(-2, "a"), (4, "b")],
            tail: "suffix 🦀",
        },
    )
    .unwrap();
    let before = store.bytes().to_vec();
    for pairs in [
        &[(-2, "new")][..],
        &[(-2, "new"), (3, "new")][..],
        &[(4, "new"), (-2, "new")][..],
        &[(-2, "new"), (-2, "new")][..],
    ] {
        let mut root = Batch::load_mut(&mut store).unwrap();
        assert_eq!(
            root.entries_mut().rewrite_values_from(pairs),
            Err(ErrorKind::InvalidBoundary.into())
        );
        assert_eq!(
            root.values_mut().rewrite_from(&["new"]),
            Err(ErrorKind::InvalidBoundary.into())
        );
        let _ = root;
        assert_eq!(store.bytes(), before);
    }
    for inputs in [&[][..], &["new"][..], &["new", "new", "new"][..]] {
        let mut root = Batch::load_mut(&mut store).unwrap();
        assert_eq!(
            root.entries_mut().rewrite_values(inputs),
            Err(ErrorKind::InvalidBoundary.into())
        );
        let _ = root;
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
    let huge = "x".repeat(128);
    let mut root = Batch::load_mut(&mut store).unwrap();
    assert_eq!(
        root.values_mut().fill(&huge),
        Err(ErrorKind::Capacity.into())
    );
    assert_eq!(
        root.entries_mut().fill_values(&huge),
        Err(ErrorKind::Capacity.into())
    );
    let _ = root;
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[blubber::schema(account)]
struct FixedBatch {
    values: List<u16>,
    entries: Map<i16, u16>,
}

#[test]
fn fixed_lists_and_maps_keep_counts_and_keys() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = FixedBatch::initialize(
        &mut store,
        FixedBatchInit {
            values: &[1, 2, 3],
            entries: &[(-1, 2), (3, 4)],
        },
    )
    .unwrap();
    root.values_mut().fill(42).unwrap();
    root.entries_mut().fill_values(99).unwrap();
    assert_eq!(root.values().iter().collect::<Vec<_>>(), [42, 42, 42]);
    assert_eq!(
        root.entries().iter().collect::<Vec<_>>(),
        [(-1, 99), (3, 99)]
    );
    root.values_mut().rewrite_from(&[9, 8, 7]).unwrap();
    root.entries_mut()
        .rewrite_values_from(&[(-1, 11), (3, 12)])
        .unwrap();
    root.entries_mut().rewrite_values(&[13, 14]).unwrap();
    assert_eq!(root.values().iter().collect::<Vec<_>>(), [9, 8, 7]);
    assert_eq!(
        root.entries().iter().collect::<Vec<_>>(),
        [(-1, 13), (3, 14)]
    );
    root.validate().unwrap();
}

#[test]
fn zero_width_and_recursive_derived_inputs_keep_their_partitions() {
    type Empty = Str;
    #[blubber::schema(account)]
    struct Zeros {
        values: List<Empty>,
        entries: Map<u8, Empty>,
        tail: Str,
    }
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = Zeros::initialize(
        &mut store,
        ZerosInit {
            values: &["", ""],
            entries: &[(1, "")],
            tail: "tail",
        },
    )
    .unwrap();
    root.values_mut().fill("").unwrap();
    root.values_mut().rewrite_from(&["", ""]).unwrap();
    root.entries_mut().fill_values("").unwrap();
    root.entries_mut().rewrite_values_from(&[(1, "")]).unwrap();
    root.entries_mut().rewrite_values(&[""]).unwrap();
    assert_eq!(root.values().len(), 2);
    assert_eq!(root.entries().len(), 1);
    assert_eq!(root.tail(), "tail");
    root.validate().unwrap();
}

struct Codec;
struct Input {
    measures: core::cell::Cell<usize>,
    encodes: core::cell::Cell<usize>,
    fault: u8,
}
impl Input {
    fn new(fault: u8) -> Self {
        Self {
            measures: 0.into(),
            encodes: 0.into(),
            fault,
        }
    }
}
impl Schema for Codec {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:rewrite:codec",
                epoch: 0,
            },
            wire: <Str as Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = Str::KIND;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = Str::LAYOUT;
    const __LEN: usize = Str::__LEN;
    const __HEADER: &'static [u32] = Str::__HEADER;
    type __Children = <Str as Schema>::__Children;
    type Init<'a> = &'a Input;
    type Read<'a> = &'a str;
    type Mut<'a> = blubber::StrMut<'a>;
    type Removed = ();
    fn read<'a>(v: blubber::View<'a>) -> blubber::Result<&'a str> {
        Str::read(v)
    }
    fn project<'a>(e: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::project::<Str>(blubber::raw::into_edit(e))
    }
    fn read_removed(v: blubber::View<'_>) -> blubber::Result<()> {
        Str::read_removed(v)
    }
    fn measure(i: &Self::Init<'_>) -> blubber::Result<u32> {
        let call = i.measures.get();
        i.measures.set(call + 1);
        match i.fault {
            1 => Err(ErrorKind::InvalidBoundary.into()),
            2 => Ok(u32::MAX),
            3 if call > 0 => Err(ErrorKind::InvalidBoundary.into()),
            4 if call > 0 => Ok(0),
            5 if call > 0 => Ok(3),
            _ => Ok(1),
        }
    }
    fn encode(_: &Self::Init<'_>, d: &mut [u8]) {
        d.fill(b'x');
    }
    fn encode_slot(i: &Self::Init<'_>, d: &mut [u8]) -> blubber::Result<usize> {
        i.encodes.set(i.encodes.get() + 1);
        Self::encode(i, d);
        match i.fault {
            6 => Err(ErrorKind::InvalidBoundary.into()),
            7 => Ok(0),
            8 => Ok(d.len() + 1),
            9 => panic!("typed codec unwind"),
            _ => Ok(d.len()),
        }
    }
    #[cfg(feature = "alloc")]
    type Owned = String;
    #[cfg(feature = "alloc")]
    fn measure_owned(v: &String) -> blubber::Result<u32> {
        Str::measure_owned(v)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(v: &String, d: &mut [u8]) {
        Str::encode_owned(v, d);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(v: blubber::View<'_>) -> blubber::Result<String> {
        Str::decode_owned(v)
    }
}

#[blubber::schema(account)]
struct FaultBatch {
    values: List<Codec>,
    entries: Map<u8, Codec>,
}

#[test]
fn counters_empty_preflight_second_pass_and_codec_poison() {
    for n in 0..=4 {
        for fault in 0..=9 {
            for mode in 0..3 {
                let map = mode != 0;
                for fill in [false, true] {
                    let good = Input::new(0);
                    let initial = [&good; 4];
                    let initial_map = [(1, &good), (2, &good), (3, &good), (4, &good)];
                    let mut bytes = [0; 128];
                    bytes[12..].fill(0xa5);
                    let mut store = SliceStore::new(&mut bytes);
                    FaultBatch::initialize(
                        &mut store,
                        FaultBatchInit {
                            values: &initial[..n],
                            entries: &initial_map[..n],
                        },
                    )
                    .unwrap();
                    let before = store.bytes().to_vec();
                    let input = Input::new(fault);
                    let inputs = [&input; 4];
                    let pairs = [(1, &input), (2, &input), (3, &input), (4, &input)];
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut root = FaultBatch::load_mut(&mut store).unwrap();
                        let result = if map {
                            if fill {
                                root.entries_mut().fill_values(&input)
                            } else if mode == 2 {
                                root.entries_mut().rewrite_values(&inputs[..n])
                            } else {
                                root.entries_mut().rewrite_values_from(&pairs[..n])
                            }
                        } else if fill {
                            root.values_mut().fill(&input)
                        } else {
                            root.values_mut().rewrite_from(&inputs[..n])
                        };
                        if result.is_ok() {
                            root.validate().unwrap();
                        }
                        result
                    }));
                    let preflight =
                        n > 0 && (fault == 1 || fault == 2 || (!fill && n > 1 && fault == 3));
                    let success = n == 0 || fault == 0 || (fill && (3..=5).contains(&fault));
                    if success {
                        result.unwrap().unwrap();
                        assert!(!store.is_poisoned());
                        assert_eq!(
                            input.measures.get(),
                            if n == 0 {
                                0
                            } else if fill {
                                1
                            } else {
                                2 * n
                            }
                        );
                        assert_eq!(input.encodes.get(), n);
                        if n == 0 {
                            assert_eq!(store.bytes(), before);
                        }
                    } else if preflight {
                        assert!(result.unwrap().is_err());
                        assert_eq!(store.bytes(), before);
                        assert!(!store.is_poisoned());
                        assert_eq!(input.encodes.get(), 0);
                    } else {
                        assert!(result.is_err() || result.unwrap().is_err());
                        assert!(store.is_poisoned());

                        // State precedence must win over malformed inputs.
                        assert!(
                            matches!(FaultBatch::load_trusted_mut(&mut store),Err(e) if e==ErrorKind::Poisoned.into())
                        );
                        assert_eq!(store.commit(32), Err(ErrorKind::Poisoned.into()));
                    }
                }
            }
        }
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn trusted_fill_repairs_leaves_but_checks_bad_directories_and_keys() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    Batch::initialize(
        &mut store,
        BatchInit {
            values: &["a", "b"],
            entries: &[(-2, "a"), (4, "b")],
            tail: "suffix 🦀",
        },
    )
    .unwrap();
    let view = blubber::View::load_trusted::<Batch>(store.bytes(), Batch::DISCRIMINATOR).unwrap();
    let offset = view.field(0).unwrap().base() as usize;
    store.bytes_mut()[offset + 8] = 0xff;
    let mut root = Batch::load_trusted_mut(&mut store).unwrap();
    root.values_mut().fill("valid").unwrap();
    root.validate().unwrap();
    let _ = root;
    store.bytes_mut()[offset + 4..offset + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = store.bytes().to_vec();
    let mut root = Batch::load_trusted_mut(&mut store).unwrap();
    assert!(root.values_mut().fill("x").is_err());
    let _ = root;
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
    // Restore list geometry, then independently reject duplicate and descending
    // keys before touching any map value, including trusted ingress.
    store.bytes_mut()[offset + 4..offset + 8].copy_from_slice(&5u32.to_le_bytes());
    Batch::load_checked_mut(&mut store).unwrap();
    let map = blubber::View::load_trusted::<Batch>(store.bytes(), Batch::DISCRIMINATOR)
        .unwrap()
        .field(1)
        .unwrap()
        .base() as usize;
    let first: [u8; 2] = store.bytes()[map + 4..map + 6].try_into().unwrap();
    let original_key: [u8; 2] = store.bytes()[map + 10..map + 12].try_into().unwrap();
    for (key, kind) in [
        (first, ErrorKind::DuplicateKey),
        ([0, 0], ErrorKind::UnsortedKeys),
    ] {
        store.bytes_mut()[map + 10..map + 12].copy_from_slice(&key);
        let before = store.bytes().to_vec();
        let mut root = Batch::load_trusted_mut(&mut store).unwrap();
        assert!(
            matches!(root.entries_mut().fill_values("repair"),Err(blubber::Error::Fault {kind:k,..}) if k==kind)
        );
        let _ = root;
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
        store.bytes_mut()[map + 10..map + 12].copy_from_slice(&original_key);
        Batch::load_checked_mut(&mut store).unwrap();
    }
}

#[cfg(feature = "alloc")]
#[test]
fn vec_relocation_and_nested_collection_rewrites_reacquire_coordinates() {
    #[blubber::schema(account)]
    struct Nested {
        values: List<List<Str>>,
        tail: Str,
    }
    let mut buffer = vec![0; 32];
    let mut store = blubber::VecStore::new(&mut buffer);
    let mut root = Nested::initialize(
        &mut store,
        NestedInit {
            values: &[&["a", "b"][..], &["c", "d"][..]],
            tail: "tail",
        },
    )
    .unwrap();
    let text = "x".repeat(1024);
    {
        let mut cursor = root.values_mut().cursor_mut().unwrap();
        {
            let mut child = cursor.next().unwrap().unwrap();
            child.fill(&text).unwrap();
            child.rewrite_from(&["", "é"]).unwrap();
        }
        cursor
            .next()
            .unwrap()
            .unwrap()
            .fill("second grows")
            .unwrap();
        assert!(cursor.next().unwrap().is_none());
    }
    root.values_mut().fill(&["nested", "values"][..]).unwrap();
    assert_eq!(
        root.values().get(0).unwrap().iter().collect::<Vec<_>>(),
        ["nested", "values"]
    );
    assert_eq!(root.tail(), "tail");
    root.validate().unwrap();
}

#[test]
fn keyless_count_rejects_before_measurement() {
    let good = Input::new(0);
    let bad = Input::new(1);
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    FaultBatch::initialize(
        &mut store,
        FaultBatchInit {
            values: &[],
            entries: &[(1, &good), (3, &good)],
        },
    )
    .unwrap();
    let before = store.bytes().to_vec();
    let mut root = FaultBatch::load_mut(&mut store).unwrap();
    assert_eq!(
        root.entries_mut().rewrite_values(&[&bad]),
        Err(ErrorKind::InvalidBoundary.into())
    );
    assert_eq!(bad.measures.get(), 0);
    assert_eq!(bad.encodes.get(), 0);
    let _ = root;
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[path = "keyless_differential.rs"]
mod keyless_differential;

#[test]
fn keyless_and_keyed_histories_have_identical_bytes_and_errors() {
    for seed in 0..if cfg!(miri) { 4u8 } else { 128u8 } {
        let data: Vec<u8> = (0..64u8)
            .map(|i| seed.wrapping_mul(37).wrapping_add(i.wrapping_mul(13)))
            .collect();
        keyless_differential::check(&data);
    }
}
