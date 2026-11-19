//! Nested typed projections checked against independently rebuilt owned trees.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use blubber::raw::Edit;
use blubber::Resize as _;
use blubber::{ErrorKind, List, Map, Result, Schema, SliceStore, Str, View};
use blubber_model::{IntWidth, Key, KeySchema, Model, PathSegment, Schema as Shape, Value};

const ID: [u8; 8] = *b"nested33";
const TEXTS: [&str; 3] = ["", "a", "é"];

// The trait dispatches real typed handles. The expected tree and encoding below
// depend only on blubber-model and std, never production encoding or offsets.
trait Node: Schema {
    fn shape() -> Shape;
    fn edit(handle: Self::Mut<'_>, path: &[PathSegment], op: u8, input: &str) -> Result<()>;
}

impl Node for Str {
    fn shape() -> Shape {
        Shape::Str
    }
    fn edit(mut handle: Self::Mut<'_>, path: &[PathSegment], op: u8, input: &str) -> Result<()> {
        assert!(path.is_empty());
        match op {
            0 => handle.set(input),
            1 => handle.push_str(input),
            2 => handle.truncate(0),
            3 => handle.clear(),
            _ => handle.truncate(usize::MAX),
        }
    }
}

impl<T: Node> Node for Option<T> {
    fn shape() -> Shape {
        Shape::Option(Box::new(T::shape()))
    }
    fn edit(handle: Self::Mut<'_>, path: &[PathSegment], op: u8, input: &str) -> Result<()> {
        assert_eq!(path[0], PathSegment::Some);
        T::edit(handle.some_mut()?, &path[1..], op, input)
    }
}

impl<T: Node> Node for List<T> {
    fn shape() -> Shape {
        Shape::List(Box::new(T::shape()))
    }
    fn edit(handle: Self::Mut<'_>, path: &[PathSegment], op: u8, input: &str) -> Result<()> {
        let PathSegment::Index(index) = path[0] else {
            panic!("list path")
        };
        T::edit(handle.at_mut(index)?, &path[1..], op, input)
    }
}

impl<T: Node> Node for Map<u8, T> {
    fn shape() -> Shape {
        Shape::Map(KeySchema::Unsigned(IntWidth::W1), Box::new(T::shape()))
    }
    fn edit(handle: Self::Mut<'_>, path: &[PathSegment], op: u8, input: &str) -> Result<()> {
        let PathSegment::Key(Key::Unsigned(key)) = path[0] else {
            panic!("map path")
        };
        T::edit(handle.at_mut(&(key as u8))?, &path[1..], op, input)
    }
}

macro_rules! pair {
    ($name:ident, $child:ty) => {
        #[blubber::schema]
        struct $name {
            left: $child,
            right: $child,
        }
        impl Node for $name {
            fn shape() -> Shape {
                Shape::Struct(vec![<$child>::shape(), <$child>::shape()])
            }
            fn edit(
                mut handle: Self::Mut<'_>,
                path: &[PathSegment],
                op: u8,
                input: &str,
            ) -> Result<()> {
                match path[0] {
                    PathSegment::Field(0) => {
                        <$child>::edit(handle.left_mut(), &path[1..], op, input)
                    }
                    PathSegment::Field(1) => {
                        <$child>::edit(handle.right_mut(), &path[1..], op, input)
                    }
                    _ => panic!("struct path"),
                }
            }
        }
    };
}
pair!(Four, List<Option<Str>>);
pair!(Inner, List<Map<u8, Option<Str>>>);
pair!(Eight, Map<u8, Option<Inner>>);

fn minimum(shape: &Shape) -> Value {
    match shape {
        Shape::Str => Value::Str(String::new()),
        Shape::Option(_) => Value::Option(None),
        Shape::List(_) => Value::List(Vec::new()),
        Shape::Map(..) => Value::Map(BTreeMap::new()),
        Shape::Struct(fields) => Value::Struct(fields.iter().map(minimum).collect()),
        _ => panic!("unexpected fixture shape"),
    }
}

// Each struct selects either field. Each collection selects (one, first),
// (two, first), or (two, last). Siblings have the child's minimum valid value.
fn geometries(shape: &Shape, text: &str) -> Vec<(Value, Vec<PathSegment>)> {
    if matches!(shape, Shape::Str) {
        return vec![(Value::Str(text.into()), vec![])];
    }
    let child = match shape {
        Shape::Struct(fields) => &fields[0],
        Shape::Option(child) | Shape::List(child) | Shape::Map(_, child) => child,
        _ => panic!("unexpected fixture shape"),
    };
    let mut out = Vec::new();
    for (value, path) in geometries(child, text) {
        let branches = if matches!(shape, Shape::Struct(_)) {
            2
        } else if matches!(shape, Shape::Option(_)) {
            1
        } else {
            3
        };
        for branch in 0..branches {
            let index = usize::from(branch == 2);
            let count = if branch == 0 { 1 } else { 2 };
            let (value, first) = match shape {
                Shape::Struct(_) => {
                    let mut fields = vec![minimum(child); 2];
                    fields[branch] = value.clone();
                    (Value::Struct(fields), PathSegment::Field(branch))
                }
                Shape::Option(_) => (
                    Value::Option(Some(Box::new(value.clone()))),
                    PathSegment::Some,
                ),
                Shape::List(_) => {
                    let mut values = vec![minimum(child); count];
                    values[index] = value.clone();
                    (Value::List(values), PathSegment::Index(index))
                }
                Shape::Map(..) => {
                    let mut entries = BTreeMap::from([(Key::Unsigned(2), minimum(child))]);
                    if count == 2 {
                        entries.insert(Key::Unsigned(4), minimum(child));
                    }
                    entries.insert(Key::Unsigned(2 + 2 * index as u128), value.clone());
                    (
                        Value::Map(entries),
                        PathSegment::Key(Key::Unsigned(2 + 2 * index as u128)),
                    )
                }
                _ => unreachable!(),
            };
            let mut selected = vec![first];
            selected.extend(path.clone());
            out.push((value, selected));
        }
    }
    out
}

fn account(shape: &Shape, value: &Value) -> Vec<u8> {
    let root = shape.encode(value).unwrap();
    let mut bytes = ID.to_vec();
    bytes.extend_from_slice(&((12 + root.len()) as u32).to_le_bytes());
    bytes.extend(root);
    bytes
}

fn absent(value: &mut Value, path: &[PathSegment], stop: usize) {
    if stop == 0 {
        *value = Value::Option(None);
        return;
    }
    let child = match (&path[0], value) {
        (PathSegment::Field(i), Value::Struct(fields)) => &mut fields[*i],
        (PathSegment::Index(i), Value::List(values)) => &mut values[*i],
        (PathSegment::Key(key), Value::Map(entries)) => entries.get_mut(key).unwrap(),
        (PathSegment::Some, Value::Option(Some(value))) => value,
        _ => panic!("oracle path"),
    };
    absent(child, &path[1..], stop - 1);
}

fn case<S: Node + blubber::RootSchema>(
    fixture: &(Value, Vec<PathSegment>),
    geometry: usize,
    old: usize,
    input: usize,
    op: u8,
    cap: usize,
    stop: Option<usize>,
) {
    let shape = S::shape();
    let (mut value, path) = fixture.clone();
    if let Some(stop) = stop {
        absent(&mut value, &path, stop);
    }
    let before = account(&shape, &value);
    assert!(before.len() <= cap && cap <= 128);
    let mut oracle = Model::new(shape.clone(), value).unwrap();
    let operation = match op {
        0 => blubber_model::Edit::Set(Value::Str(TEXTS[input].into())),
        1 => blubber_model::Edit::StringPush(TEXTS[input].into()),
        2 | 3 => blubber_model::Edit::Clear,
        _ => blubber_model::Edit::StringPush(String::new()),
    };
    let expected = oracle.apply_with_limit(&path, operation, cap - 12);
    let mut bytes = [0xa5; 128];
    bytes[..before.len()].copy_from_slice(&before);
    let original = bytes;
    let mut store = SliceStore::new(&mut bytes[..cap]);
    let result = S::edit(
        blubber::raw::project::<S>(Edit::load::<S>(&mut store, ID).unwrap()),
        &path,
        op,
        TEXTS[input],
    );
    assert_eq!(
        result,
        expected.as_ref().map(|_| ()).map_err(|error| match error {
            blubber_model::Error::Capacity => ErrorKind::Capacity.into(),
            blubber_model::Error::Missing => ErrorKind::MissingValue.into(),
            _ => panic!("unexpected oracle error: {error:?}"),
        })
    );
    assert!(!store.is_poisoned());
    assert_eq!(store.storage_len(), cap);
    let wanted = if result.is_ok() {
        account(&shape, oracle.value())
    } else {
        before.clone()
    };
    assert_eq!(
        View::load::<S>(&bytes[..cap], ID).unwrap().bytes(),
        &wanted[12..]
    );
    let mut physical = original;
    physical[..wanted.len()].copy_from_slice(&wanted);
    if result.is_ok() && wanted.len() < before.len() {
        physical[wanted.len()..before.len()].fill(0);
    }
    assert_eq!(
        bytes, physical,
        "g={geometry} s={old} t={input} o={op} c={cap} stop={stop:?}"
    );
}

fn capacities(old: usize, new: usize) -> Vec<usize> {
    let required = old.max(new);
    let mut caps = vec![old, required.saturating_sub(1), required, 128];
    caps.retain(|&c| old <= c && c <= 128);
    caps.sort_unstable();
    caps.dedup();
    caps
}

fn run<S: Node + blubber::RootSchema>(depth: usize, miri: bool) {
    let shape = S::shape();
    let fixtures = TEXTS.map(|text| geometries(&shape, text));
    let variants = &fixtures[0];
    for (g, (_, path)) in variants.iter().enumerate() {
        if miri && g != 0 && g + 1 != variants.len() {
            continue;
        }
        for (old, old_text) in TEXTS.iter().enumerate() {
            if miri && old != 2 {
                continue;
            }
            for op in 0..5 {
                for (input, input_text) in
                    TEXTS.iter().enumerate().take(if op <= 1 { 3 } else { 1 })
                {
                    if miri && input != if op <= 1 { 1 } else { 0 } {
                        continue;
                    }
                    let fixture = &fixtures[old][g];
                    let old_len = account(&shape, &fixture.0).len();
                    let new_text = match op {
                        0 => input_text.to_string(),
                        1 => format!("{old_text}{input_text}"),
                        2 | 3 => String::new(),
                        _ => old_text.to_string(),
                    };
                    // Only this string changes; every ancestor retains its
                    // count, directory width and tags. Actual result bytes
                    // and capacity errors still come from the model codec.
                    let new_len = old_len - old_text.len() + new_text.len();
                    for cap in capacities(old_len, new_len) {
                        case::<S>(fixture, g, old, input, op, cap, None);
                        if !miri {
                            println!("nested_d{depth}_g{g}_o{op}_s{old}_t{input}_c{cap}: PASS");
                        }
                    }
                }
            }
        }
        for (stop, segment) in path.iter().enumerate() {
            if *segment != PathSegment::Some {
                continue;
            }
            case::<S>(&fixtures[0][g], g, 0, 2, 0, 128, Some(stop));
            if !miri {
                println!("nested_d{depth}_g{g}_absent{stop}: PASS");
            }
        }
    }
}

/// Checks every declared path, payload and capacity geometry on the host.
pub fn all() {
    run::<Str>(1, false);
    run::<Four>(4, false);
    run::<Eight>(8, false);
}

/// Shares first/last paths, all operations and capacity/absence controls with Miri.
pub fn miri_subset() {
    run::<Str>(1, true);
    run::<Four>(4, true);
    run::<Eight>(8, true);
}
