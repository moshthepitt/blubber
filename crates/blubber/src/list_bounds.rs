//! Full-width list bounds before conversion to wire ordinals.

use blubber_core::{ErrorKind, Result};

/// Checks an ordinal against the wire count before narrowing it.
pub(crate) fn checked_list_index(index: usize, count: u32, insertion: bool) -> Result<u32> {
    let count = count as usize;
    if index > count || (!insertion && index == count) {
        return Err(ErrorKind::IndexOutOfBounds.into());
    }
    Ok(index as u32)
}

/// Clamps a requested length to the wire count before narrowing it.
pub(crate) fn truncate_len(requested: usize, count: u32) -> u32 {
    requested.min(count as usize) as u32
}
