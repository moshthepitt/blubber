//! Shared, replayable fuzz checks. The model never imports production codecs.
//!
//! Inputs select one of the fixed fixture schemas. Validation checks an exact
//! region and an envelope. Edits select arbitrary initial bytes or a valid tree
//! rebuilt by the independent model, then consume at most 32 bounded commands.
#![forbid(unsafe_code)]

mod api;
#[path = "../../crates/blubber/tests/support/differential/api_oracle.rs"]
mod api_oracle;
#[path = "../../crates/blubber/tests/support/differential/bridge.rs"]
mod bridge;
/// Authenticated SDK corrupt histories with independent model and frame oracles.
#[cfg(feature = "sdk-fuzz")]
pub mod corrupt;
mod edits;
#[path = "../../crates/blubber/tests/support/keyless_differential.rs"]
mod keyless_differential;
#[path = "../../crates/blubber/tests/support/differential/oracle.rs"]
mod oracle;
mod validation;

use blubber::{List, Map, Pubkey, Str};
use blubber_model::{Edit as ModelEdit, PathSegment as Segment, Schema as ModelSchema, Value};
use bridge::{Bridge, Observed};

const ID: [u8; 8] = *b"fuzz0034";
const SCHEMAS: usize = 32;
const MAX_INPUT: usize = 65_536;

#[blubber::schema]
struct Record {
    name: Str,
    flag: bool,
    tags: List<u16>,
    memo: Option<Str>,
}

impl Bridge for Record {
    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn value_from_read(value: Self::Read<'_>) -> Value {
        Value::Struct(vec![
            <Str as Bridge>::value_from_read(value.try_name().unwrap()),
            <bool as Bridge>::value_from_read(value.try_flag().unwrap()),
            <List<u16> as Bridge>::value_from_read(value.try_tags().unwrap()),
            <Option<Str> as Bridge>::value_from_read(value.try_memo().unwrap()),
        ])
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        Ok(Value::Struct(vec![
            <Str as Bridge>::value_from_read(handle.try_name()?),
            <bool as Bridge>::value_from_read(handle.try_flag()?),
            <List<u16> as Bridge>::value_from_read(handle.try_tags()?),
            <Option<Str> as Bridge>::value_from_read(handle.try_memo()?),
        ]))
    }
    fn api(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &blubber_model::ApiEdit,
        calls: &mut blubber_model::Callbacks,
        destination: &mut Option<Vec<u8>>,
    ) -> blubber::Result<blubber_model::ApiReturn> {
        let (Segment::Field(field), rest) = path.split_first().unwrap() else {
            panic!("field selection")
        };
        match field {
            0 => <Str as Bridge>::api(handle.name_mut(), rest, edit, calls, destination),
            1 => <bool as Bridge>::api(handle.flag_mut(), rest, edit, calls, destination),
            2 => <List<u16> as Bridge>::api(handle.tags_mut(), rest, edit, calls, destination),
            3 => <Option<Str> as Bridge>::api(handle.memo_mut(), rest, edit, calls, destination),
            _ => panic!("field ordinal"),
        }
    }

