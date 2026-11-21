//! Copyable collection readers and iterators over exact borrowed regions.

use core::{cmp::Ordering, iter::FusedIterator, marker::PhantomData};

use crate::view::{ListHeader, MapHeader};
use crate::{ErrorKind, Fixed, MapKey, Result, Schema, Str, View};
#[cfg(feature = "alloc")]
use crate::{List, Map};

#[cfg(test)]
#[path = "reader_work.rs"]
mod work;

/// A borrowed list with checked, direct element selection.
///
/// `try_` methods report detected corruption. Their infallible counterparts
/// panic on corruption; absence alone produces `None`. Unvisited boundaries
/// and children are certified only by [`Self::validate`].
pub struct ListRef<'a, T: Schema> {
    view: View<'a>,
    fixed_width: Option<u32>,
    marker: PhantomData<T>,
}

impl<T: Schema> Copy for ListRef<'_, T> {}
impl<T: Schema> Clone for ListRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T: Schema> IntoIterator for ListRef<'a, T> {
    type Item = T::Read<'a>;
    type IntoIter = ListIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Schema> ListRef<'a, T> {
    /// Checks the count and complete table/payload extent without scanning ends.
    #[doc(hidden)]
    #[inline(always)]
    pub fn from_view(view: View<'a>) -> Result<Self> {
        let fixed_width = view.list_width_for::<T>()?;
        Ok(Self {
            view,
            fixed_width,
            marker: PhantomData,
        })
    }

    /// Returns the checked element count without visiting elements.
    pub fn try_len(self) -> Result<usize> {
        Ok(self.view.walk_list_header()?.count as usize)
    }

    /// Returns the count, panicking on a malformed header or extent.
    pub fn len(self) -> usize {
        self.try_len().expect("invalid Blubber list")
    }

    /// Checks whether the list is empty, reporting malformed extents.
    pub fn try_is_empty(self) -> Result<bool> {
        Ok(self.try_len()? == 0)
    }

    /// Returns whether the list is empty, panicking on detected corruption.
    pub fn is_empty(self) -> bool {
        self.try_is_empty().expect("invalid Blubber list")
    }

    /// Reads one element after checking its exact selected boundaries.
    ///
    /// An index outside the count returns `Ok(None)`, including indices that
    /// cannot fit the wire index type. Header faults still take precedence.
    #[inline(always)]
    pub fn try_get(self, index: usize) -> Result<Option<T::Read<'a>>> {
        if let Some(width) = self
            .fixed_width
            .filter(|width| Some(*width) == T::FIXED_SIZE)
        {
            let count = blubber_core::raw::indexed_read_word(self.view.bytes(), 0);
            if index >= count as usize {
                return Ok(None);
            }
            let (bytes, offset) =
                blubber_core::point_read::fixed_region(self.view.bytes(), width, index as u32);
            let layout = if T::__READ_REGION_NEEDS_LAYOUT {
                self.view.list_element_layout()?
            } else {
                T::LAYOUT.into()
            };
            return T::read_region(
                bytes,
                layout,
                blubber_core::raw::fixed_chunk_end(self.view.base(), offset as usize),
            )
            .map(Some);
        }
        let header = self.view.walk_list_header()?;
        if index >= header.count as usize {
            return Ok(None);
        }
        T::read(header.element(self.view, index as u32)?).map(Some)
    }

    /// Returns an element or absence, panicking on detected corruption.
    pub fn get(self, index: usize) -> Option<T::Read<'a>> {
        self.try_get(index).expect("invalid Blubber list element")
    }

    /// Iterates checked elements, yielding one error before permanent exhaustion.
    #[inline(always)]
    pub fn try_iter(self) -> TryListIter<'a, T> {
        if self.view.bytes().len() == 4
            && (self.fixed_width.is_some()
                || blubber_core::raw::indexed_read_word(self.view.bytes(), 0) == 0)
        {
            return TryListIter {
                state: ListState::Error(None),
                marker: PhantomData,
            };
        }
        self.try_iter_nonempty()
    }

    #[inline(always)]
    fn try_iter_nonempty(self) -> TryListIter<'a, T> {
        // The immutable view keeps the constructor's exact extent admission.
        if self.fixed_width.is_some() && self.fixed_width == T::FIXED_SIZE {
            return TryListIter {
                state: ListState::Fixed(FixedListState {
                    remaining: &self.view.bytes()[4..],
                    end: blubber_core::raw::fixed_chunk_end(
                        self.view.base(),
                        self.view.bytes().len(),
                    ),
                    layout: if T::__READ_REGION_NEEDS_LAYOUT {
                        Some(
                            self.view
                                .list_element_layout()
                                .expect("checked list descriptor"),
                        )
                    } else {
                        None
                    },
                }),
                marker: PhantomData,
            };
        }
        let state = match self.view.walk_list_header() {
            Ok(header) if header.width.is_none() => ListState::Stream(StreamListState {
                bytes: self.view.bytes(),
                base: self.view.base(),
                layout: header.element,
                payload: header.payload as usize,
                count: header.count,
                next: 0,
                previous: 0,
            }),
            Ok(header) => ListState::Indexed {
                view: self.view,
                header,
                next: 0,
            },
            Err(error) => ListState::Error(Some(error)),
        };
        TryListIter {
            state,
            marker: PhantomData,
        }
    }

    /// Iterates elements, panicking on the first detected corruption.
    #[inline(always)]
    pub fn iter(self) -> ListIter<'a, T> {
        if self.fixed_width.is_some() && self.fixed_width == T::FIXED_SIZE {
            return ListIter {
                state: InfallibleListState::Fixed(FixedListState {
                    remaining: &self.view.bytes()[4..],
                    end: blubber_core::raw::fixed_chunk_end(
                        self.view.base(),
                        self.view.bytes().len(),
                    ),
                    layout: if T::__READ_REGION_NEEDS_LAYOUT {
                        Some(
                            self.view
                                .list_element_layout()
                                .expect("checked list descriptor"),
                        )
                    } else {
                        None
                    },
                }),
            };
        }
        ListIter {
            state: InfallibleListState::Other(self.try_iter()),
        }
    }

    /// Validates every directory end and recursively validates every element.
    pub fn validate(self) -> Result<()> {
        self.view.validate()
    }

    /// Validates and copies all elements using fallible recursive allocation.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> Result<<List<T> as Schema>::Owned> {
        List::<T>::decode_owned(self.view)
    }
}

