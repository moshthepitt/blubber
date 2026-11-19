//! Finite map and entry splice geometries with an independent format oracle.

#![forbid(unsafe_code)]

blubber::__root_schema!(__ConcreteRoot0, Map<u16, Str>);

use blubber::raw::Edit;
use blubber::Resize as _;
use blubber::{Entry, ErrorKind, Map, SliceStore, Str};

const DISCRIMINATOR: [u8; 8] = *b"mapproof";
const KEYS: [u16; 2] = [2, 4];
const TEXTS: [&str; 3] = ["", "a", "é"];

#[derive(Clone, Copy, Debug)]
enum Operation {
    Insert,
    Remove,
    Clear,
    OrInsert,
    EntryInsert,
    EntryRemove,
    Chain,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Remove => "remove",
            Self::Clear => "clear",
            Self::OrInsert => "or_insert",
            Self::EntryInsert => "entry_insert",
            Self::EntryRemove => "entry_remove",
            Self::Chain => "chain",
        }
    }
    fn has_input(self) -> bool {
        matches!(
            self,
            Self::Insert | Self::OrInsert | Self::EntryInsert | Self::Chain
        )
    }
}

// Expected bytes depend only on input values and the published wire format.
// No production descriptor, planner, reader, measurement or encoder is used.
fn wire(values: &[(u16, &str)]) -> Vec<u8> {
    let mut bytes = Vec::from(DISCRIMINATOR);
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    let mut end = 0u32;
    for (key, text) in values {
        bytes.extend_from_slice(&key.to_be_bytes());
        end += text.len() as u32;
        bytes.extend_from_slice(&end.to_le_bytes());
    }
    for (_, text) in values {
        bytes.extend_from_slice(text.as_bytes());
    }
    let used = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&used.to_le_bytes());
    bytes
}

fn capacities(old: usize, new: usize) -> Vec<usize> {
    let required = old.max(new);
    let mut caps = vec![old, required - 1, required, 64];
    caps.retain(|&cap| (old..=64).contains(&cap));
    caps.sort_unstable();
    caps.dedup();
    caps
}

fn initial(n: usize) -> Vec<(u16, &'static str)> {
    KEYS[..n].iter().copied().zip(["", "é"]).collect()
}

