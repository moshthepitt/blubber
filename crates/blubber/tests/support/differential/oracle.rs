//! Logical inputs, edits and subtree identities. No production imports.

#![forbid(unsafe_code)]

use blubber_model::{Edit, Key, KeySchema, PathSegment as Segment, Schema, Value};

#[derive(Clone, Debug)]
pub(crate) struct Command {
    pub selector: u16,
    pub kind: u8,
    pub ordinal: u16,
    pub data: Vec<u8>,
}

pub(crate) type Path = Vec<Segment>;
pub(crate) type Frame = (Path, Path);

// SplitMix64 is used only to expand a shrunk command into logical input. The
// property runner's recorded ChaCha seed controls the commands themselves.
pub(crate) struct Entropy(pub u64);
impl Entropy {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn number(&mut self) -> u128 {
        (u128::from(self.next()) << 64) | u128::from(self.next())
    }
    fn bytes(&mut self, count: usize) -> Vec<u8> {
        (0..count).map(|_| self.next() as u8).collect()
    }
    fn string(&mut self) -> String {
        let alphabet = ['\0', 'a', 'z', 'é', '\u{301}', '中', '🦀'];
        (0..self.next() % 9)
            .map(|_| alphabet[self.next() as usize % alphabet.len()])
            .collect()
    }
}

fn unsigned(value: u128, bytes: usize) -> u128 {
    if bytes == 16 {
        value
    } else {
        value & ((1u128 << (8 * bytes)) - 1)
    }
}
fn signed(value: u128, bytes: usize) -> i128 {
    let shift = 128 - 8 * bytes;
    ((value << shift) as i128) >> shift
}

pub(crate) fn key(schema: &KeySchema, entropy: &mut Entropy) -> Key {
    // A small numerical domain makes replacement and absent lookup frequent;
    // directed histories separately exercise extrema and native ordering.
    match schema {
        KeySchema::Unsigned(w) => Key::Unsigned(unsigned(entropy.next() as u128 % 513, w.bytes())),
        KeySchema::Signed(w) => Key::Signed(signed(
            (entropy.next() as i64 % 513 - 256) as u128,
            w.bytes(),
        )),
        KeySchema::Bool => Key::Bool(entropy.next() & 1 != 0),
        KeySchema::Pubkey => Key::Pubkey(entropy.bytes(32).try_into().unwrap()),
        KeySchema::Array(n) => Key::Array(entropy.bytes(*n as usize)),
    }
}

pub(crate) fn sample(schema: &Schema, entropy: &mut Entropy, expanded: bool) -> Value {
    match schema {
        Schema::Unsigned(w) => Value::Unsigned(unsigned(entropy.number(), w.bytes())),
        Schema::Signed(w) => Value::Signed(signed(entropy.number(), w.bytes())),
        Schema::Bool => Value::Bool(entropy.next() & 1 != 0),
        Schema::Pubkey => Value::Pubkey(entropy.bytes(32).try_into().unwrap()),
        Schema::Array(n) => Value::Array(entropy.bytes(*n as usize)),
        Schema::Str => Value::Str(entropy.string()),
        Schema::Struct(fields) => Value::Struct(
            fields
                .iter()
                .map(|s| sample(s, entropy, expanded))
                .collect(),
        ),
        Schema::List(child) => {
            let count = if expanded { 2 } else { entropy.next() % 3 };
            Value::List(
                (0..count)
                    .map(|_| sample(child, entropy, expanded))
                    .collect(),
            )
        }
        Schema::Map(k, child) => {
            let count = if expanded {
                2
            } else {
                entropy.next() as usize % 3
            };
            let mut entries = std::collections::BTreeMap::new();
            while entries.len() < count {
                entries.insert(key(k, entropy), sample(child, entropy, expanded));
            }
            Value::Map(entries)
        }
        Schema::Option(child) => Value::Option(
            (expanded || entropy.next() & 1 != 0)
                .then(|| Box::new(sample(child, entropy, expanded))),
        ),
    }
}

pub(crate) fn children<'a>(
    schema: &'a Schema,
    value: &'a Value,
) -> Vec<(Segment, &'a Schema, &'a Value)> {
    match (schema, value) {
        (Schema::Struct(fields), Value::Struct(values)) => fields
            .iter()
            .zip(values)
            .enumerate()
            .map(|(i, (s, v))| (Segment::Field(i), s, v))
            .collect(),
        (Schema::List(child), Value::List(values)) => values
            .iter()
            .enumerate()
            .map(|(i, v)| (Segment::Index(i), child.as_ref(), v))
            .collect(),
        (Schema::Map(_, child), Value::Map(values)) => values
            .iter()
            .map(|(k, v)| (Segment::Key(k.clone()), child.as_ref(), v))
            .collect(),
        (Schema::Option(child), Value::Option(Some(value))) => {
            vec![(Segment::Some, child.as_ref(), value.as_ref())]
        }
        _ => vec![],
    }
}

pub(crate) fn at<'a>(
    mut schema: &'a Schema,
    mut value: &'a Value,
    path: &[Segment],
) -> (&'a Schema, &'a Value) {
    for segment in path {
        let (_, s, v) = children(schema, value)
            .into_iter()
            .find(|(p, _, _)| p == segment)
            .expect("existing logical path");
        schema = s;
        value = v;
    }
    (schema, value)
}