impl<'a> ListRef<'a, Str> {
    /// Borrows the complete packed string payload after checking aggregate UTF-8.
    ///
    /// Checks the string-list schema, header extent and aggregate text. An item
    /// end inside a multibyte character can pass the aggregate check. Use
    /// [`Self::try_iter`] for individual items and [`Self::validate`] to check
    /// the complete list.
    ///
    /// Work is O(payload bytes), with constant header work and no allocation.
    /// An empty list still checks its header and returns `""`. Errors
    /// use the same account-relative offsets as other checked readers; an
    /// invalid aggregate reports its first invalid UTF-8 byte.
    ///
    /// ```
    /// # #![forbid(unsafe_code)]
    /// use blubber::{Error, ErrorKind, List, Schema, Str, View};
    /// // Two items with an end inside the UTF-8 encoding of "é".
    /// let bytes = [2, 0, 0, 0, 2, 0, 0, 0, b'a', 0xc3, 0xa9];
    /// let list = View::new(&bytes, *List::<Str>::LAYOUT, 12)?
    ///     .read::<List<Str>>()?;
    /// assert_eq!(list.try_concat_str()?, "aé");
    /// assert!(matches!(list.try_iter().next(), Some(Err(Error::Fault {
    ///     kind: ErrorKind::InvalidUtf8, ..
    /// }))));
    /// assert!(list.validate().is_err());
    /// # Ok::<(), blubber::Error>(())
    /// ```
    #[inline(always)]
    pub fn try_concat_str(self) -> Result<&'a str> {
        let element = self.view.list_element_layout()?;
        if element.tag() != 5 {
            return Err(ErrorKind::InvalidSchema.into());
        }
        // Construction already admitted the immutable complete header. Four
        // bytes encode either an empty list or one empty string.
        if self.view.bytes().len() == 4 {
            return Str::read_region(
                &self.view.bytes()[4..],
                element,
                blubber_core::raw::fixed_chunk_end(self.view.base(), 4),
            );
        }
        let (bytes, start) = self
            .view
            .relative(blubber_core::concat_read::payload(self.view.bytes()))?;
        // View admission bounds base + the entire region, including this start.
        Str::read_region(bytes, element, self.view.base() + start)
    }

    /// Iterates strings after scanning the packed payload once for UTF-8.
    ///
    /// Setup costs O(payload bytes), even for partial traversal. Use
    /// [`Self::try_iter`] for lazy reads. Invalid aggregate text or a boundary
    /// inside a character uses ordinary per-item conversion, preserving accepted
    /// prefixes and the first error's exact offset. Errors are yielded once.
    ///
    /// ```
    /// use blubber::{List, Schema, Str, View};
    /// let bytes = [2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b'];
    /// let list = View::new(&bytes, *List::<Str>::LAYOUT, 12)?.read::<List<Str>>()?;
    /// assert_eq!(list.try_iter().take(1).collect::<blubber::Result<Vec<_>>>()?, ["a"]);
    /// // Batched setup scans both payload bytes even though only one item is used.
    /// assert_eq!(list.try_iter_batched().take(1).collect::<blubber::Result<Vec<_>>>()?, ["a"]);
    /// # Ok::<(), blubber::Error>(())
    /// ```
    #[inline(always)]
    pub fn try_iter_batched(self) -> TryBatchedListIter<'a> {
        let inner = self.try_iter();
        let text = match &inner.state {
            ListState::Stream(state) => core::str::from_utf8(&state.bytes[state.payload..]).ok(),
            _ => None,
        };
        TryBatchedListIter { inner, text }
    }
}

/// A borrowed sorted map with bounded binary search and checked value selection.
///
/// Reads check only probed keys and selected value boundaries. Call
/// [`Self::validate`] to certify all key domains, uniqueness and ordering.
/// Infallible methods panic on detected corruption.
pub struct MapRef<'a, K: MapKey, V: Schema> {
    view: View<'a>,
    header: MapHeader,
    marker: PhantomData<(K, V)>,
}

