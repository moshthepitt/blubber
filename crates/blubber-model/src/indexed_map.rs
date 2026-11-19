//! Owned indexed-map oracle with independent count/directory/payload encoding.

use std::collections::BTreeMap;

use crate::{FixedMapEdit, FixedMapError};

/// Fixed-width encoded keys and arbitrary byte values in lexicographic order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedMap {
    key_width: usize,
    entries: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl IndexedMap {
    /// Constructs a map with unique, correctly sized keys and a checked wire size.
    pub fn new(
        key_width: usize,
        entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, FixedMapError> {
        if key_width == 0 || key_width > u32::MAX as usize {
            return Err(FixedMapError::Width);
        }
        let mut result = Self {
            key_width,
            entries: BTreeMap::new(),
        };
        for (key, value) in entries {
            if key.len() != key_width {
                return Err(FixedMapError::Width);
            }
            if result.entries.insert(key, value).is_some() {
                return Err(FixedMapError::Duplicate);
            }
        }
        result.check_size()?;
        Ok(result)
    }

    fn check_size(&self) -> Result<(), FixedMapError> {
        let size = self
            .entries
            .iter()
            .try_fold(4usize, |size, (_, value)| {
                size.checked_add(self.key_width)?
                    .checked_add(4)?
                    .checked_add(value.len())
            })
            .ok_or(FixedMapError::Overflow)?;
        if self.entries.len() > u32::MAX as usize || size > u32::MAX as usize {
            return Err(FixedMapError::Overflow);
        }
        Ok(())
    }

    /// Returns the logical key/value map used by the oracle.
    pub fn entries(&self) -> &BTreeMap<Vec<u8>, Vec<u8>> {
        &self.entries
    }

    /// Encodes every key/end record, including the final end, then the payload.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = (self.entries.len() as u32).to_le_bytes().to_vec();
        let mut end = 0u32;
        for (key, value) in &self.entries {
            end += value.len() as u32;
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(&end.to_le_bytes());
        }
        for value in self.entries.values() {
            bytes.extend_from_slice(value);
        }
        bytes
    }

    /// Applies one edit atomically and returns the old value when present.
    pub fn apply(
        &mut self,
        operation: FixedMapEdit,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, FixedMapError> {
        if key.len()
            != if matches!(operation, FixedMapEdit::Clear) {
                0
            } else {
                self.key_width
            }
            || (!matches!(operation, FixedMapEdit::Insert | FixedMapEdit::Replace)
                && !value.is_empty())
        {
            return Err(FixedMapError::Width);
        }
        let mut next = self.clone();
        let previous = match operation {
            FixedMapEdit::Insert => next.entries.insert(key.to_vec(), value.to_vec()),
            FixedMapEdit::Replace => {
                let old = next.entries.get_mut(key).ok_or(FixedMapError::Missing)?;
                Some(std::mem::replace(old, value.to_vec()))
            }
            FixedMapEdit::Remove => next.entries.remove(key),
            FixedMapEdit::Clear => {
                next.entries.clear();
                None
            }
        };
        next.check_size()?;
        *self = next;
        Ok(previous)
    }
}