    fn schema() -> ModelSchema {
        ModelSchema::Struct(vec![
            <Str as Bridge>::schema(),
            <bool as Bridge>::schema(),
            <List<u16> as Bridge>::schema(),
            <Option<Str> as Bridge>::schema(),
        ])
    }
    fn owned(value: &Value) -> Self::Owned {
        let Value::Struct(fields) = value else {
            panic!("record value")
        };
        RecordOwned {
            name: Str::owned(&fields[0]),
            flag: bool::owned(&fields[1]),
            tags: List::<u16>::owned(&fields[2]),
            memo: Option::<Str>::owned(&fields[3]),
        }
    }
    fn value(value: Self::Owned) -> Value {
        Value::Struct(vec![
            Str::value(value.name),
            bool::value(value.flag),
            List::<u16>::value(value.tags),
            Option::<Str>::value(value.memo),
        ])
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Present(value.is_some())
    }
    fn borrowed<'a>(value: &'a Value, arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        let Value::Struct(fields) = value else {
            panic!("record input")
        };
        RecordInit {
            name: Str::borrowed(&fields[0], arena),
            flag: bool::borrowed(&fields[1], arena),
            tags: List::<u16>::borrowed(&fields[2], arena),
            memo: Option::<Str>::borrowed(&fields[3], arena),
        }
    }
    fn apply(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        let (Segment::Field(field), rest) = path.split_first().expect("record path") else {
            panic!("record field")
        };
        match field {
            0 => Str::apply(handle.name_mut(), rest, edit),
            1 => bool::apply(handle.flag_mut(), rest, edit),
            2 => List::<u16>::apply(handle.tags_mut(), rest, edit),
            3 => Option::<Str>::apply(handle.memo_mut(), rest, edit),
            _ => unreachable!("oracle field"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
struct Packed {
    flag: bool,
    number: u16,
    raw: [u8; 3],
}

impl Bridge for Packed {
    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn value_from_read(value: Self::Read<'_>) -> Value {
        Self::value(value)
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        handle.try_get().map(Self::value)
    }

    fn schema() -> ModelSchema {
        ModelSchema::Struct(vec![bool::schema(), u16::schema(), <[u8; 3]>::schema()])
    }
    fn owned(value: &Value) -> Self {
        let Value::Struct(fields) = value else {
            panic!("packed value")
        };
        Self {
            flag: bool::owned(&fields[0]),
            number: u16::owned(&fields[1]),
            raw: <[u8; 3]>::owned(&fields[2]),
        }
    }
    fn value(value: Self) -> Value {
        Value::Struct(vec![
            bool::value(value.flag),
            u16::value(value.number),
            <[u8; 3]>::value(value.raw),
        ])
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Old(value.map(Self::value))
    }
    fn borrowed<'a>(value: &'a Value, _arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        Self::owned(value)
    }
    fn apply(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        // The independent oracle visits fixed structs by native fields, while
        // Fixed intentionally exposes a single atomic value editor. Translate
        // a field replacement into one complete fixed-value write.
        let mut value = Self::value(handle.try_get()?);
        let selected = if let [Segment::Field(index)] = path {
            let Value::Struct(fields) = &mut value else {
                unreachable!()
            };
            &mut fields[*index]
        } else {
            assert!(path.is_empty());
            &mut value
        };
        let ModelEdit::Set(replacement) = edit else {
            unreachable!()
        };
        *selected = replacement.clone();
        handle.set(Self::owned(&value))?;
        Ok(Observed::Unit)
    }
}

// Direct root fields exercise compact string/list/map loans in valid histories
// and the same bounded corrupt recovery lane as every existing fixture.
macro_rules! tail_fixture {
    ($name:ident, $init:ident, $owned:ident, $ty:ty) => {
        #[blubber::schema]
        struct $name {
            value: $ty,
        }
        impl Bridge for $name {
            fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
                handle.reborrow()
            }
            fn value_from_read(value: Self::Read<'_>) -> Value {
                Value::Struct(vec![<$ty as Bridge>::value_from_read(
                    value.try_value().unwrap(),
                )])
            }
            fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
                Ok(Value::Struct(vec![<$ty as Bridge>::value_from_read(
                    handle.try_value()?,
                )]))
            }
            fn api(
                mut handle: Self::Mut<'_>,
                path: &[Segment],
                edit: &blubber_model::ApiEdit,
                calls: &mut blubber_model::Callbacks,
                destination: &mut Option<Vec<u8>>,
            ) -> blubber::Result<blubber_model::ApiReturn> {
                let (Segment::Field(field), rest) = path.split_first().unwrap() else {
                    panic!("field selection")
                };
                match field {
                    0 => <$ty as Bridge>::api(handle.value_mut(), rest, edit, calls, destination),
                    _ => panic!("field ordinal"),
                }
            }

            fn schema() -> ModelSchema {
                ModelSchema::Struct(vec![<$ty as Bridge>::schema()])
            }
            fn owned(value: &Value) -> Self::Owned {
                let Value::Struct(fields) = value else {
                    panic!("tail fixture value")
                };
                $owned {
                    value: <$ty as Bridge>::owned(&fields[0]),
                }
            }
            fn value(value: Self::Owned) -> Value {
                Value::Struct(vec![<$ty as Bridge>::value(value.value)])
            }
            fn borrowed<'a>(value: &'a Value, arena: &'a bumpalo::Bump) -> Self::Init<'a> {
                let Value::Struct(fields) = value else {
                    panic!("tail fixture input")
                };
                $init {
                    value: <$ty as Bridge>::borrowed(&fields[0], arena),
                }
            }
            fn removed(value: Option<Self::Removed>) -> Observed {
                Observed::Present(value.is_some())
            }
            fn apply(
                mut handle: Self::Mut<'_>,
                path: &[Segment],
                edit: &ModelEdit,
            ) -> blubber::Result<Observed> {
                let (Segment::Field(0), rest) = path.split_first().expect("tail fixture path")
                else {
                    panic!("tail fixture field")
                };
                <$ty as Bridge>::apply(handle.value_mut(), rest, edit)
            }
        }
        impl validation::Walk for $name {
            fn visit(value: Self::Read<'_>, budget: &mut usize) {
                if let Ok(child) = value.try_value() {
                    <$ty as validation::Walk>::visit(child, budget);
                }
            }
        }
    };
}
tail_fixture!(TailText, TailTextInit, TailTextOwned, Str);
tail_fixture!(TailList, TailListInit, TailListOwned, List<u16>);
tail_fixture!(TailMap, TailMapInit, TailMapOwned, Map<u16, u16>);

