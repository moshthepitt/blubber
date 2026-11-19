//! Borrow-bound indexed coordinates; structural fallbacks retain no byte loan.

use super::{check_state, AccountEdit, Offset};
use crate::storage::Backend as Resize;
use crate::{Error, ErrorKind, MapKey, Result, View};
use blubber_core::raw::InlinePath;
use blubber_core::{CheckedLayout, Layout, Span};

#[cfg(all(test, feature = "derive"))]
#[path = "indexed_work.rs"]
mod work;

pub(crate) struct IndexedWalk<'a, const MAP: bool> {
    store: &'a mut dyn Resize,
    meta: Meta,
    state: State,
}

struct Meta {
    layout: CheckedLayout<'static>,
    selected: CheckedLayout<'static>,
    child: CheckedLayout<'static>,
    path: InlinePath,
    discriminator: [u8; 8],
    minimum: u32,
    // Parent prefix stays fixed while only the selected child can resize.
    fixed_offset: Offset,
    base: usize,
    payload: usize,
    directory: usize,
    stride: usize,
    count: usize,
    child_depth_exceeded: bool,
    partition_checked: bool,
    lazy: bool,
}

struct State {
    end: usize,
    span: core::ops::Range<usize>,
    next: usize,
    dirty: bool,
    exhausted: bool,
}

// The lifetime owns the storage loan, metadata identity and mutable coordinate
// witness together. No pointer into the payload survives a backend call.
pub(crate) struct IndexedLoan<'a> {
    session: &'a mut dyn Session,
}

trait Session {
    fn source_error(&mut self, error: Error) -> Error;
    fn account(&mut self) -> AccountEdit<'_>;
    fn check(&self) -> Result<()>;
    fn view(&self) -> Result<View<'_>>;
    fn read_str(&self) -> Result<&str>;
    fn used_len(&self) -> Result<u32>;
    fn storage_len(&self) -> Result<u32>;
    fn refresh(&mut self) -> Result<()>;
    fn try_string(&mut self, input: &[u8]) -> Result<bool>;
    fn destination(&mut self, len: u32) -> Result<(&mut [u8], &mut bool)>;
    fn finish_exact(&mut self);
}

impl<'a, const MAP: bool> IndexedWalk<'a, MAP> {
    // A point loan starts at a checked child rather than walking its prefix.
    // Retain integer coordinates only; Session retires them before general edits.
    #[inline(always)]
    pub(crate) fn point(edit: AccountEdit<'a>, index: usize) -> Result<Option<Self>> {
        macro_rules! source_result {
            ($result:expr) => {
                match $result {
                    Ok(value) => value,
                    Err(error) => return Err(Self::admission_error(edit.store, edit.lazy, error)),
                }
            };
        }
        let view = source_result!(edit.view());
        let header = source_result!(view.list_header());
        if index >= header.count as usize {
            return Ok(None);
        }
        let span = source_result!((|| {
            let span = header.checked_span(view, index as u32)?;
            Ok(span)
        })());
        let base = view.base() as usize;
        let end = base + view.bytes().len();
        let payload = base + 4 + header.count.saturating_sub(1) as usize * 4;
        let count = header.count as usize;
        let mut child_path = edit.path;
        child_path.push(blubber_core::raw::Step::Index(index as u32))?;
        Ok(Some(Self {
            meta: Meta {
                layout: edit.layout,
                selected: edit.selected,
                child: edit.selected.child(0, true),
                path: edit.path,
                discriminator: edit.discriminator,
                minimum: edit.minimum,
                fixed_offset: edit.fixed_offset,
                base,
                payload,
                directory: base + 4,
                stride: 4,
                count,
                child_depth_exceeded: false,
                partition_checked: false,
                lazy: edit.lazy,
            },
            store: edit.store,
            state: State {
                end,
                span: base + span.start as usize..base + span.end as usize,
                next: index + 1,
                dirty: false,
                exhausted: true,
            },
        }))
    }

