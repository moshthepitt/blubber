//! Exact tail admission and streamed slot partition controls.
#![forbid(unsafe_code)]

use blubber_core::raw::{fixed_list_payload, split_fixed_slot};
use blubber_core::{Error, ErrorKind};

#[test]
fn loans_frame_counts_prefixes_envelopes_and_every_spare_byte() {
    for width in [1, 2, 8, 16, 33] {
        for count in 0..=4 {
            for offset in [0, 1, 7] {
                let used = 16 + offset + count * width;
                let mut bytes = [0xa5u8; 256];
                bytes[..8].copy_from_slice(b"walkcore");
                bytes[8..12].copy_from_slice(&(used as u32).to_le_bytes());
                bytes[12 + offset..16 + offset].copy_from_slice(&(count as u32).to_le_bytes());
                let mut expected = bytes;
                let (mut rest, got_used, got_count) =
                    fixed_list_payload(&mut bytes, *b"walkcore", 0, offset as u32, width as u32)
                        .unwrap();
                assert_eq!((got_used, got_count), (used as u32, count as u32));
                for i in 0..count {
                    let (slot, tail) = split_fixed_slot(rest, width);
                    slot.fill(i as u8);
                    expected[16 + offset + i * width..16 + offset + (i + 1) * width].fill(i as u8);
                    rest = tail;
                }
                assert!(rest.is_empty());
                assert_eq!(bytes, expected);
            }
        }
    }
}

#[test]
fn admission_errors_preserve_the_complete_physical_buffer() {
    for (used, count, offset, width, error) in [
        (16, 0, 0, 0, ErrorKind::InvalidSchema.into()),
        (16, u32::MAX, 0, 8, ErrorKind::ArithmeticOverflow.into()),
        (16, 0, u32::MAX, 1, ErrorKind::ArithmeticOverflow.into()),
        (16, 1, 0, 1, Error::new(ErrorKind::Truncated, 16)),
        (17, 0, 0, 1, Error::new(ErrorKind::InvalidBoundary, 16)),
        (16, 0, 1, 1, Error::new(ErrorKind::Truncated, 13)),
    ] {
        let mut bytes = [0xa5u8; 64];
        bytes[..8].copy_from_slice(b"walkcore");
        bytes[8..12].copy_from_slice(&u32::to_le_bytes(used));
        bytes[12..16].copy_from_slice(&u32::to_le_bytes(count));
        let before = bytes;
        assert_eq!(
            fixed_list_payload(&mut bytes, *b"walkcore", 0, offset, width).err(),
            Some(error)
        );
        assert_eq!(bytes, before);
    }
    for len in 0..16 {
        let mut bytes = [0u8; 16];
        let before = bytes;
        assert!(fixed_list_payload(&mut bytes[..len], *b"walkcore", 0, 0, 1).is_err());
        assert_eq!(bytes, before);
    }
}
