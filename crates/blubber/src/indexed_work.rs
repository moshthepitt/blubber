//! Point-loan coordinates, capability forwarding and string work controls.
use super::*;
use crate::{List, SliceStore, Str};
use core::cell::Cell;

std::thread_local! {
    static STRING_WINDOWS: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn string_window_hit() {
    STRING_WINDOWS.with(|hits| hits.set(hits.get().saturating_add(1)));
}

#[crate::schema(account)]
struct Points {
    prefix: u16,
    list: List<Str>,
}

fn fixture() -> [u8; 128] {
    let mut bytes = [0; 128];
    {
        let mut store = SliceStore::new(&mut bytes);
        Points::initialize(
            &mut store,
            PointsInit {
                prefix: 0x1234,
                list: &["a", "bc", "def"],
            },
        )
        .unwrap();
    }
    bytes
}

#[test]
fn point_loan_retains_wire_geometry() {
    for (index, span) in [26..27, 27..29, 29..32].into_iter().enumerate() {
        let mut bytes = fixture();
        let mut store = SliceStore::new(&mut bytes);
        let edit = AccountEdit::load_trusted::<Points>(
            crate::raw::backend(&mut store),
            Points::DISCRIMINATOR,
        )
        .unwrap()
        .field(1);
        let walk = IndexedWalk::<false>::point(edit, index).unwrap().unwrap();
        assert_eq!(walk.meta.base, 14);
        assert_eq!(walk.state.end, 32);
        assert_eq!(walk.meta.payload, 26);
        assert_eq!(walk.meta.directory, 18);
        assert_eq!(walk.meta.stride, 4);
        assert_eq!(walk.meta.count, 3);
        assert_eq!(walk.state.span, span);
        assert_eq!(walk.state.next, index + 1);
        assert!(walk.state.exhausted);
    }
}

#[test]
fn indexed_loan_forwards_lengths_and_exact_string_writes() {
    let mut bytes = fixture();
    let mut store = SliceStore::new(&mut bytes);
    let edit =
        AccountEdit::load_trusted::<Points>(crate::raw::backend(&mut store), Points::DISCRIMINATOR)
            .unwrap()
            .field(1);
    let mut walk = IndexedWalk::<false>::point(edit, 1).unwrap().unwrap();
    {
        let mut loan = walk.child_loan();
        assert_eq!(loan.used_len(), Ok(32));
        assert_eq!(loan.storage_len(), Ok(128));
        assert_eq!(loan.try_string(b"XY"), Ok(true));
        assert_eq!(loan.read_str(), Ok("XY"));
        assert_eq!(loan.used_len(), Ok(32));
        assert_eq!(loan.storage_len(), Ok(128));
        loan.replace_with(
            |view| {
                assert_eq!(view.bytes(), b"XY");
                Ok((2, ()))
            },
            |(), destination| destination.copy_from_slice(b"UV"),
        )
        .unwrap();
    }
    assert!(!walk.state.dirty);
    assert_eq!(&walk.store.bytes()[27..29], b"UV");
}

#[test]
fn indexed_string_decode_keeps_window_route() {
    let bytes = b"0123456789abcdefghijklmnopqrstuv";
    assert!(blubber_core::raw::indexed_string_window(bytes, 10, 14).is_some());
    STRING_WINDOWS.with(|hits| hits.set(0));
    assert_eq!(decode_indexed_string(bytes, 10..14), Ok("abcd"));
    STRING_WINDOWS.with(|hits| assert_eq!(hits.get(), 1));
}
