//! Lazy source faults poison the session before returning.
#![forbid(unsafe_code)]
#![cfg(all(feature = "derive", not(feature = "verify-on-load")))]
use blubber::{ErrorKind, List, Map, ReadStorage, Resize, SliceStore, Str};
#[blubber::schema(account)]
struct State {
    fixed: List<u16>,
    words: List<Str>,
    pairs: Map<u8, Str>,
    values: Map<u8, bool>,
    tail: Str,
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0; 320];
    State::initialize(
        &mut SliceStore::new(&mut b),
        StateInit {
            fixed: &[2, 4, 6],
            words: &["abc", "def", "ghi", "jkl"],
            pairs: &[(1, "one"), (3, "tri"), (5, "five"), (7, "seven")],
            values: &[(1, true), (3, false), (5, true)],
            tail: "last",
        },
    )
    .unwrap();
    b
}
#[derive(Clone, Copy, Debug)]
enum Operation {
    ListInsert,
    ListRemove,
    ListTruncate,
    ListExtend,
    ListFill,
    ListWithMut,
    ListWithMutGrowShrink,
    MapInsert,
    MapRemove,
    MapFillValues,
    MapForEachMut,
    FixedListInsert,
    FixedListRemove,
    FixedListFill,
    FixedMapRemove,
    FixedMapFillValues,
    StringSet,
    ListClear,
    MapClear,
    ListAtMut,
    MapGetMut,
    MapEntry,
    ListWithReadOnlyCallback,
}
impl Operation {
    const ALL: [Self; 23] = [
        Self::ListInsert,
        Self::ListRemove,
        Self::ListTruncate,
        Self::ListExtend,
        Self::ListFill,
        Self::ListWithMut,
        Self::ListWithMutGrowShrink,
        Self::MapInsert,
        Self::MapRemove,
        Self::MapFillValues,
        Self::MapForEachMut,
        Self::FixedListInsert,
        Self::FixedListRemove,
        Self::FixedListFill,
        Self::FixedMapRemove,
        Self::FixedMapFillValues,
        Self::StringSet,
        Self::ListClear,
        Self::MapClear,
        Self::ListAtMut,
        Self::MapGetMut,
        Self::MapEntry,
        Self::ListWithReadOnlyCallback,
    ];
}
fn operation(b: &mut [u8], op: Operation) -> (blubber::Result<()>, bool) {
    let mut store = SliceStore::new(b);
    let r = State::load_trusted_mut(&mut store).and_then(|mut s| match op {
        Operation::ListInsert => s.words_mut().insert(1, "longer"),
        Operation::ListRemove => s.words_mut().remove(1).map(|_| ()),
        Operation::ListTruncate => s.words_mut().truncate(2),
        Operation::ListExtend => s.words_mut().extend_from_slice(&["a", "longer"]),
        Operation::ListFill => s.words_mut().fill("longer"),
        Operation::ListWithMut => s.words_mut().with_mut(1, |x| x.set("new")).map(|_| ()),
        Operation::ListWithMutGrowShrink => s
            .words_mut()
            .with_mut(1, |x| {
                x.set("replacement grows")?;
                x.set("z")
            })
            .map(|_| ()),
        Operation::MapInsert => s.pairs_mut().insert(4, "four").map(|_| ()),
        Operation::MapRemove => s.pairs_mut().remove(&3).map(|_| ()),
        Operation::MapFillValues => s.pairs_mut().fill_values("longer"),
        Operation::MapForEachMut => s.pairs_mut().try_for_each_mut(|_, x| x.set("new")),
        Operation::FixedListInsert => s.fixed_mut().insert(1, 9),
        Operation::FixedListRemove => s.fixed_mut().remove(1).map(|_| ()),
        Operation::FixedListFill => s.fixed_mut().fill(99),
        Operation::FixedMapRemove => s.values_mut().remove(&3).map(|_| ()),
        Operation::FixedMapFillValues => s.values_mut().fill_values(true),
        Operation::StringSet => s.tail_mut().set("replacement grows"),
        Operation::ListClear => s.words_mut().clear(),
        Operation::MapClear => s.pairs_mut().clear(),
        Operation::ListAtMut => s.words_mut().at_mut(1).map(|_| ()),
        Operation::MapGetMut => s.pairs_mut().get_mut(&3).map(|_| ()),
        Operation::MapEntry => s.pairs_mut().into_entry(3).map(|_| ()),
        Operation::ListWithReadOnlyCallback => s.words_mut().with_mut(1, |_| Ok(())).map(|_| ()),
    });
    let poisoned = store.is_poisoned();
    if poisoned {
        assert_eq!(
            State::load_trusted_mut(&mut store).err(),
            Some(ErrorKind::Poisoned.into())
        );
    }
    (r, poisoned)
}
fn assert_mutation(original: &[u8], at: usize, value: u8, op: Operation) -> bool {
    let mut bytes = vec![0xa5; 336];
    bytes[8..328].copy_from_slice(original);
    bytes[8 + at] = value;
    let before = bytes.clone();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        operation(&mut bytes[8..328], op)
    }));
    assert!(caught.is_ok(), "unwind at={at} value={value} op={op:?}");
    let (result, poisoned) = caught.unwrap();
    assert_eq!(&bytes[..8], &before[..8]);
    assert_eq!(&bytes[328..], &before[328..]);
    if result.is_err() && bytes != before {
        assert!(
            poisoned,
            "destructive error lacks poison at={at} value={value} op={op:?} result={result:?}"
        );
    }
    if matches!(
        result,
        Err(blubber::Error::Fault {
            kind: ErrorKind::Capacity
                | ErrorKind::GrowthLimit
                | ErrorKind::RentShortfall
                | ErrorKind::IndexOutOfBounds,
            ..
        })
    ) {
        assert_eq!(
            bytes, before,
            "caller error wrote at={at} value={value} op={op:?}"
        );
        assert!(
            !poisoned,
            "caller error poisoned at={at} value={value} op={op:?}"
        );
    }
    let source_fault = matches!(
        result,
        Err(blubber::Error::Fault {
            kind: ErrorKind::Truncated
                | ErrorKind::UsedLength
                | ErrorKind::ArithmeticOverflow
                | ErrorKind::InvalidBoundary
                | ErrorKind::InvalidBool
                | ErrorKind::InvalidUtf8
                | ErrorKind::InvalidTag
                | ErrorKind::UnsortedKeys
                | ErrorKind::DuplicateKey,
            ..
        })
    );
    // Envelope admission keeps its checked-ingress frame; payload faults arise in the lazy session.
    if at >= 12 && source_fault {
        assert!(
            poisoned,
            "source error lacks poison at={at} value={value} op={op:?} result={result:?}"
        );
    }
    source_fault
}