    #[inline(always)]
    pub(crate) fn new(edit: AccountEdit<'a>, key_width: Option<u32>) -> Result<Option<Self>> {
        macro_rules! source_result {
            ($result:expr) => {
                match $result {
                    Ok(value) => value,
                    Err(error) => return Err(Self::admission_error(edit.store, edit.lazy, error)),
                }
            };
        }
        let view = source_result!(edit.view());
        let (count, payload, directory, stride) = if MAP {
            let h = source_result!(view.map_header());
            let Some(key_width) = key_width else {
                return Err(Self::admission_error(
                    edit.store,
                    edit.lazy,
                    ErrorKind::InvalidSchema.into(),
                ));
            };
            if h.width.is_some() || key_width != h.key_width {
                return Err(Self::admission_error(
                    edit.store,
                    edit.lazy,
                    ErrorKind::InvalidSchema.into(),
                ));
            }
            if h.count == 0 {
                return Ok(None);
            }
            if !edit.lazy && (h.count == 1 || !edit.ingress_covers(view)) {
                source_result!(view.relative(blubber_core::validate_map_keys(
                    view.bytes(),
                    &view.layout(),
                )));
            }
            if edit.lazy {
                let at = h.payload - 4;
                let terminal = source_result!(view.word(at)) as usize;
                source_result!(blubber_core::raw::indexed_next_end(
                    0,
                    terminal,
                    view.bytes().len() - h.payload as usize,
                    true,
                )
                .map_err(|_| Error::new(ErrorKind::InvalidBoundary, view.base() + at)));
            }
            (h.count, h.payload, 4 + key_width, h.stride)
        } else {
            let h = source_result!(view.walk_list_header());
            if h.width.is_some() {
                return Err(Self::admission_error(
                    edit.store,
                    edit.lazy,
                    ErrorKind::InvalidSchema.into(),
                ));
            }
            (h.count, 4 + h.count.saturating_sub(1) * 4, 4, 4)
        };
        if !MAP && count == 0 {
            return Ok(None);
        }
        let partition_checked = MAP && !edit.lazy;
        let base = view.base() as usize;
        let end = base + view.bytes().len();
        let payload = base + payload as usize;
        let directory = base + directory as usize;
        let child_depth_exceeded = !edit.path.can_push();
        Ok(Some(Self {
            meta: Meta {
                layout: edit.layout,
                selected: edit.selected,
                child: edit.selected.child(0, true),
                path: edit.path,
                discriminator: edit.discriminator,
                minimum: edit.minimum,
                fixed_offset: edit.fixed_offset,
                lazy: edit.lazy,
                base,
                payload,
                directory,
                stride: stride as usize,
                count: count as usize,
                child_depth_exceeded,
                partition_checked,
            },
            store: edit.store,
            state: State {
                end,
                span: payload..payload,
                next: 0,
                dirty: false,
                exhausted: false,
            },
        }))
    }

    #[cold]
    #[inline(never)]
    fn admission_error(store: &mut dyn Resize, lazy: bool, error: Error) -> Error {
        if lazy {
            let mut poisoned = false;
            let _: Result<()> = crate::edit::list_pass::source(Err(error), &mut poisoned);
            if poisoned {
                store.set_poisoned(true);
            }
        }
        error
    }

