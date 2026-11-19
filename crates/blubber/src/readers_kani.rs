//! Fixed admitted iterator fixtures and the production string bridge for Kani.

#![forbid(unsafe_code)]

use super::*;

/// Exposes the production string bridge to fixed-size Kani queries.
#[doc(hidden)]
pub fn kani_batched_text<'a>(
    text: Option<&'a str>,
    prior: usize,
    chunk: &'a [u8],
    base: u32,
) -> Result<&'a str> {
    batched_text(text, prior, chunk, base)
}

/// Builds the admitted two-item list state for bounded Kani state queries.
#[doc(hidden)]
pub fn kani_batched_list_state(bytes: &[u8; 10], base: u32) -> TryBatchedListIter<'_> {
    assert!(bytes[0] == 2 && bytes[1] == 0 && bytes[2] == 0 && bytes[3] == 0);
    assert!(base <= u32::MAX - 10);
    TryBatchedListIter {
        inner: TryListIter {
            state: ListState::Stream(StreamListState {
                bytes,
                base,
                layout: *Str::LAYOUT,
                payload: 8,
                count: 2,
                next: 0,
                previous: 0,
            }),
            marker: PhantomData,
        },
        text: core::str::from_utf8(&bytes[8..]).ok(),
    }
}

/// Builds the admitted two-entry map state for bounded Kani state queries.
#[doc(hidden)]
pub fn kani_batched_map_state(bytes: &[u8; 16], base: u32) -> TryBatchedMapIter<'_, bool> {
    assert!(bytes[0] == 2 && bytes[1] == 0 && bytes[2] == 0 && bytes[3] == 0);
    assert!(base <= u32::MAX - 16);
    TryBatchedMapIter {
        inner: TryMapIter {
            state: MapState::Stream(StreamMapState {
                bytes,
                base,
                layout: *Str::LAYOUT,
                payload: 14,
                key_at: 4,
                count: 2,
                next: 0,
                previous: 0,
            }),
            marker: PhantomData,
        },
        text: core::str::from_utf8(&bytes[14..]).ok(),
    }
}