fn model<'a>(
    old: &[(u16, &'a str)],
    key: u16,
    text: &'a str,
    op: Operation,
) -> Vec<(u16, &'a str)> {
    let mut expected = old.to_vec();
    let found = expected.binary_search_by_key(&key, |&(key, _)| key);
    match op {
        Operation::Insert | Operation::EntryInsert => match found {
            Ok(index) => expected[index].1 = text,
            Err(index) => expected.insert(index, (key, text)),
        },
        Operation::OrInsert | Operation::Chain => {
            if let Err(index) = found {
                expected.insert(index, (key, text));
            }
        }
        Operation::Remove | Operation::EntryRemove => {
            if let Ok(index) = found {
                expected.remove(index);
            }
        }
        Operation::Clear => expected.clear(),
    }
    expected
}

fn case(n: usize, key: u16, text: usize, cap: usize, op: Operation) {
    let values = initial(n);
    let old = wire(&values);
    let expected = wire(&model(&values, key, TEXTS[text], op));
    let mut before = [0xa5; 64];
    before[..old.len()].copy_from_slice(&old);
    let mut bytes = before;
    let existed = values.iter().any(|&(k, _)| k == key);
    // A chained zero-length append checks the returned projection without
    // adding a second fallible size change to the insertion transaction.
    {
        let mut store = SliceStore::new(&mut bytes[..cap]);
        {
            let mut edit = Edit::load::<__ConcreteRoot0>(&mut store, DISCRIMINATOR).unwrap();
            let mut map = blubber::raw::project::<Map<u16, Str>>(edit.reborrow());
            let result = match op {
                Operation::Insert => map
                    .insert(key, TEXTS[text])
                    .map(|old| assert_eq!(old, existed.then_some(()))),
                Operation::Remove => map
                    .remove(&key)
                    .map(|old| assert_eq!(old, existed.then_some(()))),
                Operation::Clear => map.clear(),
                Operation::OrInsert => {
                    map.entry(key).unwrap().or_insert(TEXTS[text]).map(|value| {
                        assert_eq!(
                            value.try_get().unwrap(),
                            if existed {
                                values.iter().find(|&&(k, _)| k == key).unwrap().1
                            } else {
                                TEXTS[text]
                            }
                        );
                    })
                }
                Operation::EntryInsert => match map.entry(key).unwrap() {
                    Entry::Occupied(entry) => entry.insert(TEXTS[text]),
                    Entry::Vacant(entry) => entry.insert(TEXTS[text]).map(|_| ()),
                },
                Operation::EntryRemove => match map.entry(key).unwrap() {
                    Entry::Occupied(entry) => entry.remove(),
                    Entry::Vacant(_) => Ok(()),
                },
                Operation::Chain => map
                    .entry(key)
                    .unwrap()
                    .or_insert(TEXTS[text])
                    .map(|mut value| value.push_str("").unwrap()),
            };
            assert_eq!(
                result,
                if expected.len() > cap {
                    Err(ErrorKind::Capacity.into())
                } else {
                    Ok(())
                },
                "{op:?} n={n} key={key} text={text} cap={cap}"
            );
            assert_eq!(
                edit.used_len(),
                Ok(if result.is_err() {
                    old.len()
                } else {
                    expected.len()
                } as u32)
            );
        }
        assert!(!store.is_poisoned());
    }
    let mut wanted = before;
    if expected.len() <= cap {
        wanted[..expected.len()].copy_from_slice(&expected);
        if expected.len() < old.len() {
            wanted[expected.len()..old.len()].fill(0);
        }
    }
    assert_eq!(
        bytes, wanted,
        "{op:?} n={n} key={key} text={text} cap={cap}"
    );
}

/// Enumerates every declared count/query/input/capacity geometry exactly once.
#[allow(dead_code)]
pub fn all() {
    for n in 0..=2 {
        for op in [
            Operation::Insert,
            Operation::Remove,
            Operation::Clear,
            Operation::OrInsert,
            Operation::EntryInsert,
            Operation::EntryRemove,
            Operation::Chain,
        ] {
            let queries: &[u16] = if matches!(op, Operation::Clear) {
                &[0]
            } else {
                &[1, 2, 3, 4, 5]
            };
            for &key in queries {
                for (text, &input) in
                    TEXTS
                        .iter()
                        .enumerate()
                        .take(if op.has_input() { 3 } else { 1 })
                {
                    let values = initial(n);
                    let old = wire(&values);
                    let new = wire(&model(&values, key, input, op));
                    for cap in capacities(old.len(), new.len()) {
                        case(n, key, text, cap, op);
                        println!("typed_plan_{}_n{n}_k{key}_s{text}_c{cap}: PASS", op.name());
                    }
                }
            }
        }
    }
}

/// Runs the same independent oracle on representative cases in both Miri models.
#[allow(dead_code)]
pub fn miri_subset() {
    for (n, key, text, cap, op) in [
        (0, 1, 0, 16, Operation::Insert),
        (0, 1, 0, 22, Operation::Insert),
        (1, 3, 2, 64, Operation::Insert),
        (2, 3, 1, 64, Operation::Insert),
        (2, 2, 2, 64, Operation::Insert),
        (2, 4, 0, 64, Operation::Insert),
        (2, 2, 0, 64, Operation::Remove),
        (2, 5, 0, 64, Operation::Remove),
        (2, 0, 0, 64, Operation::Clear),
        (1, 2, 2, 64, Operation::OrInsert),
        (1, 1, 1, 64, Operation::EntryInsert),
        (2, 4, 0, 64, Operation::EntryRemove),
        (1, 3, 2, 64, Operation::Chain),
    ] {
        case(n, key, text, cap, op);
    }
}
