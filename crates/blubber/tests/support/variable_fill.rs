//! Variable fill call counts, returned lengths and sticky poison.
use blubber::Resize as _;
use blubber::{ErrorKind, List, Map, ReadStorage, Schema, SliceStore};
use core::cell::Cell;
struct Input {
    measures: Cell<usize>,
    encodes: Cell<usize>,
    fault: u8,
    fail_at: usize,
    text: &'static str,
}
struct Codec;
impl Schema for Codec {
    const KIND: blubber::Kind = <blubber::Str as Schema>::KIND;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = <blubber::Str as Schema>::LAYOUT;
    const __LEN: usize = <blubber::Str as Schema>::__LEN;
    const __HEADER: &'static [u32] = <blubber::Str as Schema>::__HEADER;
    type __Children = <blubber::Str as Schema>::__Children;
    type Read<'a> = &'a str;
    type Mut<'a> = blubber::StrMut<'a>;
    type Init<'a> = &'a Input;
    type Removed = ();
    fn read(v: blubber::View<'_>) -> blubber::Result<&str> {
        <blubber::Str as Schema>::read(v)
    }
    fn project(p: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::project::<blubber::Str>(blubber::raw::into_edit(p))
    }
    fn read_removed(v: blubber::View<'_>) -> blubber::Result<()> {
        <blubber::Str as Schema>::read_removed(v)
    }
    fn measure(i: &Self::Init<'_>) -> blubber::Result<u32> {
        i.measures.set(i.measures.get() + 1);
        if i.fault == 5 {
            return Err(ErrorKind::Capacity.into());
        }
        Ok(i.text.len() as u32)
    }
    fn encode(i: &Self::Init<'_>, slot: &mut [u8]) {
        slot.copy_from_slice(i.text.as_bytes());
    }
    fn encode_slot(i: &Self::Init<'_>, slot: &mut [u8]) -> blubber::Result<usize> {
        let call = i.encodes.get();
        i.encodes.set(call + 1);
        Self::encode(i, slot);
        if call == i.fail_at {
            match i.fault {
                1 => return Ok(slot.len().wrapping_sub(1)),
                2 => return Ok(slot.len() + 1),
                3 => return Err(ErrorKind::InvalidUtf8.into()),
                4 => panic!("variable encoder unwind"),
                _ => (),
            }
        }
        Ok(slot.len())
    }
    #[cfg(feature = "alloc")]
    type Owned = String;
    #[cfg(feature = "alloc")]
    fn measure_owned(v: &String) -> blubber::Result<u32> {
        Ok(v.len() as u32)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(v: &String, d: &mut [u8]) {
        d.copy_from_slice(v.as_bytes());
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(v: blubber::View<'_>) -> blubber::Result<String> {
        Ok(v.read_str()?.to_owned())
    }
}

blubber::__root_schema!(CodecList, List<Codec>);
blubber::__root_schema!(CodecMap, Map<u8, Codec>);

#[test]
pub(super) fn variable_calls_errors_and_unwinds_keep_poison() {
    for n in 0..=4usize {
        for map in [false, true] {
            for text in ["", "z", "a longer encoded value"] {
                for fault in 0..=5 {
                    for fail_at in 0..=n {
                        let mut bytes = [0xa5; 256];
                        bytes[..8].copy_from_slice(b"filltext");
                        let mut at = 16;
                        bytes[12..16].copy_from_slice(&(n as u32).to_le_bytes());
                        for i in 0..n {
                            if map {
                                bytes[at] = (i * 2 + 1) as u8;
                                at += 1;
                            }
                            if map || i + 1 < n {
                                bytes[at..at + 4].copy_from_slice(&((i + 1) as u32).to_le_bytes());
                                at += 4;
                            }
                        }
                        bytes[at..at + n].fill(b'a');
                        let used = at + n;
                        bytes[8..12].copy_from_slice(&(used as u32).to_le_bytes());
                        let before = bytes;
                        let input = Input {
                            measures: Cell::new(0),
                            encodes: Cell::new(0),
                            fault,
                            fail_at,
                            text,
                        };
                        let mut store = SliceStore::new(&mut bytes);
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if map {
                                let edit = blubber::raw::Edit::load_trusted::<CodecMap>(
                                    blubber::raw::backend(&mut store),
                                    *b"filltext",
                                )
                                .unwrap();
                                blubber::raw::project::<Map<u8, Codec>>(edit).fill_values(&input)
                            } else {
                                let edit = blubber::raw::Edit::load_trusted::<CodecList>(
                                    blubber::raw::backend(&mut store),
                                    *b"filltext",
                                )
                                .unwrap();
                                blubber::raw::project::<List<Codec>>(edit).fill(&input)
                            }
                        }));
                        let fails =
                            n > 0 && (fault == 5 || ((1..=4).contains(&fault) && fail_at < n));
                        assert_eq!(result.is_err(), n > 0 && fault == 4 && fail_at < n);
                        if let Ok(result) = result {
                            assert_eq!(result.is_err(), fails);
                        }
                        assert_eq!(input.measures.get(), usize::from(n > 0));
                        assert_eq!(
                            input.encodes.get(),
                            if n == 0 || fault == 5 {
                                0
                            } else if (1..=4).contains(&fault) && fail_at < n {
                                fail_at + 1
                            } else {
                                n
                            }
                        );
                        assert_eq!(store.is_poisoned(), fails && fault != 5);
                        if fails {
                            assert_eq!(&store.bytes()[..12], &before[..12]);
                        }
                        if n == 0 || fault == 5 {
                            assert_eq!(store.bytes(), before);
                        }
                    }
                }
            }
        }
    }
}