type Deep = List<Map<u16, List<Map<u8, List<Record>>>>>;

// Keep dispatch shared by validation, edits, seed generation and replay tests.
macro_rules! concrete_bridge {
    ($name:ident, $ty:ty) => {
        blubber::__root_schema!($name, $ty);
        impl bridge::Bridge for $name {
            fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
                <$ty as Bridge>::loan(handle)
            }
            fn value_from_read(value: Self::Read<'_>) -> Value {
                <$ty as Bridge>::value_from_read(value)
            }
            fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
                <$ty as Bridge>::snapshot(handle)
            }
            fn api(
                handle: Self::Mut<'_>,
                path: &[Segment],
                edit: &blubber_model::ApiEdit,
                calls: &mut blubber_model::Callbacks,
                destination: &mut Option<Vec<u8>>,
            ) -> blubber::Result<blubber_model::ApiReturn> {
                <$ty as Bridge>::api(handle, path, edit, calls, destination)
            }

            fn schema() -> blubber_model::Schema {
                <$ty as bridge::Bridge>::schema()
            }
            fn owned(value: &blubber_model::Value) -> Self::Owned {
                <$ty as bridge::Bridge>::owned(value)
            }
            fn value(value: Self::Owned) -> blubber_model::Value {
                <$ty as bridge::Bridge>::value(value)
            }
            fn borrowed<'a>(
                value: &'a blubber_model::Value,
                arena: &'a bumpalo::Bump,
            ) -> Self::Init<'a> {
                <$ty as bridge::Bridge>::borrowed(value, arena)
            }
            fn removed(value: Option<Self::Removed>) -> bridge::Observed {
                <$ty as bridge::Bridge>::removed(value)
            }
            fn apply(
                handle: Self::Mut<'_>,
                path: &[blubber_model::PathSegment],
                edit: &blubber_model::Edit,
            ) -> blubber::Result<bridge::Observed> {
                <$ty as bridge::Bridge>::apply(handle, path, edit)
            }
        }
        impl validation::Walk for $name {
            fn visit(value: Self::Read<'_>, budget: &mut usize) {
                <$ty as validation::Walk>::visit(value, budget);
            }
        }
    };
}

