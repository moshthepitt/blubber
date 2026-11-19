//! Lending traversal retains paths and ordinals across child edits.

use core::marker::PhantomData;

use crate::{Edit, ErrorKind, MapKey, Result, Schema};

/// An exclusive list cursor which lends one element editor at a time.
///
/// Each child borrows the cursor, preventing advancement until its last use.
/// Fixed children borrow disjoint exact slots admitted at construction. Variable
/// children may resize themselves and their descendants. Their cursor retains
/// adjacent-end coordinates and refreshes them after a structural edit. Consuming nested projections retire that cached evidence.
/// An error from [`Self::next`] exhausts the cursor permanently.
pub struct ListCursorMut<'a, T: Schema> {
    walk: ListWalk<'a>,
    marker: PhantomData<T>,
}

// Both alternatives must own their backend loan without heap allocation.
#[allow(clippy::large_enum_variant)]
enum ListWalk<'a> {
    Empty,
    Indexed(crate::edit::indexed_walk::IndexedWalk<'a, false>),
    Fixed(crate::edit_handle::FixedWalk<'a>),
}

impl<'a, T: Schema> ListCursorMut<'a, T> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Result<Self> {
        if T::FIXED_SIZE.is_some() {
            let walk = edit.fixed_list(T::FIXED_SIZE.unwrap())?;
            return Ok(Self {
                walk: if walk.bytes.is_empty() {
                    ListWalk::Empty
                } else {
                    ListWalk::Fixed(walk)
                },
                marker: PhantomData,
            });
        }
        Self::path(edit)
    }

    #[inline(always)]
    pub(crate) fn path(edit: Edit<'a>) -> Result<Self> {
        let walk =
            crate::edit::indexed_walk::IndexedWalk::<false>::new(edit.into_account()?, None)?;
        Ok(Self {
            walk: walk.map_or(ListWalk::Empty, ListWalk::Indexed),
            marker: PhantomData,
        })
    }

    /// Lends the next checked element selection, or returns exhaustion.
    ///
    /// Fixed cursors lend the next admitted slot. Variable cursors check the
    /// selected adjacent boundaries against the admitted extent. Leaf domains
    /// remain unread, allowing corrupt values to be replaced. A selection error is returned
    /// once, then calls return `None`.
    /// An error from a later child operation does not itself exhaust the cursor.
    #[allow(clippy::should_implement_trait)] // The result borrows this call.
    #[inline(always)]
    pub fn next(&mut self) -> Result<Option<T::Mut<'_>>> {
        if T::FIXED_SIZE.is_some() {
            let walk = match &mut self.walk {
                ListWalk::Fixed(walk) => walk,
                ListWalk::Empty => return Ok(None),
                _ => unreachable!("fixed cursor representation"),
            };
            if walk.bytes.is_empty() {
                return Ok(None);
            }
            if *walk.poisoned {
                walk.bytes = &mut [];
                return Err(ErrorKind::Poisoned.into());
            }
            return Ok(Some(T::project(crate::projection::Projection::new(
                walk.child::<T>(),
            ))));
        }
        let walk = match &mut self.walk {
            ListWalk::Indexed(walk) => walk,
            ListWalk::Empty => return Ok(None),
            _ => unreachable!("indexed cursor representation"),
        };
        Ok(walk.next()?.map(|child| {
            T::project(crate::projection::Projection::new(Edit::from_indexed(
                child,
            )))
        }))
    }
}

/// An exclusive map cursor which lends values with copied keys.
///
/// Lazy fixed maps check geometry and decode each visited key. Checked ingress
/// validates keys; indexed traversal retains its key preflight. See
/// [`crate::MapMut::cursor_mut`] for admission details. Value domains are checked
/// when read. Each value borrows the cursor until its last use; edits
/// may resize variable values and their descendants. Fixed values borrow exact
/// disjoint slots; variable values retain adjacent ends and ordinals between
/// calls, with checked paths available for structural edits. An error from [`Self::next`] exhausts the cursor.
pub struct MapCursorMut<'a, K: MapKey, V: Schema> {
    walk: MapWalk<'a>,
    marker: PhantomData<(K, V)>,
}

