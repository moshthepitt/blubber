//! Owned sorted-map oracle, independent of production offsets and search.

use std::collections::BTreeMap;

/// One operation on an owned encoded-key map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedMapEdit {
    /// Inserts or replaces a value, returning the previous one.
    Insert,
    /// Removes a key if present, returning the previous value.
    Remove,
    /// Replaces a present value, returning the previous one.
    Replace,
    /// Removes all entries.
    Clear,
}

/// A rejected model operation or wire encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedMapError {
    /// A key or value has an incorrect width.
    Width,
    /// A constructor received duplicate keys.
    Duplicate,
    /// Replace selected an absent key.
    Missing,
    /// The count, stride or encoding exceeds the wire domain.
    Overflow,
}

/// Fixed-width encoded keys and values in byte lexicographic order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedMap {
    key_width: usize,
    value_width: usize,
    entries: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl FixedMap {
    /// Constructs a map, checking widths, uniqueness and exact wire size.
    pub fn new(
        key_width: usize,
        value_width: usize,
        entries: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
    ) -> Result<Self, FixedMapError> {
        if key_width == 0
            || value_width == 0
            || key_width > u32::MAX as usize
            || value_width > u32::MAX as usize
        {
            return Err(FixedMapError::Width);
        }
        let mut model = Self {
            key_width,
            value_width,
            entries: BTreeMap::new(),
        };
        for (key, value) in entries {
            if key.len() != key_width || value.len() != value_width {
                return Err(FixedMapError::Width);
            }
            if model.entries.insert(key, value).is_some() {
                return Err(FixedMapError::Duplicate);
            }
        }
        model.check_size(model.entries.len())?;
        Ok(model)
    }

    fn check_size(&self, count: usize) -> Result<(), FixedMapError> {
        if count == 0 {
            return Ok(());
        }
        let size = self
            .key_width
            .checked_add(self.value_width)
            .and_then(|stride| stride.checked_mul(count))
            .and_then(|payload| payload.checked_add(4))
            .ok_or(FixedMapError::Overflow)?;
        if count > u32::MAX as usize || size > u32::MAX as usize {
            return Err(FixedMapError::Overflow);
        }
        Ok(())
    }

    /// Returns the logical key/value map.
    pub fn entries(&self) -> &BTreeMap<Vec<u8>, Vec<u8>> {
        &self.entries
    }

    /// Encodes the count and sorted interleaved key/value entries.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = (self.entries.len() as u32).to_le_bytes().to_vec();
        for (key, value) in &self.entries {
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(value);
        }
        bytes
    }

    /// Applies an operation atomically, returning the exact old value when present.
    pub fn apply(
        &mut self,
        operation: FixedMapEdit,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, FixedMapError> {
        let expected = if matches!(operation, FixedMapEdit::Insert | FixedMapEdit::Replace) {
            self.value_width
        } else {
            0
        };
        if value.len() != expected
            || key.len()
                != if matches!(operation, FixedMapEdit::Clear) {
                    0
                } else {
                    self.key_width
                }
        {
            return Err(FixedMapError::Width);
        }
        match operation {
            FixedMapEdit::Insert => {
                if !self.entries.contains_key(key) {
                    self.check_size(
                        self.entries
                            .len()
                            .checked_add(1)
                            .ok_or(FixedMapError::Overflow)?,
                    )?;
                }
                Ok(self.entries.insert(key.to_vec(), value.to_vec()))
            }
            FixedMapEdit::Remove => Ok(self.entries.remove(key)),
            FixedMapEdit::Replace => {
                let previous = self.entries.get_mut(key).ok_or(FixedMapError::Missing)?;
                Ok(Some(std::mem::replace(previous, value.to_vec())))
            }
            FixedMapEdit::Clear => {
                self.entries.clear();
                Ok(None)
            }
        }
    }
}
