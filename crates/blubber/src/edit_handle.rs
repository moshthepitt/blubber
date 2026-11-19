//! Account selections and exclusive exact-region loans share the public handle.

use crate::storage::Backend as Resize;
use blubber_core::{CheckedLayout, Layout};

use crate::edit::{AccountEdit, ListRequest};
use crate::{CommitInfo, Error, ErrorKind, Fixed, Result, RootSchema, Schema, View};

/// An exclusive selection in an account session.
///
/// Structural edits retain the storage borrow and checked integer path. Fixed
/// walks lend exact byte regions instead; their lifetime prevents resizing,
/// closing or advancing the parent while a child remains in use.
pub struct Edit<'a> {
    mode: Mode<'a>,
}

// Full-width tags let SBF forward known modes through fixed projections.
#[repr(usize)]
enum Mode<'a> {
    Account(AccountEdit<'a>),
    Indexed(Option<crate::edit::indexed_walk::IndexedLoan<'a>>),
    Region(Region<'a>),
    Slot(Slot<'a>),
    // Certified projection hint; physical geometry is still checked at removal.
    TailCandidate(AccountEdit<'a>),
}

// Retain full-word mode tags across shared collection routing.
#[repr(usize)]
enum AccountRoute {
    Account = 0,
    Tail = 4,
}

// Reclassifying the string projection avoids transporting the inactive account
// path through every indexed child return on SBF.
#[allow(clippy::large_enum_variant)]
pub(crate) enum StringEdit<'a> {
    General(Edit<'a>),
    Indexed(crate::edit::indexed_walk::IndexedLoan<'a>),
}

impl StringEdit<'_> {
    #[inline(always)]
    pub(crate) fn reborrow(&mut self) -> StringEdit<'_> {
        match self {
            Self::General(edit) => StringEdit::General(edit.reborrow()),
            Self::Indexed(loan) => StringEdit::Indexed(loan.reborrow()),
        }
    }
    #[inline(always)]
    pub(crate) fn read_str(&self) -> Result<&str> {
        match self {
            Self::General(edit) => read_selected_string(edit),
            Self::Indexed(loan) => loan.read_str(),
        }
    }
    #[inline(always)]
    pub(crate) fn try_string(&mut self, input: &[u8]) -> Result<bool> {
        match self {
            Self::General(edit) => edit.try_string(input),
            Self::Indexed(loan) => loan.try_string(input),
        }
    }
    #[inline(always)]
    pub(crate) fn replace_bytes(&mut self, input: &[u8]) -> Result<()> {
        match self {
            Self::General(edit) => edit.replace_bytes(input),
            Self::Indexed(loan) => loan.with_account(|edit| edit.replace_bytes(input)),
        }
    }
    #[inline(always)]
    pub(crate) fn replace_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        match self {
            Self::General(edit) => edit.replace_source_with(measure, encode),
            Self::Indexed(loan) => loan.replace_source_with(measure, encode),
        }
    }
}

// A usable region is fixed and bytes.len() == layout.header(1). Slot admission,
// checked collection spans and field subloans establish this invariant; only
// error-bearing projections may lack it. No region operation resizes bytes.
pub(crate) struct Region<'a> {
    pub(crate) bytes: &'a mut [u8],
    pub(crate) poisoned: &'a mut bool,
    pub(crate) layout: CheckedLayout<'static>,
    pub(crate) base: u32,
    pub(crate) used: u32,
    pub(crate) storage_len: u32,
    pub(crate) error: Option<Error>,
}

// Keep this policy explicit: bool metadata makes the fixed-walk Result
// much more expensive for the record proofs.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum WalkPolicy {
    Checked,
    Lazy,
}

impl WalkPolicy {
    #[inline(always)]
    pub(crate) fn new(lazy: bool) -> Self {
        if lazy {
            Self::Lazy
        } else {
            Self::Checked
        }
    }
}

pub(crate) struct FixedMeta {
    pub(crate) layout: CheckedLayout<'static>,
    pub(crate) base: u32,
    pub(crate) used: u32,
    pub(crate) storage_len: u32,
    pub(crate) payload_len: usize,
    pub(crate) lazy: WalkPolicy,
}

#[derive(Clone, Copy)]
struct SlotMeta {
    layout: CheckedLayout<'static>,
    base: u32,
    used: u32,
    storage_len: u32,
}

impl FixedMeta {
    #[inline(always)]
    fn slot(&self) -> SlotMeta {
        SlotMeta {
            layout: self.layout,
            base: self.base,
            used: self.used,
            storage_len: self.storage_len,
        }
    }
}

pub(crate) struct Slot<'a> {
    bytes: &'a mut [u8],
    poisoned: &'a mut bool,
    meta: SlotMeta,
    offset: usize,
}

