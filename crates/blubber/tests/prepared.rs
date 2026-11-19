//! Controls for prepared initialization, immediate plans, and retained typed editors.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "../../blubber-core/tests/prepared.rs"]
mod kernel;

use blubber::__private::{MeasuredInit, Projection};
#[cfg(not(feature = "verify-on-load"))]
use blubber::raw::Resize;
use blubber::Resize as _;
use blubber::{
    AccountSchema, ErrorKind, List, Map, ReadStorage, RootSchema, Schema, SliceStore, Str, View,
};
use core::cell::Cell;

thread_local! {
    static MEASUREMENTS: Cell<u32> = const { Cell::new(0) };
    static REGION_READS: Cell<u32> = const { Cell::new(0) };
}

struct CountedBool;

impl Schema for CountedBool {
    const KIND: blubber::Kind = <bool as Schema>::KIND;
    const FIXED_SIZE: Option<u32> = <bool as Schema>::FIXED_SIZE;
    const MIN_SIZE: u32 = <bool as Schema>::MIN_SIZE;
    const LAYOUT: &'static blubber_core::Layout = <bool as Schema>::LAYOUT;
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        <bool as Schema>::WIRE_DESCRIPTOR;
    const __LEN: usize = <bool as Schema>::__LEN;
    const __HEADER: &'static [u32] = <bool as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = bool;
    type Mut<'a> = ();
    type Init<'a> = Option<bool>;
    type Removed = bool;

