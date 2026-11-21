//! Typed projections with short reads and reborrowed in-place edits.

use core::marker::PhantomData;

use crate::edit::ListRequest;

use crate::{
    Edit, Entry, ErrorKind, Fixed, List, ListCursorMut, ListIter, ListRef, Map, MapCursorMut,
    MapIter, MapKey, MapRef, OccupiedEntry, Result, Schema, Str, TryListIter, TryMapIter,
    VacantEntry,
};

/// A copied fixed value editor. Setting never changes its encoded width.
pub struct ScalarMut<'a, T: Fixed> {
    edit: crate::edit_handle::ScalarEdit<'a>,
    marker: PhantomData<T>,
}

impl<'a, T: Fixed> ScalarMut<'a, T> {
    /// Wraps an exclusive selection without reading storage.
    #[doc(hidden)]
    #[inline(always)]
    pub fn from_edit(edit: crate::__private::Projection<'a, T>) -> Self
    where
        T: Schema,
    {
        Self::from_raw(edit.edit)
    }

    #[inline(always)]
    pub(crate) fn from_raw(edit: Edit<'a>) -> Self {
        Self {
            edit: edit.into_scalar(),
            marker: PhantomData,
        }
    }

    /// Temporarily reborrows this selection for another operation.
    #[inline(always)]
    pub fn reborrow(&mut self) -> ScalarMut<'_, T> {
        ScalarMut {
            edit: self.edit.reborrow(),
            marker: PhantomData,
        }
    }

    /// Decodes the current value, panicking on detected corruption or invalid state.
    pub fn get(&self) -> T {
        self.try_get().expect("invalid Blubber scalar")
    }

    /// Decodes the current exact value and checks its domain.
    #[inline(always)]
    pub fn try_get(&self) -> Result<T> {
        self.edit.read_fixed::<T>()
    }

    /// Replaces the value in one checked operation, preserving its width.
    #[inline(always)]
    pub fn set(&mut self, value: T) -> Result<()> {
        self.edit.set_fixed(value)
    }
}

/// An exact UTF-8 region editor with allocation-free replacement and append.
pub struct StrMut<'a> {
    edit: crate::edit_handle::StringEdit<'a>,
}

impl<'a> StrMut<'a> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Self {
        Self {
            edit: edit.into_string(),
        }
    }

    /// Temporarily reborrows this selection for another operation.
    #[inline(always)]
    pub fn reborrow(&mut self) -> StrMut<'_> {
        StrMut {
            edit: self.edit.reborrow(),
        }
    }

    /// Borrows UTF-8, panicking on detected corruption or invalid state.
    #[inline(always)]
    pub fn get(&self) -> &str {
        self.try_get().expect("invalid Blubber string")
    }

    /// Borrows checked UTF-8 until this editor's next exclusive borrow.
    #[inline(always)]
    pub fn try_get(&self) -> Result<&str> {
        self.edit.read_str()
    }

    /// Replaces the complete string in one checked operation.
    #[inline(always)]
    pub fn set(&mut self, value: &str) -> Result<()> {
        if self.edit.try_string(value.as_bytes())? {
            return Ok(());
        }
        self.edit.replace_bytes(value.as_bytes())
    }

    /// Appends UTF-8, retaining the existing prefix by its coordinates.
    ///
    /// No reference into storage survives growth or suffix movement.
    pub fn push_str(&mut self, suffix: &str) -> Result<()> {
        self.edit.replace_with(
            |view| {
                let old_len = view.read::<Str>()?.len();
                let len = crate::schema::checked_add(
                    blubber_core::checked_u32(old_len)?,
                    Str::measure(&suffix)?,
                )?;
                Ok((len, old_len))
            },
            |old_len, destination| destination[old_len..].copy_from_slice(suffix.as_bytes()),
        )
    }

    /// Keeps the prefix through a UTF-8 boundary; larger lengths are a no-op.
    ///
    /// Splitting a code point returns `Utf8Boundary` with no wire offset.
    pub fn truncate(&mut self, byte_len: usize) -> Result<()> {
        self.edit.replace_with(
            |view| {
                let value = view.read::<Str>()?;
                let len = byte_len.min(value.len());
                if !value.is_char_boundary(len) {
                    return Err(ErrorKind::Utf8Boundary.into());
                }
                Ok((len as u32, ()))
            },
            |(), _| {},
        )
    }

    /// Replaces the complete string with an empty encoding.
    pub fn clear(&mut self) -> Result<()> {
        self.set("")
    }
}

/// An option editor; selecting a child consumes the parent's exclusive borrow.
pub struct OptionMut<'a, T: Schema> {
    edit: Edit<'a>,
    marker: PhantomData<T>,
}

impl<'a, T: Schema> OptionMut<'a, T> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Self {
        Self {
            edit,
            marker: PhantomData,
        }
    }

    /// Temporarily reborrows this selection for another operation.
    #[inline(always)]
    pub fn reborrow(&mut self) -> OptionMut<'_, T> {
        OptionMut::new(self.edit.reborrow())
    }

    /// Reads the option, panicking on detected corruption or invalid state.
    pub fn get(&self) -> Option<T::Read<'_>> {
        self.try_get().expect("invalid Blubber option")
    }

    /// Reads the current tag and, when present, the checked payload.
    pub fn try_get(&self) -> Result<Option<T::Read<'_>>> {
        self.edit.view()?.read::<Option<T>>()
    }

    /// Replaces the complete tag and borrowed payload in one operation.
    pub fn set(&mut self, value: Option<T::Init<'_>>) -> Result<()> {
        self.edit.replace_with(
            |_| Ok((<Option<T>>::measure(&value)?, ())),
            |(), destination| <Option<T>>::encode(&value, destination),
        )
    }

    /// Selects a present child, or returns absence without changing storage.
    pub fn get_mut(mut self) -> Result<Option<T::Mut<'a>>> {
        if !self
            .edit
            .select_source(|edit| edit.view()?.option_present())?
        {
            return Ok(None);
        }
        let edit = self.edit.some();
        edit.check_path()?;
        Ok(Some(T::project(crate::projection::Projection::new(edit))))
    }

    /// Selects the present child; absence returns `MissingValue`.
    #[inline(always)]
    pub fn some_mut(self) -> Result<T::Mut<'a>> {
        self.get_mut()?.ok_or(ErrorKind::MissingValue.into())
    }
}