impl<'a> Slot<'a> {
    #[inline(always)]
    fn check(&self) -> Result<()> {
        if *self.poisoned {
            Err(ErrorKind::Poisoned.into())
        } else {
            Ok(())
        }
    }
    #[inline(always)]
    fn view(&self) -> Result<View<'_>> {
        self.check()?;
        View::inherited(
            self.bytes,
            Layout::Flat {
                checked: self.meta.layout,
            },
            self.meta.base.saturating_add(self.offset as u32),
        )
    }
    #[inline(always)]
    fn reborrow(&mut self) -> Slot<'_> {
        Slot {
            bytes: self.bytes,
            poisoned: self.poisoned,
            meta: self.meta,
            offset: self.offset,
        }
    }
    #[inline(always)]
    fn region(self) -> Region<'a> {
        Region {
            bytes: self.bytes,
            poisoned: self.poisoned,
            layout: self.meta.layout,
            base: self.meta.base.saturating_add(self.offset as u32),
            used: self.meta.used,
            storage_len: self.meta.storage_len,
            error: None,
        }
    }
}

pub(crate) struct FixedWalk<'a> {
    pub(crate) bytes: &'a mut [u8],
    pub(crate) poisoned: &'a mut bool,
    pub(crate) meta: FixedMeta,
}

pub(crate) struct FixedMapWalk<'a> {
    pub(crate) walk: FixedWalk<'a>,
}

impl FixedMapWalk<'_> {
    #[inline(always)]
    pub(crate) fn native_entry<K: crate::MapKey, V: Schema>(&mut self) -> Result<(K, Slot<'_>)> {
        // Clear the suffix before decoding: a codec error or panic exhausts a
        // manual cursor, just as its path-based counterpart does.
        let remaining = core::mem::take(&mut self.walk.bytes);
        let offset =
            blubber_core::raw::fixed_chunk_base(self.walk.meta.payload_len as u32, remaining.len())
                as usize;
        let (bytes, value, tail) = blubber_core::raw::split_fixed_map_entry(
            remaining,
            K::SIZE as usize,
            V::FIXED_SIZE.expect("fixed map schema") as usize,
        );
        let key = match K::decode_key(&bytes[..K::SIZE as usize]) {
            Ok(key) => key,
            Err(error) => {
                let error = error.rebase(self.walk.meta.base.saturating_add(offset as u32));
                if self.walk.meta.lazy == WalkPolicy::Lazy {
                    let _ = crate::edit::list_pass::source::<()>(Err(error), self.walk.poisoned);
                }
                return Err(error);
            }
        };
        self.walk.bytes = tail;
        Ok((
            key,
            Slot {
                bytes: value,
                poisoned: self.walk.poisoned,
                meta: self.walk.meta.slot(),
                offset: offset + K::SIZE as usize,
            },
        ))
    }
}

impl FixedWalk<'_> {
    #[inline(always)]
    pub(crate) fn child<T: Schema>(&mut self) -> Edit<'_> {
        let width = T::FIXED_SIZE.expect("fixed walk schema") as usize;
        let remaining = core::mem::take(&mut self.bytes);
        let offset = self.meta.payload_len - remaining.len();
        let (bytes, tail) = blubber_core::raw::split_fixed_slot(remaining, width);
        self.bytes = tail;
        Edit {
            mode: Mode::Slot(Slot {
                bytes,
                poisoned: self.poisoned,
                meta: self.meta.slot(),
                offset,
            }),
        }
    }
}

impl Region<'_> {
    #[inline(always)]
    fn check(&self) -> Result<()> {
        if *self.poisoned {
            return Err(ErrorKind::Poisoned.into());
        }
        self.error.map_or(Ok(()), Err)
    }

    #[inline(always)]
    fn view(&self) -> Result<View<'_>> {
        self.check()?;
        View::inherited(
            self.bytes,
            Layout::Flat {
                checked: self.layout,
            },
            self.base,
        )
    }

    #[inline(always)]
    fn reborrow(&mut self) -> Region<'_> {
        Region {
            bytes: self.bytes,
            poisoned: self.poisoned,
            layout: self.layout,
            base: self.base,
            used: self.used,
            storage_len: self.storage_len,
            error: self.error,
        }
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    fn field(mut self, declaration: u16) -> Self {
        if self.error.is_some() {
            return self;
        }
        let result = (|| {
            if self.layout.header(0) != 9 {
                return Err(ErrorKind::InvalidSchema.into());
            }
            let wire = self
                .layout
                .declaration_index(declaration)
                .ok_or(ErrorKind::IndexOutOfBounds)?;
            if wire >= self.layout.header(3) as usize {
                return Err(ErrorKind::InvalidSchema.into());
            }
            let offset = self.layout.field(wire, false).1;
            let child = self.layout.child(wire, false);
            let base = self
                .base
                .checked_add(offset)
                .ok_or(ErrorKind::ArithmeticOverflow)?;
            let end = offset
                .checked_add(child.header(1))
                .ok_or(ErrorKind::ArithmeticOverflow)?;
            if end as usize > self.bytes.len() {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            Ok((offset, end, base, child))
        })();
        match result {
            Ok((offset, end, base, child)) => {
                self.bytes = &mut self.bytes[offset as usize..end as usize];
                self.base = base;
                self.layout = child;
            }
            Err(error) => self.error = Some(error),
        }
        self
    }

    // Generated fixed ordinals bypass declaration lookup. Check the hint
    // against the inherited certificate so foreign/manual projections retain
    // the ordinary selector's behavior and depth budget.
    #[inline(always)]
    fn field_fixed(mut self, declaration: u16, index: u16, offset: u32) -> Self {
        if self.error.is_some() {
            return self;
        }
        if self.layout.header(0) != 9 || index as u32 >= self.layout.header(3) {
            return self.field(declaration);
        }
        let (actual, position, child) = self.layout.field(index as usize, false);
        if actual != declaration || position != offset {
            return self.field(declaration);
        }
        let Some(base) = self.base.checked_add(offset) else {
            self.error = Some(ErrorKind::ArithmeticOverflow.into());
            return self;
        };
        let bytes = core::mem::take(&mut self.bytes);
        match blubber_core::raw::fixed_slot_region(bytes, offset, child.header(1)) {
            Ok(bytes) => Self {
                bytes,
                base,
                layout: child,
                ..self
            },
            Err(error) => {
                self.error = Some(error);
                self
            }
        }
    }
}