    #[inline(always)]
    pub(crate) fn next(&mut self) -> Result<Option<IndexedLoan<'_>>> {
        if self.state.next >= self.meta.count {
            return Ok(None);
        }
        let result = self
            .refresh()
            .and_then(|()| advance_coordinates::<MAP>(self.store, &self.meta, &mut self.state));
        if let Err(error) = result {
            self.state.next = self.meta.count;
            return Err(error);
        }
        // Selection succeeded after checked refresh; publish that fact at the
        // lending boundary so the SBF caller can retain it across inlining.
        self.state.dirty = false;
        Ok(Some(IndexedLoan { session: self }))
    }

    #[inline(always)]
    pub(crate) fn begin(&mut self) -> bool {
        if self.state.exhausted || self.state.next >= self.meta.count {
            return false;
        }
        self.state.exhausted = true;
        true
    }

    #[inline(always)]
    pub(crate) fn map_item<K: MapKey>(&mut self) -> Result<(K, IndexedLoan<'_>)> {
        if self.meta.lazy {
            let key = self.prepare_lazy_map_item::<K>()?;
            self.state.dirty = false;
            return Ok((key, IndexedLoan { session: self }));
        }
        self.refresh()?;
        let bytes = self.store.bytes();
        let at = blubber_core::raw::indexed_coordinate(
            self.meta.directory.wrapping_sub(K::SIZE as usize),
            self.state.next,
            self.meta.stride,
        );
        let (_, tail) = bytes.split_at(at);
        let (encoded, _) = tail.split_at(K::SIZE as usize);
        let key = K::decode_key(encoded).map_err(|error| error.rebase(at as u32))?;
        // Checked opening retains the complete partition for this visit.
        advance_bytes::<MAP>(bytes, &self.meta, &mut self.state, MAP)?;
        self.state.dirty = false;
        Ok((key, IndexedLoan { session: self }))
    }
    // Called only after successful selection. The callback returns its copied
    // key before borrowing the session, keeping the loan out of its Result.
    #[inline(always)]
    pub(crate) fn child_loan(&mut self) -> IndexedLoan<'_> {
        IndexedLoan { session: self }
    }

    // Keep the cursor's combined key/loan return separate: its SBF caller needs
    // that shape. Both paths use the same checked key and coordinate kernels.
    #[inline(always)]
    pub(crate) fn prepare_map_item<K: MapKey>(&mut self) -> Result<K> {
        if self.meta.lazy {
            let key = self.prepare_lazy_map_item::<K>()?;
            // Retain successful selection at this caller's lending boundary.
            self.state.dirty = false;
            return Ok(key);
        }
        self.refresh()?;
        let bytes = self.store.bytes();
        let at = blubber_core::raw::indexed_coordinate(
            self.meta.directory.wrapping_sub(K::SIZE as usize),
            self.state.next,
            self.meta.stride,
        );
        let (_, tail) = bytes.split_at(at);
        let (encoded, _) = tail.split_at(K::SIZE as usize);
        let key = K::decode_key(encoded).map_err(|error| error.rebase(at as u32))?;
        // Checked opening retains the complete partition for this visit.
        advance_bytes::<MAP>(bytes, &self.meta, &mut self.state, MAP)?;
        self.state.dirty = false;
        Ok(key)
    }

    #[inline(never)]
    fn prepare_lazy_map_item<K: MapKey>(&mut self) -> Result<K> {
        if let Err(error) = self.refresh() {
            return Err(self.map_source_error(error));
        }
        if self.state.next >= self.meta.count {
            return Err(ErrorKind::IndexOutOfBounds.into());
        }
        let bytes = self.store.bytes();
        let at = blubber_core::raw::indexed_coordinate(
            self.meta.directory - K::SIZE as usize,
            self.state.next,
            self.meta.stride,
        );
        let encoded = match blubber_core::raw::indexed_map_key(
            bytes,
            at,
            K::SIZE as usize,
            self.meta.stride,
            self.state.next == 0,
        ) {
            Ok(encoded) => encoded,
            Err(Error::Fault { kind, .. }) => {
                return Err(self.map_source_error(Error::new(kind, at as u32)))
            }
            Err(error) => return Err(self.map_source_error(error)),
        };
        let key = match K::decode_key(encoded) {
            Ok(key) => key,
            Err(error) => return Err(self.map_source_error(error.rebase(at as u32))),
        };
        if let Err(error) = advance_bytes::<MAP>(bytes, &self.meta, &mut self.state, false) {
            return Err(self.map_source_error(error));
        }
        self.state.dirty = false;
        Ok(key)
    }

    #[cold]
    #[inline(never)]
    fn map_source_error(&mut self, error: Error) -> Error {
        if self.meta.lazy {
            let mut poisoned = false;
            let _: Result<()> = crate::edit::list_pass::source(Err(error), &mut poisoned);
            if poisoned {
                // control: walk-source-poison
                self.store.set_poisoned(true);
                self.state.dirty = true;
            }
        }
        error
    }
}