impl<K: MapKey, V: Schema> Copy for MapRef<'_, K, V> {}
impl<K: MapKey, V: Schema> Clone for MapRef<'_, K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, K: MapKey, V: Schema> IntoIterator for MapRef<'a, K, V> {
    type Item = (K, V::Read<'a>);
    type IntoIter = MapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: MapKey, V: Schema> MapRef<'a, K, V> {
    /// Checks the count and complete table/payload extent without scanning ends.
    #[doc(hidden)]
    #[inline(always)]
    pub fn from_view(view: View<'a>) -> Result<Self> {
        let header = view.map_header()?;
        Ok(Self {
            view,
            header,
            marker: PhantomData,
        })
    }

    /// Returns the checked entry count without visiting keys or values.
    pub fn try_len(self) -> Result<usize> {
        Ok(self.header.count as usize)
    }

    /// Returns the count, panicking on a malformed header or extent.
    pub fn len(self) -> usize {
        self.try_len().expect("invalid Blubber map")
    }

    /// Checks whether the map is empty, reporting malformed extents.
    pub fn try_is_empty(self) -> Result<bool> {
        Ok(self.try_len()? == 0)
    }

    /// Returns whether the map is empty, panicking on detected corruption.
    pub fn is_empty(self) -> bool {
        self.try_is_empty().expect("invalid Blubber map")
    }

    #[inline(always)]
    fn search(self, header: MapHeader, key: &K) -> Result<Option<u32>> {
        let query = key.encode_key();
        // Handwritten codecs cannot make an invalid query look like absence.
        if query.as_ref().len() != K::SIZE as usize {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        K::decode_key(query.as_ref()).map_err(|error| match error {
            crate::Error::Fault { kind, .. } => kind.into(),
            error => error,
        })?;
        let mut lo = 0;
        let mut hi = header.count;
        #[cfg(test)]
        let mut probes = work::SearchProbes::new();
        while lo < hi {
            #[cfg(test)]
            probes.step();
            let mid = lo + (hi - lo) / 2;
            let (candidate, base) = header.key(self.view, mid)?;
            K::decode_key(candidate).map_err(|error| error.rebase(base))?;
            if candidate == query.as_ref() {
                return Ok(Some(mid));
            }
            if blubber_core::compare_keys(candidate, query.as_ref()) == Ordering::Less {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Ok(None)
    }

    /// Looks up a key, checking probed keys and the found value's exact region.
    ///
    /// Search takes at most 32 probes, including on malformed ordering. Absence
    /// has its ordinary meaning only after ordering has been validated.
    #[inline(always)]
    pub fn try_get(self, key: &K) -> Result<Option<V::Read<'a>>> {
        let header = self.header;
        self.search(header, key)?
            .map(|index| V::read(header.value(self.view, index)?))
            .transpose()
    }

    /// Returns a value or absence, panicking on detected corruption.
    #[inline(always)]
    pub fn get(self, key: &K) -> Option<V::Read<'a>> {
        self.try_get(key).expect("invalid Blubber map value")
    }

    /// Checks for a key without decoding its value.
    pub fn try_contains_key(self, key: &K) -> Result<bool> {
        Ok(self.search(self.header, key)?.is_some())
    }

    /// Tests key presence, panicking on detected header or key corruption.
    pub fn contains_key(self, key: &K) -> bool {
        self.try_contains_key(key).expect("invalid Blubber map key")
    }

    /// Iterates copied keys and borrowed/copied values, stopping after one error.
    ///
    /// Variable values retain checked directory and payload coordinates. Keys
    /// still use their codec, and the final stored end must consume the payload.
    /// Variable directories with matching key widths call `Schema::read_region`
    /// once per visited value.
    #[inline(always)]
    pub fn try_iter(self) -> TryMapIter<'a, K, V> {
        if self.view.bytes().len() == 4 {
            return TryMapIter {
                state: MapState::Error(None),
                marker: PhantomData,
            };
        }
        let state = match self.header {
            header
                if header.width == V::FIXED_SIZE
                    && header.width.is_some()
                    && header.key_width == K::SIZE =>
            {
                MapState::Fixed(FixedMapState {
                    remaining: &self.view.bytes()[4..],
                    end: blubber_core::raw::fixed_chunk_end(
                        self.view.base(),
                        self.view.bytes().len(),
                    ),
                    layout: if V::__READ_REGION_NEEDS_LAYOUT {
                        Some(header.value)
                    } else {
                        None
                    },
                })
            }
            header if header.width.is_none() && header.key_width == K::SIZE => {
                MapState::Stream(StreamMapState {
                    bytes: self.view.bytes(),
                    base: self.view.base(),
                    layout: header.value,
                    payload: header.payload as usize,
                    key_at: 4,
                    count: header.count,
                    next: 0,
                    previous: 0,
                })
            }
            header => MapState::Indexed {
                view: self.view,
                header,
                next: 0,
            },
        };
        TryMapIter {
            state,
            marker: PhantomData,
        }
    }

    /// Iterates entries, panicking on the first detected corruption.
    ///
    /// Uses the same streamed variable-value selection as [`Self::try_iter`].
    #[inline(always)]
    pub fn iter(self) -> MapIter<'a, K, V> {
        if V::FIXED_SIZE.is_none() {
            return MapIter {
                state: InfallibleMapState::Other(self.try_iter()),
                marker: PhantomData,
            };
        }
        if self.view.bytes().len() == 4 && V::FIXED_SIZE.is_some() {
            return MapIter {
                state: InfallibleMapState::Fixed(FixedMapState {
                    remaining: &[],
                    end: self.view.base(),
                    layout: None,
                }),
                marker: PhantomData,
            };
        }
        let header = self.header;
        if header.width.is_some() && header.width == V::FIXED_SIZE && header.key_width == K::SIZE {
            return MapIter {
                state: InfallibleMapState::Fixed(FixedMapState {
                    remaining: &self.view.bytes()[4..],
                    end: blubber_core::raw::fixed_chunk_end(
                        self.view.base(),
                        self.view.bytes().len(),
                    ),
                    layout: if V::__READ_REGION_NEEDS_LAYOUT {
                        Some(header.value)
                    } else {
                        None
                    },
                }),
                marker: PhantomData,
            };
        }
        self.iter_other()
    }

    #[inline(never)]
    fn iter_other(self) -> MapIter<'a, K, V> {
        MapIter {
            state: InfallibleMapState::Other(self.try_iter()),
            marker: PhantomData,
        }
    }

    /// Certifies all keys, directory ends and recursively encoded values.
    pub fn validate(self) -> Result<()> {
        self.view.validate()
    }

    /// Returns canonical sorted owned entries using fallible allocation.
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> Result<<Map<K, V> as Schema>::Owned> {
        Map::<K, V>::decode_owned(self.view)
    }
}