concrete_bridge!(__Root0, bool);
concrete_bridge!(__Root1, u8);
concrete_bridge!(__Root2, u16);
concrete_bridge!(__Root3, u32);
concrete_bridge!(__Root4, u64);
concrete_bridge!(__Root5, u128);
concrete_bridge!(__Root6, i8);
concrete_bridge!(__Root7, i16);
concrete_bridge!(__Root8, i32);
concrete_bridge!(__Root9, i64);
concrete_bridge!(__Root10, i128);
concrete_bridge!(__Root11, Pubkey);
concrete_bridge!(__Root12, [u8; 33]);
concrete_bridge!(__Root13, Str);
concrete_bridge!(__Root14, Option<Str>);
concrete_bridge!(__Root15, List<bool>);
concrete_bridge!(__Root16, List<Str>);
concrete_bridge!(__Root17, Map<bool, bool>);
concrete_bridge!(__Root18, Map<i16, Str>);
concrete_bridge!(__Root19, Record);
concrete_bridge!(__Root20, List<Record>);
concrete_bridge!(__Root21, Map<[u8; 3], Record>);
concrete_bridge!(__Root22, Deep);
concrete_bridge!(__Root23, Map<Pubkey, List<u128>>);
concrete_bridge!(__Root24, Packed);
concrete_bridge!(__Root25, List<Packed>);
concrete_bridge!(__Root26, Map<u16, Packed>);
concrete_bridge!(__Root27, Option<bool>);
concrete_bridge!(__Root28, List<Map<bool, Option<Str>>>);
concrete_bridge!(__Root29, TailText);
concrete_bridge!(__Root30, TailList);
concrete_bridge!(__Root31, TailMap);

macro_rules! dispatch {
    ($selector:expr, $function:ident $(, $arg:expr)*) => {
        match usize::from($selector) % SCHEMAS {
            0 => $function::<__Root0>($($arg),*),
            1 => $function::<__Root1>($($arg),*),
            2 => $function::<__Root2>($($arg),*),
            3 => $function::<__Root3>($($arg),*),
            4 => $function::<__Root4>($($arg),*),
            5 => $function::<__Root5>($($arg),*),
            6 => $function::<__Root6>($($arg),*),
            7 => $function::<__Root7>($($arg),*),
            8 => $function::<__Root8>($($arg),*),
            9 => $function::<__Root9>($($arg),*),
            10 => $function::<__Root10>($($arg),*),
            11 => $function::<__Root11>($($arg),*),
            12 => $function::<__Root12>($($arg),*),
            13 => $function::<__Root13>($($arg),*),
            14 => $function::<__Root14>($($arg),*),
            15 => $function::<__Root15>($($arg),*),
            16 => $function::<__Root16>($($arg),*),
            17 => $function::<__Root17>($($arg),*),
            18 => $function::<__Root18>($($arg),*),
            19 => $function::<__Root19>($($arg),*),
            20 => $function::<__Root20>($($arg),*),
            21 => $function::<__Root21>($($arg),*),
            22 => $function::<__Root22>($($arg),*),
            23 => $function::<__Root23>($($arg),*),
            24 => $function::<__Root24>($($arg),*),
            25 => $function::<__Root25>($($arg),*),
            26 => $function::<__Root26>($($arg),*),
            27 => $function::<__Root27>($($arg),*),
            28 => $function::<__Root28>($($arg),*),
            29 => $function::<__Root29>($($arg),*),
            30 => $function::<__Root30>($($arg),*),
            31 => $function::<__Root31>($($arg),*),
            _ => unreachable!(),
        }
    };
}

/// Checks corrupt exact-region bytes, account envelopes and fallible readers.
///
/// Byte zero selects the schema, byte one selects an offset probe. The remaining
/// bytes are checked both as an exact subtree and as an arbitrary full account.
/// A second account wraps the subtree in a correct envelope, reaching child
/// faults even when mutations destroy the arbitrary account's discriminator.
pub fn validate(data: &[u8]) {
    let data = &data[..data.len().min(MAX_INPUT)];
    if let [selector, probe, bytes @ ..] = data {
        use validation::check;
        dispatch!(*selector, check, bytes, *probe);
    }
}

/// Runs bounded edits with an independent byte oracle and atomic-error checks.
///
/// See `fuzz/README.md` for the binary grammar and resource limits. Malformed
/// states use only checked APIs; a successful local edit need not repair faults
/// in unrelated children. Full model equality is required whenever the whole
/// initial state validates.
pub fn edit(data: &[u8]) {
    keyless_differential::check(data);
    let data = &data[..data.len().min(MAX_INPUT)];
    if let [selector, rest @ ..] = data {
        use edits::check;
        dispatch!(*selector, check, rest);
    }
}

