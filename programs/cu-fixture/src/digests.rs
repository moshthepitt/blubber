//! Canonical word digests, read dynamically on host and SBF.
use blubber::{List, Map, Pubkey, RootSchema, Str};
use blubber_core::CheckedLayout;

#[allow(dead_code)]
#[path = "../../blubber-demo/src/state.rs"]
mod demo;

#[derive(Clone, Copy, blubber::Fixed)]
struct Packed {
    byte: u8,
    word: u64,
    flag: bool,
    tail: u16,
}
#[blubber::schema]
struct Sentinels {
    before: [u8; 257],
    text: Str,
    packed: Packed,
    after: u32,
}
#[blubber::schema]
struct Generic<T> {
    first: u16,
    value: T,
    tail: Str,
}

/// Number of concrete representative schemas in this build.
pub const fn count() -> usize {
    if cfg!(feature = "depth16") {
        9
    } else {
        8
    }
}

/// Concrete table; references are constructed in code, without static handles.
pub fn table(index: usize) -> Option<(&'static str, CheckedLayout<'static>)> {
    Some(match index {
        0 => ("demo", demo::MyAccount::__layout()),
        1 => ("sentinels", Sentinels::__layout()),
        2 => ("fixed_list", blubber::__layout_of!(List<Packed>)),
        3 => ("indexed_list", blubber::__layout_of!(List<Str>)),
        4 => ("fixed_map", blubber::__layout_of!(Map<Pubkey, u64>)),
        5 => ("indexed_map", blubber::__layout_of!(Map<u16, Str>)),
        6 => (
            "generic",
            blubber::__layout_of!(Generic<Map<Pubkey, Option<Str>>>),
        ),
        7 => (
            "depth8",
            blubber::__layout_of!(List<List<List<List<List<List<List<u64>>>>>>>),
        ),
        #[cfg(feature = "depth16")]
        8 => (
            "depth16",
            blubber::__layout_of!(
                List<
                    List<
                        List<
                            List<
                                List<
                                    List<List<List<List<List<List<List<List<List<List<u64>>>>>>>>>>,
                                >,
                            >,
                        >,
                    >,
                >
            ),
        ),
        _ => return None,
    })
}

/// Hashes canonical little-endian words through dynamic, safe table reads.
/// The input seed prevents replacing the result with a compile-time constant.
pub fn digest(table: CheckedLayout<'_>, seed: u8) -> u64 {
    let words = core::hint::black_box(table.words());
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(seed);
    for value in [words.len() as u32, table.root() as u32]
        .into_iter()
        .chain(words.iter().copied())
    {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}