impl<'a, K: MapKey, V: Fixed + Schema> MapRef<'a, K, V> {
    /// Iterates copied fixed values in physical slot order without decoding keys.
    ///
    /// Reuses the reader's checked geometry and checks matching key/value widths
    /// during setup. A setup fault is yielded once. Each subsequent item checks only
    /// that value's domain, rebasing codec faults to its account-relative offset.
    /// The slot is consumed before decoding, including if the codec unwinds.
    /// A value fault consumes its slot; later slots remain readable. Exhaustion
    /// is fused, and cloning retains the same immutable bytes and position.
    ///
    /// Iteration uses no allocation. Call [`Self::validate`] to check key
    /// domains, ordering and uniqueness. [`Self::try_iter`] decodes each key
    /// before its value and stops after its first error.
    ///
    /// ```
    /// use blubber::{Error, ErrorKind, Map, Schema, View};
    /// let bytes = [3, 0, 0, 0, 1, 1, 2, 2, 3, 0];
    /// let map = View::new(&bytes, *<Map<u8, bool> as Schema>::LAYOUT, 20)?
    ///     .read::<Map<u8, bool>>()?;
    /// let mut values = map.try_values();
    /// assert_eq!(values.next().transpose()?, Some(true));
    /// assert!(matches!(values.next(), Some(Err(Error::Fault {
    ///     kind: ErrorKind::InvalidBool, offset: 27, ..
    /// }))));
    /// assert_eq!(values.next().transpose()?, Some(false));
    /// assert!(values.next().is_none());
    /// let mut entries = map.try_iter();
    /// assert!(entries.next().is_some_and(|item| item.is_ok()));
    /// assert!(entries.next().is_some_and(|item| item.is_err()));
    /// assert!(entries.next().is_none()); // Ordinary iteration stops at the error.
    /// # Ok::<(), blubber::Error>(())
    /// ```
    #[inline(always)]
    pub fn try_values(self) -> TryMapValues<'a, K, V> {
        // from_view admitted the complete header; the immutable loan retains it.
        let matches = match self.view.layout() {
            blubber_core::Layout::Flat { checked } => {
                checked.header(6) == K::SIZE
                    && crate::view::width_for_region(&blubber_core::Layout::Flat {
                        checked: checked.child(0, true),
                    }) == Some(V::SIZE)
            }
            layout => match layout.head() {
                blubber_core::LayoutHead::Map { key, value } => {
                    key.width == K::SIZE && crate::view::width_for_region(&value) == Some(V::SIZE)
                }
                _ => false,
            },
        };
        match matches {
            true => TryMapValues {
                remaining: &self.view.bytes()[4..],
                end: blubber_core::raw::fixed_chunk_end(self.view.base(), self.view.bytes().len()),
                error: None,
                marker: PhantomData,
            },
            false => TryMapValues {
                remaining: &[],
                end: 0,
                error: Some(ErrorKind::InvalidSchema.into()),
                marker: PhantomData,
            },
        }
    }
}

/// A cloneable, fused iterator over fixed map values, with one result per slot.
///
/// Setup errors appear once. Value errors retain their exact absolute offsets
/// and iteration continues at the following slot. Keys are never formed; see
/// [`MapRef::try_values`] for the admission and validation contract.
#[derive(Clone)]
pub struct TryMapValues<'a, K: MapKey, V: Fixed> {
    remaining: &'a [u8],
    end: u32,
    error: Option<crate::Error>,
    marker: PhantomData<(K, V)>,
}

impl<K: MapKey, V: Fixed> Iterator for TryMapValues<'_, K, V> {
    type Item = Result<V>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(error) = self.error.take() {
            return Some(Err(error));
        }
        let stride = (K::SIZE as usize).saturating_add(V::SIZE as usize).max(1);
        let (entry, tail) = blubber_core::raw::split_fixed_chunk(self.remaining, stride)?;
        self.remaining = tail;
        let value = blubber_core::map_values::region(entry, K::SIZE as usize);
        Some(V::__decode_region(value).map_err(|error| {
            let base = self.end - self.remaining.len() as u32 - V::SIZE;
            error.rebase(base)
        }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let stride = (K::SIZE as usize).saturating_add(V::SIZE as usize).max(1);
        let remaining = self.remaining.len() / stride + usize::from(self.error.is_some());
        (remaining, Some(remaining))
    }
}

impl<K: MapKey, V: Fixed> ExactSizeIterator for TryMapValues<'_, K, V> {}
impl<K: MapKey, V: Fixed> FusedIterator for TryMapValues<'_, K, V> {}

impl<'a, K: MapKey> MapRef<'a, K, Str> {
    /// Iterates copied keys and strings after one packed-payload UTF-8 scan.
    ///
    /// Setup costs O(payload bytes), even for `take(1)`. Use [`Self::try_iter`]
    /// to read only part of the map. Both iterators check keys and item ends in
    /// the same order and stop at the same first error. Invalid aggregate text
    /// or an end inside a character triggers per-item UTF-8 conversion.
    /// Call [`Self::validate`] to check ordering and uniqueness.
    ///
    /// ```
    /// use blubber::{Map, Schema, Str, View};
    /// // Two keys/end words followed by the packed "ab" payload.
    /// let bytes = [2, 0, 0, 0, 1, 1, 0, 0, 0, 2, 2, 0, 0, 0, b'a', b'b'];
    /// let map = View::new(&bytes, *Map::<u8, Str>::LAYOUT, 12)?
    ///     .read::<Map<u8, Str>>()?;
    /// assert_eq!(map.try_iter().take(1).collect::<blubber::Result<Vec<_>>>()?, [(1, "a")]);
    /// // Batched setup visits "b" too, although only the first pair is returned.
    /// assert_eq!(map.try_iter_batched().take(1).collect::<blubber::Result<Vec<_>>>()?, [(1, "a")]);
    /// # Ok::<(), blubber::Error>(())
    /// ```
    #[inline(always)]
    pub fn try_iter_batched(self) -> TryBatchedMapIter<'a, K> {
        let inner = self.try_iter();
        let text = match &inner.state {
            MapState::Stream(state) => core::str::from_utf8(&state.bytes[state.payload..]).ok(),
            _ => None,
        };
        TryBatchedMapIter { inner, text }
    }
}