const MUTATION_BYTES: [u8; 10] = [0, 1, 2, 3, 4, 15, 127, 128, 254, 255];

#[test]
fn invalid_byte_sweep_contains_writes_and_never_unwinds() {
    let original = fixture();
    let used = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    let mut cases = 0;
    for op in Operation::ALL {
        let mut faults = 0;
        for at in 12..used {
            for value in MUTATION_BYTES {
                faults += usize::from(assert_mutation(&original, at, value, op));
                cases += 1;
            }
        }
        assert!(faults > 0, "{op:?} must detect source faults");
    }
    eprintln!("payload_sweep_cases={cases}");
}

#[test]
fn envelope_byte_sweep_preserves_rejected_ingress() {
    let original = fixture();
    for at in 8..12 {
        for value in MUTATION_BYTES {
            for op in Operation::ALL {
                assert_mutation(&original, at, value, op);
            }
        }
    }
}
#[blubber::schema(account)]
struct TailFlags {
    value: List<bool>,
}
#[blubber::schema(account)]
struct TailFlagMap {
    value: Map<u8, bool>,
}
#[test]
fn lazy_tail_source_fault_must_poison() {
    let mut states = Vec::new();
    for map in [false, true] {
        let mut b = [0; 40];
        if map {
            TailFlagMap::initialize(
                &mut SliceStore::new(&mut b),
                TailFlagMapInit {
                    value: &[(1, true)],
                },
            )
            .unwrap();
            b[17] = 2;
        } else {
            TailFlags::initialize(
                &mut SliceStore::new(&mut b),
                TailFlagsInit { value: &[true] },
            )
            .unwrap();
            b[16] = 2;
        }
        let before = b;
        let mut store = SliceStore::new(&mut b);
        let r = if map {
            TailFlagMap::load_trusted_mut(&mut store)
                .unwrap()
                .value_mut()
                .remove(&1)
                .map(|_| ())
        } else {
            TailFlags::load_trusted_mut(&mut store)
                .unwrap()
                .value_mut()
                .remove(0)
                .map(|_| ())
        };
        assert_eq!(
            r,
            Err(blubber::Error::new(
                ErrorKind::InvalidBool,
                if map { 17 } else { 16 }
            ))
        );
        assert_eq!(store.bytes(), before);
        let reopen = if map {
            TailFlagMap::load_trusted_mut(&mut store).err()
        } else {
            TailFlags::load_trusted_mut(&mut store).err()
        };
        eprintln!(
            "map={map} error={r:?} poison={} reopen_error={reopen:?}",
            store.is_poisoned()
        );
        states.push(store.is_poisoned());
    }
    assert!(
        states.iter().all(|p| *p),
        "detected lazy source fault left session reusable: {states:?}"
    );
}