impl<'a> IndexedLoan<'a> {
    #[inline(always)]
    pub(crate) fn source_error(&mut self, error: Error) -> Error {
        self.session.source_error(error)
    }
    #[inline(always)]
    pub(crate) fn reborrow(&mut self) -> IndexedLoan<'_> {
        IndexedLoan {
            session: self.session,
        }
    }
    #[inline(never)]
    pub(crate) fn into_account(self) -> AccountEdit<'a> {
        self.session.account()
    }
    #[inline(always)]
    pub(crate) fn check(&self) -> Result<()> {
        self.session.check()
    }
    #[inline(always)]
    pub(crate) fn view(&self) -> Result<View<'_>> {
        self.session.view()
    }
    #[inline(always)]
    pub(crate) fn read_str(&self) -> Result<&str> {
        self.session.read_str()
    }
    #[inline(always)]
    pub(crate) fn used_len(&self) -> Result<u32> {
        self.session.used_len()
    }
    #[inline(always)]
    pub(crate) fn storage_len(&self) -> Result<u32> {
        self.session.storage_len()
    }
    #[inline(always)]
    pub(crate) fn try_string(&mut self, input: &[u8]) -> Result<bool> {
        self.session.try_string(input)
    }
    #[inline(never)]
    pub(crate) fn with_account<R>(
        &mut self,
        f: impl FnOnce(&mut AccountEdit<'_>) -> Result<R>,
    ) -> Result<R> {
        self.session.check()?;
        let result = f(&mut self.session.account());
        let refreshed = self.session.refresh();
        match result {
            Err(error) => Err(error),
            Ok(value) => refreshed.map(|()| value),
        }
    }
    #[inline(always)]
    pub(crate) fn replace_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        self.replace_with_policy::<false, I>(measure, encode)
    }

    #[inline(always)]
    pub(crate) fn replace_source_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        self.replace_with_policy::<true, I>(measure, encode)
    }

    #[inline(always)]
    fn replace_with_policy<const SOURCE: bool, I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        self.session.refresh()?;
        let view = self.view()?;
        let old_len = view.bytes().len();
        let (len, input) = match measure(view) {
            Ok(pair) => pair,
            Err(error) => {
                if SOURCE
                    && matches!(error, Error::Fault { offset, .. } if offset != Error::NO_OFFSET)
                {
                    return Err(self.source_error(error));
                }
                return Err(error);
            }
        };
        if len as usize != old_len {
            return self.with_account(|edit| edit.replace_with(|_| Ok((len, input)), encode));
        }
        let (destination, poisoned) = self.session.destination(len)?;
        *poisoned = true;
        encode(input, destination);
        *poisoned = false;
        self.session.finish_exact();
        Ok(())
    }
}
impl<const MAP: bool> IndexedWalk<'_, MAP> {
    #[inline(never)]
    fn dirty_span(&self) -> Result<Span> {
        let mut path = self.meta.path;
        path.push(blubber_core::raw::Step::Index((self.state.next - 1) as u32))?;
        blubber_core::raw::resolve_local(
            self.store.bytes(),
            &Layout::Flat {
                checked: self.meta.layout,
            },
            &path,
        )
    }

    #[inline(never)]
    fn refresh_dirty(&mut self) -> Result<()> {
        let edit = self.meta.account(self.store);
        let view = edit.view()?;
        if view.base() as usize != self.meta.base {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        let span = if MAP {
            let h = view.map_header()?;
            if h.count as usize != self.meta.count {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            if self.state.next == 0 {
                None
            } else {
                Some(h.checked_span(view, (self.state.next - 1) as u32)?)
            }
        } else {
            let h = view.list_header()?;
            if h.count as usize != self.meta.count {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            if self.state.next == 0 {
                None
            } else {
                Some(h.checked_span(view, (self.state.next - 1) as u32)?)
            }
        };
        self.state.end = self.meta.base + view.bytes().len();
        if let Some(span) = span {
            self.state.span =
                self.meta.base + span.start as usize..self.meta.base + span.end as usize;
        }
        self.state.dirty = false;
        Ok(())
    }
}
impl<const MAP: bool> Session for IndexedWalk<'_, MAP> {
    #[inline(always)]
    fn source_error(&mut self, error: Error) -> Error {
        self.map_source_error(error)
    }
    #[inline(never)]
    fn account(&mut self) -> AccountEdit<'_> {
        self.state.dirty = true;
        self.meta
            .account(self.store)
            .index((self.state.next - 1) as u32)
    }
    #[inline(always)]
    fn destination(&mut self, len: u32) -> Result<(&mut [u8], &mut bool)> {
        self.refresh()?;
        let span = self.state.span.clone();
        if len as usize != span.end - span.start {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        let access = self.store.exact_access();
        let destination =
            blubber_core::raw::indexed_slot_region(access.bytes, span.start, span.end);
        self.state.dirty = true;
        Ok((destination, access.poisoned))
    }
    #[inline(always)]
    fn check(&self) -> Result<()> {
        // The exclusive session excludes external close/poison. Every escape
        // to general edits or destructive encoder first retires this witness.
        if self.state.dirty {
            check_state(self.store, false)
        } else {
            Ok(())
        }
    }
    #[inline(always)]
    fn finish_exact(&mut self) {
        self.state.dirty = false;
    }

    #[inline(always)]
    fn view(&self) -> Result<View<'_>> {
        self.check()?;
        let span = if self.state.dirty {
            {
                let s = self.dirty_span()?;
                s.start as usize..s.end as usize
            }
        } else {
            self.state.span.clone()
        };
        View::inherited_for_edit(
            &self.store.bytes()[span.start..span.end],
            Layout::Flat {
                checked: self.meta.child,
            },
            span.start as u32,
        )
    }

    #[inline(always)]
    fn read_str(&self) -> Result<&str> {
        self.check()?;
        let span = if self.state.dirty {
            {
                let s = self.dirty_span()?;
                s.start as usize..s.end as usize
            }
        } else {
            self.state.span.clone()
        };
        decode_indexed_string(self.store.bytes(), span.clone()).map_err(|error| {
            Error::new(ErrorKind::InvalidUtf8, error.valid_up_to() as u32).rebase(span.start as u32)
        })
    }

    #[inline(always)]
    fn used_len(&self) -> Result<u32> {
        self.check()?;
        blubber_core::read_u32(self.store.bytes(), 8)
    }

    #[inline(always)]
    fn storage_len(&self) -> Result<u32> {
        self.check()?;
        blubber_core::checked_u32(self.store.storage_len())
    }

    #[inline(always)]
    fn refresh(&mut self) -> Result<()> {
        self.check()?;
        if self.state.dirty {
            self.refresh_dirty()?;
        }
        Ok(())
    }

    #[inline(always)]
    fn try_string(&mut self, input: &[u8]) -> Result<bool> {
        self.refresh()?;
        let span = self.state.span.clone();
        if input.len() != span.end - span.start {
            return Ok(false);
        }
        let access = self.store.exact_access();
        let destination =
            blubber_core::raw::indexed_slot_region(access.bytes, span.start, span.end);
        self.state.dirty = true;
        *access.poisoned = true;
        destination.copy_from_slice(input);
        *access.poisoned = false;
        self.state.dirty = false;
        Ok(true)
    }
}