// Safe string APIs stay in typed glue. The byte-level batch/ordinary relation
// is proved in blubber_core::batched_read; Kani checks this SDK bridge directly.
#[inline(always)]
fn batched_text<'a>(
    text: Option<&'a str>,
    prior: usize,
    chunk: &'a [u8],
    base: u32,
) -> Result<&'a str> {
    let end = blubber_core::raw::indexed_coordinate(prior, 1, chunk.len());
    if let Some(text) = text {
        if let Some(value) = text.get(prior..end) {
            return Ok(value);
        }
    }
    batched_text_fallback(chunk, base)
}

#[cold]
#[inline(never)]
fn batched_text_fallback(chunk: &[u8], base: u32) -> Result<&str> {
    Str::read_region(chunk, *Str::LAYOUT, base)
}

#[cfg(kani)]
#[path = "readers_kani.rs"]
mod kani_support;
#[cfg(kani)]
#[doc(hidden)]
pub use kani_support::{kani_batched_list_state, kani_batched_map_state, kani_batched_text};

/// A cloneable, fused batched string-list iterator.
///
/// Items borrow the original immutable frame and can outlive this iterator.
/// Size hints and error ordering match [`TryListIter`].
#[derive(Clone)]
pub struct TryBatchedListIter<'a> {
    inner: TryListIter<'a, Str>,
    text: Option<&'a str>,
}

impl<'a> Iterator for TryBatchedListIter<'a> {
    type Item = Result<&'a str>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let ListState::Stream(state) = &mut self.inner.state else {
            return self.inner.next();
        };
        if state.next == state.count {
            return None;
        }
        let index = state.next;
        let result = match blubber_core::list_read::region(
            state.bytes,
            state.payload,
            state.next as usize,
            state.count as usize,
            state.previous,
        ) {
            Ok((chunk, start)) => {
                let base = blubber_core::raw::fixed_chunk_end(state.base, start as usize);
                let result = batched_text(self.text, state.previous, chunk, base);
                state.previous =
                    blubber_core::raw::indexed_coordinate(state.previous, 1, chunk.len());
                result
            }
            Err(error) => Err(error.rebase(state.base)),
        };
        state.next = state.next.wrapping_add(1);
        debug_assert!(state.next > index, "batch list iterator must advance");
        if result.is_err() {
            state.next = state.count;
        }
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl FusedIterator for TryBatchedListIter<'_> {}

/// A cloneable, fused batched string-map iterator.
///
/// Keys are copied and strings borrow the immutable source frame. Size hints
/// and key/boundary/value error ordering match [`TryMapIter`].
pub struct TryBatchedMapIter<'a, K: MapKey> {
    inner: TryMapIter<'a, K, Str>,
    text: Option<&'a str>,
}

impl<K: MapKey> Clone for TryBatchedMapIter<'_, K> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            text: self.text,
        }
    }
}

impl<'a, K: MapKey> Iterator for TryBatchedMapIter<'a, K> {
    type Item = Result<(K, &'a str)>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let MapState::Stream(state) = &mut self.inner.state else {
            return self.inner.next();
        };
        if state.next == state.count {
            return None;
        }
        let next = state.next.wrapping_add(1);
        let key_bytes =
            blubber_core::map_read::key(state.bytes, state.payload, state.key_at, K::SIZE as usize);
        let key_base = blubber_core::raw::fixed_chunk_end(state.base, state.key_at);
        let key = match K::decode_key(key_bytes) {
            Ok(key) => key,
            Err(error) => {
                state.next = state.count;
                return Some(Err(error.rebase(key_base)));
            }
        };
        let end_at = blubber_core::raw::indexed_coordinate(state.key_at, 1, K::SIZE as usize);
        let (chunk, start) = match blubber_core::map_read::region(
            state.bytes,
            state.payload,
            end_at,
            next == state.count,
            state.previous,
        ) {
            Ok(region) => region,
            Err(error) => {
                state.next = state.count;
                return Some(Err(error.rebase(state.base)));
            }
        };
        let base = blubber_core::raw::fixed_chunk_end(state.base, start as usize);
        let value = match batched_text(self.text, state.previous, chunk, base) {
            Ok(value) => value,
            Err(error) => {
                state.next = state.count;
                return Some(Err(error));
            }
        };
        state.previous = blubber_core::raw::indexed_coordinate(state.previous, 1, chunk.len());
        state.key_at = blubber_core::raw::indexed_coordinate(end_at, 1, 4);
        debug_assert!(next > state.next, "batch map iterator must advance");
        state.next = next;
        Some(Ok((key, value)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<K: MapKey> FusedIterator for TryBatchedMapIter<'_, K> {}

/// A cloneable, fused list iterator that reports corruption exactly once.
///
/// Its upper size hint is the remaining slot count. The lower bound is one for
/// nonempty state because an error can exhaust the iterator early.
pub struct TryListIter<'a, T: Schema> {
    state: ListState<'a>,
    marker: PhantomData<T>,
}

#[derive(Clone)]
enum ListState<'a> {
    Fixed(FixedListState<'a>),
    Stream(StreamListState<'a>),
    Indexed {
        view: View<'a>,
        header: ListHeader,
        next: u32,
    },
    Error(Option<crate::Error>),
}

#[derive(Clone)]
struct StreamListState<'a> {
    bytes: &'a [u8],
    base: u32,
    layout: blubber_core::Layout,
    payload: usize,
    count: u32,
    next: u32,
    previous: usize,
}

impl<'a> StreamListState<'a> {
    #[inline(always)]
    fn next<T: Schema>(&mut self) -> Option<Result<T::Read<'a>>> {
        if self.next == self.count {
            return None;
        }
        let index = self.next;
        let selected = blubber_core::list_read::region(
            self.bytes,
            self.payload,
            index as usize,
            self.count as usize,
            self.previous,
        );
        let result = match selected {
            Ok((chunk, start)) => {
                let base = blubber_core::raw::fixed_chunk_end(self.base, start as usize);
                let result = T::read_region(chunk, self.layout, base);
                // Keep the selected region intact if a custom decoder unwinds.
                self.previous =
                    blubber_core::raw::indexed_coordinate(self.previous, 1, chunk.len());
                result
            }
            Err(error) => Err(error.rebase(self.base)),
        };
        self.next += 1;
        debug_assert!(self.next > index, "list iterator must advance");
        if result.is_err() {
            self.next = self.count;
        }
        Some(result)
    }
}

#[derive(Clone)]
struct FixedListState<'a> {
    remaining: &'a [u8],
    end: u32,
    layout: Option<blubber_core::Layout>,
}