// Both alternatives must own their backend loan without heap allocation.
#[allow(clippy::large_enum_variant)]
enum MapWalk<'a> {
    Empty,
    Indexed(crate::edit::indexed_walk::IndexedWalk<'a, true>),
    Fixed(crate::edit_handle::FixedMapWalk<'a>),
}

impl<'a, K: MapKey, V: Schema> MapCursorMut<'a, K, V> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Result<Self> {
        if let Some(width) = V::FIXED_SIZE {
            let walk = edit.fixed_map::<K>(width)?;
            return Ok(Self {
                walk: if walk.walk.bytes.is_empty() {
                    MapWalk::Empty
                } else {
                    MapWalk::Fixed(walk)
                },
                marker: PhantomData,
            });
        }
        Self::path(edit)
    }

    #[inline(never)]
    fn path(edit: Edit<'a>) -> Result<Self> {
        let walk = crate::edit::indexed_walk::IndexedWalk::<true>::new(
            edit.into_account()?,
            Some(K::SIZE),
        )?;
        Ok(Self {
            walk: walk.map_or(MapWalk::Empty, MapWalk::Indexed),
            marker: PhantomData,
        })
    }

    /// Lends the next checked value selection and a copied, immutable key.
    ///
    /// Fixed walks split the next admitted entry; variable walks resolve the
    /// admitted coordinates, refreshing structural fallbacks. Both decode copied
    /// keys with account-relative errors. Value domains remain unvisited. A
    /// selection error is returned once, then calls return `None`. A source fault
    /// poisons a lazy indexed session. Errors from later child
    /// operations do not themselves exhaust the cursor.
    #[allow(clippy::should_implement_trait)] // The result borrows this call.
    #[inline(always)]
    pub fn next(&mut self) -> Result<Option<(K, V::Mut<'_>)>> {
        if V::FIXED_SIZE.is_some() {
            let walk = match &mut self.walk {
                MapWalk::Fixed(walk) => walk,
                MapWalk::Empty => return Ok(None),
                _ => unreachable!("fixed map representation"),
            };
            if walk.walk.bytes.is_empty() {
                return Ok(None);
            }
            if *walk.walk.poisoned {
                walk.walk.bytes = &mut [];
                return Err(ErrorKind::Poisoned.into());
            }
            let (key, slot) = walk.native_entry::<K, V>()?;
            return Ok(Some((
                key,
                V::project(crate::projection::Projection::new(Edit::from_slot(slot))),
            )));
        }
        let walk = match &mut self.walk {
            MapWalk::Indexed(walk) => walk,
            MapWalk::Empty => return Ok(None),
            _ => unreachable!("indexed map representation"),
        };
        if !walk.begin() {
            return Ok(None);
        }
        let (key, child) = walk.map_item::<K>()?;
        Ok(Some((
            key,
            V::project(crate::projection::Projection::new(Edit::from_indexed(
                child,
            ))),
        )))
    }
}

impl<T: Schema> core::fmt::Debug for ListCursorMut<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug = f.debug_struct("ListCursorMut");
        match &self.walk {
            ListWalk::Empty => debug.field("representation", &"empty"),
            ListWalk::Fixed(walk) => debug.field("remaining_bytes", &walk.bytes.len()),
            ListWalk::Indexed(walk) => debug.field("indexed", walk),
        };
        debug.finish_non_exhaustive()
    }
}

impl<K: MapKey, V: Schema> core::fmt::Debug for MapCursorMut<'_, K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug = f.debug_struct("MapCursorMut");
        match &self.walk {
            MapWalk::Empty => debug.field("representation", &"empty"),
            MapWalk::Fixed(walk) => debug.field("remaining_bytes", &walk.walk.bytes.len()),
            MapWalk::Indexed(walk) => debug.field("indexed", walk),
        };
        debug.finish_non_exhaustive()
    }
}