#[inline(always)]
fn decode_indexed_string(
    storage: &[u8],
    span: core::ops::Range<usize>,
) -> core::result::Result<&str, core::str::Utf8Error> {
    if let Some((window, low, high)) =
        blubber_core::raw::indexed_string_window(storage, span.start, span.end)
    {
        if let Ok(text) = core::str::from_utf8(window) {
            // Preserve the known width across the pinned converter's call.
            if text.len() == window.len() {
                if let Some(value) = text.get(low..).and_then(|tail| tail.get(..high - low)) {
                    #[cfg(all(test, feature = "derive"))]
                    work::string_window_hit();
                    return Ok(value);
                }
            }
        }
    }
    decode_string_fallback(storage, span)
}

#[cold]
#[inline(never)]
fn decode_string_fallback(
    storage: &[u8],
    span: core::ops::Range<usize>,
) -> core::result::Result<&str, core::str::Utf8Error> {
    core::str::from_utf8(&storage[span])
}

#[inline(always)]
fn advance_coordinates<const MAP: bool>(
    store: &dyn Resize,
    meta: &Meta,
    state: &mut State,
) -> Result<()> {
    let partition_checked = if MAP { meta.partition_checked } else { false };
    advance_bytes::<MAP>(store.bytes(), meta, state, partition_checked)
}

#[inline(always)]
fn advance_bytes<const MAP: bool>(
    bytes: &[u8],
    meta: &Meta,
    state: &mut State,
    partition_checked: bool,
) -> Result<()> {
    if meta.child_depth_exceeded {
        return Err(ErrorKind::DepthExceeded.into());
    }
    let index = state.next;
    let at = blubber_core::raw::indexed_coordinate(meta.directory, index, meta.stride);
    let previous = state.span.end.wrapping_sub(meta.payload);
    let extent = state.end.wrapping_sub(meta.payload);
    let current = if !MAP && index.wrapping_add(1) == meta.count {
        extent
    } else {
        blubber_core::raw::indexed_read_word(bytes, at) as usize
    };
    let end = if partition_checked {
        current
    } else {
        blubber_core::raw::indexed_next_end(
            previous,
            current,
            extent,
            MAP && index + 1 == meta.count,
        )
        .map_err(|error| error.rebase(at as u32))?
    };
    state.span = state.span.end..meta.payload.wrapping_add(end);
    state.next = index.wrapping_add(1);
    if MAP {
        state.exhausted = false;
    }
    Ok(())
}