/// An exclusive list selection with short reads and consuming child projections.
///
/// Reborrow to retain this handle across child edits. Every operation resolves
/// its integer path against current storage, including after relocation.
///
/// Lazy indexed edits check the header, selected interval and movement bounds
/// before writing, then check retained ends while patching. Wire faults detected
/// by removed-value validation, admission or patching poison the store, including
/// before writes. A later patch failure can leave changed bytes with the old used
/// length; return that error so the transaction rolls back. Checked ingress
/// retains its full scan.
/// Input, index, capacity and funding errors precede the first write.
pub struct ListMut<'a, T: Schema> {
    edit: Edit<'a>,
    marker: PhantomData<T>,
}

impl<'a, T: Schema> ListMut<'a, T> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Self {
        Self {
            edit,
            marker: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn read(&self) -> Result<ListRef<'_, T>> {
        self.edit.read_view()?.read::<List<T>>()
    }

    /// Temporarily reborrows this list for another operation.
    #[inline(always)]
    pub fn reborrow(&mut self) -> ListMut<'_, T> {
        ListMut::new(self.edit.reborrow())
    }

    /// Returns the checked element count.
    pub fn try_len(&self) -> Result<usize> {
        self.read()?.try_len()
    }

    /// Returns the count, panicking on detected corruption.
    pub fn len(&self) -> usize {
        self.try_len().expect("invalid Blubber list")
    }

    /// Checks whether this list is empty.
    pub fn try_is_empty(&self) -> Result<bool> {
        self.read()?.try_is_empty()
    }

    /// Returns whether the list is empty, panicking on detected corruption.
    pub fn is_empty(&self) -> bool {
        self.try_is_empty().expect("invalid Blubber list")
    }

    /// Reads a child with a borrow of this handle; out-of-range indices are absent.
    pub fn try_get(&self, index: usize) -> Result<Option<T::Read<'_>>> {
        self.read()?.try_get(index)
    }

    /// Reads a child, panicking on detected corruption.
    pub fn get(&self, index: usize) -> Option<T::Read<'_>> {
        self.try_get(index).expect("invalid Blubber list element")
    }

    /// Borrows an iterator over the current list; detected corruption panics.
    #[inline(always)]
    pub fn iter(&self) -> ListIter<'_, T> {
        self.read().expect("invalid Blubber list").iter()
    }

    /// Borrows an iterator which returns its first child fault once.
    /// Parent-path and header faults are returned before iteration starts.
    #[inline(always)]
    pub fn try_iter(&self) -> Result<TryListIter<'_, T>> {
        self.read().map(ListRef::try_iter)
    }

    /// Checks every element and boundary in this selected list.
    pub fn validate(&self) -> Result<()> {
        self.edit.view()?.validate()
    }

    /// Appends a borrowed input without allocating an intermediate encoding.
    pub fn push(&mut self, value: T::Init<'_>) -> Result<()> {
        if self.edit.try_tail::<T>(None, &value)? {
            return Ok(());
        }
        self.edit
            .list_edit::<T, _>(ListRequest::Push, Some(&value), |_| Ok(()))
    }

    /// Inserts before `index`, accepting the position just past the last item.
    ///
    /// An index beyond the count returns `IndexOutOfBounds`. Input and capacity
    /// errors preserve storage. Lazy source faults follow [`ListMut`]'s contract.
    pub fn insert(&mut self, index: usize, value: T::Init<'_>) -> Result<()> {
        self.edit
            .list_edit::<T, _>(ListRequest::Insert(index), Some(&value), |_| Ok(()))
    }

    /// Appends all inputs in one checked plan, without an intermediate allocation.
    ///
    /// Input and capacity errors preserve storage. Lazy source faults follow
    /// [`ListMut`]'s contract.
    pub fn extend_from_slice(&mut self, values: &[T::Init<'_>]) -> Result<()> {
        self.bulk_insert(None, values)
    }

    /// Replaces every element with `value`, preserving the count.
    ///
    /// Use this instead of setting each element through a callback when all
    /// replacements are the same. An empty list stays empty. `InvalidBoundary`
    /// means collection boundaries or encoded lengths do not fit the list.
    /// Return the error so the transaction rolls back. See the
    /// [failure contract](#failure-contract) for storage and codec failures.
    ///
    /// ```
    /// # #[cfg(feature = "derive")] {
    /// use blubber::{List, Map, SliceStore, Str};
    /// #[blubber::schema(account)]
    /// struct State { values: List<Str>, by_id: Map<u8, Str> }
    /// let mut room = [0; 128];
    /// let mut store = SliceStore::new(&mut room);
    /// let mut root = State::initialize(&mut store, StateInit {
    ///     values: &["old", "old"], by_id: &[(1, "a"), (3, "b")],
    /// })?;
    /// root.values_mut().fill("same")?;
    /// root.values_mut().rewrite_from(&["", "longer"])?;
    /// root.by_id_mut().fill_values("same")?;
    /// root.by_id_mut().rewrite_values_from(&[(1, "first"), (3, "last")])?;
    /// assert_eq!(root.values().get(1), Some("longer"));
    /// root.commit()?;
    /// # }
    /// # Ok::<(), blubber::Error>(())
    /// ```
    ///
    /// # Failure contract
    ///
    /// All fill/rewrite methods preserve bytes and used length on preflight
    /// errors. Reserve can grow physical storage before a later failure; the
    /// backend's allocation/reborrow failures can close or poison the store.
    /// An encoding error, wrong byte count or unwind leaves sticky poison and
    /// prevents commit. Rewrite replay errors and inconsistent final totals do too.
    /// Return the error so the transaction rolls back.
    ///
    /// Detected source faults on a lazy mutable path set sticky poison even
    /// when admission returns before a write. Checked admission preserves its
    /// pre-write frame. Caller-input, capacity and funding errors return before
    /// the first write without poisoning the store.
    ///
    /// Fills measure once when nonempty and encode once per element. Rewrites
    /// measure each input at most twice and encode it once, with constant
    /// auxiliary memory. Recursive codecs may measure children separately.
    /// Codecs must replay deterministically; inputs cannot borrow the destination.
    #[inline(always)]
    pub fn fill(&mut self, value: T::Init<'_>) -> Result<()> {
        if matches!(T::FIXED_SIZE,Some(width) if width > 0) {
            return self.edit.fill_fixed_list::<T>(&value);
        }
        self.edit.rewrite_with::<List<T>, _>(
            #[inline(always)]
            |view, admitted| {
                crate::rewrite::prepare_fill_policy::<T>(view, admitted, false, 0, &value)
            },
            #[inline(always)]
            |(count, len), stream| {
                crate::rewrite::encode_fill::<T, crate::List<T>>(&value, count, len, 0, stream)
            },
        )
    }

    /// Replaces all elements with an exact-count borrowed input slice.
    ///
    /// Supply exactly one input per existing element, in ordinal order; this
    /// replaces a loop that sets each element to a different value. A count
    /// mismatch returns `InvalidBoundary` before writes. That error also covers
    /// invalid collection boundaries or inconsistent encoded lengths. Inputs
    /// must live outside the destination. Return the error so the transaction
    /// rolls back. See the [failure contract](ListMut#failure-contract).
    #[inline(always)]
    pub fn rewrite_from(&mut self, values: &[T::Init<'_>]) -> Result<()> {
        self.edit.rewrite_with::<List<T>, _>(
            #[inline(always)]
            |view, admitted| crate::rewrite::prepare_list_policy::<T>(view, admitted, values),
            #[inline(always)]
            |(), stream| {
                for value in values {
                    crate::rewrite::encode_rewrite_slot::<T>(value, T::measure(value)?, stream)?;
                }
                Ok(())
            },
        )
    }

    /// Inserts a run before `index`, accepting the position after the last item.
    ///
    /// Measures the complete input run before any write. An empty run checks
    /// the header and insertion bounds without moving retained ends. Retained
    /// payload bytes move at most once. Lazy source faults follow [`ListMut`]'s
    /// contract; encoding errors or unwind leave the store poisoned.
    pub fn insert_from_slice(&mut self, index: usize, values: &[T::Init<'_>]) -> Result<()> {
        self.bulk_insert(Some(index), values)
    }

    fn bulk_insert(&mut self, index: Option<usize>, values: &[T::Init<'_>]) -> Result<()> {
        if T::FIXED_SIZE.is_none() {
            if let Some(result) = self.edit.try_bulk_insert::<T>(index, values) {
                return result;
            }
        }
        let staged = core::cell::Cell::new(None);
        self.edit.try_replace_with(
            |view| {
                let plan = crate::list_bulk::Insert::prepare::<T>(view, index, values)?;
                let len = plan.encoded_len();
                staged.set(Some(plan));
                Ok((len, ()))
            },
            |(), destination| {
                staged
                    .take()
                    .expect("one measured bulk layout")
                    .encode::<T>(values, destination)
            },
        )
    }

    /// Replaces an existing element, allowing its encoded length to change.
    ///
    /// This discards the old bytes without checking their leaf domains, so a
    /// valid replacement can repair an otherwise structurally valid element.
    pub fn replace(&mut self, index: usize, value: T::Init<'_>) -> Result<()> {
        self.edit
            .list_edit::<T, _>(ListRequest::Replace(index), Some(&value), |_| Ok(()))
    }

    /// Removes an element, copying its native value or returning `()`.
    ///
    /// The complete removed value is checked before any write. No borrowed
    /// view survives compaction; an invalid index returns `IndexOutOfBounds`.
    pub fn remove(&mut self, index: usize) -> Result<T::Removed> {
        self.edit.list_edit::<T, _>(
            ListRequest::Remove(index),
            None,
            #[inline(always)]
            |view| T::read_removed(view.ok_or(ErrorKind::InvalidPlan)?),
        )
    }

    /// Copies an exact validated subtree, then removes it without allocating.
    ///
    /// Destination capacity and removed-value validation fail before either
    /// buffer changes. A successful export can precede a later lazy source
    /// fault; see [`ListMut`]. Only the encoded prefix of `destination` changes;
    /// its remaining bytes are retained.
    pub fn remove_into(&mut self, index: usize, destination: &mut [u8]) -> Result<u32> {
        self.edit.list_remove_into(index, destination)
    }

    /// Appends a borrowed owned value without allocating an Init array.
    #[cfg(feature = "alloc")]
    pub fn push_owned(&mut self, value: &T::Owned) -> Result<()> {
        self.edit.list_encoded::<_>(
            ListRequest::Push,
            || T::measure_owned(value),
            |_| Ok(()),
            |destination| T::encode_owned(value, destination),
        )
    }

    /// Inserts a borrowed owned value directly into a checked plan.
    #[cfg(feature = "alloc")]
    pub fn insert_owned(&mut self, index: usize, value: &T::Owned) -> Result<()> {
        self.edit.list_encoded::<_>(
            ListRequest::Insert(index),
            || T::measure_owned(value),
            |_| Ok(()),
            |destination| T::encode_owned(value, destination),
        )
    }

    /// Replaces an existing element from a borrowed owned value.
    #[cfg(feature = "alloc")]
    pub fn replace_owned(&mut self, index: usize, value: &T::Owned) -> Result<()> {
        self.edit.list_encoded::<_>(
            ListRequest::Replace(index),
            || T::measure_owned(value),
            |_| Ok(()),
            |destination| T::encode_owned(value, destination),
        )
    }

    /// Allocates and decodes the old value before modifying account storage.
    #[cfg(feature = "alloc")]
    pub fn remove_owned(&mut self, index: usize) -> Result<T::Owned> {
        self.edit.list_encoded::<_>(
            ListRequest::Remove(index),
            || Ok(0),
            |view| T::decode_owned(view.ok_or(ErrorKind::InvalidPlan)?),
            |_| {},
        )
    }

    /// Copies and removes the last value, returning absence for an empty list.
    #[cfg(feature = "alloc")]
    pub fn pop_owned(&mut self) -> Result<Option<T::Owned>> {
        self.edit.list_encoded::<_>(
            ListRequest::Pop,
            || Ok(0),
            |view| view.map(T::decode_owned).transpose(),
            |_| {},
        )
    }

    /// Removes and checks the last value, or returns `None` without a change.
    ///
    /// Native fixed codecs return their copied value. Schema views and
    /// variable elements return `Some(())`, including zero-byte elements.
    pub fn pop(&mut self) -> Result<Option<T::Removed>> {
        self.edit.list_edit::<T, _>(
            ListRequest::Pop,
            None,
            #[inline(always)]
            |view| view.map(T::read_removed).transpose(),
        )
    }

    /// Keeps at most `len` elements in one edit; larger lengths are a no-op.
    ///
    /// Discarded elements' leaf domains are left unvisited. Lazy indexed edits
    /// check header, selected endpoint and ancestor geometry; only retained ends
    /// used by movement are checked. Source faults follow [`ListMut`]'s contract.
    pub fn truncate(&mut self, len: usize) -> Result<()> {
        if len == 0 && T::FIXED_SIZE.is_none() {
            if let Some(result) = self.edit.try_clear_list() {
                return result;
            }
        }
        self.edit
            .list_edit::<T, _>(ListRequest::Truncate(len), None, |_| Ok(()))
    }

    /// Discards every element in one edit, leaving a zero count word.
    ///
    /// Lazy indexed edits check header and ancestor geometry, then discard old
    /// ends. A detected header error poisons that session. Checked edits retain
    /// complete source-partition checks. Discarded leaf domains are unvisited.
    pub fn clear(&mut self) -> Result<()> {
        if T::FIXED_SIZE.is_none() {
            if let Some(result) = self.edit.try_clear_list() {
                return result;
            }
        }
        self.edit
            .list_edit::<T, _>(ListRequest::Clear, None, |_| Ok(()))
    }

    /// Consumes this handle to select a child, returning absence out of range.
    ///
    /// For loops over many elements, use [`Self::cursor_mut`],
    /// [`Self::try_for_each_mut`] or [`Self::fill`] to avoid repeating selection.
    /// At n=1000, a fixed-list `get_mut(i)` + `set` loop costs about 856 CU/item
    /// in the mixed-program benchmark; scoped setters cost about 8.2 CU/item.
    /// Selection cost per call stays constant as the fixed list grows.
    pub fn get_mut(mut self, index: usize) -> Result<Option<T::Mut<'a>>> {
        let Some((selected, fixed)) = self.edit.select_source(|edit| {
            let view = edit.view()?;
            let header = view.list_header()?;
            if index >= header.count as usize {
                return Ok(None);
            }
            if let Some(width) = T::FIXED_SIZE {
                let span = header.checked_span(view, index as u32)?;
                let base = view.base();
                let span = blubber_core::Span {
                    start: crate::schema::checked_add(base, span.start)?,
                    end: crate::schema::checked_add(base, span.end)?,
                };
                return Ok(Some((index as u32, Some((span, width)))));
            }
            header.check_element(view, index as u32)?;
            Ok(Some((index as u32, None)))
        })?
        else {
            return Ok(None);
        };
        if let Some((span, width)) = fixed {
            return Ok(Some(T::project(crate::projection::Projection::new(
                self.edit.fixed_child(span, width)?,
            ))));
        }
        let edit = self.edit.index(selected);
        edit.check_path()?;
        Ok(Some(T::project(crate::projection::Projection::new(edit))))
    }

    /// Lends one child to a callback, returning absence when `index` is out of range.
    ///
    /// Equal-size updates reuse the selected span. Size changes use the ordinary
    /// growing edit path and refresh the span before another operation. Prefer
    /// [`Self::get_mut`] when the child needs to live beyond a single callback.
    /// The child and its borrowed reads cannot escape this callback.
    ///
    /// Stops on a selection or callback error. Completed edits remain, including
    /// edits made before the callback returns an error. An absent index does not
    /// call the callback.
    ///
    /// ```
    /// # #![forbid(unsafe_code)]
    /// # #[cfg(feature = "derive")] {
    /// use blubber::{List, SliceStore, Str};
    /// #[blubber::schema(account)]
    /// struct Words { values: List<Str> }
    /// let mut bytes = [0; 128];
    /// let mut store = SliceStore::new(&mut bytes);
    /// let mut root = Words::initialize(&mut store, WordsInit { values: &["old"] })?;
    /// let length = root.values_mut().with_mut(0, |word| {
    ///     word.set("new")?;
    ///     Ok(word.get().len())
    /// })?;
    /// assert_eq!(length, Some(3));
    /// assert_eq!(root.values_mut().with_mut(8, |_| Ok(()))?, None);
    /// let result = root.values_mut().with_mut(0, |word| {
    ///     word.set("done")?;
    ///     Err::<(), blubber::Error>(blubber::ErrorKind::InvalidBoundary.into())
    /// });
    /// assert!(result.is_err());
    /// assert_eq!(root.values().try_get(0)?, Some("done"));
    /// // Host inspection only. An instruction handler should return the error.
    /// root.commit()?;
    /// # }
    /// # Ok::<(), blubber::Error>(())
    /// ```
    #[inline(always)]
    pub fn with_mut<R>(
        &mut self,
        index: usize,
        callback: impl for<'s> FnOnce(&mut T::Mut<'s>) -> Result<R>,
    ) -> Result<Option<R>> {
        if T::FIXED_SIZE.is_some() {
            return self
                .reborrow()
                .get_mut(index)?
                .map(|mut child| callback(&mut child))
                .transpose();
        }
        let Some(mut point) = crate::edit::indexed_walk::IndexedWalk::<false>::point(
            self.edit.reborrow().into_account()?,
            index,
        )?
        else {
            return Ok(None);
        };
        let mut child = T::project(crate::projection::Projection::new(Edit::from_indexed(
            point.child_loan(),
        )));
        callback(&mut child).map(Some)
    }

    /// Selects a child, returning `IndexOutOfBounds` when absent.
    #[inline(always)]
    pub fn at_mut(self, index: usize) -> Result<T::Mut<'a>> {
        self.get_mut(index)?
            .ok_or(ErrorKind::IndexOutOfBounds.into())
    }

    /// Consumes this list to lend children in ordinal order.
    ///
    /// Checks the current path and header before traversal. The cursor retains
    /// the outer count; child edits cannot change it. Reborrow first to keep
    /// this list available after traversal.
    #[inline(always)]
    pub fn cursor_mut(self) -> Result<ListCursorMut<'a, T>> {
        ListCursorMut::new(self.edit)
    }

    /// Visits each element through a fresh, scoped mutable projection.
    ///
    /// Stops at the first selection or callback error. Earlier successful edits
    /// remain, including edits made by a callback before it returns an error.
    /// A callback cannot retain its child or reads from it between calls.
    ///
    /// Growing each element in place through callbacks has no fast path on
    /// indexed lists: about 12,000 CU/item at n=1000. Use [`Self::fill`] or
    /// [`Self::rewrite_from`]. For old-value-dependent edits, stage independent
    /// replacements in caller storage, end all reads, then rewrite. Staging
    /// adds caller storage and CU; inputs cannot borrow this destination.
    #[inline(always)]
    pub fn try_for_each_mut(
        &mut self,
        mut f: impl for<'s> FnMut(&mut T::Mut<'s>) -> Result<()>,
    ) -> Result<()> {
        if let Some(width) = T::FIXED_SIZE {
            if matches!(T::LAYOUT, blubber_core::Layout::Leaf(_)) {
                return self.for_each_leaf(f);
            }
            let mut walk = self.edit.borrow_fixed_list(width)?;
            while !walk.bytes.is_empty() {
                if *walk.poisoned {
                    return Err(ErrorKind::Poisoned.into());
                }
                let mut child = T::project(crate::projection::Projection::new(walk.child::<T>()));
                f(&mut child)?;
            }
            return Ok(());
        }
        let Some(mut walk) = crate::edit::indexed_walk::IndexedWalk::<false>::new(
            self.edit.reborrow().into_account()?,
            None,
        )?
        else {
            return Ok(());
        };
        while let Some(loan) = walk.next()? {
            let mut child =
                T::project(crate::projection::Projection::new(Edit::from_indexed(loan)));
            f(&mut child)?;
        }
        Ok(())
    }

    // Leaf callbacks need a separate frame; generated structs retain the
    // caller's constant inherited field metadata through the inline path.
    #[inline(never)]
    fn for_each_leaf(
        &mut self,
        mut f: impl for<'s> FnMut(&mut T::Mut<'s>) -> Result<()>,
    ) -> Result<()> {
        // Also removes unused non-leaf monomorphizations before callback
        // inlining, so generated/indexed callbacks keep their single use.
        assert!(matches!(T::LAYOUT, blubber_core::Layout::Leaf(_)));
        let width = T::FIXED_SIZE.expect("fixed leaf callback");
        let mut walk = self.edit.borrow_fixed_list(width)?;
        while !walk.bytes.is_empty() {
            if *walk.poisoned {
                return Err(ErrorKind::Poisoned.into());
            }
            let mut child = T::project(crate::projection::Projection::new(walk.child::<T>()));
            f(&mut child)?;
        }
        Ok(())
    }
}

