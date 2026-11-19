//! Fixed fill call counts, frames, metadata rejection and poison controls.

use blubber::Resize as _;
use blubber::{ErrorKind, List, Map, ReadStorage, Schema, SliceStore};
use core::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
struct Record {
    flag: bool,
    number: u16,
}

#[blubber::schema(account)]
struct Records {
    list: List<Record>,
    map: Map<u8, Record>,
    tail: u32,
}

#[test]
fn fixed_records_preserve_keys_count_used_and_spare() {
    for n in 0..=4 {
        let initial = Record {
            flag: false,
            number: 7,
        };
        let replacement = Record {
            flag: true,
            number: 513,
        };
        let list = [initial; 4];
        let map = [(1, initial), (3, initial), (5, initial), (7, initial)];
        let mut bytes = [0; 128];
        {
            let mut store = SliceStore::new(&mut bytes);
            Records::initialize(
                &mut store,
                RecordsInit {
                    list: &list[..n],
                    map: &map[..n],
                    tail: 123,
                },
            )
            .unwrap()
            .commit()
            .unwrap();
        }
        let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        bytes[used..].fill(0xa5);
        let before = bytes;
        {
            let mut store = SliceStore::new(&mut bytes);
            let mut root = Records::load_mut(&mut store).unwrap();
            root.list_mut().fill(replacement).unwrap();
            root.map_mut().fill_values(replacement).unwrap();
            assert_eq!(root.list().iter().collect::<Vec<_>>(), vec![replacement; n]);
            assert_eq!(
                root.map().iter().collect::<Vec<_>>(),
                map[..n]
                    .iter()
                    .map(|(k, _)| (*k, replacement))
                    .collect::<Vec<_>>()
            );
            assert_eq!(root.tail(), 123);
            root.commit().unwrap();
        }
        assert_eq!(bytes[..12], before[..12]);
        assert_eq!(bytes[used..], before[used..]);
    }
}

struct Input {
    measures: Cell<usize>,
    encodes: Cell<usize>,
    fault: u8,
}
struct Codec;
impl Schema for Codec {
    const KIND: blubber::Kind = <u8 as Schema>::KIND;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = <u8 as Schema>::__Children;
    type Read<'a> = u8;
    type Mut<'a> = blubber::ScalarMut<'a, u8>;
    type Init<'a> = &'a Input;
    type Removed = u8;
    fn read(v: blubber::View<'_>) -> blubber::Result<u8> {
        <u8 as Schema>::read(v)
    }
    fn project(p: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::project::<u8>(blubber::raw::into_edit(p))
    }
    fn read_removed(v: blubber::View<'_>) -> blubber::Result<u8> {
        <u8 as Schema>::read_removed(v)
    }
    fn measure(i: &Self::Init<'_>) -> blubber::Result<u32> {
        i.measures.set(i.measures.get() + 1);
        Ok(if i.fault == 5 { 2 } else { 1 })
    }
    fn encode(_: &Self::Init<'_>, slot: &mut [u8]) {
        slot[0] = 42;
    }
    fn encode_slot(i: &Self::Init<'_>, slot: &mut [u8]) -> blubber::Result<usize> {
        i.encodes.set(i.encodes.get() + 1);
        Self::encode(i, slot);
        match i.fault {
            1 => Ok(0),
            2 => Ok(2),
            3 => Err(ErrorKind::InvalidBoundary.into()),
            4 => panic!("fixed encoder unwind"),
            _ => Ok(1),
        }
    }
    #[cfg(feature = "alloc")]
    type Owned = u8;
    #[cfg(feature = "alloc")]
    fn measure_owned(_: &u8) -> blubber::Result<u32> {
        Ok(1)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(v: &u8, d: &mut [u8]) {
        d[0] = *v;
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(v: blubber::View<'_>) -> blubber::Result<u8> {
        <u8 as Schema>::read(v)
    }
}

blubber::__root_schema!(CodecList, List<Codec>);
blubber::__root_schema!(CodecMap, Map<u8, Codec>);

#[test]
fn fixed_calls_wrong_lengths_errors_and_unwinds_keep_poison() {
    for n in 0..=4 {
        for fault in 0..=5 {
            for mode in 0..2 {
                let mut bytes = [0xa5; 40];
                bytes[..8].copy_from_slice(b"fillmap0");
                let stride = if mode == 0 { 1 } else { 2 };
                let used = 16 + n * stride;
                bytes[8..12].copy_from_slice(&(used as u32).to_le_bytes());
                bytes[12..16].copy_from_slice(&(n as u32).to_le_bytes());
                for i in 0..n {
                    bytes[16 + i * stride] = (i * 2 + 1) as u8;
                }
                let before = bytes;
                let input = Input {
                    measures: Cell::new(0),
                    encodes: Cell::new(0),
                    fault,
                };
                let mut store = SliceStore::new(&mut bytes);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if mode == 0 {
                        let edit = blubber::raw::Edit::load_trusted::<CodecList>(
                            blubber::raw::backend(&mut store),
                            *b"fillmap0",
                        )
                        .unwrap();
                        blubber::raw::project::<List<Codec>>(edit).fill(&input)
                    } else {
                        let edit = blubber::raw::Edit::load_trusted::<CodecMap>(
                            blubber::raw::backend(&mut store),
                            *b"fillmap0",
                        )
                        .unwrap();
                        let mut map = blubber::raw::project::<Map<u8, Codec>>(edit);
                        map.fill_values(&input)
                    }
                }));
                assert_eq!(outcome.is_err(), n > 0 && fault == 4);
                if let Ok(result) = outcome {
                    assert_eq!(result.is_ok(), n == 0 || fault == 0);
                }
                assert_eq!(input.measures.get(), usize::from(n > 0));
                assert_eq!(
                    input.encodes.get(),
                    if n == 0 || fault == 5 {
                        0
                    } else if fault == 0 {
                        n
                    } else {
                        1
                    }
                );
                let poisoned = n > 0 && (1..=4).contains(&fault);
                assert_eq!(store.is_poisoned(), poisoned);
                assert_eq!(store.bytes()[..16], before[..16]);
                assert_eq!(store.bytes()[used..], before[used..]);
                if mode != 0 {
                    for i in 0..n {
                        assert_eq!(store.bytes()[16 + i * stride], before[16 + i * stride]);
                    }
                }
                assert_eq!(
                    blubber::raw::Resize::commit(blubber::raw::backend(&mut store), used as u32)
                        .is_ok(),
                    !poisoned
                );
            }
        }
    }
}

#[test]
fn foreign_geometry_rejects_before_measurement() {
    let input = Input {
        measures: Cell::new(0),
        encodes: Cell::new(0),
        fault: 0,
    };
    let bytes = [1, 0, 0, 0, 1, 2, 9];
    let layout = blubber_core::Layout::Map {
        key: blubber_core::KeyLayout {
            width: 2,
            domain: blubber_core::KeyDomain::Raw,
        },
        value: <u8 as Schema>::LAYOUT,
    };
    let view = blubber::View::new(&bytes, layout, 0).unwrap();
    assert!(
        matches!(blubber::raw::prepare_fill::<Codec>(view,true,true,1,&&input), Err(e) if e == ErrorKind::InvalidSchema.into())
    );
    assert_eq!(input.measures.get(), 0);
    assert_eq!(input.encodes.get(), 0);
    // Codec width mismatch is also rejected before any destination is issued.
    assert!(blubber::raw::prepare_fill::<u16>(view, true, true, 2, &7).is_err());
}