pub(crate) fn nodes(schema: &Schema, value: &Value) -> Vec<Path> {
    fn visit(schema: &Schema, value: &Value, path: &mut Path, output: &mut Vec<Path>) {
        for (segment, child, value) in children(schema, value) {
            path.push(segment);
            visit(child, value, path, output);
            path.pop();
        }
        if !matches!(schema, Schema::Struct(_)) {
            output.push(path.clone());
        }
    }
    let mut output = Vec::new();
    visit(schema, value, &mut vec![], &mut output);
    output
}

pub(crate) fn action(schema: &Schema, value: &Value, command: &Command) -> (Path, Edit) {
    let nodes = nodes(schema, value);
    let mut path = nodes[command.selector as usize % nodes.len()].clone();
    let (schema, value) = at(schema, value, &path);
    let mut entropy = Entropy(
        command
            .data
            .iter()
            .fold(u64::from(command.ordinal), |a, b| {
                a.wrapping_mul(257).wrapping_add(u64::from(*b))
            }),
    );
    let ordinal = command.ordinal as usize;
    let edit = match (schema, value) {
        (Schema::Str, Value::Str(old)) => {
            let fresh = entropy.string();
            match command.kind % 6 {
                0 => Edit::Set(Value::Str(String::new())),
                1 => Edit::Set(Value::Str("x".repeat(old.len()))),
                2 => Edit::Set(Value::Str(format!("{old}{fresh}🦀"))),
                3 => Edit::Set(Value::Str(
                    old.chars().take(old.chars().count() / 2).collect(),
                )),
                4 => Edit::StringPush(fresh),
                _ => Edit::Clear,
            }
        }
        (Schema::List(child), Value::List(values)) => match command.kind % 8 {
            0 | 1 => Edit::ListInsert {
                index: ordinal % (values.len() + 1),
                values: (0..command.data.first().copied().unwrap_or(command.kind) % 5)
                    .map(|_| sample(child, &mut entropy, false))
                    .collect(),
            },
            2 => Edit::ListRemove(ordinal % (values.len() + 1)),
            3 => Edit::ListPop,
            4 => Edit::ListTruncate(ordinal % (values.len() + 2)),
            5 => Edit::Clear,
            6 => {
                path.push(Segment::Index(ordinal % (values.len() + 1)));
                Edit::Set(sample(child, &mut entropy, false))
            }
            _ => Edit::ListInsert {
                index: values.len() + 1,
                values: (0..command.data.first().copied().unwrap_or(command.kind) % 5)
                    .map(|_| sample(child, &mut entropy, false))
                    .collect(),
            },
        },
        (Schema::Map(k, child), Value::Map(values)) => {
            let k = if command.kind & 1 == 0 && !values.is_empty() {
                values.keys().nth(ordinal % values.len()).unwrap().clone()
            } else {
                key(k, &mut entropy)
            };
            match command.kind % 5 {
                0..=2 => Edit::MapInsert {
                    key: k,
                    value: sample(child, &mut entropy, false),
                },
                3 => Edit::MapRemove(k),
                _ => Edit::Clear,
            }
        }
        (Schema::Option(child), Value::Option(old)) => match command.kind % 4 {
            0 => Edit::Set(Value::Option(None)),
            1 | 2 => Edit::Set(Value::Option(Some(Box::new(sample(
                child,
                &mut entropy,
                false,
            ))))),
            _ if old.is_none() => {
                path.push(Segment::Some);
                Edit::Set(sample(child, &mut entropy, false))
            }
            _ => Edit::Clear,
        },
        _ => Edit::Set(sample(schema, &mut entropy, false)),
    };
    (path, edit)
}

// Return maximal untouched logical regions, mapping old list ordinals through
// insert/removal. Physical byte addresses are resolved only by the caller.
pub(crate) fn frames(schema: &Schema, value: &Value, path: &[Segment], edit: &Edit) -> Vec<Frame> {
    let mut output = vec![];
    let mut prefix = vec![];
    let (mut schema, mut value) = (schema, value);
    for selected in path {
        for (segment, _, _) in children(schema, value) {
            if &segment != selected {
                let mut sibling = prefix.clone();
                sibling.push(segment);
                output.push((sibling.clone(), sibling));
            }
        }
        (schema, value) = at(schema, value, std::slice::from_ref(selected));
        prefix.push(selected.clone());
    }
    for (segment, _, _) in children(schema, value) {
        let relocated = match (&segment, edit) {
            (Segment::Index(i), Edit::ListInsert { index, values }) => Some(Segment::Index(
                i + if i >= index { values.len() } else { 0 },
            )),
            (Segment::Index(i), Edit::ListRemove(index)) if i != index => {
                Some(Segment::Index(i - usize::from(i > index)))
            }
            (Segment::Index(i), Edit::ListPop) => {
                let Value::List(values) = value else {
                    unreachable!()
                };
                (i + 1 < values.len()).then_some(segment.clone())
            }
            (Segment::Index(i), Edit::ListTruncate(count)) if i < count => Some(segment.clone()),
            (Segment::Key(k), Edit::MapInsert { key, .. } | Edit::MapRemove(key)) if k != key => {
                Some(segment.clone())
            }
            _ => None,
        };
        if let Some(new) = relocated {
            let mut old_path = prefix.clone();
            old_path.push(segment);
            let mut new_path = prefix.clone();
            new_path.push(new);
            output.push((old_path, new_path));
        }
    }
    output
}

pub(crate) fn depth(schema: &Schema) -> usize {
    1 + match schema {
        Schema::Struct(fields) => fields.iter().map(depth).max().unwrap(),
        Schema::List(child) | Schema::Map(_, child) | Schema::Option(child) => depth(child),
        _ => 0,
    }
}