// Scalar projections need neither a descriptor nor account-frame metadata.
// Retaining only their exact loan also keeps callback arguments small on SBF.
// Account ownership must remain available in no-alloc builds.
#[allow(clippy::large_enum_variant)]
pub(crate) enum ScalarEdit<'a> {
    Account(AccountEdit<'a>),
    Region(ScalarRegion<'a>),
}

pub(crate) struct ScalarRegion<'a> {
    bytes: &'a mut [u8],
    poisoned: &'a mut bool,
    // Widen before addition; clamp only when an error needs its wire offset.
    // This equals u32::saturating_add without charging successful scalar reads.
    base: u64,
    state: ScalarState,
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ScalarState {
    Slot,
    Region,
    Error(Error),
}

impl ScalarRegion<'_> {
    #[inline(always)]
    fn check(&self) -> Result<()> {
        if *self.poisoned {
            return Err(ErrorKind::Poisoned.into());
        }
        if let ScalarState::Error(error) = self.state {
            return Err(error);
        }
        Ok(())
    }

    #[inline(always)]
    fn reborrow(&mut self) -> ScalarRegion<'_> {
        ScalarRegion {
            bytes: self.bytes,
            poisoned: self.poisoned,
            base: self.base,
            state: self.state,
        }
    }

    #[inline(always)]
    fn set<T: Fixed>(&mut self, value: T) -> Result<()> {
        self.check()?;
        if matches!(self.state, ScalarState::Region) && self.bytes.len() != T::SIZE as usize {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        let encoded = value.encode();
        let bytes = &mut *self.bytes;
        let poison = &mut *self.poisoned;
        if (!matches!(self.state, ScalarState::Region) && bytes.len() != T::SIZE as usize)
            || encoded.as_ref().len() != bytes.len()
        {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        *poison = true;
        bytes.copy_from_slice(encoded.as_ref());
        drop(encoded);
        *poison = false;
        Ok(())
    }
}

#[inline(never)]
fn read_account_fixed<T: Fixed>(edit: &AccountEdit<'_>) -> Result<T> {
    edit.view()?.read_fixed::<T>()
}

// The width guard folds native decode errors before LLVM prices an ordinary
// closure. Pass only bytes/base so cursor loans can stay off the stack.
#[cold]
#[inline(never)]
fn read_scalar_region_slow<T: Fixed>(bytes: &[u8], base: u64) -> Result<T> {
    T::__decode_region(bytes).map_err(|error| error.rebase(base.min(u32::MAX as u64) as u32))
}

#[inline(never)]
fn set_account_fixed<T: Fixed>(edit: &mut AccountEdit<'_>, value: T) -> Result<()> {
    if let Some(result) = edit.set_fixed(value) {
        return result;
    }
    // Stage wide encodings after resolution. These callbacks run sequentially.
    let encoded = core::cell::Cell::new(None);
    edit.replace_with(
        |_| {
            let bytes = value.encode();
            if bytes.as_ref().len() != T::SIZE as usize {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            encoded.set(Some(bytes));
            Ok((T::SIZE, ()))
        },
        |(), destination| {
            let bytes = encoded.take().expect("measured fixed input");
            destination.copy_from_slice(bytes.as_ref());
        },
    )
}

impl ScalarEdit<'_> {
    #[inline(always)]
    pub(crate) fn reborrow(&mut self) -> ScalarEdit<'_> {
        match self {
            Self::Account(edit) => ScalarEdit::Account(edit.reborrow()),
            Self::Region(region) => ScalarEdit::Region(region.reborrow()),
        }
    }

    #[inline(always)]
    pub(crate) fn read_fixed<T: Fixed>(&self) -> Result<T> {
        match self {
            Self::Account(edit) => read_account_fixed::<T>(edit),
            Self::Region(region) => {
                // Keep the Result<()> check out of callback expansion while
                // preserving poison-before-projection-error precedence.
                if *region.poisoned {
                    return Err(ErrorKind::Poisoned.into());
                }
                if let ScalarState::Error(error) = region.state {
                    return Err(error);
                }
                if region.bytes.len() != T::SIZE as usize {
                    // Preserve even handwritten decoders' behavior on a
                    // mismatched width, including their error and offset.
                    return read_scalar_region_slow::<T>(region.bytes, region.base);
                }
                T::__decode_region(region.bytes)
                    .map_err(|error| error.rebase(region.base.min(u32::MAX as u64) as u32))
            }
        }
    }

    #[inline(always)]
    pub(crate) fn set_fixed<T: Fixed>(&mut self, value: T) -> Result<()> {
        match self {
            Self::Account(edit) => set_account_fixed(edit, value),
            Self::Region(region) => region.set(value),
        }
    }
}

