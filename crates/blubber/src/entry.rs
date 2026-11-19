//! Exclusive sorted-map entries with owned keys and integer selections.

use core::marker::PhantomData;

use blubber_core::raw::MapEdit;

use crate::{Edit, ErrorKind, MapKey, Result, Schema};

/// An exclusive occupied or vacant selection in a sorted map.
///
/// `MapMut::entry` borrows the map until the entry's last use.
/// `MapMut::into_entry` consumes it for an account-lifetime selection.
pub enum Entry<'a, K: MapKey, V: Schema> {
    /// The key already has a value.
    Occupied(OccupiedEntry<'a, K, V>),
    /// The key is absent and its insertion ordinal has been checked.
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: MapKey, V: Schema> Entry<'a, K, V> {
    /// Selects the current value, inserting `value` only when the key is absent.
    ///
    /// Occupied entries leave the unused input unmeasured and unencoded. The
    /// returned editor retains the exclusive account borrow through relocation.
    pub fn or_insert(self, value: V::Init<'_>) -> Result<V::Mut<'a>> {
        match self {
            Self::Occupied(entry) => Ok(entry.into_mut()),
            Self::Vacant(entry) => entry.insert(value),
        }
    }

    /// Selects the value, calling `make` once only when the key is absent.
    ///
    /// The initializer may borrow caller inputs; those borrows end after encoding.
    /// Occupied entries do not build, measure or encode the unused initializer.
    ///
    /// ```
    /// # fn example(mut map: blubber::MapMut<'_, u16, blubber::Str>) -> blubber::Result<()> {
    /// let input = String::from("new value");
    /// let mut text = map.entry(7)?.or_insert_with(|| input.as_str())?;
    /// text.push_str("!")?;
    /// # Ok(()) }
    /// ```
    #[inline]
    pub fn or_insert_with<'i, F>(self, make: F) -> Result<V::Mut<'a>>
    where
        F: FnOnce() -> V::Init<'i>,
    {
        match self {
            Self::Occupied(entry) => Ok(entry.into_mut()),
            Self::Vacant(entry) => entry.insert(make()),
        }
    }

    /// Selects the value, constructing its default initializer only on vacancy.
    ///
    /// ```
    /// # fn example(mut map: blubber::MapMut<'_, u16, u16>) -> blubber::Result<()> {
    /// let value = map.entry(7)?.or_default()?;
    /// assert_eq!(value.get(), 0);
    /// # Ok(()) }
    /// ```
    #[inline]
    pub fn or_default<'i>(self) -> Result<V::Mut<'a>>
    where
        V::Init<'i>: Default,
    {
        self.or_insert_with(Default::default)
    }

    /// Calls `modify` once on occupancy, then returns the entry for chaining.
    ///
    /// The callback receives a short exclusive reborrow. Completed edits remain
    /// when it returns an error or unwinds; propagate errors to the caller.
    /// Later value selections resolve the retained ordinal against current bytes,
    /// including after growth or storage relocation inside the callback.
    ///
    /// A chained `or_insert_with` resolves an occupied value again. For hot
    /// paths that retain the modified editor, match on [`Entry::Occupied`] and
    /// call [`OccupiedEntry::into_mut`] once.
    ///
    /// ```
    /// # fn example(mut map: blubber::MapMut<'_, u16, blubber::Str>) -> blubber::Result<()> {
    /// map.entry(7)?
    ///     .and_modify(|text| text.push_str("!"))?
    ///     .or_insert_with(|| "new value")?;
    /// # Ok(()) }
    /// ```
    #[inline]
    pub fn and_modify<F>(mut self, modify: F) -> Result<Self>
    where
        F: for<'s> FnOnce(&mut V::Mut<'s>) -> Result<()>,
    {
        if let Self::Occupied(entry) = &mut self {
            let mut value = V::project(crate::projection::Projection::new(
                entry.edit.reborrow().index(entry.index),
            ));
            modify(&mut value)?;
        }
        Ok(self)
    }
}