impl<'a> FixedListState<'a> {
    #[inline(always)]
    fn next_with<T: Schema, R>(&mut self, f: impl FnOnce(Result<T::Read<'a>>) -> R) -> Option<R> {
        let width = T::FIXED_SIZE.expect("fixed chunk schema") as usize;
        let (chunk, tail) = blubber_core::raw::split_fixed_chunk(self.remaining, width)?;
        let base = blubber_core::raw::fixed_chunk_base(self.end, self.remaining.len());
        let layout = if T::__READ_REGION_NEEDS_LAYOUT {
            self.layout.expect("checked fixed descriptor")
        } else {
            *T::LAYOUT
        };
        let result = T::read_region(chunk, layout, base);
        self.remaining = tail;
        if result.is_err() {
            self.remaining = &[];
        }
        Some(f(result))
    }
}

impl<T: Schema> Clone for TryListIter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}

impl<'a, T: Schema> TryListIter<'a, T> {
    #[inline(always)]
    fn next_with<R>(&mut self, f: impl FnOnce(Result<T::Read<'a>>) -> R) -> Option<R> {
        let result = match &mut self.state {
            ListState::Fixed(state) => return state.next_with::<T, R>(f),
            ListState::Stream(state) => return state.next::<T>().map(f),
            ListState::Indexed { view, header, next } => {
                if *next == header.count {
                    return None;
                }
                // Foreign fixed descriptors retain ordinary point selection.
                let result = header.element(*view, *next).and_then(T::read);
                *next += 1;
                if result.is_err() {
                    *next = header.count;
                }
                result
            }
            ListState::Error(error) => return error.take().map(|error| f(Err(error))),
        };
        Some(f(result))
    }
}

impl<'a, T: Schema> Iterator for TryListIter<'a, T> {
    type Item = Result<T::Read<'a>>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(test)]
        let before = work::list_remaining(&self.state);
        let result = self.next_with(|result| result);
        #[cfg(test)]
        work::assert_progress(before, work::list_remaining(&self.state), result.is_some());
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.state {
            ListState::Fixed(state) => {
                state.remaining.len() / T::FIXED_SIZE.expect("fixed chunk schema") as usize
            }
            ListState::Indexed { header, next, .. } => (header.count - next) as usize,
            ListState::Stream(state) => (state.count - state.next) as usize,
            ListState::Error(error) => usize::from(error.is_some()),
        };
        (usize::from(remaining > 0), Some(remaining))
    }
}
impl<T: Schema> FusedIterator for TryListIter<'_, T> {}

/// A cloneable, fused list iterator that panics on detected corruption.
///
/// It retains only a shared borrow of the bytes; items can outlive the iterator.
/// Fixed lists retain compact chunk state for both stepping and folds.
pub struct ListIter<'a, T: Schema> {
    state: InfallibleListState<'a, T>,
}
enum InfallibleListState<'a, T: Schema> {
    Fixed(FixedListState<'a>),
    Other(TryListIter<'a, T>),
}
impl<T: Schema> Clone for ListIter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            state: match &self.state {
                InfallibleListState::Fixed(fixed) => InfallibleListState::Fixed(fixed.clone()),
                InfallibleListState::Other(iter) => InfallibleListState::Other(iter.clone()),
            },
        }
    }
}
impl<'a, T: Schema> Iterator for ListIter<'a, T> {
    type Item = T::Read<'a>;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(test)]
        let before = work::infallible_list_remaining(&self.state);
        let item = |result| match result {
            Ok(value) => value,
            Err(error) => invalid_list_element(error),
        };
        let result = match &mut self.state {
            InfallibleListState::Fixed(fixed) => fixed.next_with::<T, _>(item),
            InfallibleListState::Other(iter) => iter.next_with(item),
        };
        #[cfg(test)]
        work::assert_progress(
            before,
            work::infallible_list_remaining(&self.state),
            result.is_some(),
        );
        result
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.state {
            InfallibleListState::Fixed(fixed) => {
                fixed.remaining.len() / T::FIXED_SIZE.expect("fixed chunk schema") as usize
            }
            InfallibleListState::Other(iter) => return iter.size_hint(),
        };
        (usize::from(remaining > 0), Some(remaining))
    }
    #[inline(always)]
    fn fold<B, F>(self, mut accumulator: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        match self.state {
            InfallibleListState::Fixed(fixed) => {
                let width = T::FIXED_SIZE.expect("fixed chunk schema") as usize;
                let layout = if T::__READ_REGION_NEEDS_LAYOUT {
                    fixed.layout.expect("checked fixed descriptor")
                } else {
                    *T::LAYOUT
                };
                let mut remaining = fixed.remaining;
                while let Some((chunk, tail)) =
                    blubber_core::raw::split_fixed_chunk(remaining, width)
                {
                    let base = blubber_core::raw::fixed_chunk_base(fixed.end, remaining.len());
                    let value = T::read_region(chunk, layout, base)
                        .unwrap_or_else(|error| invalid_list_element(error));
                    accumulator = f(accumulator, value);
                    remaining = tail;
                }
            }
            InfallibleListState::Other(iter) => {
                for item in iter {
                    let value = item.unwrap_or_else(|error| invalid_list_element(error));
                    accumulator = f(accumulator, value);
                }
            }
        }
        accumulator
    }
}
impl<T: Schema> FusedIterator for ListIter<'_, T> {}