impl Meta {
    #[inline(always)]
    fn account<'a>(&self, store: &'a mut dyn Resize) -> AccountEdit<'a> {
        AccountEdit {
            store,
            layout: self.layout,
            selected: self.selected,
            minimum: self.minimum,
            discriminator: self.discriminator,
            path: self.path,
            path_error: None,
            root: false,
            ingress: None,
            lazy: self.lazy,
            fixed_offset: self.fixed_offset,
        }
    }
}

impl<const MAP: bool> core::fmt::Debug for IndexedWalk<'_, MAP> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IndexedWalk")
            .field("count", &self.meta.count)
            .field("next", &self.state.next)
            .field("exhausted", &self.state.exhausted)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{List, ReadStorage as _, Resize as _, SliceStore, Str};
    crate::__root_schema!(__TerminalWalk, List<Str>);

    #[test]
    fn terminal_reacquisition_disables_coordinate_access() {
        let mut room = [0; 64];
        let mut store = SliceStore::new(&mut room);
        let edit = AccountEdit::initialize::<__TerminalWalk>(&mut store, *b"idxclose", &["a", "b"])
            .unwrap();
        let mut walk = IndexedWalk::<false>::new(edit, None).unwrap().unwrap();
        let mut loan = walk.next().unwrap().unwrap();
        assert_eq!(loan.read_str().unwrap(), "a");
        let result = loan.with_account(|edit| {
            // Model D17's terminal backend boundary without constructing SDK
            // memory: closure closes the sealed store before reacquisition.
            edit.store.set_poisoned(true);
            assert_eq!(edit.store.commit(0), Err(ErrorKind::Poisoned.into()));
            Err::<(), _>(ErrorKind::Capacity.into())
        });
        assert_eq!(result, Err(ErrorKind::Capacity.into()));
        assert_eq!(loan.check(), Err(ErrorKind::Closed.into()));
        assert!(matches!(loan.view(), Err(error) if error == ErrorKind::Closed.into()));
        assert!(matches!(walk.next(), Err(error) if error == ErrorKind::Closed.into()));
    }

    crate::__root_schema!(__MapTerminalWalk, crate::Map<u8, Str>);

    #[test]
    fn lazy_map_visit_rechecks_the_retained_terminal_after_fault_injection() {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        let mut edit = AccountEdit::initialize::<__MapTerminalWalk>(
            &mut store,
            *b"map-last",
            &[(2, "a"), (4, "b"), (6, "cc")],
        )
        .unwrap();
        edit.lazy = true;
        let mut walk = IndexedWalk::<true>::new(edit, Some(1)).unwrap().unwrap();
        assert_eq!(walk.map_item::<u8>().unwrap().0, 2);
        assert_eq!(walk.map_item::<u8>().unwrap().0, 4);
        let terminal = walk.meta.directory + 2 * walk.meta.stride;
        // The partition remains untrusted; inject a terminal within its extent
        // and above the preceding end, so only terminal equality detects it.
        walk.store.bytes_mut()[terminal..terminal + 4].copy_from_slice(&3u32.to_le_bytes());
        let before = walk.store.bytes().to_vec();
        assert!(matches!(
            walk.map_item::<u8>(),
            Err(Error::Fault {
                kind: ErrorKind::InvalidBoundary,
                ..
            })
        ));
        assert_eq!(walk.store.bytes(), before);
        assert!(walk.store.is_poisoned());
    }

    #[test]
    fn checked_map_open_fault_keeps_the_original_source_frame() {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        let mut edit = AccountEdit::initialize::<__MapTerminalWalk>(
            &mut store,
            *b"map-open",
            &[(2, "a"), (4, "bb"), (6, "ccc")],
        )
        .unwrap();
        assert!(!edit.lazy);
        let view = edit.view().unwrap();
        let terminal = view.base() + view.map_header().unwrap().payload - 4;
        edit.store.bytes_mut()[terminal as usize..terminal as usize + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        edit.ingress = None;
        let before = edit.store.bytes().to_vec();
        let result = IndexedWalk::<true>::new(edit, Some(1));
        assert!(result.is_err());
        drop(result);
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
}