impl<'s, T: Schema> IntoIterator for &'s ListMut<'_, T> {
    type Item = T::Read<'s>;
    type IntoIter = ListIter<'s, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An exclusive sorted-map selection; projections always select values, never keys.
pub struct MapMut<'a, K: MapKey, V: Schema> {
    edit: Edit<'a>,
    marker: PhantomData<(K, V)>,
}

impl<'s, K: MapKey, V: Schema> IntoIterator for &'s MapMut<'_, K, V> {
    type Item = (K, V::Read<'s>);
    type IntoIter = MapIter<'s, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: MapKey, V: Schema> MapMut<'a, K, V> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Self {
        Self {
            edit,
            marker: PhantomData,
        }
    }

    fn read(&self) -> Result<MapRef<'_, K, V>> {
        self.edit.view()?.read::<Map<K, V>>()
    }

    /// Temporarily reborrows this map for another operation.
    #[inline(always)]
    pub fn reborrow(&mut self) -> MapMut<'_, K, V> {
        MapMut::new(self.edit.reborrow())
    }

    /// Returns the checked entry count.
    pub fn try_len(&self) -> Result<usize> {
        self.read()?.try_len()
    }

    /// Returns the count, panicking on detected corruption.
    pub fn len(&self) -> usize {
        self.try_len().expect("invalid Blubber map")
    }

    /// Checks whether this map is empty.
    pub fn try_is_empty(&self) -> Result<bool> {
        self.read()?.try_is_empty()
    }

    /// Returns whether the map is empty, panicking on detected corruption.
    pub fn is_empty(&self) -> bool {
        self.try_is_empty().expect("invalid Blubber map")
    }

    /// Searches and reads a value, borrowing this handle for the result.
    pub fn try_get(&self, key: &K) -> Result<Option<V::Read<'_>>> {
        self.read()?.try_get(key)
    }

    /// Reads a value, panicking on detected corruption.
    pub fn get(&self, key: &K) -> Option<V::Read<'_>> {
        self.try_get(key).expect("invalid Blubber map value")
    }

    /// Searches for a key without decoding its value.
    pub fn try_contains_key(&self, key: &K) -> Result<bool> {
        self.read()?.try_contains_key(key)
    }

    /// Returns key presence, panicking on detected corruption.
    pub fn contains_key(&self, key: &K) -> bool {
        self.try_contains_key(key).expect("invalid Blubber map key")
    }

    /// Borrows an iterator over the current entries; detected corruption panics.
    ///
    /// For hot indexed-map reads, use [`Self::try_for_each_mut`] or
    /// [`Self::cursor_mut`]. See [`MapRef::try_iter`] for measured read-pass costs.
    pub fn iter(&self) -> MapIter<'_, K, V> {
        self.read().expect("invalid Blubber map").iter()
    }

    /// Borrows an iterator which returns its first child fault once.
    /// Parent-path and header faults are returned before iteration starts.
    ///
    /// For hot indexed-map reads, use [`Self::try_for_each_mut`] or
    /// [`Self::cursor_mut`]. At 1,000 entries, diagnostic read passes cost
    /// 158,109 CU for callbacks and 176,229 for cursors, versus this iterator's
    /// 660,767/657,767 CU on v0/v3; all three have overhead subtracted.
    pub fn try_iter(&self) -> Result<TryMapIter<'_, K, V>> {
        Ok(self.read()?.try_iter())
    }

    /// Checks every key, boundary and value in this selected map.
    pub fn validate(&self) -> Result<()> {
        self.edit.view()?.validate()
    }

    /// Replaces every value with `value`, preserving the count and all map keys.
    ///
    /// Lazy fixed maps check geometry without scanning stored key domains or
    /// order. Checked ingress validates those keys before access; raw rewrites
    /// without admitted source evidence validate them during preflight.
    ///
    /// Use this instead of setting each value through a callback when all
    /// replacements are the same. An empty map stays empty. `InvalidBoundary`
    /// means collection boundaries or encoded lengths do not fit the map.
    /// Return the error so the transaction rolls back. See the
    /// [failure contract](ListMut#failure-contract) and [`ListMut::fill`]'s example.
    #[inline(always)]
    pub fn fill_values(&mut self, value: V::Init<'_>) -> Result<()> {
        self.edit.rewrite_with::<Map<K, V>, _>(
            #[inline(always)]
            |view, admitted| {
                crate::rewrite::prepare_fill_policy::<V>(view, admitted, true, K::SIZE, &value)
            },
            #[inline(always)]
            |(count, len), stream| {
                crate::rewrite::encode_fill::<V, crate::Map<K, V>>(
                    &value,
                    count,
                    len,
                    K::SIZE,
                    stream,
                )
            },
        )
    }

    /// Replaces values in the map's current entry order.
    ///
    /// `values[i]` replaces the value of entry `i`; keys and count stay unchanged.
    /// Lazy fixed maps retain key bytes without scanning their domains or order.
    /// Checked ingress validates the keys before access.
    /// Supply exactly one input per entry. A count mismatch returns
    /// `InvalidBoundary` before measurement or writes. That error also covers
    /// invalid boundaries or inconsistent encoded lengths. Use
    /// [`Self::rewrite_values_from`] when supplied keys must also match.
    ///
    /// Preflight finishes before reserve. Each input is measured at most twice
    /// and encoded once; an indexed map stores one end per entry. Auxiliary
    /// memory is constant. Inputs must live outside the destination and replay
    /// deterministically. Second-pass failure or unwind leaves sticky poison.
    /// Return errors so the transaction rolls back. See the
    /// [failure contract](ListMut#failure-contract) for backend failures.
    ///
    /// ```
    /// # #[cfg(feature = "derive")] {
    /// use blubber::{Map, SliceStore, Str};
    /// #[blubber::schema(account)]
    /// struct State { by_id: Map<u8, Str> }
    /// let mut room = [0; 128];
    /// let mut store = SliceStore::new(&mut room);
    /// let mut root = State::initialize(&mut store, StateInit {
    ///     by_id: &[(1, "old"), (3, "old")],
    /// })?;
    /// root.by_id_mut().rewrite_values(&["first", "longer last"])?;
    /// assert_eq!(root.by_id().get(&3), Some("longer last"));
    /// root.commit()?;
    /// # }
    /// # Ok::<(), blubber::Error>(())
    /// ```
    #[inline(always)]
    pub fn rewrite_values(&mut self, values: &[V::Init<'_>]) -> Result<()> {
        self.edit.rewrite_with::<Map<K, V>, _>(
            #[inline(always)]
            |view, admitted| {
                crate::rewrite::prepare_map_values_policy::<K, V>(view, admitted, values)
            },
            #[inline(always)]
            |(), stream| {
                for value in values {
                    crate::rewrite::encode_rewrite_slot::<V>(value, V::measure(value)?, stream)?;
                }
                Ok(())
            },
        )
    }

    /// Replaces values from ordered `(key, input)` pairs matching this map.
    ///
    /// Supply exactly one pair per existing entry, with the same keys in their
    /// current entry order. A valid map has sorted keys. This replaces a loop
    /// that sets each value to a
    /// different replacement; keys and count stay unchanged. A count or key
    /// mismatch returns `InvalidBoundary` before writes. That error also covers
    /// invalid collection boundaries or inconsistent encoded lengths. Inputs
    /// must live outside the destination. Return the error so the transaction
    /// rolls back. See the [failure contract](ListMut#failure-contract).
    #[inline(always)]
    pub fn rewrite_values_from(&mut self, entries: &[(K, V::Init<'_>)]) -> Result<()> {
        self.edit.rewrite_with::<Map<K, V>, _>(
            #[inline(always)]
            |view, admitted| crate::rewrite::prepare_map_policy::<K, V>(view, admitted, entries),
            #[inline(always)]
            |(), stream| {
                for (_, value) in entries {
                    crate::rewrite::encode_rewrite_slot::<V>(value, V::measure(value)?, stream)?;
                }
                Ok(())
            },
        )
    }

    /// Inserts a borrowed value, returning the checked previous value if present.
    ///
    /// On an initially valid map, keys remain canonical, unique and sorted.
    /// Local checks cover the search path and changed neighbors; full validation
    /// checks untouched keys and boundaries. Native fixed values are copied;
    /// variable values and schema views validate their old region and return
    /// `Some(())`. Caller input and capacity errors preserve storage. Detected
    /// source faults poison a lazy session, including read-only rejection.
    // Keep wide inputs and returned values out of the caller's SBF frame.
    #[cfg_attr(target_os = "solana", inline(never))]
    pub fn insert(&mut self, key: K, value: V::Init<'_>) -> Result<Option<V::Removed>> {
        let query = key.encode_key();
        if self.edit.try_fixed_map::<V>(query.as_ref(), &value)? {
            return Ok(None);
        }
        self.insert_planned(query.as_ref(), &value)
    }

    // The v0 fast path leaves the planner's temporaries in a separate frame.
    // V3 reserves a full frame per call, so keep this step in the caller.
    #[cfg_attr(
        all(target_os = "solana", not(target_feature = "static-syscalls")),
        inline(never)
    )]
    #[cfg_attr(target_feature = "static-syscalls", inline(always))]
    fn insert_planned(&mut self, key: &[u8], value: &V::Init<'_>) -> Result<Option<V::Removed>> {
        self.edit
            .map_edit::<V, _>(blubber_core::raw::MapEdit::Insert, key, Some(value), {
                #[inline(always)]
                |view| view.map(V::read_removed).transpose()
            })
    }

    /// Removes a matching key and checks its old value before compaction.
    ///
    /// Missing keys return `None` without changing any bytes. This operation
    /// leaves physical storage capacity unchanged.
    // Keep wide inputs and returned values out of the caller's SBF frame.
    #[cfg_attr(target_os = "solana", inline(never))]
    pub fn remove(&mut self, key: &K) -> Result<Option<V::Removed>> {
        let query = key.encode_key();
        self.edit
            .map_edit::<V, _>(blubber_core::raw::MapEdit::Remove, query.as_ref(), None, {
                #[inline(always)]
                |view| view.map(V::read_removed).transpose()
            })
    }

    /// Extracts a validated encoded value before removing its key and entry.
    ///
    /// Absence leaves both buffers unchanged. Validation and output capacity
    /// errors preserve both buffers. A later lazy patch fault keeps poison armed;
    /// the destination can already contain the validated value. Success changes
    /// only the destination prefix holding the encoded value.
    pub fn remove_into(&mut self, key: &K, destination: &mut [u8]) -> Result<Option<u32>> {
        let query = key.encode_key();
        self.edit.map_remove_into(query.as_ref(), destination)
    }

    /// Inserts a borrowed owned value and returns the checked previous value.
    #[cfg(feature = "alloc")]
    pub fn insert_owned(&mut self, key: K, value: &V::Owned) -> Result<Option<V::Removed>> {
        let query = key.encode_key();
        self.edit.map_encoded::<_>(
            blubber_core::raw::MapEdit::Insert,
            query.as_ref(),
            || V::measure_owned(value),
            {
                #[inline(always)]
                |view| view.map(V::read_removed).transpose()
            },
            |destination| V::encode_owned(value, destination),
        )
    }

    /// Allocates and decodes a present value before compacting the account.
    #[cfg(feature = "alloc")]
    pub fn remove_owned(&mut self, key: &K) -> Result<Option<V::Owned>> {
        let query = key.encode_key();
        self.edit.map_encoded::<_>(
            blubber_core::raw::MapEdit::Remove,
            query.as_ref(),
            || Ok(0),
            {
                #[inline(always)]
                |view| view.map(V::decode_owned).transpose()
            },
            |_| {},
        )
    }

    /// Discards all entries after checking structural extents.
    ///
    /// Discarded value domains are unvisited. The vacated used tail is zeroed;
    /// physical storage capacity and existing spare bytes are retained.
    #[cfg_attr(target_os = "solana", inline(never))]
    pub fn clear(&mut self) -> Result<()> {
        self.edit
            .map_edit::<V, _>(blubber_core::raw::MapEdit::Clear, &[], None, |_| Ok(()))
    }

    /// Borrows this map for an exclusive occupied or vacant selection.
    ///
    /// The map can be reused after the selection's last use. Bind a temporary
    /// map before retaining its entry, or use [`Self::into_entry`]. The
    /// [`Entry`] methods describe when each insertion or edit callback runs.
    #[inline(always)]
    pub fn entry(&mut self, key: K) -> Result<Entry<'_, K, V>> {
        self.reborrow().into_entry(key)
    }

    /// Consumes this map to retain an exclusive occupied or vacant selection.
    ///
    /// Selection checks local extents, probed keys and insertion neighbors. Handles retain
    /// only an owned key and integer path, so value edits may relocate storage.
    /// The selected editor can outlive the map handle.
    ///
    /// ```
    /// use blubber::{MapMut, Result, Str, StrMut};
    ///
    /// fn select(map: MapMut<'_, u16, Str>) -> Result<StrMut<'_>> {
    ///     map.into_entry(2)?.or_insert("two")
    /// }
    /// ```
    pub fn into_entry(mut self, key: K) -> Result<Entry<'a, K, V>> {
        let query = key.encode_key();
        let found = self.edit.select_source(|edit| {
            let view = edit.view()?;
            view.relative(blubber_core::search_map_local(
                view.bytes(),
                &view.layout(),
                query.as_ref(),
            ))
        })?;
        self.edit.reborrow().index(found.index).check_path()?;
        if found.found {
            Ok(Entry::Occupied(OccupiedEntry::new(
                self.edit,
                key,
                found.index,
            )))
        } else {
            Ok(Entry::Vacant(VacantEntry::new(self.edit, key, found.index)))
        }
    }

    /// Borrows this map to select a value, returning absence for a missing key.
    ///
    /// The map can be reused after the child's last use. For a retained child
    /// from a generated field, bind the map first. A temporary such as
    /// `root.labels_mut().get_mut(&key)?` supports an immediate edit within the
    /// same statement. Use [`Self::into_get_mut`] to keep that child longer.
    ///
    /// ```
    /// # #[cfg(feature = "derive")]
    /// # fn main() -> blubber::Result<()> {
    /// use blubber::{ErrorKind, Map, SliceStore, Str};
    ///
    /// #[blubber::schema(account)]
    /// struct Labels { labels: Map<u16, Str> }
    ///
    /// # let mut bytes = [0; 64];
    /// # let mut store = SliceStore::new(&mut bytes);
    /// # let mut root = Labels::initialize(&mut store, LabelsInit { labels: &[(1, "one")] })?;
    /// let mut map = root.labels_mut();
    /// map.get_mut(&1)?.ok_or(ErrorKind::MissingKey)?.push_str("!")?;
    /// map.entry(2)?.or_insert_with(|| "two")?.push_str("!")?;
    /// assert_eq!(map.get_mut(&2)?.ok_or(ErrorKind::MissingKey)?.get(), "two!");
    /// # root.commit()?;
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "derive"))]
    /// # fn main() {}
    /// ```
    #[inline(always)]
    pub fn get_mut(&mut self, key: &K) -> Result<Option<V::Mut<'_>>> {
        self.reborrow().into_get_mut(key)
    }

    /// Consumes this handle to select a value, returning absence for a missing key.
    ///
    /// Selection checks the query and touched keys; global order requires validation.
    /// This works on a temporary generated field. The returned child borrows the
    /// parent for the lifetime of the consumed map.
    ///
    /// For loops over many entries, use [`Self::cursor_mut`],
    /// [`Self::try_for_each_mut`] or [`Self::fill_values`] to avoid repeating
    /// lookup and selection. [`ListMut::get_mut`] gives a measured selection/set
    /// contrast for fixed lists. Map lookups also repeat key checks.
    pub fn into_get_mut(mut self, key: &K) -> Result<Option<V::Mut<'a>>> {
        let Some((selected, fixed)) = self.edit.select_source(|edit| {
            let view = edit.view()?;
            let query = key.encode_key();
            let found = view.relative(blubber_core::search_map_local(
                view.bytes(),
                &view.layout(),
                query.as_ref(),
            ))?;
            if !found.found {
                return Ok(None);
            }
            let header = view.map_header()?;
            if let Some(width) = V::FIXED_SIZE {
                let span = header.checked_span(view, found.index)?;
                let base = view.base();
                let span = blubber_core::Span {
                    start: crate::schema::checked_add(base, span.start)?,
                    end: crate::schema::checked_add(base, span.end)?,
                };
                return Ok(Some((found.index, Some((span, width)))));
            }
            header.check_value(view, found.index)?;
            Ok(Some((found.index, None)))
        })?
        else {
            return Ok(None);
        };
        if let Some((span, width)) = fixed {
            return Ok(Some(V::project(crate::projection::Projection::new(
                self.edit.fixed_child(span, width)?,
            ))));
        }
        let edit = self.edit.index(selected);
        edit.check_path()?;
        Ok(Some(V::project(crate::projection::Projection::new(edit))))
    }

    /// Consumes the map to select a value with [`Self::into_get_mut`], returning
    /// `MissingKey` when absent.
    #[inline(always)]
    pub fn at_mut(self, key: &K) -> Result<V::Mut<'a>> {
        self.into_get_mut(key)?.ok_or(ErrorKind::MissingKey.into())
    }

    /// Consumes this map to lend values with copied keys in sorted order.
    ///
    /// Lazy fixed maps check exact geometry and decode each visited key before
    /// lending its value. Key order and unvisited key domains remain unscanned.
    /// Checked ingress validates keys. Lazy indexed walks check each visited key,
    /// its predecessor and adjacent ends; source faults poison the session.
    /// Value domains are checked when read. Reborrow first
    /// to retain this map.
    #[inline(always)]
    pub fn cursor_mut(self) -> Result<MapCursorMut<'a, K, V>> {
        MapCursorMut::new(self.edit)
    }

    /// Visits each copied key and a fresh, scoped value projection.
    ///
    /// Stops at the first selection or callback error, preserving successful
    /// edits. A callback cannot retain its child or reads from it between calls.
    /// Keys are copied and cannot be changed through a value editor. Fixed
    /// maps use the admission policy described by [`Self::cursor_mut`].
    ///
    /// Growing each value in place through callbacks has no fast path on
    /// indexed maps: about 17,000 CU/item at n=1000. Use [`Self::fill_values`]
    /// or [`Self::rewrite_values`]. For old-value-dependent edits, stage
    /// independent replacement values in caller storage, end all
    /// reads, then rewrite in the existing sorted key order. Staging adds caller
    /// storage and CU; inputs cannot borrow this destination.
    #[inline(never)]
    pub fn try_for_each_mut(
        &mut self,
        mut f: impl for<'s> FnMut(K, &mut V::Mut<'s>) -> Result<()>,
    ) -> Result<()> {
        let Some(width) = V::FIXED_SIZE else {
            return self.for_each_path(&mut f);
        };
        let mut walk = self.edit.borrow_fixed_map::<K>(width)?;
        while !walk.walk.bytes.is_empty() {
            if *walk.walk.poisoned {
                return Err(ErrorKind::Poisoned.into());
            }
            let (key, slot) = walk.native_entry::<K, V>()?;
            let mut child = V::project(crate::projection::Projection::new(Edit::from_slot(slot)));
            f(key, &mut child)?;
        }
        Ok(())
    }

    #[inline(always)]
    fn for_each_path<F>(&mut self, f: &mut F) -> Result<()>
    where
        F: for<'s> FnMut(K, &mut V::Mut<'s>) -> Result<()>,
    {
        let Some(mut walk) = crate::edit::indexed_walk::IndexedWalk::<true>::new(
            self.edit.reborrow().into_account()?,
            Some(K::SIZE),
        )?
        else {
            return Ok(());
        };
        while walk.begin() {
            let key = walk.prepare_map_item::<K>()?;
            let mut child = V::project(crate::projection::Projection::new(Edit::from_indexed(
                walk.child_loan(),
            )));
            f(key, &mut child)?;
        }
        Ok(())
    }
}

