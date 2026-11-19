//! Prepared ownership, protocol byte frames and publication fault controls.
#![forbid(unsafe_code)]

use blubber_core::encoding_publication::{publish, settle, withdraw};
use blubber_core::encoding_replay::{write, Input, Prepared, Projected};
use core::cell::Cell;

struct CountedEncode<'a> {
    encodes: &'a Cell<usize>,
}

impl Input for CountedEncode<'_> {
    fn measure(&self) -> blubber_core::Result<u32> {
        Ok(2)
    }

    fn encode(&self, destination: &mut [u8]) {
        self.encodes.set(self.encodes.get() + 1);
        destination.copy_from_slice(&[3, 5]);
    }
}

#[test]
fn each_protocol_keeps_one_observable_encode_per_destination() {
    for replay in [false, true] {
        let encodes = Cell::new(0);
        let source = CountedEncode { encodes: &encodes };
        let mut bytes = [0xA5; 4];
        write(&source, &mut bytes, 1, 2, replay).unwrap();
        assert_eq!(encodes.get(), 1);
        assert_eq!(bytes, [0xA5, 3, 5, 0xA5]);
    }
}

#[test]
fn protocols_preserve_empty_utf8_and_both_destination_frames() {
    for text in ["", "a", "λ\0字"] {
        let input = Projected {
            value: blubber_core::encoding_text::text(text),
        };
        let measured = input.measure().unwrap();
        let mut retained = vec![0xA5; text.len() + 4];
        let mut elided = retained.clone();
        write(&input, &mut retained, 2, measured, true).unwrap();
        write(&input, &mut elided, 2, measured, false).unwrap();
        assert_eq!(retained, elided);
        assert_eq!(&retained[2..2 + text.len()], text.as_bytes());
        assert_eq!(&retained[..2], &[0xA5; 2]);
        assert_eq!(&retained[2 + text.len()..], &[0xA5; 2]);
    }
}

#[test]
fn owned_preparation_survives_destination_relocation() {
    let source = [3u8, 5, 7, 11];
    let prepared = Prepared::new(Projected {
        value: blubber_core::encoding_leaf::raw(&source),
    })
    .unwrap();
    let mut destination = Vec::with_capacity(1);
    destination.resize(4096, 0xA5);
    let (input, measured) = prepared.replay().unwrap();
    write(&input, &mut destination, 17, measured, true).unwrap();
    assert_eq!(&destination[17..21], &source);
    assert!(destination[..17].iter().all(|byte| *byte == 0xA5));
    assert!(destination[21..].iter().all(|byte| *byte == 0xA5));
}

#[test]
fn publication_waits_for_encoding_acquisition_and_admission() {
    for encoded in [Ok(23), Err(31)] {
        for acquired in [Ok(()), Err(37)] {
            for admitted in [Ok(7), Err(41)] {
                let mut new = true;
                let mut prefix = Some(5);
                withdraw(&mut new, &mut prefix);
                assert!(!new && prefix.is_none());
                let result = settle(encoded, acquired).and_then(|receipt| {
                    publish(&mut prefix, admitted)?;
                    Ok(receipt)
                });
                let expected = encoded.and(acquired).and(admitted).map(|_| 23);
                assert_eq!(result, expected);
                assert_eq!(prefix.is_some(), result.is_ok());
            }
        }
    }
}

struct RejectedWithCleanup<'a> {
    drops: &'a Cell<usize>,
    panic_on_drop: bool,
}

impl Input for RejectedWithCleanup<'_> {
    fn measure(&self) -> blubber_core::Result<u32> {
        Err(blubber_core::Error::new(
            blubber_core::ErrorKind::InvalidBoundary,
            17,
        ))
    }

    fn encode(&self, destination: &mut [u8]) {
        destination.copy_from_slice(&[3, 5]);
    }
}

impl Drop for RejectedWithCleanup<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if self.panic_on_drop {
            panic!("caller cleanup");
        }
    }
}

#[test]
fn failed_preparation_retains_caller_cleanup_and_unwind() {
    for panic_on_drop in [false, true] {
        let drops = Cell::new(0);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Prepared::new(RejectedWithCleanup {
                drops: &drops,
                panic_on_drop,
            })
            .err()
        }));
        assert_eq!(drops.get(), 1);
        if panic_on_drop {
            assert!(outcome.is_err());
        } else {
            assert_eq!(
                outcome.unwrap(),
                Some(blubber_core::Error::new(
                    blubber_core::ErrorKind::InvalidBoundary,
                    17
                ))
            );
        }
    }
}