/// An existing map value, retaining its key and checked ordinal exclusively.
pub struct OccupiedEntry<'a, K: MapKey, V: Schema> {
    edit: Edit<'a>,
    key: K,
    index: u32,
    marker: PhantomData<V>,
}

impl<'a, K: MapKey, V: Schema> OccupiedEntry<'a, K, V> {
    pub(crate) fn new(edit: Edit<'a>, key: K, index: u32) -> Self {
        Self {
            edit,
            key,
            index,
            marker: PhantomData,
        }
    }

    /// Copies the selected key without borrowing account bytes.
    pub fn key(&self) -> K {
        self.key
    }

    /// Consumes the entry to edit its value, preserving every map key.
    pub fn into_mut(self) -> V::Mut<'a> {
        V::project(crate::projection::Projection::new(
            self.edit.index(self.index),
        ))
    }

    /// Replaces the value and returns its checked copied or discarded old value.
    ///
    /// Measurement, capacity checks and old-value decoding precede all writes.
    #[cfg_attr(target_os = "solana", inline(never))]
    pub fn insert(mut self, value: V::Init<'_>) -> Result<V::Removed> {
        let query = self.key.encode_key();
        self.edit.map_entry::<V, _>(
            MapEdit::Replace,
            query.as_ref(),
            blubber_core::Search {
                index: self.index,
                found: true,
            },
            Some(&value),
            |view| V::read_removed(view.ok_or(ErrorKind::InvalidPlan)?),
        )
    }

    /// Removes the entry, checking and copying its old value before compaction.
    #[cfg_attr(target_os = "solana", inline(never))]
    pub fn remove(mut self) -> Result<V::Removed> {
        let query = self.key.encode_key();
        self.edit.map_entry::<V, _>(
            MapEdit::Remove,
            query.as_ref(),
            blubber_core::Search {
                index: self.index,
                found: true,
            },
            None,
            |view| V::read_removed(view.ok_or(ErrorKind::InvalidPlan)?),
        )
    }
}

/// A missing map key and its insertion ordinal, retaining exclusive access.
pub struct VacantEntry<'a, K: MapKey, V: Schema> {
    edit: Edit<'a>,
    key: K,
    index: u32,
    marker: PhantomData<V>,
}

impl<'a, K: MapKey, V: Schema> VacantEntry<'a, K, V> {
    pub(crate) fn new(edit: Edit<'a>, key: K, index: u32) -> Self {
        Self {
            edit,
            key,
            index,
            marker: PhantomData,
        }
    }

    /// Copies the missing key without borrowing account bytes.
    pub fn key(&self) -> K {
        self.key
    }

    /// Inserts a borrowed input and consumes the entry to edit the new value.
    ///
    /// The key is written by the core planner, outside the value destination.
    /// No reference into storage survives growth or movement.
    pub fn insert(mut self, value: V::Init<'_>) -> Result<V::Mut<'a>> {
        let query = self.key.encode_key();
        self.edit.map_entry::<V, _>(
            MapEdit::Insert,
            query.as_ref(),
            blubber_core::Search {
                index: self.index,
                found: false,
            },
            Some(&value),
            |view| {
                if view.is_some() {
                    return Err(ErrorKind::InvalidPlan.into());
                }
                Ok(())
            },
        )?;
        Ok(V::project(crate::projection::Projection::new(
            self.edit.index(self.index),
        )))
    }
}

impl<K: MapKey, V: Schema> core::fmt::Debug for Entry<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Occupied(entry) => f.debug_tuple("Occupied").field(entry).finish(),
            Self::Vacant(entry) => f.debug_tuple("Vacant").field(entry).finish(),
        }
    }
}

impl<K: MapKey, V: Schema> core::fmt::Debug for OccupiedEntry<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<K: MapKey, V: Schema> core::fmt::Debug for VacantEntry<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VacantEntry")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}
