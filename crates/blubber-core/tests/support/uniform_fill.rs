//! Repeated-value streams compared with individual slot publication.

use blubber_core::fill::ValueEncoder;
use blubber_core::raw::RewritePlan;
use blubber_core::{ErrorKind, KeyDomain, KeyLayout, Layout};
use core::cell::Cell;

struct Encoder {
    calls: Cell<usize>,
    width: usize,
    key: u32,
    fail_at: usize,
    fault: u8,
}
impl ValueEncoder for Encoder {
    fn geometry(&self) -> (u32, u32) {
        (0, self.key)
    }
    fn encode(&self, slot: &mut [u8]) -> blubber_core::Result<usize> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        assert_eq!(slot.len(), self.width);
        slot.fill(b'k' + call as u8);
        if call == self.fail_at {
            match self.fault {
                1 => return Ok(self.width.wrapping_sub(1)),
                2 => return Ok(self.width + 1),
                3 => return Err(ErrorKind::InvalidUtf8.into()),
                4 => panic!("uniform encoder unwind"),
                _ => (),
            }
        }
        Ok(self.width)
    }
}
fn layout(map: bool) -> Layout {
    if map {
        Layout::Map {
            key: KeyLayout {
                width: 1,
                domain: KeyDomain::Raw,
            },
            value: &Layout::Str,
        }
    } else {
        Layout::List {
            element: &Layout::Str,
        }
    }
}
fn source(map: bool, n: usize, width: usize) -> Vec<u8> {
    let mut bytes = (n as u32).to_le_bytes().to_vec();
    for i in 0..n {
        if map {
            bytes.push((2 * i + 1) as u8);
        }
        if map || i + 1 < n {
            bytes.extend_from_slice(&((i + 1) * width).to_le_bytes()[..4]);
        }
    }
    bytes.resize(bytes.len() + n * width, b'a');
    bytes
}

#[test]
pub(super) fn uniform_fill_matches_individual_slots_including_partial_errors() {
    for map in [false, true] {
        for n in 0..=4 {
            for width in 0..=4 {
                for fault in 0..=3 {
                    for fail_at in 0..=n {
                        let old = source(map, n, 1);
                        let plan =
                            RewritePlan::check(&old, &layout(map), n as u32, (n * width) as u32)
                                .unwrap();
                        let mut fast = old.clone();
                        fast.resize(plan.encoded_len() as usize, 0xa5);
                        let mut serial = fast.clone();
                        let make_encoder = || Encoder {
                            calls: Cell::new(0),
                            width,
                            key: u32::from(map),
                            fail_at,
                            fault,
                        };
                        let a = make_encoder();
                        let b = make_encoder();
                        let mut s = plan.begin(&mut fast).unwrap();
                        let actual = s.fill_uniform(width as u32, u32::from(map), &a);
                        let finished = s.finish();
                        let mut s = plan.begin(&mut serial).unwrap();
                        let expected = (|| {
                            for _ in 0..n {
                                s.prepare_slot(width as u32)?;
                                let written = b.encode(s.destination())?;
                                s.advance(written)?;
                            }
                            Ok(())
                        })();
                        assert_eq!(
                            actual, expected,
                            "map={map} n={n} width={width} fault={fault} at={fail_at}"
                        );
                        assert_eq!(finished.is_ok(), expected.is_ok());
                        assert_eq!(fast, serial);
                        assert_eq!(a.calls.get(), b.calls.get());
                        if expected.is_ok() {
                            assert_eq!(a.calls.get(), n);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn uniform_fill_rejects_wrong_geometry_and_used_stream_without_encoding() {
    let old = source(false, 2, 1);
    let plan = RewritePlan::check(&old, &layout(false), 2, 4).unwrap();
    for (width, key) in [(1, 0), (3, 0), (2, 1), (u32::MAX, 0)] {
        let mut bytes = source(false, 2, 2);
        let before = bytes.clone();
        let encoder = Encoder {
            calls: Cell::new(0),
            width: 2,
            key: 0,
            fail_at: 9,
            fault: 0,
        };
        let mut stream = plan.begin(&mut bytes).unwrap();
        assert!(stream.fill_uniform(width, key, &encoder).is_err());
        assert_eq!(encoder.calls.get(), 0);
        assert!(stream.finish().is_err());
        assert_eq!(bytes, before);
    }
    let mut bytes = source(false, 2, 2);
    let encoder = Encoder {
        calls: Cell::new(0),
        width: 2,
        key: 0,
        fail_at: 9,
        fault: 0,
    };
    let mut stream = plan.begin(&mut bytes).unwrap();
    stream.prepare_slot(2).unwrap();
    stream.destination().fill(b'a');
    stream.advance(2).unwrap();
    assert!(stream.fill_uniform(2, 0, &encoder).is_err());
    assert_eq!(encoder.calls.get(), 0);
}

#[test]
pub(super) fn uniform_fill_unwind_matches_individual_prefix_bytes() {
    for map in [false, true] {
        for n in 1..=4 {
            for width in [0, 1, 3] {
                for fail_at in 0..n {
                    let old = source(map, n, 1);
                    let plan = RewritePlan::check(&old, &layout(map), n as u32, (n * width) as u32)
                        .unwrap();
                    let mut fast = old.clone();
                    fast.resize(plan.encoded_len() as usize, 0xa5);
                    let mut serial = fast.clone();
                    let make = || Encoder {
                        calls: Cell::new(0),
                        width,
                        key: u32::from(map),
                        fail_at,
                        fault: 4,
                    };
                    let a = make();
                    let b = make();
                    let actual = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut stream = plan.begin(&mut fast).unwrap();
                        stream
                            .fill_uniform(width as u32, u32::from(map), &a)
                            .unwrap();
                    }));
                    let expected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut stream = plan.begin(&mut serial).unwrap();
                        for _ in 0..n {
                            stream.prepare_slot(width as u32).unwrap();
                            let written = b.encode(stream.destination()).unwrap();
                            stream.advance(written).unwrap();
                        }
                    }));
                    assert!(actual.is_err());
                    assert!(expected.is_err());
                    assert_eq!(a.calls.get(), fail_at + 1);
                    assert_eq!(a.calls.get(), b.calls.get());
                    assert_eq!(fast, serial, "map={map} n={n} width={width} at={fail_at}");
                }
            }
        }
    }
}