impl<'a> Edit<'a> {
    #[inline(always)]
    pub(crate) fn initialize_measured<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
        measured: crate::edit::MeasuredInit<'_, S>,
    ) -> Result<Self> {
        AccountEdit::initialize_measured::<S>(store, discriminator, measured)
            .map(Self::from_account)
    }

    #[inline(always)]
    pub(crate) fn into_string(self) -> StringEdit<'a> {
        match self.mode {
            Mode::Indexed(loan) => StringEdit::Indexed(loan.unwrap()),
            mode => StringEdit::General(Self { mode }),
        }
    }
    #[inline(always)]
    pub(crate) fn from_indexed(loan: crate::edit::indexed_walk::IndexedLoan<'a>) -> Self {
        Self {
            mode: Mode::Indexed(Some(loan)),
        }
    }

    #[inline(always)]
    fn materialize(&mut self) {
        if matches!(self.mode, Mode::Indexed(_)) {
            self.materialize_indexed();
        }
    }

    #[inline(never)]
    fn materialize_indexed(&mut self) {
        if let Mode::Indexed(loan) = &mut self.mode {
            self.mode = Mode::Account(loan.take().expect("live indexed loan").into_account());
        }
    }

    #[inline(always)]
    fn with_account<R>(&mut self, f: impl FnOnce(&mut AccountEdit<'_>) -> Result<R>) -> Result<R> {
        if let Mode::Indexed(loan) = &mut self.mode {
            return loan.as_mut().expect("live indexed loan").with_account(f);
        }
        f(self.account()?)
    }
    #[inline(always)]
    pub(crate) fn from_slot(slot: Slot<'a>) -> Self {
        Self {
            mode: Mode::Slot(slot),
        }
    }

    /// Opens an account with checked envelope and lazy domain validation.
    #[doc(hidden)]
    #[inline(always)]
    pub fn load_trusted<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
    ) -> Result<Self> {
        Ok(Self {
            mode: Mode::Account(AccountEdit::load_trusted::<S>(store, discriminator)?),
        })
    }

    /// Fully validates an account and recovers poison only on success.
    #[doc(hidden)]
    #[inline(always)]
    pub fn load<S: RootSchema>(store: &'a mut dyn Resize, discriminator: [u8; 8]) -> Result<Self> {
        Ok(Self {
            mode: Mode::Account(AccountEdit::load::<S>(store, discriminator)?),
        })
    }

    /// Initializes a previously uninitialized account from a measured input.
    #[doc(hidden)]
    #[inline(always)]
    pub fn initialize<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
        init: S::Init<'_>,
    ) -> Result<Self> {
        Ok(Self {
            mode: Mode::Account(AccountEdit::initialize::<S>(store, discriminator, init)?),
        })
    }

    #[inline(always)]
    pub(crate) fn fixed_child(self, span: blubber_core::Span, expected: u32) -> Result<Self> {
        match self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => Ok(Self {
                mode: Mode::Region(edit.fixed_child(span, expected)?),
            }),
            Mode::Indexed(loan) => {
                Self::from_account(loan.unwrap().into_account()).fixed_child(span, expected)
            }
            Mode::Region(region) => {
                region.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
            Mode::Slot(slot) => {
                slot.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
        }
    }

    #[inline(always)]
    pub(crate) fn borrow_fixed_list(&mut self, width: u32) -> Result<FixedWalk<'_>> {
        self.account()?.borrow_fixed_list(width)
    }

    #[inline(always)]
    pub(crate) fn fixed_list(self, width: u32) -> Result<FixedWalk<'a>> {
        match self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.fixed_list(width),
            Mode::Indexed(loan) => loan.unwrap().into_account().fixed_list(width),
            Mode::Region(region) => {
                region.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
            Mode::Slot(slot) => {
                slot.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
        }
    }

    #[inline(always)]
    pub(crate) fn borrow_fixed_map<K: crate::MapKey>(
        &mut self,
        width: u32,
    ) -> Result<FixedMapWalk<'_>> {
        match &mut self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => {
                edit.reborrow().fixed_map::<K>(width)
            }
            Mode::Indexed(loan) => loan
                .as_mut()
                .unwrap()
                .reborrow()
                .into_account()
                .fixed_map::<K>(width),
            Mode::Region(region) => {
                region.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
            Mode::Slot(slot) => {
                slot.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
        }
    }

    #[inline(always)]
    pub(crate) fn fixed_map<K: crate::MapKey>(self, width: u32) -> Result<FixedMapWalk<'a>> {
        self.into_account()?.fixed_map::<K>(width)
    }

    #[inline(always)]
    pub(crate) fn from_account(edit: AccountEdit<'a>) -> Self {
        Self {
            mode: Mode::Account(edit),
        }
    }

    #[inline(always)]
    pub(crate) fn into_account(self) -> Result<AccountEdit<'a>> {
        match self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => Ok(edit),
            Mode::Indexed(loan) => Ok(loan.unwrap().into_account()),
            Mode::Slot(slot) => {
                slot.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
            Mode::Region(region) => {
                region.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
        }
    }

    /// Borrows the current exact selection and checks its domain on access.
    #[inline(always)]
    pub fn view(&self) -> Result<View<'_>> {
        match &self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.view(),
            Mode::Indexed(loan) => loan.as_ref().unwrap().view(),
            Mode::Region(region) => region.view(),
            Mode::Slot(slot) => slot.view(),
        }
    }

    #[inline(always)]
    pub(crate) fn read_view(&self) -> Result<View<'_>> {
        match &self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.read_view(),
            Mode::Indexed(loan) => loan.as_ref().unwrap().view(),
            Mode::Region(region) => region.view(),
            Mode::Slot(slot) => slot.view(),
        }
    }

    /// Lends this selection for a shorter exclusive lifetime.
    #[inline(always)]
    pub fn reborrow(&mut self) -> Edit<'_> {
        Edit {
            mode: match &mut self.mode {
                Mode::Account(edit) => Mode::Account(edit.reborrow()),
                Mode::TailCandidate(edit) => Mode::TailCandidate(edit.reborrow()),
                Mode::Indexed(loan) => Mode::Indexed(Some(loan.as_mut().unwrap().reborrow())),
                Mode::Region(region) => Mode::Region(region.reborrow()),
                Mode::Slot(slot) => Mode::Slot(slot.reborrow()),
            },
        }
    }

    /// Reborrows and projects a concrete generated field in one dispatch.
    ///
    /// `tail_prefix` is the certified fixed-prefix width when the parent has
    /// exactly one variable field; other parents pass `None`.
    /// Collection candidates retain this hint in the existing edit dispatch.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_reborrow_certified(
        &mut self,
        declaration: u16,
        offset: Option<u32>,
        selected: CheckedLayout<'static>,
        tail_prefix: Option<u32>,
    ) -> Edit<'_> {
        Edit {
            mode: match &mut self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => {
                    let mut child = edit.reborrow();
                    child.field_certified_hint_mut(declaration, offset, selected, tail_prefix);
                    if tail_prefix.is_some() && matches!(selected.header(0), 7 | 8) {
                        Mode::TailCandidate(child)
                    } else {
                        Mode::Account(child)
                    }
                }
                Mode::Indexed(loan) => {
                    let mut child = loan.as_mut().unwrap().reborrow().into_account();
                    child.field_certified_hint_mut(declaration, offset, selected, tail_prefix);
                    if tail_prefix.is_some() && matches!(selected.header(0), 7 | 8) {
                        Mode::TailCandidate(child)
                    } else {
                        Mode::Account(child)
                    }
                }
                Mode::Region(region) => Mode::Region(region.reborrow().field(declaration)),
                Mode::Slot(slot) => Mode::Region(slot.reborrow().region().field(declaration)),
            },
        }
    }

    /// Reborrows and projects a generic generated field in one dispatch.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_reborrow_static(
        &mut self,
        declaration: u16,
        fixed: Option<(u16, u32)>,
        variable: u16,
    ) -> Edit<'_> {
        Edit {
            mode: match &mut self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => {
                    let mut child = edit.reborrow();
                    child.field_static_mut(declaration, fixed, variable);
                    Mode::Account(child)
                }
                Mode::Indexed(loan) => {
                    let mut child = loan.as_mut().unwrap().reborrow().into_account();
                    child.field_static_mut(declaration, fixed, variable);
                    Mode::Account(child)
                }
                Mode::Region(region) => Mode::Region(match fixed {
                    Some((index, offset)) => {
                        region.reborrow().field_fixed(declaration, index, offset)
                    }
                    None => region.reborrow().field(declaration),
                }),
                Mode::Slot(slot) => match fixed {
                    Some((index, offset)) => Mode::Region(slot.reborrow().region().field_fixed(
                        declaration,
                        index,
                        offset,
                    )),
                    None => Mode::Region(slot.reborrow().region().field(declaration)),
                },
            },
        }
    }

    /// Reborrows a generated fixed field using its certified wire ordinal.
    ///
    /// Exact loans check the ordinal and offset against their inherited table
    /// and lend only that field. A mismatched hint uses declaration selection.
    /// Account selections retain their existing checked fixed-offset path.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_reborrow_fixed(
        &mut self,
        declaration: u16,
        index: u16,
        offset: u32,
        selected: CheckedLayout<'static>,
    ) -> Edit<'_> {
        Edit {
            mode: match &mut self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => {
                    let mut child = edit.reborrow();
                    child.field_certified_hint_mut(declaration, Some(offset), selected, None);
                    Mode::Account(child)
                }
                Mode::Indexed(loan) => {
                    let mut child = loan.as_mut().unwrap().reborrow().into_account();
                    child.field_certified_hint_mut(declaration, Some(offset), selected, None);
                    Mode::Account(child)
                }
                Mode::Region(region) => {
                    Mode::Region(region.reborrow().field_fixed(declaration, index, offset))
                }
                Mode::Slot(slot) => Mode::Region(slot.reborrow().region().field_fixed(
                    declaration,
                    index,
                    offset,
                )),
            },
        }
    }

    /// Projects a declaration ordinal without decoding its value.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field(self, declaration: u16) -> Self {
        Self {
            mode: match self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => {
                    Mode::Account(edit.field(declaration))
                }
                Mode::Indexed(loan) => {
                    Mode::Account(loan.unwrap().into_account().field(declaration))
                }
                Mode::Region(region) => Mode::Region(region.field(declaration)),
                Mode::Slot(slot) => Mode::Region(slot.region().field(declaration)),
            },
        }
    }

    /// Projects a generated field using certified wire metadata.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_static(self, declaration: u16, fixed: Option<(u16, u32)>, variable: u16) -> Self {
        Self {
            mode: match self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => {
                    Mode::Account(edit.field_static(declaration, fixed, variable))
                }
                Mode::Indexed(loan) => Mode::Account(loan.unwrap().into_account().field_static(
                    declaration,
                    fixed,
                    variable,
                )),
                Mode::Region(region) => Mode::Region(match fixed {
                    Some((index, offset)) => region.field_fixed(declaration, index, offset),
                    None => region.field(declaration),
                }),
                Mode::Slot(slot) => match fixed {
                    Some((index, offset)) => {
                        Mode::Region(slot.region().field_fixed(declaration, index, offset))
                    }
                    None => Mode::Region(slot.region().field(declaration)),
                },
            },
        }
    }

    /// Projects a concrete generated field with its certified child table.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_certified(
        self,
        declaration: u16,
        offset: Option<u32>,
        selected: CheckedLayout<'static>,
    ) -> Self {
        Self {
            mode: match self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => {
                    Mode::Account(edit.field_certified(declaration, offset, selected))
                }
                Mode::Indexed(loan) => Mode::Account(loan.unwrap().into_account().field_certified(
                    declaration,
                    offset,
                    selected,
                )),
                Mode::Region(region) => Mode::Region(region.field(declaration)),
                Mode::Slot(slot) => Mode::Region(slot.region().field(declaration)),
            },
        }
    }

    #[inline(always)]
    pub(crate) fn index(mut self, index: u32) -> Self {
        if let Mode::Account(edit) | Mode::TailCandidate(edit) = &mut self.mode {
            edit.select_mut(blubber_core::raw::Step::Index(index));
            return self;
        }
        Self {
            mode: match self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => Mode::Account(edit.index(index)),
                Mode::Indexed(loan) => Mode::Account(loan.unwrap().into_account().index(index)),
                Mode::Slot(slot) => {
                    let mut region = slot.region();
                    region.error = Some(ErrorKind::InvalidSchema.into());
                    Mode::Region(region)
                }
                Mode::Region(mut region) => {
                    region.error = Some(ErrorKind::InvalidSchema.into());
                    Mode::Region(region)
                }
            },
        }
    }

    #[inline(always)]
    pub(crate) fn some(mut self) -> Self {
        if let Mode::Account(edit) | Mode::TailCandidate(edit) = &mut self.mode {
            edit.select_mut(blubber_core::raw::Step::Some);
            return self;
        }
        Self {
            mode: match self.mode {
                Mode::Account(edit) | Mode::TailCandidate(edit) => Mode::Account(edit.some()),
                Mode::Indexed(loan) => Mode::Account(loan.unwrap().into_account().some()),
                Mode::Slot(slot) => {
                    let mut region = slot.region();
                    region.error = Some(ErrorKind::InvalidSchema.into());
                    Mode::Region(region)
                }
                Mode::Region(mut region) => {
                    region.error = Some(ErrorKind::InvalidSchema.into());
                    Mode::Region(region)
                }
            },
        }
    }

    pub(crate) fn check_path(&self) -> Result<()> {
        match &self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.check_path(),
            Mode::Indexed(loan) => loan.as_ref().unwrap().check(),
            Mode::Region(region) => region.check(),
            Mode::Slot(slot) => slot.check(),
        }
    }

    #[inline(always)]
    pub(crate) fn into_scalar(self) -> ScalarEdit<'a> {
        match self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => ScalarEdit::Account(edit),
            Mode::Indexed(loan) => ScalarEdit::Account(loan.unwrap().into_account()),
            Mode::Region(region) => ScalarEdit::Region(ScalarRegion {
                bytes: region.bytes,
                poisoned: region.poisoned,
                base: u64::from(region.base),
                state: region.error.map_or(ScalarState::Region, ScalarState::Error),
            }),
            Mode::Slot(slot) => ScalarEdit::Region(ScalarRegion {
                bytes: slot.bytes,
                poisoned: slot.poisoned,
                base: u64::from(slot.meta.base) + u64::from(slot.offset as u32),
                state: ScalarState::Slot,
            }),
        }
    }

    #[inline(always)]
    pub(crate) fn select_source<R>(
        &mut self,
        select: impl FnOnce(&Self) -> Result<R>,
    ) -> Result<R> {
        let result = select(self);
        result.map_err(|error| self.selection_error(error))
    }

    #[cold]
    #[inline(never)]
    fn selection_error(&mut self, error: Error) -> Error {
        match &mut self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.source_error(error),
            Mode::Indexed(loan) => loan.as_mut().unwrap().source_error(error),
            Mode::Region(region) => {
                let _ = crate::edit::list_pass::source::<()>(Err(error), region.poisoned);
                error
            }
            Mode::Slot(slot) => {
                let _ = crate::edit::list_pass::source::<()>(Err(error), slot.poisoned);
                error
            }
        }
    }

    #[inline(always)]
    fn account(&mut self) -> Result<&mut AccountEdit<'a>> {
        self.materialize();
        match &mut self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => Ok(edit),
            Mode::Indexed(_) => unreachable!("materialized indexed loan"),
            Mode::Slot(slot) => {
                slot.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
            Mode::Region(region) => {
                region.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
        }
    }

    #[inline(always)]
    fn account_route(&mut self) -> Result<(&mut AccountEdit<'a>, AccountRoute)> {
        self.materialize();
        match &mut self.mode {
            Mode::Account(edit) => Ok((edit, AccountRoute::Account)),
            Mode::TailCandidate(edit) => Ok((edit, AccountRoute::Tail)),
            Mode::Indexed(_) => unreachable!("materialized indexed loan"),
            Mode::Slot(slot) => {
                slot.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
            Mode::Region(region) => {
                region.check()?;
                Err(ErrorKind::InvalidSchema.into())
            }
        }
    }

    /// Returns the containing account's logical used length.
    pub fn used_len(&self) -> Result<u32> {
        match &self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.used_len(),
            Mode::Indexed(loan) => loan.as_ref().unwrap().used_len(),
            Mode::Slot(slot) => {
                slot.check()?;
                Ok(slot.meta.used)
            }
            Mode::Region(region) => {
                region.check()?;
                Ok(region.used)
            }
        }
    }

    /// Returns the containing account's exposed physical length.
    pub fn storage_len(&self) -> Result<u32> {
        match &self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.storage_len(),
            Mode::Indexed(loan) => loan.as_ref().unwrap().storage_len(),
            Mode::Slot(slot) => {
                slot.check()?;
                Ok(slot.meta.storage_len)
            }
            Mode::Region(region) => {
                region.check()?;
                Ok(region.storage_len)
            }
        }
    }

    /// Validates the complete selected subtree without changing poison.
    pub fn validate(&self) -> Result<()> {
        self.view()?.validate()
    }

    pub(crate) fn verify(&self) -> Result<()> {
        match &self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.verify_root(),
            // Exact child loans deliberately carry no enclosing immutable slice.
            _ => Err(ErrorKind::InvalidBoundary.into()),
        }
    }

    /// Ensures physical room beyond the root's logical length.
    pub fn reserve(&mut self, additional: u32) -> Result<()> {
        self.account()?.reserve(additional)
    }

    /// Attempts or schedules physical compaction of the root account.
    pub fn shrink_to_fit(&mut self) -> Result<bool> {
        self.account()?.shrink_to_fit()
    }

    /// Consumes and completes an account root; child selections are rejected.
    #[inline(always)]
    pub fn commit(self) -> Result<CommitInfo> {
        self.into_account()?.commit()
    }
}