fn assert_source_poison(op: Operation, at: usize, byte: u8, kind: ErrorKind, offset: u32) {
    let mut b = fixture();
    b[at] = byte;
    let before = b.clone();
    let (r, poisoned) = operation(&mut b, op);
    eprintln!("op={op:?} offset={at} byte={byte} error={r:?} poison={poisoned}");
    assert_eq!(r, Err(blubber::Error::new(kind, offset)), "{op:?}");
    assert_eq!(b, before);
    assert!(poisoned, "detected source fault left lazy session reusable");
}
#[test]
fn list_fill_source_poison() {
    assert_source_poison(Operation::ListFill, 12, 0, ErrorKind::InvalidBoundary, 32);
}
#[test]
fn map_fill_source_poison() {
    assert_source_poison(
        Operation::MapFillValues,
        16,
        0,
        ErrorKind::InvalidBoundary,
        43,
    );
}
#[test]
fn fixed_list_insert_source_poison() {
    assert_source_poison(
        Operation::FixedListInsert,
        12,
        127,
        ErrorKind::InvalidBoundary,
        blubber::Error::NO_OFFSET,
    );
}
#[test]
fn fixed_list_remove_source_poison() {
    assert_source_poison(
        Operation::FixedListRemove,
        12,
        127,
        ErrorKind::InvalidBoundary,
        blubber::Error::NO_OFFSET,
    );
}
#[test]
fn fixed_list_fill_source_poison() {
    assert_source_poison(Operation::FixedListFill, 12, 0, ErrorKind::Truncated, 28);
}
#[test]
fn fixed_map_remove_source_poison() {
    assert_source_poison(
        Operation::FixedMapRemove,
        20,
        127,
        ErrorKind::InvalidBoundary,
        blubber::Error::NO_OFFSET,
    );
}
#[test]
fn fixed_map_values_fill_source_poison() {
    assert_source_poison(
        Operation::FixedMapFillValues,
        20,
        0,
        ErrorKind::InvalidBoundary,
        38,
    );
}
#[test]
fn direct_string_set_source_poison() {
    assert_source_poison(
        Operation::StringSet,
        24,
        127,
        ErrorKind::InvalidBoundary,
        blubber::Error::NO_OFFSET,
    );
}

