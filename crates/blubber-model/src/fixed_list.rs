//! Owned fixed-list oracle, independent of the production kernel.

/// A fixed-list operation over owned element bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedListEdit {
    /// Inserts before the given ordinal, including the append position.
    Insert(usize),
    /// Removes an existing ordinal and returns its bytes.
    Remove(usize),
    /// Replaces an existing ordinal.
    Replace(usize),
    /// Returns the last element, if present.
    Pop,
    /// Keeps at most the requested count.
    Truncate(usize),
    /// Removes every element.
    Clear,
}

/// An invalid model operation or unrepresentable wire encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedListError {
    /// An input element has the wrong byte width.
    Width,
    /// An ordinal lies outside the operation's allowed range.
    Index,
    /// The count or complete encoding exceeds the wire domain.
    Overflow,
}

/// A list modeled as owned element byte sequences, without wire offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedList {
    width: usize,
    elements: Vec<Vec<u8>>,
}

impl FixedList {
    /// Checks element widths and wire representability before constructing a list.
    pub fn new(width: usize, elements: Vec<Vec<u8>>) -> Result<Self, FixedListError> {
        if elements.iter().any(|element| element.len() != width) {
            return Err(FixedListError::Width);
        }
        Self::check_size(width, elements.len())?;
        Ok(Self { width, elements })
    }

    fn check_size(width: usize, count: usize) -> Result<(), FixedListError> {
        let size = count
            .checked_mul(width)
            .and_then(|payload| payload.checked_add(4))
            .ok_or(FixedListError::Overflow)?;
        if count > u32::MAX as usize || size > u32::MAX as usize {
            return Err(FixedListError::Overflow);
        }
        Ok(())
    }

    /// Returns the element sequences in logical order.
    pub fn elements(&self) -> &[Vec<u8>] {
        &self.elements
    }

    /// Serializes a count followed by each element, with no directory or padding.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = (self.elements.len() as u32).to_le_bytes().to_vec();
        for element in &self.elements {
            bytes.extend_from_slice(element);
        }
        bytes
    }

    /// Applies one operation atomically and returns the removed element, if any.
    ///
    /// Insert/replace take one complete element; other operations require empty
    /// input. A zero-width removal returns `Some(Vec::new())`.
    pub fn apply(
        &mut self,
        operation: FixedListEdit,
        input: &[u8],
    ) -> Result<Option<Vec<u8>>, FixedListError> {
        let expected = match operation {
            FixedListEdit::Insert(_) | FixedListEdit::Replace(_) => self.width,
            _ => 0,
        };
        if input.len() != expected {
            return Err(FixedListError::Width);
        }
        let count = self.elements.len();
        match operation {
            FixedListEdit::Insert(index) => {
                if index > count {
                    return Err(FixedListError::Index);
                }
                Self::check_size(
                    self.width,
                    count.checked_add(1).ok_or(FixedListError::Overflow)?,
                )?;
                self.elements.insert(index, input.to_vec());
            }
            FixedListEdit::Remove(index) => {
                if index >= count {
                    return Err(FixedListError::Index);
                }
                return Ok(Some(self.elements.remove(index)));
            }
            FixedListEdit::Replace(index) => {
                let value = self.elements.get_mut(index).ok_or(FixedListError::Index)?;
                *value = input.to_vec();
            }
            FixedListEdit::Pop => return Ok(self.elements.pop()),
            FixedListEdit::Truncate(count) => self.elements.truncate(count),
            FixedListEdit::Clear => self.elements.clear(),
        }
        Ok(None)
    }
}