#[inline(never)]
fn read_selected_string<'s>(edit: &'s Edit<'_>) -> Result<&'s str> {
    edit.view()?.read_str()
}

impl Edit<'_> {
    #[inline(always)]
    pub(crate) fn replace_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        if let Mode::Indexed(loan) = &mut self.mode {
            return loan.as_mut().unwrap().replace_with(measure, encode);
        }
        self.account()?.replace_with::<I>(measure, encode)
    }
    #[inline(always)]
    fn replace_source_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        if let Mode::Indexed(loan) = &mut self.mode {
            return loan.as_mut().unwrap().replace_source_with(measure, encode);
        }
        self.account()?.replace_source_with(measure, encode)
    }

    #[inline(always)]
    pub(crate) fn try_replace_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]) -> Result<()>,
    ) -> Result<()> {
        self.with_account(|edit| edit.try_replace_with::<I>(measure, encode))
    }

    #[inline(always)]
    pub(crate) fn fill_fixed_list<T: Schema>(&mut self, value: &T::Init<'_>) -> Result<()> {
        self.with_account(
            #[inline(always)]
            |edit| edit.fill_fixed_list::<T>(value),
        )
    }

    #[inline(always)]
    pub(crate) fn rewrite_with<C: crate::rewrite::FillLayout, I>(
        &mut self,
        measure: impl FnOnce(
            View<'_>,
            bool,
        ) -> crate::edit::Preparation<(blubber_core::raw::RewritePlan, I)>,
        encode: impl FnOnce(I, &mut blubber_core::raw::RewriteStream<'_>) -> Result<()>,
    ) -> Result<()> {
        self.with_account(
            #[inline(always)]
            |edit| edit.rewrite_with::<C, _>(measure, encode),
        )
    }
    #[inline(always)]
    pub(crate) fn try_string(&mut self, input: &[u8]) -> Result<bool> {
        if let Mode::Indexed(loan) = &mut self.mode {
            return loan.as_mut().unwrap().try_string(input);
        }
        self.account()?.try_string(input)
    }
    #[inline(always)]
    pub(crate) fn replace_bytes(&mut self, input: &[u8]) -> Result<()> {
        self.with_account(|edit| edit.replace_bytes(input))
    }
    #[inline(always)]
    pub(crate) fn try_fixed_map<T: Schema>(
        &mut self,
        key: &[u8],
        input: &T::Init<'_>,
    ) -> Result<bool> {
        self.account()?.try_fixed_map::<T>(key, input)
    }
    #[inline(always)]
    pub(crate) fn try_tail<T: Schema>(
        &mut self,
        key: Option<&[u8]>,
        input: &T::Init<'_>,
    ) -> Result<bool> {
        self.account()?.try_tail::<T>(key, input)
    }
    #[inline(always)]
    pub(crate) fn try_clear_list(&mut self) -> Option<Result<()>> {
        self.materialize();
        match &mut self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => edit.try_clear_list(),
            Mode::Region(_) | Mode::Slot(_) => None,
            Mode::Indexed(_) => unreachable!("materialized indexed loan"),
        }
    }

    pub(crate) fn try_bulk_insert<T: Schema>(
        &mut self,
        index: Option<usize>,
        values: &[T::Init<'_>],
    ) -> Option<Result<()>> {
        self.materialize();
        match &mut self.mode {
            Mode::Account(edit) | Mode::TailCandidate(edit) => {
                edit.try_bulk_insert::<T>(index, values)
            }
            Mode::Region(_) | Mode::Slot(_) => None,
            Mode::Indexed(_) => unreachable!("materialized indexed loan"),
        }
    }

    #[inline(always)]
    pub(crate) fn list_edit<T: Schema, R>(
        &mut self,
        operation: ListRequest,
        input: Option<&T::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        let (edit, route) = self.account_route()?;
        if matches!(route, AccountRoute::Tail) {
            edit.list_tail_edit::<T, R>(operation, input, read_removed)
        } else {
            edit.list_edit::<T, R>(operation, input, read_removed)
        }
    }
    #[inline(always)]
    #[cfg(feature = "alloc")]
    pub(crate) fn list_encoded<R>(
        &mut self,
        operation: ListRequest,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        self.account()?
            .owned_list_encoded::<R>(operation, measure, read_removed, encode)
    }
    #[inline(always)]
    pub(crate) fn map_edit<V: Schema, R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        input: Option<&V::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        let (edit, route) = self.account_route()?;
        if matches!(route, AccountRoute::Tail) {
            edit.map_tail_edit::<V, R>(operation, key, input, read_removed)
        } else {
            edit.map_edit::<V, R>(operation, key, input, read_removed)
        }
    }
    #[inline(always)]
    pub(crate) fn map_entry<V: Schema, R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        hint: blubber_core::Search,
        input: Option<&V::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        self.account()?
            .map_entry::<V, R>(operation, key, hint, input, read_removed)
    }
    #[inline(always)]
    #[cfg(feature = "alloc")]
    pub(crate) fn map_encoded<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        self.account()?
            .dispatch_map_encoded::<R>(operation, key, measure, read_removed, encode)
    }
    #[inline(always)]
    pub(crate) fn list_remove_into(&mut self, index: usize, destination: &mut [u8]) -> Result<u32> {
        self.account()?.list_remove_into(index, destination)
    }
    #[inline(always)]
    pub(crate) fn map_remove_into(
        &mut self,
        key: &[u8],
        destination: &mut [u8],
    ) -> Result<Option<u32>> {
        self.account()?.map_remove_into(key, destination)
    }
}

#[cfg(all(test, feature = "derive"))]
impl Edit<'_> {
    pub(crate) fn replace_bytes_generic(&mut self, input: &[u8]) -> Result<()> {
        self.account()?.replace_bytes_generic(input)
    }
}