struct InputFault;
impl blubber::Schema for InputFault {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        <u8 as blubber::Schema>::WIRE_DESCRIPTOR;
    const KIND: blubber::Kind = <u8 as blubber::Schema>::KIND;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as blubber::Schema>::LAYOUT;
    const __LEN: usize = <u8 as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as blubber::Schema>::__HEADER;
    type __Children = <u8 as blubber::Schema>::__Children;
    type Read<'a> = u8;
    type Mut<'a> = blubber::ScalarMut<'a, u8>;
    type Init<'a> = bool;
    type Removed = u8;
    fn read(v: blubber::View<'_>) -> blubber::Result<u8> {
        v.read::<u8>()
    }
    fn project(p: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::project::<u8>(blubber::raw::into_edit(p))
    }
    fn read_removed(v: blubber::View<'_>) -> blubber::Result<u8> {
        v.read::<u8>()
    }
    fn measure(fail: &bool) -> blubber::Result<u32> {
        if *fail {
            Err(blubber::Error::new(ErrorKind::InvalidBoundary, 17))
        } else {
            Ok(1)
        }
    }
    fn encode(_: &bool, slot: &mut [u8]) {
        slot[0] = 42;
    }
    #[cfg(feature = "alloc")]
    type Owned = u8;
    #[cfg(feature = "alloc")]
    fn decode_owned(v: blubber::View<'_>) -> blubber::Result<u8> {
        v.read::<u8>()
    }
    #[cfg(feature = "alloc")]
    fn measure_owned(_: &u8) -> blubber::Result<u32> {
        Ok(1)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &u8, slot: &mut [u8]) {
        slot[0] = *value;
    }
}
#[blubber::schema(account, discriminator = [8,8,8,8,8,8,8,8])]
struct Inputs {
    list: List<InputFault>,
    map: Map<u8, InputFault>,
    option: Option<InputFault>,
}
#[test]
fn caller_codec_fault_with_offset_preserves_bytes_and_poison() {
    #[derive(Clone, Copy, Debug)]
    enum CallerOperation {
        ListFill,
        MapFillValues,
        ListRewrite,
        MapRewriteValues,
        OptionSet,
        ListInsert,
        MapInsert,
        MapEntryOrInsert,
        ListReplace,
    }
    use CallerOperation::*;
    for operation in [
        ListFill,
        MapFillValues,
        ListRewrite,
        MapRewriteValues,
        OptionSet,
        ListInsert,
        MapInsert,
        MapEntryOrInsert,
        ListReplace,
    ] {
        let mut bytes = [0; 96];
        let mut store = SliceStore::new(&mut bytes);
        Inputs::initialize(
            &mut store,
            InputsInit {
                list: &[false],
                map: &[(1, false)],
                option: Some(false),
            },
        )
        .unwrap();
        let before = store.bytes().to_vec();
        let mut root = Inputs::load_trusted_mut(&mut store).unwrap();
        let result = match operation {
            ListFill => root.list_mut().fill(true),
            MapFillValues => root.map_mut().fill_values(true),
            ListRewrite => root.list_mut().rewrite_from(&[true]),
            MapRewriteValues => root.map_mut().rewrite_values(&[true]),
            OptionSet => root.option_mut().set(Some(true)),
            ListInsert => root.list_mut().insert(0, true),
            MapInsert => root.map_mut().insert(2, true).map(|_| ()),
            MapEntryOrInsert => root
                .map_mut()
                .into_entry(2)
                .unwrap()
                .or_insert(true)
                .map(|_| ()),
            ListReplace => root.list_mut().replace(0, true),
        };
        assert_eq!(
            result,
            Err(blubber::Error::new(ErrorKind::InvalidBoundary, 17)),
            "{operation:?}"
        );
        let _ = root;
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
}

#[blubber::schema(account)]
struct CountWords {
    values: List<u64>,
}
#[test]
fn lazy_count_overflow_reports_the_count_word_and_poisons() {
    #[derive(Clone, Copy, Debug)]
    enum ListOperation {
        ForEachMut,
        AtMut,
        Insert,
        Remove,
        Fill,
        Push,
    }
    use ListOperation::*;
    for operation in [ForEachMut, AtMut, Insert, Remove, Fill, Push] {
        let mut bytes = [0; 64];
        CountWords::initialize(
            &mut SliceStore::new(&mut bytes),
            CountWordsInit { values: &[1] },
        )
        .unwrap();
        bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let mut root = CountWords::load_trusted_mut(&mut store).unwrap();
        let result = match operation {
            ForEachMut => root.values_mut().try_for_each_mut(|_| Ok(())),
            AtMut => root.values_mut().at_mut(0).map(|_| ()),
            Insert => root.values_mut().insert(0, 7),
            Remove => root.values_mut().remove(0).map(|_| ()),
            Fill => root.values_mut().fill(7),
            Push => root.values_mut().push(7),
        };
        assert_eq!(
            result,
            Err(blubber::Error::new(ErrorKind::ArithmeticOverflow, 12)),
            "operation={operation:?}"
        );
        let _ = root;
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[blubber::schema(account)]
struct BoolKeys {
    values: Map<bool, u8>,
}
#[test]
fn fixed_map_visited_key_fault_poisons_before_callback() {
    for cursor in [false, true] {
        let mut bytes = [0; 64];
        BoolKeys::initialize(
            &mut SliceStore::new(&mut bytes),
            BoolKeysInit {
                values: &[(false, 1), (true, 2)],
            },
        )
        .unwrap();
        bytes[16] = 2;
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let mut root = BoolKeys::load_trusted_mut(&mut store).unwrap();
        let result = if cursor {
            root.values_mut().cursor_mut().unwrap().next().map(|_| ())
        } else {
            root.values_mut()
                .try_for_each_mut(|_, _| panic!("invalid key reached callback"))
        };
        assert_eq!(result, Err(blubber::Error::new(ErrorKind::InvalidBool, 16)));
        let _ = root;
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn lazy_map_count_overflow_reports_the_count_word_and_poisons() {
    #[derive(Clone, Copy, Debug)]
    enum MapOperation {
        Entry,
        GetMut,
        Insert,
        Remove,
        FillValues,
    }
    use MapOperation::*;
    for operation in [Entry, GetMut, Insert, Remove, FillValues] {
        let mut bytes = [0; 64];
        BoolKeys::initialize(
            &mut SliceStore::new(&mut bytes),
            BoolKeysInit {
                values: &[(false, 1), (true, 2)],
            },
        )
        .unwrap();
        bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let mut root = BoolKeys::load_trusted_mut(&mut store).unwrap();
        let result = match operation {
            Entry => root.values_mut().into_entry(false).map(|_| ()),
            GetMut => root.values_mut().get_mut(&false).map(|_| ()),
            Insert => root.values_mut().insert(false, 7).map(|_| ()),
            Remove => root.values_mut().remove(&false).map(|_| ()),
            FillValues => root.values_mut().fill_values(7),
        };
        assert_eq!(
            result,
            Err(blubber::Error::new(ErrorKind::ArithmeticOverflow, 12)),
            "operation={operation:?}"
        );
        let _ = root;
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn string_reading_callbacks_poison_invalid_utf8() {
    for indexed in [false, true] {
        for truncate in [false, true] {
            let mut bytes = fixture();
            let at = if indexed {
                bytes.windows(3).position(|part| part == b"abc").unwrap()
            } else {
                u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize - 1
            };
            bytes[at] = 0xff;
            let before = bytes.clone();
            let mut store = SliceStore::new(&mut bytes);
            let result = {
                let mut edit = State::load_trusted_mut(&mut store).unwrap();
                let mutate = |value: &mut blubber::StrMut<'_>| {
                    if truncate {
                        value.truncate(0)
                    } else {
                        value.push_str("more")
                    }
                };
                if indexed {
                    edit.words_mut().with_mut(0, mutate).map(|_| ())
                } else {
                    mutate(&mut edit.tail_mut())
                }
            };
            assert!(
                matches!(
                    result,
                    Err(blubber::Error::Fault {
                        kind: ErrorKind::InvalidUtf8,
                        ..
                    })
                ),
                "{result:?}"
            );
            assert!(store.is_poisoned());
            assert_eq!(store.bytes(), before);
            assert_eq!(
                State::load_trusted_mut(&mut store).err(),
                Some(ErrorKind::Poisoned.into())
            );
        }
    }
}

#[blubber::schema(account)]
struct SourceOption {
    value: Option<bool>,
}

#[test]
fn lazy_option_selection_fault_poisons_before_writes() {
    let mut bytes = [0; 24];
    SourceOption::initialize(
        &mut SliceStore::new(&mut bytes),
        SourceOptionInit { value: Some(true) },
    )
    .unwrap();
    bytes[12] = 2;
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    let result = SourceOption::load_trusted_mut(&mut store)
        .unwrap()
        .value_mut()
        .get_mut()
        .map(|_| ());
    assert_eq!(result, Err(blubber::Error::new(ErrorKind::InvalidTag, 12)));
    assert!(store.is_poisoned());
    assert_eq!(store.bytes(), before);
    assert_eq!(
        SourceOption::load_trusted_mut(&mut store).err(),
        Some(ErrorKind::Poisoned.into())
    );
}