#[blubber::schema]
struct Group {
    number: u16,
    names: List<blubber::Str>,
    map: Map<u8, blubber::Str>,
    suffix: blubber::Str,
}
#[blubber::schema(account)]
struct Mixed {
    prefix: blubber::Str,
    groups: List<Option<Group>>,
    suffix: List<blubber::Str>,
}

#[test]
fn nested_variable_fills_match_replacement_arrays_and_frame_spare() {
    for n in 0..=4 {
        for text in ["", "b", "longer than every old value"] {
            let names = ["", "é", "abc", "z"];
            let map = [(1, "é"), (3, ""), (5, "abc"), (7, "z")];
            let mut filled = [0u8; 512];
            let mut rewritten = [0u8; 512];
            for bytes in [&mut filled, &mut rewritten] {
                let mut store = SliceStore::new(bytes);
                Mixed::initialize(
                    &mut store,
                    MixedInit {
                        prefix: "prefix",
                        groups: &[
                            None,
                            Some(GroupInit {
                                number: 513,
                                names: &names[..n],
                                map: &map[..n],
                                suffix: "inside suffix",
                            }),
                            None,
                        ],
                        suffix: &["outside suffix", "end"],
                    },
                )
                .unwrap()
                .commit()
                .unwrap();
                let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
                bytes[used..].fill(0xa5);
            }
            for (bytes, uniform) in [(&mut filled, true), (&mut rewritten, false)] {
                let mut store = SliceStore::new(bytes);
                let mut root = Mixed::load_mut(&mut store).unwrap();
                {
                    let mut group = root
                        .groups_mut()
                        .get_mut(1)
                        .unwrap()
                        .unwrap()
                        .some_mut()
                        .unwrap();
                    if uniform {
                        group.names_mut().fill(text).unwrap();
                        group.map_mut().fill_values(text).unwrap();
                    } else {
                        group.names_mut().rewrite_from(&[text; 4][..n]).unwrap();
                        group.map_mut().rewrite_values(&[text; 4][..n]).unwrap();
                    }
                }
                root.commit().unwrap();
            }
            assert_eq!(filled, rewritten, "n={n} text={text}");
        }
    }
}