impl<T: Fixed> core::fmt::Debug for ScalarMut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ScalarMut")
            .field("schema", &core::any::type_name::<Self>())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for StrMut<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StrMut")
            .field("schema", &core::any::type_name::<Self>())
            .finish_non_exhaustive()
    }
}

impl<T: Schema> core::fmt::Debug for OptionMut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OptionMut")
            .field("schema", &core::any::type_name::<Self>())
            .finish_non_exhaustive()
    }
}

impl<T: Schema> core::fmt::Debug for ListMut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListMut")
            .field("schema", &core::any::type_name::<Self>())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "alloc")]
impl<K: MapKey> MapMut<'_, K, Str> {
    /// Replaces every string using its old value, preserving keys and entry count.
    ///
    /// The map's directory and keys are admitted before any callback runs. The
    /// callback runs once per visited value in entry order. It receives the old
    /// string and the remaining space in a heap buffer of `output_capacity`
    /// bytes. Write the replacement at the start of that slice and return its
    /// length. A returned length larger than the slice produces `Capacity`.
    /// An admitted empty map calls the callback zero times and allocates nothing.
    ///
    /// The old strings are read in one batched pass. Staging allocates the byte
    /// buffer once and tracks end offsets if replacement lengths differ. One
    /// UTF-8 scan checks the packed output, with checked replacement boundaries.
    /// Callback allocations count toward the runtime heap limit too. Callback,
    /// allocation and output errors preserve the account and leave poison clear.
    /// Invalid output returns `InvalidUtf8` with an offset into the staged bytes.
    /// Detected source corruption sets sticky poison on lazy paths. Checked
    /// admission preserves the pre-write frame. See the
    /// [failure contract](ListMut#failure-contract) for capacity, funding and
    /// backend errors.
    pub fn rewrite_values_with(
        &mut self,
        output_capacity: usize,
        transform: impl FnMut(&str, &mut [u8]) -> Result<usize>,
    ) -> Result<()> {
        self.edit.rewrite_with::<Map<K, Str>, _>(
            |view, admitted| {
                crate::rewrite::stage_strings::<K>(view, admitted, output_capacity, transform)
            },
            |staged, stream| staged.encode(stream, K::SIZE),
        )
    }
}

