//! Ordered admission matches the retained full-directory implementation.
#![forbid(unsafe_code)]

use blubber_core::raw::{plan_map_edit, InlinePath, MapEdit};
use blubber_core::{KeyDomain, KeyLayout, Layout};

#[test]
fn suffix_admission_keeps_first_error_codes_and_offsets() {
    let root = [2u8, 0, 0, 0, 1, 1, 0, 0, 0, 3, 2, 0, 0, 0, b'a', b'b'];
    let mut original = vec![0x55; 64];
    original[8..12].copy_from_slice(&28u32.to_le_bytes());
    original[12..28].copy_from_slice(&root);
    let mut count = 0u32;
    let mut digest = 0xcbf29ce484222325u64;
    for mutation in 0..14 {
        let mut bytes = original.clone();
        match mutation {
            0 => {}
            1..=3 => {
                bytes[12..16].copy_from_slice(&[0u32, 1, u32::MAX][mutation - 1].to_le_bytes())
            }
            4..=6 => {
                bytes[17..21].copy_from_slice(&[0u32, 3, u32::MAX][mutation - 4].to_le_bytes())
            }
            7..=9 => {
                bytes[22..26].copy_from_slice(&[0u32, 3, u32::MAX][mutation - 7].to_le_bytes())
            }
            10 => bytes[21] = 1,
            11 => bytes[16] = 255,
            12 => bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => bytes[8..12].copy_from_slice(&11u32.to_le_bytes()),
        }
        for domain in [KeyDomain::Raw, KeyDomain::Bool] {
            let layout = Layout::Map {
                key: KeyLayout { width: 1, domain },
                value: &Layout::Str,
            };
            for op in [
                MapEdit::Insert,
                MapEdit::Replace,
                MapEdit::Remove,
                MapEdit::Clear,
            ] {
                for query in [b"".as_slice(), &[1], &[2], &[4]] {
                    for input in [0, 1, u32::MAX] {
                        for limit in [0, bytes.len() as u32, u32::MAX] {
                            let result = plan_map_edit(
                                &bytes,
                                u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
                                &layout,
                                &InlinePath::new(),
                                op,
                                query,
                                input,
                                limit,
                            );
                            let outcome = match result {
                                Ok(plan) => format!("ok {:?}", plan.check_ready(&bytes)),
                                Err(error) => format!("{error:?}"),
                            };
                            for byte in outcome.bytes() {
                                digest = (digest ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                            }
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!((count, digest), (4032, 0xc8d7b929080a2b6d));
}
