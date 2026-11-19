//! Same inputs and current keys must give the same errors and complete bytes.
#![forbid(unsafe_code)]
use blubber::Resize as _;
use blubber::{Map, ReadStorage, SliceStore, Str};

#[blubber::schema(account)]
struct State {
    entries: Map<i16, Str>,
    tail: Str,
}

pub fn check(data: &[u8]) {
    let at = |i: usize| data.get(i).copied().unwrap_or(0) as usize;
    let n = at(0) % 5;
    let initial: Vec<_> = (0..n).map(|i| (i as i16 - 2, "old")).collect();
    let mut left = vec![0xa5; 256];
    left[..12].fill(0);
    let mut right = left.clone();
    {
        let mut store = SliceStore::new(&mut left);
        State::initialize(
            &mut store,
            StateInit {
                entries: &initial,
                tail: "suffix",
            },
        )
        .unwrap();
    }
    right.copy_from_slice(&left);
    // Small capacities exercise capacity errors as well as count rejection.
    let used = u32::from_le_bytes(left[8..12].try_into().unwrap()) as usize;
    let capacity = (used + at(1) % 180).min(left.len());
    left.truncate(capacity);
    right.truncate(capacity);
    for round in 0..16 {
        let requested = if at(2 + round) % 3 == 0 {
            at(3 + round) % 6
        } else {
            n
        };
        let texts: Vec<String> = (0..requested)
            .map(|i| {
                char::from(b'a' + (at(round + i + 4) % 26) as u8)
                    .to_string()
                    .repeat(at(round + i + 5) % 18)
            })
            .collect();
        let values: Vec<&str> = texts.iter().map(String::as_str).collect();
        let mut a = SliceStore::new(&mut left);
        let mut b = SliceStore::new(&mut right);
        let mut root = State::load_mut(&mut a).unwrap();
        // Read current keys, then end reads before either rewrite starts.
        let keys: Vec<i16> = root.entries().iter().map(|(key, _)| key).collect();
        let entries: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(i, value)| (keys.get(i).copied().unwrap_or(100), *value))
            .collect();
        let ra = root.entries_mut().rewrite_values(&values);
        let mut keyed = State::load_mut(&mut b).unwrap();
        let rb = keyed.entries_mut().rewrite_values_from(&entries);
        assert_eq!(ra, rb);
        root.validate().unwrap();
        keyed.validate().unwrap();
        assert_eq!(a.is_poisoned(), b.is_poisoned());
        assert_eq!(a.bytes(), b.bytes());
    }
}