impl<K: MapKey, V: Schema> core::fmt::Debug for MapMut<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MapMut")
            .field("schema", &core::any::type_name::<Self>())
            .finish_non_exhaustive()
    }
}

#[cfg(all(test, feature = "derive"))]
mod tests {
    #[crate::schema(account)]
    struct TextState {
        text: crate::Str,
    }

    #[test]
    fn string_clear_publishes_an_empty_root_and_clears_only_the_old_tail() {
        let mut bytes = [0xa5; 64];
        bytes[..12].fill(0);
        {
            let mut store = crate::SliceStore::new(&mut bytes);
            TextState::initialize(&mut store, TextStateInit { text: "café" })
                .unwrap()
                .commit()
                .unwrap();
        }
        let old_used = TextState::load(&bytes).unwrap().used_len() as usize;
        let before = bytes;
        {
            let mut store = crate::SliceStore::new(&mut bytes);
            let mut root = TextState::load_mut(&mut store).unwrap();
            root.text_mut().clear().unwrap();
            assert_eq!(root.text(), "");
            assert_eq!(root.used_len(), 12);
            root.validate().unwrap();
            root.commit().unwrap();
        }
        assert_eq!(TextState::load(&bytes).unwrap().text(), "");
        assert_eq!(&bytes[..8], &before[..8]);
        assert!(bytes[12..old_used].iter().all(|byte| *byte == 0));
        assert_eq!(&bytes[old_used..], &before[old_used..]);
    }
}
