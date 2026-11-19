//! finite record geometries, checked against a standalone format encoder.

#![forbid(unsafe_code)]

#[path = "../../src/list_bounds.rs"]
mod bounds;

blubber::__root_schema!(__ConcreteRoot0, List<Record>);
blubber::__root_schema!(__ConcreteRoot1, List<u32>);

use super::record::{Record, RecordInit, Records};
use blubber::raw::Edit;
use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, ReadStorage, SliceStore};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Insert,
    Replace,
    Remove,
    Pop,
    Truncate,
    Clear,
}

fn word(bytes: &mut [u8], offset: usize, value: usize) {
    bytes[offset..offset + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
}

// Encode from values and the published format alone. No production layout,
// planner, reader, measure or encode helper contributes expected bytes.
fn envelope(count: usize) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes[..8].copy_from_slice(&Records::DISCRIMINATOR);
    word(&mut bytes, 12, count);
    bytes
}

fn wire(records: &[RecordInit<'_>]) -> Vec<u8> {
    let mut bytes = envelope(records.len());
    let mut ends = Vec::new();
    let mut payload = Vec::new();
    for record in records {
        payload.extend_from_slice(&(record.name.len() as u32).to_le_bytes());
        payload.extend_from_slice(record.name.as_bytes());
        payload.extend_from_slice(&(record.tags.len() as u32).to_le_bytes());
        for tag in record.tags {
            payload.extend_from_slice(&tag.to_le_bytes());
        }
        ends.push(payload.len());
    }
    for &end in ends.iter().take(ends.len().saturating_sub(1)) {
        bytes.extend_from_slice(&(end as u32).to_le_bytes());
    }
    bytes.extend_from_slice(&payload);
    let used = bytes.len();
    word(&mut bytes, 8, used);
    bytes
}

fn fixed_wire(tags: &[u32]) -> Vec<u8> {
    let mut bytes = envelope(tags.len());
    for tag in tags {
        bytes.extend_from_slice(&tag.to_le_bytes());
    }
    let used = bytes.len();
    word(&mut bytes, 8, used);
    bytes
}

fn initial(tags: &[u32; 2]) -> [RecordInit<'_>; 2] {
    [
        RecordInit {
            name: "",
            tags: &[],
        },
        RecordInit {
            name: "x",
            tags: &tags[..1],
        },
    ]
}

fn storage(encoded: &[u8]) -> [u8; 48] {
    // Nonzero spare catches writes beyond the old/new used range.
    let mut bytes = [0xa5; 48];
    bytes[..encoded.len()].copy_from_slice(encoded);
    bytes
}

fn expected_storage(before: [u8; 48], old_used: usize, encoded: &[u8]) -> [u8; 48] {
    let mut bytes = before;
    bytes[..encoded.len()].copy_from_slice(encoded);
    if encoded.len() < old_used {
        bytes[encoded.len()..old_used].fill(0);
    }
    bytes
}

fn variable_case(n: usize, index: usize, cap: usize, name: usize, tags: usize, op: Operation) {
    // Geometry enumeration is exhaustive; payload content is representative.
    // Full-domain content exploration belongs to steps 32 (proptest) and 34 (fuzz).
    for (old_tags, new_tags) in [
        ([0, 0], [0, 0]),
        ([u32::MAX, 1], [1, u32::MAX]),
        ([0x0123_4567, 0x89ab_cdef], [0xfedc_ba98, 0x7654_3210]),
    ] {
        let records = initial(&old_tags);
        let input = RecordInit {
            name: ["", "a", "é"][name],
            tags: &new_tags[..tags],
        };
        let old = wire(&records[..n]);
        let mut expected = records[..n].to_vec();
        match op {
            Operation::Insert => expected.insert(index, input),
            Operation::Replace => expected[index] = input,
            Operation::Remove => {
                expected.remove(index);
            }
            Operation::Pop => {
                expected.pop();
            }
            Operation::Truncate => expected.truncate(index),
            Operation::Clear => expected.clear(),
        }
        let encoded = wire(&expected);
        let before = storage(&old);
        let mut bytes = before;
        {
            let mut store = SliceStore::new(&mut bytes[..cap]);
            {
                let mut edit =
                    Edit::load::<__ConcreteRoot0>(&mut store, Records::DISCRIMINATOR).unwrap();
                let result = {
                    let mut list = blubber::raw::project::<List<Record>>(edit.reborrow());
                    match op {
                        Operation::Insert => list.insert(index, input),
                        Operation::Replace => list.replace(index, input),
                        Operation::Remove => list.remove(index),
                        Operation::Pop => list
                            .pop()
                            .map(|removed| assert_eq!(removed, (n != 0).then_some(()))),
                        Operation::Truncate => list.truncate(index),
                        Operation::Clear => list.clear(),
                    }
                };
                if encoded.len() > cap {
                    assert_eq!(
                        result,
                        Err(ErrorKind::Capacity.into()),
                        "{op:?} n={n} i={index} c={cap}"
                    );
                    assert_eq!(edit.used_len(), Ok(old.len() as u32));
                } else {
                    assert_eq!(result, Ok(()), "{op:?} n={n} i={index} c={cap}");
                    assert_eq!(edit.used_len(), Ok(encoded.len() as u32));
                }
            }
            assert!(!store.is_poisoned());
        }
        assert_eq!(
            bytes,
            if encoded.len() > cap {
                before
            } else {
                expected_storage(before, old.len(), &encoded)
            }
        );
    }
}

fn fixed_case(n: usize, index: usize, cap: usize, insert: bool) {
    for (tags, value) in [
        ([0, 0], 0),
        ([u32::MAX, 1], u32::MAX),
        ([0x0123_4567, 0x89ab_cdef], 0xfedc_ba98),
    ] {
        let old = fixed_wire(&tags[..n]);
        let mut expected = tags[..n].to_vec();
        if insert {
            expected.insert(index, value);
        } else {
            expected.remove(index);
        }
        let encoded = fixed_wire(&expected);
        let before = storage(&old);
        let mut bytes = before;
        {
            let mut store = SliceStore::new(&mut bytes[..cap]);
            {
                let mut edit =
                    Edit::load::<__ConcreteRoot1>(&mut store, Records::DISCRIMINATOR).unwrap();
                let result = {
                    let mut list = blubber::raw::project::<List<u32>>(edit.reborrow());
                    if insert {
                        list.insert(index, value)
                    } else {
                        list.remove(index)
                            .map(|removed| assert_eq!(removed, tags[index]))
                    }
                };
                assert_eq!(
                    result,
                    if encoded.len() > cap {
                        Err(ErrorKind::Capacity.into())
                    } else {
                        Ok(())
                    }
                );
                assert_eq!(
                    edit.used_len(),
                    Ok(if result.is_ok() {
                        encoded.len()
                    } else {
                        old.len()
                    } as u32)
                );
            }
            assert!(!store.is_poisoned());
        }
        assert_eq!(
            bytes,
            if encoded.len() > cap {
                before
            } else {
                expected_storage(before, old.len(), &encoded)
            }
        );
    }
}

// D15 runs use a larger buffer so every admitted input shape also has a
// successful capacity, including three records inserted into a two-record list.
fn bulk_capacities(old: usize, new: usize) -> Vec<usize> {
    let mut caps = vec![old, old.max(new) - 1, old.max(new), 128];
    caps.retain(|cap| (old..=128).contains(cap));
    caps.sort_unstable();
    caps.dedup();
    caps
}

#[allow(clippy::too_many_arguments)] // Finite oracle geometry and operation.
fn bulk_case(
    fixed: bool,
    n: usize,
    index: usize,
    k: usize,
    cap: usize,
    shape: (usize, usize),
    extend: bool,
) {
    for (old_tags, new_tags) in [
        ([0, 0], [0, 0]),
        ([u32::MAX, 1], [1, u32::MAX]),
        ([0x0123_4567, 0x89ab_cdef], [0xfedc_ba98, 0x7654_3210]),
    ] {
        let records = initial(&old_tags);
        let input = RecordInit {
            name: ["", "a", "é"][shape.0],
            tags: &new_tags[..shape.1],
        };
        let inputs = vec![input; k];
        let tags: Vec<_> = (0..k).map(|i| new_tags[i % 2]).collect();
        let old = if fixed {
            fixed_wire(&old_tags[..n])
        } else {
            wire(&records[..n])
        };
        let encoded = if fixed {
            let mut expected = old_tags[..n].to_vec();
            expected.splice(index..index, tags.iter().copied());
            fixed_wire(&expected)
        } else {
            let mut expected = records[..n].to_vec();
            expected.splice(index..index, inputs.iter().copied());
            wire(&expected)
        };
        let mut before = [0xa5; 128];
        before[..old.len()].copy_from_slice(&old);
        let mut bytes = before;
        {
            let mut store = SliceStore::new(&mut bytes[..cap]);
            let result = if fixed {
                let edit =
                    Edit::load::<__ConcreteRoot1>(&mut store, Records::DISCRIMINATOR).unwrap();
                let mut list = blubber::raw::project::<List<u32>>(edit);
                if extend {
                    list.extend_from_slice(&tags)
                } else {
                    list.insert_from_slice(index, &tags)
                }
            } else {
                let edit =
                    Edit::load::<__ConcreteRoot0>(&mut store, Records::DISCRIMINATOR).unwrap();
                let mut list = blubber::raw::project::<List<Record>>(edit);
                if extend {
                    list.extend_from_slice(&inputs)
                } else {
                    list.insert_from_slice(index, &inputs)
                }
            };
            assert_eq!(
                result,
                if encoded.len() > cap {
                    Err(ErrorKind::Capacity.into())
                } else {
                    Ok(())
                },
                "bulk fixed={fixed} n={n} i={index} k={k} cap={cap} shape={shape:?}"
            );
            assert_eq!(
                blubber_core::read_u32(store.bytes(), 8).unwrap() as usize,
                if result.is_ok() {
                    encoded.len()
                } else {
                    old.len()
                }
            );
            assert!(!store.is_poisoned());
        }
        let mut expected = before;
        if encoded.len() <= cap {
            expected[..encoded.len()].copy_from_slice(&encoded);
        }
        assert_eq!(bytes, expected);
        if k == 1 {
            let mut single = before;
            let mut store = SliceStore::new(&mut single[..cap]);
            if fixed {
                let edit =
                    Edit::load::<__ConcreteRoot1>(&mut store, Records::DISCRIMINATOR).unwrap();
                let _ = blubber::raw::project::<List<u32>>(edit).insert(index, tags[0]);
            } else {
                let edit =
                    Edit::load::<__ConcreteRoot0>(&mut store, Records::DISCRIMINATOR).unwrap();
                let _ = blubber::raw::project::<List<Record>>(edit).insert(index, input);
            }
            assert_eq!(single, bytes, "one-element bulk differs from insert");
        }
    }
}

fn admission(fixed: bool, n: usize) {
    let tags = [0x0123_4567, u32::MAX];
    let records = initial(&tags);
    let encoded = if fixed {
        fixed_wire(&tags[..n])
    } else {
        wire(&records[..n])
    };
    for cap in 0..=48 {
        let before = storage(&encoded);
        let mut bytes = before;
        {
            let mut store = SliceStore::new(&mut bytes[..cap]);
            let result = if fixed {
                Edit::load::<__ConcreteRoot1>(&mut store, Records::DISCRIMINATOR)
            } else {
                Edit::load::<__ConcreteRoot0>(&mut store, Records::DISCRIMINATOR)
            };
            match result {
                Ok(edit) => {
                    assert!(cap >= encoded.len());
                    assert_eq!(edit.used_len(), Ok(encoded.len() as u32));
                }
                Err(error) => {
                    assert!(cap < encoded.len());
                    assert_eq!(
                        error,
                        Error::new(
                            if cap < 12 {
                                ErrorKind::Truncated
                            } else {
                                ErrorKind::UsedLength
                            },
                            if cap < 12 { 0 } else { 8 }
                        )
                    );
                }
            }
            assert!(!store.is_poisoned());
        }
        assert_eq!(bytes, before);
    }
}

fn capacities(old: usize, new: usize) -> Vec<usize> {
    let required = old.max(new);
    let mut caps = vec![old, required - 1, required, 48];
    caps.retain(|cap| (old..=48).contains(cap));
    caps.sort_unstable();
    caps.dedup();
    caps
}

/// Run the original 327 geometries/controls and D15 bulk cases, printing names.
#[allow(dead_code)] // list_edits uses only miri_subset.
pub fn run_all() {
    let mut families = [0; 10];
    for fixed in [true, false] {
        for n in 0usize..=2 {
            admission(fixed, n);
            families[0] += 1;
            println!(
                "record_admit_{}_n{n}: PASS",
                if fixed { "fixed" } else { "record" }
            );
        }
    }
    for n in 0usize..=2 {
        for insert in [true, false] {
            let new_n = if insert { n + 1 } else { n.saturating_sub(1) };
            for index in 0..n + usize::from(insert) {
                for cap in capacities(16 + 4 * n, 16 + 4 * new_n) {
                    fixed_case(n, index, cap, insert);
                    families[if insert { 1 } else { 2 }] += 1;
                    println!(
                        "record_fixed_{}_n{n}_i{index}_c{cap}: PASS",
                        if insert { "insert" } else { "remove" }
                    );
                }
            }
        }
        for (family, (name, op)) in [
            ("insert", Operation::Insert),
            ("replace", Operation::Replace),
            ("remove", Operation::Remove),
            ("pop", Operation::Pop),
            ("truncate", Operation::Truncate),
            ("clear", Operation::Clear),
        ]
        .into_iter()
        .enumerate()
        {
            let indices: Vec<usize> = match op {
                Operation::Insert => (0..=n).collect(),
                Operation::Replace | Operation::Remove => (0..n).collect(),
                Operation::Truncate => vec![0, 1, 2, 255],
                _ => vec![0],
            };
            let inputs = if family < 2 { 3 } else { 1 };
            for index in indices {
                for name_len in 0..inputs {
                    for tags in 0..inputs {
                        let tags_data = [0; 2];
                        let records = initial(&tags_data);
                        let old = &records[..n];
                        let mut new = old.to_vec();
                        let input = RecordInit {
                            name: ["", "a", "é"][name_len],
                            tags: &tags_data[..tags],
                        };
                        match op {
                            Operation::Insert => new.insert(index, input),
                            Operation::Replace => new[index] = input,
                            Operation::Remove => {
                                new.remove(index);
                            }
                            Operation::Pop => {
                                new.pop();
                            }
                            Operation::Truncate => new.truncate(index),
                            Operation::Clear => new.clear(),
                        }
                        for cap in capacities(wire(old).len(), wire(&new).len()) {
                            variable_case(n, index, cap, name_len, tags, op);
                            families[family + 3] += 1;
                            println!(
                                "record_{name}_n{n}_i{index}_c{cap}_s{name_len}_t{tags}: PASS"
                            );
                        }
                    }
                }
            }
        }
    }
    let mut bulk_total = 0;
    for fixed in [true, false] {
        for n in 0..=2 {
            for k in 0..=3 {
                for extend in [false, true] {
                    let indices: Vec<_> = if extend { vec![n] } else { (0..=n).collect() };
                    for index in indices {
                        for name in 0..if fixed { 1 } else { 3 } {
                            for tags in 0..if fixed { 1 } else { 3 } {
                                let old = if fixed { 16 + 4 * n } else { [16, 24, 41][n] };
                                let new = if fixed {
                                    old + 4 * k
                                } else {
                                    16 + (n + k).saturating_sub(1) * 4
                                        + [0, 8, 21][n]
                                        + k * (8 + name + 4 * tags)
                                };
                                for cap in bulk_capacities(old, new) {
                                    bulk_case(fixed, n, index, k, cap, (name, tags), extend);
                                    bulk_total += 1;
                                    println!("record_bulk_{}_{}_n{n}_i{index}_k{k}_c{cap}_s{name}_t{tags}: PASS",
                                        if fixed { "fixed" } else { "record" },
                                        if extend { "extend" } else { "insert" });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(bulk_total > 0);
    // Native boundary controls supplement the full-domain Kani guard proofs.
    for count in [0, 1, 2, u32::MAX] {
        for index in [0, 1, 2, 255, u32::MAX as usize, usize::MAX] {
            for insertion in [false, true] {
                let valid = index <= count as usize && (insertion || index < count as usize);
                assert_eq!(
                    bounds::checked_list_index(index, count, insertion),
                    if valid {
                        Ok(index as u32)
                    } else {
                        Err(ErrorKind::IndexOutOfBounds.into())
                    }
                );
            }
            assert_eq!(
                bounds::truncate_len(index, count) as usize,
                index.min(count as usize)
            );
        }
    }
    families[9] = 2;
    println!("record_indices_full_width: PASS");
    println!("record_truncation_full_width: PASS");
    assert_eq!(families, [6, 24, 6, 162, 85, 6, 6, 24, 6, 2]);
    assert_eq!(families.iter().sum::<usize>(), 327);
}

/// Execute shared oracle cases under both Miri alias models in list_edits.
#[allow(dead_code)] // exhaustive_record uses only run_all.
pub fn miri_subset() {
    for (n, index, cap, name, tags, op) in [
        (0, 0, 23, 0, 0, Operation::Insert), // capacity failure
        (0, 0, 48, 2, 2, Operation::Insert),
        (1, 0, 48, 0, 0, Operation::Insert), // 1 -> 2 directory
        (2, 1, 48, 0, 0, Operation::Replace),
        (2, 0, 48, 0, 0, Operation::Remove), // 2 -> 1 elision
        (2, 0, 48, 0, 0, Operation::Pop),
        (2, 1, 48, 0, 0, Operation::Truncate),
        (2, 0, 48, 0, 0, Operation::Clear),
    ] {
        variable_case(n, index, cap, name, tags, op);
    }
    for (fixed, n, index, k, cap, shape, extend) in [
        (true, 2, 1, 3, 128, (0, 0), false),
        (true, 2, 2, 2, 31, (0, 0), true),
        (false, 0, 0, 3, 128, (2, 2), true),
        (false, 1, 0, 2, 128, (0, 0), false),
        (false, 2, 1, 2, 128, (2, 1), false),
        (false, 2, 2, 1, 128, (1, 0), true),
        (false, 2, 0, 0, 41, (0, 0), false),
        (false, 2, 0, 3, 41, (2, 2), false),
    ] {
        bulk_case(fixed, n, index, k, cap, shape, extend);
    }
    fixed_case(1, 0, 23, true);
    fixed_case(2, 0, 48, false);
}