fn sample<S: Bridge>() -> Vec<u8> {
    assert!(oracle::depth(&S::schema()) <= 8);
    S::schema()
        .encode(&oracle::sample(
            &S::schema(),
            &mut oracle::Entropy(34),
            true,
        ))
        .unwrap()
}

/// Returns small directed validation seeds for every fixture and corrupt family.
pub fn validation_seeds() -> Vec<(String, Vec<u8>)> {
    let mut seeds = vec![];
    for selector in 0..SCHEMAS as u8 {
        let bytes = dispatch!(selector, sample);
        let mut valid = vec![selector, 0];
        valid.extend_from_slice(&bytes);
        seeds.push((format!("s{selector:02}-valid"), valid.clone()));
        let mut envelope = vec![selector, 0];
        envelope.extend(account(&bytes));
        seeds.push((format!("s{selector:02}-envelope"), envelope));
        for (name, raw) in [
            ("empty", vec![]),
            ("huge-count", u32::MAX.to_le_bytes().to_vec()),
            ("bad-domain", vec![2, 0xff]),
        ] {
            let mut seed = vec![selector, 255];
            seed.extend(raw);
            seeds.push((format!("s{selector:02}-{name}"), seed));
        }
    }
    for (name, selector, bytes) in [
        (
            "descending-ends",
            16,
            vec![3, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b'],
        ),
        ("overlarge-end", 16, vec![2, 0, 0, 0, 255, 255, 255, 255]),
        ("duplicate-keys", 17, vec![2, 0, 0, 0, 0, 0, 0, 1]),
        ("descending-keys", 17, vec![2, 0, 0, 0, 1, 0, 0, 1]),
        ("bad-key-domain", 17, vec![1, 0, 0, 0, 2, 1]),
        ("overlong-utf8", 13, vec![0xc0, 0x80]),
        ("surrogate-utf8", 13, vec![0xed, 0xa0, 0x80]),
        ("none-with-tail", 14, vec![0, b'x']),
    ] {
        let mut seed = vec![selector, 0];
        seed.extend(bytes);
        seeds.push((name.into(), seed));
    }
    seeds
}

/// Returns valid histories and corrupt-state seeds for the edit target.
pub fn edit_seeds() -> Vec<(String, Vec<u8>)> {
    let mut seeds = vec![];
    for selector in 0..SCHEMAS as u8 {
        for mode in 0..4 {
            // selector; mode; capacity; initial length; initial entropy/bytes.
            let initial = if mode == 3 {
                dispatch!(selector, sample)
            } else {
                34u64.to_le_bytes().to_vec()
            };
            let mut seed = vec![selector, mode, 255, 0];
            seed.extend_from_slice(&(initial.len() as u16).to_le_bytes());
            seed.extend_from_slice(&initial);
            for i in 0..32u8 {
                seed.extend_from_slice(&[i, 0, i, i, 0, i, 255, 2]);
            }
            seeds.push((format!("s{selector:02}-mode{mode}"), seed));
        }
    }
    let mut distant_end = sample::<__Root19>();
    distant_end[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut duplicate = 4u32.to_le_bytes().to_vec();
    for key in [2u16, 2, 3, 4] {
        duplicate.extend_from_slice(&key.to_be_bytes());
        duplicate.extend_from_slice(&key.to_le_bytes());
    }
    let mut descending = duplicate.clone();
    descending[4..6].copy_from_slice(&3u16.to_be_bytes());
    for (name, selector, root) in [
        ("local-distant-end", 19, distant_end),
        ("local-distant-duplicate", 31, duplicate),
        ("local-distant-order", 31, descending),
        (
            "local-full-width-count",
            30,
            u32::MAX.to_le_bytes().to_vec(),
        ),
    ] {
        let mut seed = vec![selector, 3, 255, 0];
        seed.extend_from_slice(&(root.len() as u16).to_le_bytes());
        seed.extend(root);
        for i in 0..32u8 {
            seed.extend_from_slice(&[i, 0, 1, 255, 0, i, 255, 2]);
        }
        seeds.push((name.into(), seed));
    }
    seeds
}

fn account(root: &[u8]) -> Vec<u8> {
    let mut bytes = ID.to_vec();
    bytes.extend_from_slice(&((12 + root.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes
}