#[cold]
#[inline(never)]
fn invalid_list_element(error: crate::Error) -> ! {
    panic!("invalid Blubber list element: {error:?}")
}

/// A cloneable, fused map iterator that reports corruption exactly once.
///
/// Keys are decoded by value. It visits physical entry order; only full
/// validation certifies strict ordering. Its size hint has the same early-error
/// behavior as [`TryListIter`].
pub struct TryMapIter<'a, K: MapKey, V: Schema> {
    state: MapState<'a>,
    marker: PhantomData<(K, V)>,
}

#[derive(Clone)]
enum MapState<'a> {
    Fixed(FixedMapState<'a>),
    Stream(StreamMapState<'a>),
    Indexed {
        view: View<'a>,
        header: MapHeader,
        next: u32,
    },
    Error(Option<crate::Error>),
}

#[derive(Clone)]
struct FixedMapState<'a> {
    remaining: &'a [u8],
    end: u32,
    layout: Option<blubber_core::Layout>,
}

impl<'a> FixedMapState<'a> {
    #[inline(always)]
    fn next_with<K: MapKey, V: Schema, R>(
        &mut self,
        f: impl FnOnce(Result<(K, V::Read<'a>)>) -> R,
    ) -> Option<R> {
        let key_width = K::SIZE as usize;
        let stride = key_width + V::FIXED_SIZE.expect("fixed map schema") as usize;
        let (entry, tail) = blubber_core::raw::split_fixed_chunk(self.remaining, stride)?;
        let base = blubber_core::raw::fixed_chunk_base(self.end, self.remaining.len());
        let (key, value) = entry.split_at(key_width);
        let key = match K::decode_key(key) {
            Ok(key) => key,
            Err(error) => {
                self.remaining = &[];
                return Some(f(Err(error.rebase(base))));
            }
        };
        let value = match V::read_region(
            value,
            if V::__READ_REGION_NEEDS_LAYOUT {
                self.layout.expect("checked map descriptor")
            } else {
                *V::LAYOUT
            },
            base + K::SIZE,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.remaining = &[];
                return Some(f(Err(error)));
            }
        };
        self.remaining = tail;
        Some(f(Ok((key, value))))
    }
}

#[derive(Clone)]
struct StreamMapState<'a> {
    bytes: &'a [u8],
    base: u32,
    layout: blubber_core::Layout,
    payload: usize,
    key_at: usize,
    count: u32,
    next: u32,
    previous: usize,
}

impl<'a> StreamMapState<'a> {
    #[inline(always)]
    fn next<K: MapKey, V: Schema>(&mut self) -> Option<Result<(K, V::Read<'a>)>> {
        if self.next == self.count {
            return None;
        }
        let next = self.next.wrapping_add(1);
        let key_bytes =
            blubber_core::map_read::key(self.bytes, self.payload, self.key_at, K::SIZE as usize);
        let key_base = blubber_core::raw::fixed_chunk_end(self.base, self.key_at);
        let key = match K::decode_key(key_bytes) {
            Ok(key) => key,
            Err(error) => {
                self.next = self.count;
                return Some(Err(error.rebase(key_base)));
            }
        };
        let end_at = blubber_core::raw::indexed_coordinate(self.key_at, 1, K::SIZE as usize);
        let (chunk, start) = match blubber_core::map_read::region(
            self.bytes,
            self.payload,
            end_at,
            next == self.count,
            self.previous,
        ) {
            Ok(region) => region,
            Err(error) => {
                self.next = self.count;
                return Some(Err(error.rebase(self.base)));
            }
        };
        let base = blubber_core::raw::fixed_chunk_end(self.base, start as usize);
        let value = match V::read_region(chunk, self.layout, base) {
            Ok(value) => value,
            Err(error) => {
                self.next = self.count;
                return Some(Err(error));
            }
        };
        // Retain the selected state if a custom decoder unwinds.
        self.previous = blubber_core::raw::indexed_coordinate(self.previous, 1, chunk.len());
        self.key_at = blubber_core::raw::indexed_coordinate(end_at, 1, 4);
        self.next = next;
        Some(Ok((key, value)))
    }
}