    fn project(_: Projection<'_, Self>) {}

    fn read(view: View<'_>) -> blubber::Result<bool> {
        view.read_fixed::<bool>()
    }

    fn read_region(bytes: &[u8], layout: blubber_core::Layout, base: u32) -> blubber::Result<bool> {
        REGION_READS.with(|calls| calls.set(calls.get() + 1));
        bool::read_region(bytes, layout, base)
    }

    fn read_removed(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }

    fn __read_fixed(view: View<'_>) -> blubber::Result<Option<bool>> {
        Self::read(view).map(Some)
    }

    fn measure(init: &Option<bool>) -> blubber::Result<u32> {
        MEASUREMENTS.with(|calls| calls.set(calls.get() + 1));
        // Missing input deliberately reports a width below the schema minimum.
        Ok(u32::from(init.is_some()))
    }

    fn encode(init: &Option<bool>, destination: &mut [u8]) {
        destination[0] = u8::from(init.unwrap());
    }

    blubber::__private::with_alloc! {
        type Owned = Option<bool>;
        fn measure_owned(init: &Self::Owned) -> blubber::Result<u32> { Self::measure(init) }
        fn encode_owned(init: &Self::Owned, destination: &mut [u8]) { Self::encode(init, destination); }
        fn decode_owned(view: View<'_>) -> blubber::Result<Self::Owned> { Self::__read_fixed(view) }
    }
}

#[blubber::schema(account)]
struct CountedRoot {
    values: List<Option<CountedBool>>,
}

#[blubber::schema(account)]
struct MinimumRoot {
    value: CountedBool,
}

#[blubber::schema(account)]
struct PreparedRoot {
    counter: u64,
    flag: bool,
    text: Str,
    values: List<Str>,
}

#[test]
fn fixed_map_iterator_uses_region_hook_for_each_value() {
    let bytes = [2, 0, 0, 0, 1, 1, 3, 0];
    let map = View::new(&bytes, Map::<u8, CountedBool>::LAYOUT, 19)
        .unwrap()
        .read::<Map<u8, CountedBool>>()
        .unwrap();
    REGION_READS.with(|calls| calls.set(0));
    let mut iter = map.try_iter();
    assert_eq!(iter.next(), Some(Ok((1, true))));
    REGION_READS.with(|calls| assert_eq!(calls.get(), 1));
    assert_eq!(iter.next(), Some(Ok((3, false))));
    assert_eq!(iter.next(), None);
    REGION_READS.with(|calls| assert_eq!(calls.get(), 2));
}

#[blubber::schema(account)]
struct ArrayPrefixRoot {
    raw: [u8; 3],
    text: Str,
}

#[test]
fn account_prefix_copies_fixed_byte_array() {
    let mut bytes = [0; 64];
    ArrayPrefixRoot::initialize(
        &mut SliceStore::new(&mut bytes),
        ArrayPrefixRootInit {
            raw: [0, 128, 255],
            text: "tail",
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    let view = View::load::<ArrayPrefixRoot>(&bytes, ArrayPrefixRoot::DISCRIMINATOR).unwrap();
    assert_eq!(
        ArrayPrefixRoot::read_prefix(view).unwrap().raw,
        [0, 128, 255]
    );
}

#[test]
fn map_markers_each_require_audited_values() {
    use std::hint::black_box;

    assert!(black_box(Map::<u8, bool>::__ENCODER_AUDITED));
    assert!(black_box(Map::<u8, bool>::__MEASURE_REUSABLE));
    assert!(!black_box(Map::<u8, CountedBool>::__ENCODER_AUDITED));
    assert!(!black_box(Map::<u8, CountedBool>::__MEASURE_REUSABLE));
}

#[test]
fn prepared_initialization_replays_handwritten_measurement() {
    const { assert!(!CountedRoot::__MEASURE_REUSABLE) }
    MEASUREMENTS.with(|calls| calls.set(0));
    let mut bytes = [0xA5; 128];
    bytes[..8].fill(0);
    let initial = bytes;
    let mut store = SliceStore::new(&mut bytes);
    let measured = MeasuredInit::<CountedRoot>::new(CountedRootInit {
        values: &[Some(Some(true))],
    })
    .unwrap();
    measured.check_minimum().unwrap();
    assert_eq!(store.bytes(), initial);
    MEASUREMENTS.with(|calls| assert_eq!(calls.get(), 1));

    let receipt = Projection::<CountedRoot>::initialize_measured(&mut store, measured)
        .unwrap()
        .commit()
        .unwrap();
    // Preflight, root replay, struct child sizing, list child sizing.
    MEASUREMENTS.with(|calls| assert_eq!(calls.get(), 4));
    let view = CountedRoot::load(&bytes).unwrap();
    assert_eq!(view.values().try_get(0).unwrap(), Some(Some(true)));
    assert!(bytes[receipt.used_len as usize..]
        .iter()
        .all(|byte| *byte == 0));
}

#[test]
fn prepared_initialization_rejects_below_minimum_without_writes() {
    let mut bytes = [0xA5; 128];
    bytes[..8].fill(0);
    let initial = bytes;
    let mut store = SliceStore::new(&mut bytes);
    let measured = MeasuredInit::<MinimumRoot>::new(MinimumRootInit { value: None }).unwrap();
    let error = ErrorKind::InvalidBoundary.into();
    assert_eq!(measured.used(), Ok(12));
    assert_eq!(measured.check_minimum(), Err(error));
    assert_eq!(
        Projection::<MinimumRoot>::initialize_measured(&mut store, measured).err(),
        Some(error),
    );
    assert_eq!(store.bytes(), initial);
    assert!(!store.is_poisoned());
}

#[test]
fn prepared_initialization_matches_independent_bytes_at_and_above_minimum() {
    use blubber_model::{IntWidth::W8, Schema as Model, Value};

    const { assert!(PreparedRoot::__MEASURE_REUSABLE) }
    let model = Model::Struct(vec![
        Model::Unsigned(W8),
        Model::Bool,
        Model::Str,
        Model::List(Box::new(Model::Str)),
    ]);
    for (text, values) in [("", &[][..]), ("λabc", &["", "bc"][..])] {
        let expected = model
            .encode_account(
                PreparedRoot::DISCRIMINATOR,
                &Value::Struct(vec![
                    Value::Unsigned(7),
                    Value::Bool(true),
                    Value::Str(text.into()),
                    Value::List(
                        values
                            .iter()
                            .map(|value| Value::Str((*value).into()))
                            .collect(),
                    ),
                ]),
                256,
            )
            .unwrap();
        let used = u32::from_le_bytes(expected[8..12].try_into().unwrap());
        assert_eq!(used == 12 + PreparedRoot::MIN_SIZE, values.is_empty());
        let mut bytes = [0xA5; 256];
        bytes[..8].fill(0);
        let initial = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let measured = MeasuredInit::<PreparedRoot>::new(PreparedRootInit {
            counter: 7,
            flag: true,
            text,
            values,
        })
        .unwrap();
        assert_eq!(measured.used(), Ok(used));
        assert_eq!(measured.check_minimum(), Ok(()));
        assert_eq!(store.bytes(), initial);

        let receipt = Projection::<PreparedRoot>::initialize_measured(&mut store, measured)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(receipt.used_len, used);
        assert_eq!(receipt.storage_len, 256);
        assert_eq!(bytes.as_slice(), expected);
    }
}

#[blubber::schema(account)]
struct Root {
    text: Str,
    numbers: blubber::List<u16>,
    map: blubber::Map<u8, Str>,
}

#[test]
fn retained_typed_handles_resolve_current_bytes_once_per_operation() {
    let mut bytes = [0; 256];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = Root::initialize(
        &mut store,
        RootInit {
            text: "old",
            numbers: &[1, 2],
            map: &[(2, "old")],
        },
    )
    .unwrap();
    let mut text = root.text_mut();
    text.push_str(" 🦀").unwrap();
    text.truncate(3).unwrap();
    text.set("replaced").unwrap();
    let mut numbers = root.numbers_mut();
    numbers.push(3).unwrap();
    numbers.insert(0, 0).unwrap();
    numbers.extend_from_slice(&[]).unwrap();
    numbers.extend_from_slice(&[4, 5]).unwrap();
    numbers.truncate(usize::MAX).unwrap();
    let mut map = root.map_mut();
    map.entry(2)
        .unwrap()
        .or_insert("unused")
        .unwrap()
        .set("repaired")
        .unwrap();
    map.entry(3)
        .unwrap()
        .or_insert("three")
        .unwrap()
        .push_str("!")
        .unwrap();
    root.validate().unwrap();
    assert_eq!(root.text(), "replaced");
    assert_eq!(
        root.numbers().iter().collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(root.map().get(&3), Some("three!"));
    assert!(!store.is_poisoned());
    assert!(store.bytes()[12..].contains(&b'!'));
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn sticky_projection_fault_precedes_partition_checks_after_the_envelope() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    Root::initialize(
        &mut store,
        RootInit {
            text: "x",
            numbers: &[],
            map: &[],
        },
    )
    .unwrap();
    store.bytes_mut()[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = store.bytes().to_vec();
    let edit = blubber::raw::Edit::load_trusted::<Root>(&mut store, Root::DISCRIMINATOR).unwrap();
    let mut field = blubber::raw::project::<Str>(edit.field(u16::MAX));
    assert_eq!(
        field.set("replacement"),
        Err(blubber::ErrorKind::IndexOutOfBounds.into())
    );
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[test]
fn flat_string_loans_match_strict_replacement_including_spare_bytes() {
    use blubber_core::raw::{
        begin_apply, plan_replace, prepare_byte_replacement, InlinePath, Step,
    };
    use blubber_core::Layout;
    let mut bytes = [0; 256];
    let mut store = SliceStore::new(&mut bytes);
    Root::initialize(
        &mut store,
        RootInit {
            text: "old",
            numbers: &[1, 2],
            map: &[(1, "a"), (2, "middle"), (3, "last")],
        },
    )
    .unwrap();
    let initial = store.bytes().to_vec();
    let checked = Root::__layout();
    for steps in [
        vec![Step::Field(0)],
        vec![Step::Field(2), Step::Index(0)],
        vec![Step::Field(2), Step::Index(1)],
        vec![Step::Field(2), Step::Index(2)],
    ] {
        let mut path = InlinePath::new();
        for step in steps {
            path.push(step).unwrap();
        }
        for input in ["", "new", "a longer 🦀 replacement"] {
            let mut strict = initial.clone();
            let used = blubber_core::read_u32(&strict, 8).unwrap();
            let plan = plan_replace(
                &strict,
                used,
                &Layout::Flat { checked },
                &path,
                input.len() as u32,
                u32::MAX,
            )
            .unwrap();
            let mut apply = begin_apply(&mut strict, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(input.as_bytes());
            apply.finish();
            let mut compact = initial.clone();
            let mut write = prepare_byte_replacement(
                &mut compact,
                Root::DISCRIMINATOR,
                Root::MIN_SIZE,
                checked,
                &path,
                input.len(),
            )
            .unwrap();
            write.destination().copy_from_slice(input.as_bytes());
            write.finish();
            assert_eq!(compact, strict);
        }
    }
}

#[test]
fn flat_string_loan_failures_preserve_the_entire_initialized_buffer() {
    use blubber_core::raw::{prepare_byte_replacement, InlinePath, Step};
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    Root::initialize(
        &mut store,
        RootInit {
            text: "old",
            numbers: &[1],
            map: &[(2, "two")],
        },
    )
    .unwrap();
    let initial = store.bytes().to_vec();
    let checked = Root::__layout();
    let mut text = InlinePath::new();
    text.push(Step::Field(0)).unwrap();
    let mut wrong = InlinePath::new();
    wrong.push(Step::Field(1)).unwrap();
    let mut absent = InlinePath::new();
    absent.push(Step::Field(2)).unwrap();
    absent.push(Step::Index(1)).unwrap();
    for (path, input) in [(text, 1024), (wrong, 3), (absent, 3)] {
        let mut actual = initial.clone();
        assert!(prepare_byte_replacement(
            &mut actual,
            Root::DISCRIMINATOR,
            Root::MIN_SIZE,
            checked,
            &path,
            input
        )
        .is_err());
        assert_eq!(actual, initial);
    }
    for field in [0usize, 8, 12] {
        let mut actual = initial.clone();
        actual[field..field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = actual.clone();
        assert!(prepare_byte_replacement(
            &mut actual,
            Root::DISCRIMINATOR,
            Root::MIN_SIZE,
            checked,
            &text,
            4
        )
        .is_err());
        assert_eq!(actual, before);
    }
}
