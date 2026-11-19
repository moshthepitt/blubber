//! Finite work checks for the complete unit-test mutation oracle.
use super::*;

fn fail_progress(message: &str) -> ! {
    std::eprintln!("{message}");
    // A caught unwind must not turn a violated work contract into a passing test.
    std::process::exit(101)
}

pub(super) struct SearchProbes(u8);

impl SearchProbes {
    pub(super) fn new() -> Self {
        Self(32)
    }

    pub(super) fn step(&mut self) {
        self.0 = self
            .0
            .checked_sub(1)
            .unwrap_or_else(|| fail_progress("map point search exceeded 32 probes"));
    }
}

pub(super) fn assert_progress(before: Option<usize>, after: Option<usize>, emitted: bool) {
    match (emitted, before, after) {
        (true, Some(before), Some(after)) if after >= before => {
            fail_progress("iterator emitted an item without consuming a slot");
        }
        _ => {}
    }
}

pub(super) fn list_remaining(state: &ListState<'_>) -> Option<usize> {
    match state {
        ListState::Fixed(_) => None,
        ListState::Stream(state) => Some((state.count - state.next) as usize),
        ListState::Indexed { header, next, .. } => Some((header.count - next) as usize),
        ListState::Error(error) => Some(usize::from(error.is_some())),
    }
}

pub(super) fn infallible_list_remaining<T: Schema>(
    state: &InfallibleListState<'_, T>,
) -> Option<usize> {
    match state {
        InfallibleListState::Fixed(_) => None,
        InfallibleListState::Other(iter) => list_remaining(&iter.state),
    }
}

pub(super) fn map_remaining(state: &MapState<'_>) -> Option<usize> {
    match state {
        MapState::Fixed(_) => None,
        MapState::Stream(state) => Some((state.count - state.next) as usize),
        MapState::Indexed { header, next, .. } => Some((header.count - next) as usize),
        MapState::Error(error) => Some(usize::from(error.is_some())),
    }
}

pub(super) fn infallible_map_remaining<K: MapKey, V: Schema>(
    state: &InfallibleMapState<'_, K, V>,
) -> Option<usize> {
    match state {
        InfallibleMapState::Fixed(_) => None,
        InfallibleMapState::Other(iter) => map_remaining(&iter.state),
    }
}

#[test]
fn fixed_map_iterator_decodes_and_exhausts_within_its_count() {
    let bytes = [2, 0, 0, 0, 1, 7, 0, 3, 9, 0];
    let map = View::new(&bytes, <crate::Map<u8, u16>>::LAYOUT, 19)
        .unwrap()
        .read::<crate::Map<u8, u16>>()
        .unwrap();
    let mut iter = map.try_iter();
    assert_eq!(iter.size_hint(), (1, Some(2)));
    assert_eq!(iter.next(), Some(Ok((1, 7))));
    assert_eq!(iter.size_hint(), (1, Some(1)));
    assert_eq!(iter.next(), Some(Ok((3, 9))));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}

#[test]
fn stream_list_size_hint_tracks_remaining_slots() {
    let bytes = [3, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, b'a', b'b', b'c'];
    let list = View::new(&bytes, <crate::List<Str>>::LAYOUT, 23)
        .unwrap()
        .read::<crate::List<Str>>()
        .unwrap();
    let mut iter = list.try_iter();
    assert_eq!(iter.size_hint(), (1, Some(3)));
    assert_eq!(iter.next(), Some(Ok("a")));
    assert_eq!(iter.size_hint(), (1, Some(2)));
    assert_eq!(iter.next(), Some(Ok("")));
    assert_eq!(iter.size_hint(), (1, Some(1)));
    assert_eq!(iter.next(), Some(Ok("bc")));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}

#[test]
fn foreign_list_size_hint_tracks_remaining_slots() {
    let bytes = [3, 0, 0, 0, b'a', b'b', b'c', b'd', b'e', b'f'];
    let list = View::new(&bytes, <crate::List<u16>>::LAYOUT, 23)
        .unwrap()
        .read::<crate::List<Str>>()
        .unwrap();
    let mut iter = list.try_iter();
    assert_eq!(iter.size_hint(), (1, Some(3)));
    assert_eq!(iter.next(), Some(Ok("ab")));
    assert_eq!(iter.size_hint(), (1, Some(2)));
    assert_eq!(iter.next(), Some(Ok("cd")));
    assert_eq!(iter.size_hint(), (1, Some(1)));
    assert_eq!(iter.next(), Some(Ok("ef")));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}