impl<K: MapKey, V: Schema> Clone for TryMapIter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}
impl<'a, K: MapKey, V: Schema> TryMapIter<'a, K, V> {
    #[inline(always)]
    fn next_with<R>(&mut self, f: impl FnOnce(Result<(K, V::Read<'a>)>) -> R) -> Option<R> {
        match &mut self.state {
            MapState::Stream(state) => state.next::<K, V>().map(f),
            MapState::Fixed(state) => state.next_with::<K, V, R>(f),
            MapState::Indexed { view, header, next } => {
                if *next == header.count {
                    return None;
                }
                let result = (|| {
                    let (key, base) = header.key(*view, *next)?;
                    let key = K::decode_key(key).map_err(|error| error.rebase(base))?;
                    let value = V::read(header.value(*view, *next)?)?;
                    Ok((key, value))
                })();
                *next += 1;
                if result.is_err() {
                    *next = header.count;
                }
                Some(f(result))
            }
            MapState::Error(error) => error.take().map(|error| f(Err(error))),
        }
    }
}
impl<'a, K: MapKey, V: Schema> Iterator for TryMapIter<'a, K, V> {
    type Item = Result<(K, V::Read<'a>)>;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(test)]
        let before = work::map_remaining(&self.state);
        if let MapState::Fixed(state) = &mut self.state {
            return state.next_with::<K, V, _>(|result| result);
        }
        let result = self.next_with(|result| result);
        #[cfg(test)]
        work::assert_progress(before, work::map_remaining(&self.state), result.is_some());
        result
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match &self.state {
            MapState::Fixed(state) => {
                state.remaining.len()
                    / (K::SIZE as usize + V::FIXED_SIZE.expect("fixed map schema") as usize)
            }
            MapState::Indexed { header, next, .. } => (header.count - next) as usize,
            MapState::Stream(state) => (state.count - state.next) as usize,
            MapState::Error(error) => usize::from(error.is_some()),
        };
        (usize::from(remaining > 0), Some(remaining))
    }
}
impl<K: MapKey, V: Schema> FusedIterator for TryMapIter<'_, K, V> {}

/// A cloneable, fused map iterator that panics on invalid entries.
///
/// Consuming folds decode each copied key before its value, just as repeated
/// calls to [`Iterator::next`] do. Fixed maps reuse exact borrowed chunks;
/// custom codecs retain their calls, error offsets and unwind behavior.
pub struct MapIter<'a, K: MapKey, V: Schema> {
    state: InfallibleMapState<'a, K, V>,
    marker: PhantomData<(K, V)>,
}

enum InfallibleMapState<'a, K: MapKey, V: Schema> {
    Fixed(FixedMapState<'a>),
    Other(TryMapIter<'a, K, V>),
}

impl<K: MapKey, V: Schema> Clone for MapIter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            state: match &self.state {
                InfallibleMapState::Fixed(state) => InfallibleMapState::Fixed(state.clone()),
                InfallibleMapState::Other(iter) => InfallibleMapState::Other(iter.clone()),
            },
            marker: PhantomData,
        }
    }
}
impl<'a, K: MapKey, V: Schema> Iterator for MapIter<'a, K, V> {
    type Item = (K, V::Read<'a>);
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(test)]
        let before = work::infallible_map_remaining(&self.state);
        let decode = |result| match result {
            Ok(item) => item,
            Err(error) => invalid_map_entry(error),
        };
        let result = match &mut self.state {
            InfallibleMapState::Fixed(state) => state.next_with::<K, V, _>(decode),
            InfallibleMapState::Other(iter) => iter.next_with(decode),
        };
        #[cfg(test)]
        work::assert_progress(
            before,
            work::infallible_map_remaining(&self.state),
            result.is_some(),
        );
        result
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.state {
            InfallibleMapState::Fixed(state) => {
                let n = state.remaining.len()
                    / (K::SIZE as usize + V::FIXED_SIZE.expect("fixed map schema") as usize);
                (usize::from(n > 0), Some(n))
            }
            InfallibleMapState::Other(iter) => iter.size_hint(),
        }
    }
    #[inline(always)]
    fn fold<B, F>(mut self, initial: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let mut accumulator = initial;
        match &mut self.state {
            InfallibleMapState::Fixed(state) => {
                while let Some(item) = state.next_with::<K, V, _>(|result| {
                    result.unwrap_or_else(|error| invalid_map_entry(error))
                }) {
                    accumulator = f(accumulator, item);
                }
            }
            InfallibleMapState::Other(iter) => {
                for item in iter {
                    let item = item.unwrap_or_else(|error| invalid_map_entry(error));
                    accumulator = f(accumulator, item);
                }
            }
        }
        accumulator
    }
}
impl<K: MapKey, V: Schema> FusedIterator for MapIter<'_, K, V> {}

#[cold]
#[inline(never)]
fn invalid_map_entry(error: crate::Error) -> ! {
    panic!("invalid Blubber map entry: {error:?}")
}

impl<'a, T: Schema> IntoIterator for &ListRef<'a, T> {
    type Item = T::Read<'a>;
    type IntoIter = ListIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Schema> core::fmt::Debug for ListRef<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ListRef")
            .field("base", &self.view.base())
            .field("bytes", &self.view.bytes().len())
            .finish_non_exhaustive()
    }
}

impl<'a, K: MapKey, V: Schema> IntoIterator for &MapRef<'a, K, V> {
    type Item = (K, V::Read<'a>);
    type IntoIter = MapIter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<K: MapKey, V: Schema> core::fmt::Debug for MapRef<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MapRef")
            .field("base", &self.view.base())
            .field("bytes", &self.view.bytes().len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod batched_tests {
    use super::*;

    #[test]
    fn batched_setup_caches_the_complete_checked_payload() {
        let bytes = [2, 0, 0, 0, 2, 0, 0, 0, 195, 169];
        let list =
            <crate::List<Str> as Schema>::read_region(&bytes, *crate::List::<Str>::LAYOUT, 29)
                .unwrap();
        let mut iter = list.try_iter_batched();
        let text = iter.text.expect("batch setup must retain valid text");
        assert_eq!(text, "é");
        assert!(core::ptr::eq(text.as_ptr(), bytes[8..].as_ptr()));
        assert_eq!(iter.next(), Some(Ok("é")));
        assert_eq!(iter.next(), Some(Ok("")));
        assert_eq!(iter.text, Some(text));

        let bytes = [2, 0, 0, 0, 0, 2, 0, 0, 0, 1, 2, 0, 0, 0, 195, 169];
        let map = <crate::Map<bool, Str> as Schema>::read_region(
            &bytes,
            *crate::Map::<bool, Str>::LAYOUT,
            29,
        )
        .unwrap();
        let mut iter = map.try_iter_batched();
        let text = iter.text.expect("batch setup must retain valid text");
        assert_eq!(text, "é");
        assert!(core::ptr::eq(text.as_ptr(), bytes[14..].as_ptr()));
        assert_eq!(iter.next(), Some(Ok((false, "é"))));
        assert_eq!(iter.next(), Some(Ok((true, ""))));
        assert_eq!(iter.text, Some(text));

        let bytes = [2, 0, 0, 0, 1, 0, 0, 0, b'x', 255];
        let list =
            <crate::List<Str> as Schema>::read_region(&bytes, *crate::List::<Str>::LAYOUT, 29)
                .unwrap();
        let mut iter = list.try_iter_batched();
        assert!(iter.text.is_none());
        assert_eq!(iter.next(), Some(Ok("x")));
        assert!(iter.next().unwrap().is_err());
    }
}
