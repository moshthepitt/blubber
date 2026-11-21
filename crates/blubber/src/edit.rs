//! Exclusive account roots with checked lifecycle and short read borrows.

use crate::storage::Backend as Resize;
use blubber_core::raw::{InlinePath, ListEdit, Plan, Step};
use blubber_core::{checked_u32, CheckedLayout, Layout, Span};

use crate::{CommitInfo, Error, ErrorKind, Result, RootSchema, Schema, View};

// Keep the full comparison out of the ingress pointer fast path.
#[cold]
#[inline(never)]
fn same_words(left: &[u32], right: &[u32]) -> bool {
    left == right
}

#[path = "indexed_walk.rs"]
pub(crate) mod indexed_walk;

#[path = "edit/list_pass.rs"]
pub(crate) mod list_pass;

#[path = "edit/bulk_pass.rs"]
mod bulk_pass;

/// Owns an initializer and its measured length across preflight and encoding.
/// The input cannot be replaced after measurement.
pub struct MeasuredInit<'i, S: Schema> {
    prepared: blubber_core::encoding_replay::Prepared<InitSource<'i, S>>,
}

// Typed projection only. The marker contract supplies purity for covered
// graphs; default-false callbacks retain every call through Prepared::replay.
struct InitSource<'i, S: Schema> {
    init: S::Init<'i>,
}

impl<S: Schema> blubber_core::encoding_replay::Input for InitSource<'_, S> {
    #[inline(always)]
    fn measure(&self) -> Result<u32> {
        S::measure(&self.init)
    }

    #[inline(always)]
    fn encode(&self, destination: &mut [u8]) {
        S::encode(&self.init, destination);
    }
}

impl<'i, S: Schema> MeasuredInit<'i, S> {
    /// Measures and retains the exact initializer.
    pub fn new(init: S::Init<'i>) -> Result<Self> {
        let prepared = blubber_core::encoding_replay::Prepared::new(InitSource::<S> { init })?;
        Ok(Self { prepared })
    }

    /// Checks the envelope plus measured payload length.
    pub fn used(&self) -> Result<u32> {
        crate::schema::checked_add(12, self.prepared.measured_len())
    }

    /// Rejects a payload below the schema minimum.
    pub fn check_minimum(&self) -> Result<()> {
        if self.prepared.measured_len() < S::MIN_SIZE {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        Ok(())
    }
}

/// An exclusive account session, borrowing its storage rather than its bytes.
///
/// Reads borrow the current storage briefly, so reserve may safely relocate it
/// after those reads end. Generated account handles wrap this shared engine.
#[repr(C)]
pub(crate) struct AccountEdit<'a> {
    path_error: Option<ErrorKind>,
    // Keep policy in prefix padding, retaining the original hot-field offsets.
    // Authenticated history never supplies a full-validation certificate.
    lazy: bool,
    store: &'a mut dyn Resize,
    layout: CheckedLayout<'static>,
    minimum: u32,
    discriminator: [u8; 8],
    selected: CheckedLayout<'static>,
    path: InlinePath,
    root: bool,
    ingress: Option<ValidatedIngress>,
    fixed_offset: Offset,
}

// Affine evidence: only full validation creates this token. Reborrowing moves
// it into the child and leaves the parent empty. Every mutable storage access
// destroys it, so no retained parent can certify bytes changed by a child.
struct ValidatedIngress;

#[derive(Clone, Copy)]
enum Offset {
    Fixed(u32),
    Tail(u32),
    Unknown,
}
impl Offset {
    #[inline(always)]
    fn base(self) -> Option<u32> {
        match self {
            Self::Fixed(x) | Self::Tail(x) => Some(x),
            Self::Unknown => None,
        }
    }
    #[inline(always)]
    fn fixed(self) -> Option<u32> {
        match self {
            Self::Fixed(x) => Some(x),
            _ => None,
        }
    }
    #[inline(always)]
    fn field(self, parent: CheckedLayout<'static>, fixed: Option<u32>) -> Self {
        if let (Some(base), Some(offset)) = (self.base(), fixed) {
            return base.checked_add(offset).map_or(Self::Unknown, Self::Fixed);
        }
        if let Self::Tail(base) = self {
            if parent.header(0) == 9 && parent.header(4) == 1 {
                #[cfg(all(test, feature = "derive"))]
                crate::edit_routes::hit(0);
                return base
                    .checked_add(parent.header(1))
                    .map_or(Self::Unknown, Self::Tail);
            }
        }
        Self::Unknown
    }
}

macro_rules! source_result {
    ($edit:expr, $result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => return Err($edit.source_error(error)),
        }
    };
}

impl<'a> AccountEdit<'a> {
    #[cold]
    #[inline(never)]
    pub(crate) fn source_error(&mut self, error: Error) -> Error {
        if self.lazy {
            let mut poisoned = false;
            let _: Result<()> = list_pass::source(Err(error), &mut poisoned);
            if poisoned {
                self.store.set_poisoned(true);
            }
        }
        error
    }

    /// Opens an account, checking state and its envelope without scanning data.
    ///
    /// Trust requires bytes written by this program through blubber, with owner
    /// and discriminator checked. Edits check only touched structure; malformed
    /// directories can alias siblings within the account.
    #[doc(hidden)]
    #[inline(always)]
    pub fn load_trusted<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
    ) -> Result<Self> {
        check_state(store, false)?;
        View::load_trusted::<S>(store.bytes(), discriminator)?;
        // A shared mutable temporary adds SBF v0 instructions to native edits.
        #[cfg(not(feature = "verify-on-load"))]
        {
            // control: lazy-ingress
            Ok(Self::new::<S>(store, discriminator, true))
        }
        #[cfg(feature = "verify-on-load")]
        {
            let mut edit = Self::new::<S>(store, discriminator, false);
            edit.ingress = Some(ValidatedIngress);
            Ok(edit)
        }
    }

    /// Fully validates an account, clearing poison only after validation passes.
    ///
    /// This is the recovery path after an interrupted host encoding. Invalid
    /// bytes leave the poison flag set; a closed session cannot be reopened.
    #[doc(hidden)]
    #[inline(always)]
    pub fn load<S: RootSchema>(store: &'a mut dyn Resize, discriminator: [u8; 8]) -> Result<Self> {
        validate_load::<S>(store, discriminator)?;
        let mut edit = Self::new::<S>(store, discriminator, false);
        edit.ingress = Some(ValidatedIngress);
        Ok(edit)
    }

    /// Initializes uninitialized storage after completing all fallible checks.
    ///
    /// A short or empty zero discriminator prefix is accepted, allowing an
    /// empty Vec to grow. Any nonzero identity byte is `AlreadyInitialized` at
    /// zero. Success zeroes spare storage and publishes the envelope last.
    /// Errors before encoding preserve bytes, length and poison state. An
    /// unexpected encoder panic leaves the store poisoned.
    #[doc(hidden)]
    pub fn initialize<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
        init: S::Init<'_>,
    ) -> Result<Self> {
        check_state(store, false)?;
        let root_len = S::measure(&init)?;
        Self::initialize_encoding::<S>(store, discriminator, InitSource { init }, root_len)
    }

    pub(crate) fn initialize_measured<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
        measured: MeasuredInit<'_, S>,
    ) -> Result<Self> {
        check_state(store, false)?;
        let policy = blubber_core::encoding_publication::init_policy(
            crate::product::markers::<S>(),
            false,
            cfg!(feature = "verify-on-load"),
        );
        let (source, root_len) = if policy.replay {
            measured.prepared.replay()?
        } else {
            measured.prepared.into_measured()
        };
        Self::initialize_encoding::<S>(store, discriminator, source, root_len)
    }

    fn initialize_encoding<S: RootSchema>(
        store: &'a mut dyn Resize,
        discriminator: [u8; 8],
        source: InitSource<'_, S>,
        root_len: u32,
    ) -> Result<Self> {
        let used = crate::schema::checked_add(12, root_len)?;
        if discriminator == [0; 8] {
            return Err(ErrorKind::InvalidSchema.into());
        }
        View::inherited(
            &[],
            Layout::Flat {
                checked: S::__layout(),
            },
            12,
        )?;
        if root_len < S::MIN_SIZE {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        checked_u32(store.storage_len())?;
        if store.bytes().iter().take(8).any(|byte| *byte != 0) {
            return Err(Error::new(ErrorKind::AlreadyInitialized, 0));
        }
        store.check_initialize_resize(used as usize)?;
        if used as usize > store.storage_len() {
            store.resize(used as usize)?;
        }
        store.set_poisoned(true);
        let bytes = store.bytes_mut();
        bytes.fill(0);
        // Measurement and any required replay completed before storage changed.
        blubber_core::encoding_replay::write(&source, bytes, 12, root_len, false)?;
        bytes[..8].copy_from_slice(&discriminator);
        bytes[8..12].copy_from_slice(&used.to_le_bytes());
        #[cfg(feature = "verify-on-load")]
        View::load::<S>(bytes, discriminator)?;
        store.set_poisoned(false);
        #[allow(unused_mut)]
        let mut edit = Self::new::<S>(store, discriminator, false);
        #[cfg(not(feature = "verify-on-load"))]
        {
            edit.lazy = edit.store.authenticated();
        }
        #[cfg(feature = "verify-on-load")]
        {
            edit.ingress = Some(ValidatedIngress);
        }
        Ok(edit)
    }

    #[inline(always)]
    fn new<S: RootSchema>(store: &'a mut dyn Resize, discriminator: [u8; 8], lazy: bool) -> Self {
        Self {
            store,
            layout: S::__layout(),
            minimum: S::MIN_SIZE,
            discriminator,
            selected: S::__layout(),
            path: InlinePath::new(),
            path_error: None,
            root: true,
            ingress: None,
            lazy,
            fixed_offset: if S::FIXED_SIZE.is_some() {
                Offset::Fixed(0)
            } else {
                Offset::Tail(0)
            },
        }
    }

    pub(crate) fn verify_root(&self) -> Result<()> {
        if !self.root {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        self.root_view()?.validate()
    }

    fn root_view(&self) -> Result<View<'_>> {
        check_state(self.store, false)?;
        View::load_layout(
            self.store.bytes(),
            self.discriminator,
            Layout::Flat {
                checked: self.layout,
            },
            self.minimum,
        )
    }

    /// Borrows the selected region after checking state, envelope and path.
    ///
    /// A root view checks only its envelope. Fixed prefixes use checked offsets;
    /// other nested views check visited headers and selected adjacent ends.
    /// Full validation checks untouched boundaries, key order and child domains.
    /// No saved byte address survives an edit.
    pub fn view(&self) -> Result<View<'_>> {
        let root = self.root_view()?;
        self.check_path()?;
        if self.root {
            return Ok(root);
        }
        if let Offset::Tail(offset) = self.fixed_offset {
            let bytes = root
                .bytes()
                .get(offset as usize..)
                .ok_or(ErrorKind::InvalidBoundary)?;
            return View::inherited(
                bytes,
                Layout::Flat {
                    checked: self.selected,
                },
                12 + offset,
            );
        }
        if let Some(offset) = self.fixed_offset.fixed() {
            let end = offset
                .checked_add(self.selected.header(1))
                .ok_or(ErrorKind::ArithmeticOverflow)?;
            let bytes = root
                .bytes()
                .get(offset as usize..end as usize)
                .ok_or(ErrorKind::InvalidBoundary)?;
            return View::inherited(
                bytes,
                Layout::Flat {
                    checked: self.selected,
                },
                12 + offset,
            );
        }
        let span = blubber_core::raw::resolve_local(
            self.store.bytes(),
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
        )?;
        View::inherited(
            &self.store.bytes()[span.start as usize..span.end as usize],
            Layout::Flat {
                checked: self.selected,
            },
            span.start,
        )
    }

    // Tail reads need no transported root descriptor. Recheck the current
    // state/envelope before the saved path error, as the general view does.
    #[inline(always)]
    pub(crate) fn read_view(&self) -> Result<View<'_>> {
        if let Offset::Tail(offset) = self.fixed_offset {
            check_state(self.store, false)?;
            let bytes = self.store.bytes();
            let used = blubber_core::raw::account_used(bytes, self.discriminator, self.minimum)?;
            self.check_path()?;
            if offset > used - 12 {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            let base = 12 + offset;
            return View::inherited_for_edit(
                &bytes[base as usize..used as usize],
                Layout::Flat {
                    checked: self.selected,
                },
                base,
            );
        }
        self.view()
    }

    /// Temporarily reborrows this selection, retaining its integer path.
    #[inline(always)]
    pub fn reborrow(&mut self) -> AccountEdit<'_> {
        AccountEdit {
            store: self.store,
            layout: self.layout,
            minimum: self.minimum,
            discriminator: self.discriminator,
            selected: self.selected,
            path: self.path,
            path_error: self.path_error,
            root: self.root,
            ingress: self.ingress.take(),
            lazy: self.lazy,
            fixed_offset: self.fixed_offset,
        }
    }

    /// Appends a declaration ordinal without reading storage.
    ///
    /// Generated depth checks make this infallible for supported schemas.
    /// Misuse records a sticky error reported by the next checked operation.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field(mut self, declaration: u16) -> Self {
        self.select_mut(Step::Field(declaration));
        self
    }

    /// Projects a generated field using its constant wire ordinal and prefix offset.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_static(
        mut self,
        declaration: u16,
        fixed: Option<(u16, u32)>,
        variable: u16,
    ) -> Self {
        self.field_static_mut(declaration, fixed, variable);
        self
    }

    #[inline(always)]
    pub(crate) fn field_static_mut(
        &mut self,
        declaration: u16,
        fixed: Option<(u16, u32)>,
        variable: u16,
    ) {
        if self.path_error.is_none() {
            if let (Some(base), Some((index, offset))) = (self.fixed_offset.base(), fixed) {
                if let Some(absolute) = base.checked_add(offset) {
                    self.root = false;
                    self.fixed_offset = Offset::Fixed(absolute);
                    self.selected = self.selected.child(index as usize, false);
                    return;
                }
            }
        }
        self.root = false;
        if self.path.push(Step::Field(declaration)).is_err() {
            self.path_error = Some(ErrorKind::DepthExceeded);
        }
        if self.path_error.is_none() {
            if let Some((index, offset)) = fixed {
                self.fixed_offset = self.fixed_offset.field(self.selected, Some(offset));
                self.selected = self.selected.child(index as usize, false);
            } else {
                self.fixed_offset = self.fixed_offset.field(self.selected, None);
                self.selected = self.selected.child(variable as usize, true);
            }
        }
    }

    /// Projects a concrete generated field using its certified child metadata.
    ///
    /// Generated callers supply the declared child certificate and its fixed
    /// prefix offset, or `None` for a variable field.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_certified(
        mut self,
        declaration: u16,
        offset: Option<u32>,
        selected: CheckedLayout<'static>,
    ) -> Self {
        self.field_certified_mut(declaration, offset, selected);
        self
    }

    #[inline(always)]
    pub(crate) fn field_certified_mut(
        &mut self,
        declaration: u16,
        offset: Option<u32>,
        selected: CheckedLayout<'static>,
    ) {
        if self.path_error.is_none() {
            if let (Some(base), Some(offset)) = (self.fixed_offset.base(), offset) {
                if let Some(absolute) = base.checked_add(offset) {
                    self.root = false;
                    self.fixed_offset = Offset::Fixed(absolute);
                    self.selected = selected;
                    return;
                }
            }
        }
        self.root = false;
        if self.path.push(Step::Field(declaration)).is_err() {
            self.path_error = Some(ErrorKind::DepthExceeded);
        }
        if self.path_error.is_none() {
            self.fixed_offset = self.fixed_offset.field(self.selected, offset);
            self.selected = selected;
        }
    }

    #[inline(always)]
    pub(crate) fn field_certified_hint_mut(
        &mut self,
        declaration: u16,
        offset: Option<u32>,
        selected: CheckedLayout<'static>,
        tail_prefix: Option<u32>,
    ) {
        if self.path_error.is_none() {
            if let (Some(base), Some(offset)) = (self.fixed_offset.base(), offset) {
                if let Some(absolute) = base.checked_add(offset) {
                    self.root = false;
                    self.fixed_offset = Offset::Fixed(absolute);
                    self.selected = selected;
                    return;
                }
            }
        }
        self.root = false;
        if self.path.push(Step::Field(declaration)).is_err() {
            self.path_error = Some(ErrorKind::DepthExceeded);
        }
        if self.path_error.is_none() {
            self.fixed_offset = match (self.fixed_offset, offset, tail_prefix) {
                (Offset::Tail(base), None, Some(prefix)) => base
                    .checked_add(prefix)
                    .map_or(Offset::Unknown, Offset::Tail),
                _ => Offset::Unknown,
            };
            self.selected = selected;
        }
    }

    pub(crate) fn index(mut self, index: u32) -> Self {
        self.select_mut(Step::Index(index));
        self
    }

    pub(crate) fn some(mut self) -> Self {
        self.select_mut(Step::Some);
        self
    }

    pub(crate) fn check_path(&self) -> Result<()> {
        self.path_error.map_or(Ok(()), |kind| Err(kind.into()))
    }

    #[inline(always)]
    pub(crate) fn select_mut(&mut self, step: Step) {
        self.root = false;
        if self.path.push(step).is_err() {
            self.path_error = Some(ErrorKind::DepthExceeded);
        }
        if self.path_error.is_none() {
            let parent = self.selected;
            self.fixed_offset = match (parent.header(0), step) {
                (9, Step::Field(id)) => {
                    parent
                        .declaration_index(id)
                        .map_or(Offset::Unknown, |wire| {
                            let fixed = (wire < parent.header(3) as usize)
                                .then(|| parent.field(wire, false).1);
                            self.fixed_offset.field(parent, fixed)
                        })
                }
                _ => Offset::Unknown,
            };
            let child = match (self.selected.header(0), step) {
                (9, Step::Field(id)) => self
                    .selected
                    .declaration_index(id)
                    .map(|wire| {
                        let nf = self.selected.header(3) as usize;
                        self.selected
                            .child(if wire >= nf { wire - nf } else { wire }, wire >= nf)
                    })
                    .ok_or(ErrorKind::IndexOutOfBounds),
                (7 | 8, Step::Index(_)) | (6, Step::Some) => Ok(self.selected.child(0, true)),
                _ => Err(ErrorKind::InvalidSchema),
            };
            match child {
                Ok(layout) => self.selected = layout,
                Err(kind) => self.path_error = Some(kind),
            }
        }
    }

    // The parent view and the collection kernel established the absolute span.
    // Reacquire the storage loan and use the proved overwrite gateway; no byte
    // address or storage guard is detached from this exclusive session.
    #[inline(always)]
    pub(crate) fn fixed_child(
        mut self,
        span: Span,
        expected: u32,
    ) -> Result<crate::edit_handle::Region<'a>> {
        check_state(self.store, false)?;
        self.check_path()?;
        let child = self.selected.child(0, true);
        let fixed =
            matches!(child.header(0), 1..=4) || (child.header(0) == 9 && child.header(4) == 0);
        if !fixed || span.end.checked_sub(span.start) != Some(expected) {
            return Err(ErrorKind::InvalidSchema.into());
        }
        let offset = span
            .start
            .checked_sub(12)
            .ok_or(ErrorKind::InvalidBoundary)?;
        self.ingress = None;
        let access = self.store.exact_access();
        let storage_len = checked_u32(access.bytes.len())?;
        let used = blubber_core::read_u32(access.bytes, 8)?;
        let bytes = blubber_core::raw::fixed_destination(
            access.bytes,
            self.discriminator,
            self.minimum,
            None,
            offset,
            expected,
        )?;
        Ok(crate::edit_handle::Region {
            bytes,
            poisoned: access.poisoned,
            layout: child,
            base: span.start,
            used,
            storage_len,
            error: None,
        })
    }

    #[inline(always)]
    pub(crate) fn borrow_fixed_list(
        &mut self,
        expected: u32,
    ) -> Result<crate::edit_handle::FixedWalk<'_>> {
        if let Offset::Tail(offset) = self.fixed_offset {
            if self.path_error.is_none() && self.selected.header(0) == 7 {
                let child = self.selected.child(0, true);
                let fixed = fixed_list_child(child);
                if fixed && child.header(1) == expected {
                    self.ingress = None;
                    #[cfg(all(test, feature = "derive"))]
                    crate::edit_routes::hit(1);
                    return tail_walk(
                        self.store.fixed_access()?,
                        &self.lazy,
                        self.discriminator,
                        self.minimum,
                        offset,
                        expected,
                        child,
                    );
                }
            }
        }
        self.reborrow().fixed_list(expected)
    }

    #[inline(always)]
    pub(crate) fn fixed_list(mut self, expected: u32) -> Result<crate::edit_handle::FixedWalk<'a>> {
        if let Offset::Tail(offset) = self.fixed_offset {
            if self.path_error.is_none() && self.selected.header(0) == 7 {
                let child = self.selected.child(0, true);
                let fixed = fixed_list_child(child);
                if fixed && child.header(1) == expected {
                    #[cfg(all(test, feature = "derive"))]
                    crate::edit_routes::hit(2);
                    return tail_walk(
                        self.store.fixed_access()?,
                        &self.lazy,
                        self.discriminator,
                        self.minimum,
                        offset,
                        expected,
                        child,
                    );
                }
            }
        }
        let root = source_result!(self, self.root_view());
        source_result!(self, self.check_path());
        let used = 12 + root.bytes().len() as u32;
        let view = if let Some(declaration) = self.path.direct_field() {
            source_result!(self, root.field(declaration))
        } else {
            source_result!(self, self.view())
        };
        let header = source_result!(self, view.list_header());
        let width = header.width.ok_or(ErrorKind::InvalidSchema)?;
        if width != expected {
            return Err(ErrorKind::InvalidSchema.into());
        }
        let base = source_result!(
            self,
            view.base()
                .checked_add(4)
                .ok_or_else(|| ErrorKind::ArithmeticOverflow.into())
        );
        let end = view.base() as usize + view.bytes().len();
        let layout = self.selected.child(0, true);
        self.ingress = None;
        let access = self.store.exact_access();
        let storage_len = access.bytes.len() as u32;
        Ok(crate::edit_handle::FixedWalk {
            bytes: &mut access.bytes[base as usize..end],
            poisoned: access.poisoned,
            meta: crate::edit_handle::FixedMeta {
                lazy: crate::edit_handle::WalkPolicy::Checked,
                layout,
                base,
                used,
                storage_len,
                payload_len: end - base as usize,
            },
        })
    }

    // Recheck the selector against the root certificate before reusing ingress.
    // Generated hints are public hidden APIs; foreign hints must still scan.
    fn ingress_covers(&self, view: View<'_>) -> bool {
        if self.ingress.is_none() {
            return false;
        }
        let actual = (|| -> Result<View<'_>> {
            let mut actual = self.root_view()?;
            for index in 0..blubber_core::MAX_DEPTH {
                let Some(step) = self.path.step(index)? else {
                    break;
                };
                actual = match step {
                    Step::Field(declaration) => actual.field(declaration)?,
                    Step::Index(index) => match actual.layout().head() {
                        blubber_core::LayoutHead::List { .. } => {
                            actual.list_header()?.element(actual, index)?
                        }
                        blubber_core::LayoutHead::Map { .. } => {
                            actual.map_header()?.value(actual, index)?
                        }
                        _ => return Err(ErrorKind::InvalidSchema.into()),
                    },
                    Step::Some => actual.option()?.ok_or(ErrorKind::InvalidSchema)?,
                };
            }
            Ok(actual)
        })();
        let Ok(actual) = actual else {
            return false;
        };
        let Layout::Flat { checked } = actual.layout() else {
            return false;
        };
        // Equal words, root and budget identify the same layout regardless of const addresses.
        actual.base() == view.base()
            && actual.bytes().len() == view.bytes().len()
            && (core::ptr::eq(checked.words(), self.selected.words())
                || same_words(checked.words(), self.selected.words()))
            && checked.root() == self.selected.root()
            && checked.budget() == self.selected.budget()
    }

    #[inline(always)]
    pub(crate) fn fixed_map<K: crate::MapKey>(
        mut self,
        expected: u32,
    ) -> Result<crate::edit_handle::FixedMapWalk<'a>> {
        let view = source_result!(self, self.view());
        let header = source_result!(self, view.map_header());
        if header.width != Some(expected) || header.key_width != K::SIZE {
            return Err(ErrorKind::InvalidSchema.into());
        }
        // Tiny maps cost less to validate directly than to compare ingress identity.
        // control: cursor-scan
        // control: fixed-map-scan
        if !self.lazy && (header.count <= 1 || !self.ingress_covers(view)) {
            #[cfg(all(test, feature = "derive"))]
            crate::edit_routes::hit(3);
            source_result!(
                self,
                view.relative(blubber_core::validate_map_keys(
                    view.bytes(),
                    &view.layout(),
                ))
            );
        }
        let base = source_result!(
            self,
            view.base()
                .checked_add(4)
                .ok_or_else(|| ErrorKind::ArithmeticOverflow.into())
        );
        let end = view.base() as usize + view.bytes().len();
        let layout = self.selected.child(0, true);
        let used = source_result!(self, blubber_core::read_u32(self.store.bytes(), 8));
        self.ingress = None;
        let access = self.store.exact_access();
        let storage_len = access.bytes.len() as u32;
        Ok(crate::edit_handle::FixedMapWalk {
            walk: crate::edit_handle::FixedWalk {
                bytes: &mut access.bytes[base as usize..end],
                poisoned: access.poisoned,
                meta: crate::edit_handle::FixedMeta {
                    lazy: crate::edit_handle::WalkPolicy::new(self.lazy),
                    layout,
                    base,
                    used,
                    storage_len,
                    payload_len: end - base as usize,
                },
            },
        })
    }

    #[inline(always)]
    pub(crate) fn set_fixed<T: crate::Fixed>(&mut self, value: T) -> Option<Result<()>> {
        if self.path_error.is_some() {
            return None;
        }
        let offset = self.fixed_offset.fixed()?;
        Some(self.overwrite_fixed(offset, value))
    }

    #[inline(always)]
    fn overwrite_fixed<T: crate::Fixed>(&mut self, offset: u32, value: T) -> Result<()> {
        check_state(self.store, false)?;
        self.check_path()?;
        let root_tag = self.layout.header(0);
        let root_width = (matches!(root_tag, 1..=4)
            || (root_tag == 9 && self.layout.header(4) == 0))
            .then(|| self.layout.header(1));
        if self.selected.header(1) != T::SIZE {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        self.ingress = None;
        let access = self.store.exact_access();
        let destination = blubber_core::raw::fixed_destination(
            access.bytes,
            self.discriminator,
            self.minimum,
            root_width,
            offset,
            T::SIZE,
        )?;
        let encoded = value.encode();
        if encoded.as_ref().len() != destination.len() {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        *access.poisoned = true;
        destination.copy_from_slice(encoded.as_ref());
        drop(encoded);
        *access.poisoned = false;
        Ok(())
    }

    // Resolve before measuring user input. The same exclusive bytes then flow
    // through planning, old-value inspection and the phase-zero transition.
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

    // Fixed fills retain one checked payload loan and never change its extent.
    #[inline(always)]
    pub(crate) fn fill_fixed_list<T: Schema>(&mut self, value: &T::Init<'_>) -> Result<()> {
        check_state(self.store, false)?;
        self.check_path()?;
        let width = T::FIXED_SIZE.ok_or(ErrorKind::InvalidSchema)?;
        let (hard, limit, limit_error) = {
            let access = self.store.edit_access();
            (access.hard_limit, access.limit, access.limit_error)
        };
        let walk = self.borrow_fixed_list(width)?;
        if walk.bytes.is_empty() {
            return Ok(());
        }
        if T::measure(value)? != width {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        if walk.meta.used as usize > hard {
            return Err(ErrorKind::Capacity.into());
        }
        if walk.meta.used as usize > limit {
            return Err(limit_error.into());
        }
        *walk.poisoned = true;
        crate::rewrite::encode_fixed_fill::<T, crate::List<T>>(value, walk.bytes)?;
        *walk.poisoned = false;
        Ok(())
    }

    // Keep batch plans in their own SBF frame when a caller uses several APIs.
    #[cfg_attr(target_os = "solana", inline(never))]
    pub(crate) fn rewrite_with<C: crate::rewrite::FillLayout, I>(
        &mut self,
        measure: impl FnOnce(View<'_>, bool) -> Preparation<(blubber_core::raw::RewritePlan, I)>,
        encode: impl FnOnce(I, &mut blubber_core::raw::RewriteStream<'_>) -> Result<()>,
    ) -> Result<()> {
        check_state(self.store, false)?;
        self.check_path()?;
        // The rewrite resolver selects from the root's real descriptor and path,
        // rather than public projection hints. Exclusive ingress therefore covers
        // the exact view passed to measurement. Consume it before mutable access.
        let admitted = self.ingress.take().is_some();
        let lazy = self.lazy;
        crate::rewrite::rewrite_collection_policy(
            self.store,
            lazy,
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            (self.discriminator, self.minimum),
            #[inline(always)]
            |view| {
                // The typed collection supplies lazy admission; the existing
                // plan still checks the selected descriptor and geometry.
                let fixed_map = lazy && C::FIXED_MAP;
                // control: rewrite-scan
                measure(view, admitted || fixed_map)
            },
            encode,
        )
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    fn replace_with_policy<const SOURCE: bool, I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<()> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        let mut stage = EditStage {
            input: None,
            removed: Err(ErrorKind::InvalidPlan.into()),
            encode: Some(encode),
        };
        self.ingress = None;
        let access = self.store.edit_access();
        let outcome = stage_replacement::<SOURCE, _, _>(
            self.lazy,
            blubber_core::raw::resolve_account_edit(
                access.bytes,
                self.discriminator,
                self.minimum,
                &Layout::Flat {
                    checked: self.layout,
                },
                &self.path,
            ),
            access.poisoned,
            (access.hard_limit, access.limit, access.limit_error),
            &mut LoanStage {
                data: &mut stage,
                #[cfg(feature = "pinocchio")]
                preflight: access.preflight,
            },
            measure,
        );
        if !matches!(outcome, Ok(None)) {
            finish_growth(outcome, self.store, &mut stage)?;
        }
        stage.removed
    }

    // A fallible encoder runs only after replace_with has armed poison. Keep
    // that state on failure even when the outer replacement can finish framing.
    #[inline(always)]
    pub(crate) fn try_replace_with<I>(
        &mut self,
        measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8]) -> Result<()>,
    ) -> Result<()> {
        let mut encoded = Ok(());
        self.replace_with(measure, |input, destination| {
            encoded = encode(input, destination);
        })?;
        if encoded.is_err() {
            self.store.set_poisoned(true);
        }
        encoded
    }

    #[inline(always)]
    pub(crate) fn try_string(&mut self, input: &[u8]) -> Result<bool> {
        let Some(declaration) = self.path.direct_field() else {
            return Ok(false);
        };
        // Trying a tail loan before the general replacement pays off only for
        // the final variable field. Other fields keep the checked subtree loan.
        let variables = self.layout.header(4);
        if self.layout.header(0) != 9
            || variables == 0
            || self.layout.field_word(variables as usize - 1, true, 1) != u32::from(declaration)
        {
            return Ok(false);
        }
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            return Ok(false);
        }
        self.ingress = None;
        let access = self.store.edit_access();
        #[cfg(all(test, feature = "derive"))]
        crate::edit_routes::hit(4);
        let prepared = match blubber_core::raw::prepare_string_write(
            access.bytes,
            self.discriminator,
            self.minimum,
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            input.len(),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(input_extent_error(
                    error,
                    access.bytes.len(),
                    self.lazy,
                    access.poisoned,
                ))
            }
        };
        if let Some(mut write) = prepared {
            if let Some(required) = write.growth_len() {
                if required as usize > access.hard_limit {
                    return Err(ErrorKind::Capacity.into());
                }
                if required as usize > access.limit {
                    return Err(access.limit_error.into());
                }
            }
            #[cfg(feature = "pinocchio")]
            access
                .preflight
                .check(write.growth_len().unwrap_or(0) as usize)?;
            *access.poisoned = true;
            write.destination().copy_from_slice(input);
            write.finish();
            *access.poisoned = false;
            return Ok(true);
        }
        Ok(false)
    }

    // StrMut first tries the direct string loan. Nested replacements retain
    // the immediate checked subtree loan and detached growth fallback.
    #[inline(always)]
    pub(crate) fn replace_bytes(&mut self, input: &[u8]) -> Result<()> {
        if self.lazy {
            self.replace_bytes_checked(input)
        } else {
            self.replace_bytes_strict(input)
        }
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    fn replace_bytes_checked(&mut self, input: &[u8]) -> Result<()> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        {
            self.ingress = None;
            let access = self.store.edit_access();
            #[cfg(feature = "pinocchio")]
            let old_used = blubber_core::read_u32(access.bytes, 8)? as usize;
            let prepared = blubber_core::raw::prepare_checked_byte_replacement(
                access.bytes,
                self.discriminator,
                self.minimum,
                self.layout,
                &self.path,
                input.len(),
                access.poisoned,
            );
            if !matches!(
                &prepared,
                Err(Error::Fault {
                    kind: ErrorKind::Capacity,
                    ..
                })
            ) {
                return prepared.and_then(|write| {
                    let required = write.required_len() as usize;
                    if required > access.hard_limit {
                        write.check_before_error(access.poisoned)?;
                        return Err(ErrorKind::Capacity.into());
                    }
                    if required > access.limit {
                        write.check_before_error(access.poisoned)?;
                        return Err(access.limit_error.into());
                    }
                    #[cfg(feature = "pinocchio")]
                    if required > old_used {
                        if let Err(error) = access.preflight.check(required) {
                            write.check_before_error(access.poisoned)?;
                            return Err(error);
                        }
                    }
                    let mut write = write.move_regions(access.poisoned).destination();
                    write.bytes().copy_from_slice(input);
                    write.finish(access.poisoned)
                });
            }
        }
        self.replace_bytes_generic(input)
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    fn replace_bytes_strict(&mut self, input: &[u8]) -> Result<()> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        {
            self.ingress = None;
            let access = self.store.edit_access();
            #[cfg(feature = "pinocchio")]
            let old_used = blubber_core::read_u32(access.bytes, 8)? as usize;
            match blubber_core::raw::prepare_byte_replacement(
                access.bytes,
                self.discriminator,
                self.minimum,
                self.layout,
                &self.path,
                input.len(),
            ) {
                Ok(mut write) => {
                    let required = write.required_len() as usize;
                    if required > access.hard_limit {
                        return Err(ErrorKind::Capacity.into());
                    }
                    if required > access.limit {
                        return Err(access.limit_error.into());
                    }
                    #[cfg(feature = "pinocchio")]
                    if required > old_used {
                        access.preflight.check(required)?;
                    }
                    *access.poisoned = true;
                    write.destination().copy_from_slice(input);
                    write.finish();
                    *access.poisoned = false;
                    return Ok(());
                }
                Err(Error::Fault {
                    kind: ErrorKind::Capacity,
                    ..
                }) => (),
                Err(error) => return Err(error),
            }
        }
        self.replace_bytes_generic(input)
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    pub(crate) fn replace_bytes_generic(&mut self, input: &[u8]) -> Result<()> {
        #[cfg(all(test, feature = "derive"))]
        crate::edit_routes::hit(5);
        let mut data;
        let outcome = {
            self.ingress = None;
            let access = self.store.edit_access();
            let resolved = blubber_core::raw::resolve_account_edit(
                access.bytes,
                self.discriminator,
                self.minimum,
                &Layout::Flat {
                    checked: self.layout,
                },
                &self.path,
            )?;
            let len = checked_u32(input.len())?;
            let span = resolved.selection().0;
            if len == span.end - span.start {
                let destination = resolved.into_destination();
                *access.poisoned = true;
                destination.copy_from_slice(input);
                *access.poisoned = false;
                return Ok(());
            }
            data = EditStage {
                input: None,
                removed: Err(ErrorKind::InvalidPlan.into()),
                encode: Some(|(), destination: &mut [u8]| destination.copy_from_slice(input)),
            };
            replace_generic_different(
                self.lazy,
                resolved,
                access.poisoned,
                (access.hard_limit, access.limit, access.limit_error),
                &mut LoanStage {
                    data: &mut data,
                    #[cfg(feature = "pinocchio")]
                    preflight: access.preflight,
                },
                len,
                (),
                span,
            )
        };
        if !matches!(outcome, Ok(None)) {
            finish_growth(outcome, self.store, &mut data)?;
        }
        data.removed
    }

    #[inline(always)]
    pub(crate) fn try_fixed_map<T: Schema>(
        &mut self,
        key: &[u8],
        input: &T::Init<'_>,
    ) -> Result<bool> {
        let Some(width) = T::FIXED_SIZE else {
            return Ok(false);
        };
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            return Ok(false);
        }
        self.ingress = None;
        let access = self.store.edit_access();
        let Some(mut tail) = (match blubber_core::raw::prepare_fixed_insert(
            access.bytes,
            self.discriminator,
            self.minimum,
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            width,
            key,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(input_extent_error(
                    error,
                    access.bytes.len(),
                    self.lazy,
                    access.poisoned,
                ))
            }
        }) else {
            return Ok(false);
        };
        let required = tail.required_len() as usize;
        if required > access.hard_limit {
            return Err(ErrorKind::Capacity.into());
        }
        if required > access.limit {
            return Err(access.limit_error.into());
        }
        #[cfg(feature = "pinocchio")]
        access.preflight.check(required)?;
        if T::measure(input)? != width {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        *access.poisoned = true;
        T::encode(input, tail.destination());
        tail.finish();
        *access.poisoned = false;
        Ok(true)
    }
    #[inline(always)]
    pub(crate) fn try_tail<T: Schema>(
        &mut self,
        key: Option<&[u8]>,
        input: &T::Init<'_>,
    ) -> Result<bool> {
        let Some(width) = T::FIXED_SIZE else {
            return Ok(false);
        };
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            return Ok(false);
        }
        self.ingress = None;
        let access = self.store.edit_access();
        let Some(mut tail) = (match blubber_core::raw::prepare_tail_append(
            access.bytes,
            self.discriminator,
            self.minimum,
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            width,
            key,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(input_extent_error(
                    error,
                    access.bytes.len(),
                    self.lazy,
                    access.poisoned,
                ))
            }
        }) else {
            return Ok(false);
        };
        let required = tail.required_len() as usize;
        if required > access.hard_limit {
            return Err(ErrorKind::Capacity.into());
        }
        if required > access.limit {
            return Err(access.limit_error.into());
        }
        #[cfg(feature = "pinocchio")]
        access.preflight.check(required)?;
        if T::measure(input)? != width {
            return Err(ErrorKind::InvalidBoundary.into());
        }
        *access.poisoned = true;
        Self::encode_list_child::<T>(input, tail.destination());
        tail.finish();
        *access.poisoned = false;
        Ok(true)
    }

    // Retain direct codec calls for inputs larger than 512 bytes.
    #[inline(always)]
    fn encode_list_child<T: Schema>(input: &T::Init<'_>, destination: &mut [u8]) {
        if core::mem::size_of::<T::Init<'_>>() > 512 {
            T::encode(input, destination);
        } else {
            Self::encode_small_list_child::<T>(input, destination);
        }
    }

    // Keep primitive codec temporaries outside the edit frame on SBF.
    #[cfg_attr(target_os = "solana", inline(never))]
    fn encode_small_list_child<T: Schema>(input: &T::Init<'_>, destination: &mut [u8]) {
        T::encode(input, destination);
    }

    // A lazy clear discards every old end. The generic subtree loan retains
    // ancestor admission and publication; its input is a complete empty list.
    pub(crate) fn try_clear_list(&mut self) -> Option<Result<()>> {
        if !self.lazy {
            return None;
        }
        let result = self.replace_with(
            |view| {
                view.list_header()?;
                Ok((4, ()))
            },
            |(), destination| {
                blubber_core::patch_pass::write_empty_list(destination)
                    .expect("exact empty-list loan");
            },
        );
        if matches!(
            result,
            Err(Error::Fault {
                kind: ErrorKind::Truncated
                    | ErrorKind::Discriminator
                    | ErrorKind::UsedLength
                    | ErrorKind::ArithmeticOverflow
                    | ErrorKind::InvalidBoundary,
                ..
            })
        ) {
            // control: clear-poison
            self.store.set_poisoned(true);
        }
        Some(result)
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    pub(crate) fn list_edit<T: Schema, R>(
        &mut self,
        operation: ListRequest,
        input: Option<&T::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        let measure = || Ok(input.map(T::measure).transpose()?.unwrap_or(0));
        let encode = |destination: &mut [u8]| {
            if let Some(input) = input {
                Self::encode_list_child::<T>(input, destination);
            }
        };
        if T::FIXED_SIZE.is_some() {
            self.list_encoded(operation, measure, read_removed, encode)
        } else {
            self.dispatch_list_encoded(operation, measure, read_removed, encode)
        }
    }

    /// Tries a checked tail loan before the unchanged staged planner.
    #[inline(always)]
    pub(crate) fn list_tail_edit<T: Schema, R>(
        &mut self,
        operation: ListRequest,
        input: Option<&T::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        let mut read_removed = Some(read_removed);
        // Wide Removed values retain the existing staged return frame.
        if core::mem::size_of::<R>() <= 512 && input.is_none() {
            if let ListRequest::Remove(index) = operation {
                if let Some(result) = self.try_tail_remove(index, None, &mut read_removed)? {
                    return Ok(result);
                }
            }
        }
        self.list_edit::<T, R>(
            operation,
            input,
            read_removed.expect("unused removal reader"),
        )
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        not(all(target_os = "solana", feature = "max-depth-16")),
        inline(always)
    )]
    pub(crate) fn list_encoded<R>(
        &mut self,
        operation: ListRequest,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        let child = self.collection_child()?;
        let lazy = self.lazy;
        self.execute::<true, _, _>(
            |resolved| {
                let span = resolved.selection().0;
                let count = blubber_core::read_u32(resolved.inspect_resolved(), span.start)?;
                let operation = operation.checked(count)?;
                let len = measure().map_err(PreparationError::input)?;
                if lazy && resolved.has_ancestor_run(2) {
                    prepared_input(
                        resolved.prepare_list_ancestors(operation, len, u32::MAX),
                        (),
                    )
                } else {
                    prepared_input(resolved.prepare_list(operation, len, u32::MAX), ())
                }
            },
            |span, bytes, _| read_removed(removed_view(span, bytes, child)?),
            |(), destination| encode(destination),
        )
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        not(all(target_os = "solana", feature = "max-depth-16")),
        inline(always)
    )]
    pub(crate) fn map_edit<V: Schema, R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        input: Option<&V::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        if V::FIXED_SIZE.is_some() {
            self.map_encoded::<R>(
                operation,
                key,
                || Ok(input.map(V::measure).transpose()?.unwrap_or(0)),
                read_removed,
                |destination| {
                    if let Some(input) = input {
                        V::encode(input, destination);
                    }
                },
            )
        } else {
            self.dispatch_map_encoded::<R>(
                operation,
                key,
                || Ok(input.map(V::measure).transpose()?.unwrap_or(0)),
                read_removed,
                {
                    #[inline(always)]
                    |destination| {
                        if let Some(input) = input {
                            V::encode(input, destination);
                        }
                    }
                },
            )
        }
    }

    /// Tries a checked tail loan before the unchanged staged planner.
    pub(crate) fn map_tail_edit<V: Schema, R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        input: Option<&V::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        let mut read_removed = Some(read_removed);
        if core::mem::size_of::<R>() <= 512
            && matches!(operation, blubber_core::raw::MapEdit::Remove)
            && input.is_none()
        {
            if let Some(result) = self.try_tail_remove(0, Some(key), &mut read_removed)? {
                return Ok(result);
            }
        }
        self.map_edit::<V, R>(
            operation,
            key,
            input,
            read_removed.expect("unused removal reader"),
        )
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    pub(crate) fn map_entry<V: Schema, R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        hint: blubber_core::Search,
        input: Option<&V::Init<'_>>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
    ) -> Result<R> {
        if V::FIXED_SIZE.is_some() {
            self.map_encoded_at::<R>(
                operation,
                key,
                Some(hint),
                || Ok(input.map(V::measure).transpose()?.unwrap_or(0)),
                read_removed,
                |destination| {
                    if let Some(input) = input {
                        V::encode(input, destination);
                    }
                },
            )
        } else {
            self.dispatch_map_entry_encoded::<R>(
                operation,
                key,
                hint,
                || Ok(input.map(V::measure).transpose()?.unwrap_or(0)),
                read_removed,
                {
                    #[inline(always)]
                    |destination| {
                        if let Some(input) = input {
                            V::encode(input, destination);
                        }
                    }
                },
            )
        }
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        not(all(target_os = "solana", feature = "max-depth-16")),
        inline(always)
    )]
    pub(crate) fn map_encoded<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        self.map_encoded_at(operation, key, None, measure, read_removed, encode)
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        all(target_os = "solana", not(feature = "max-depth-16")),
        inline(always)
    )]
    pub(crate) fn map_encoded_at<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        hint: Option<blubber_core::Search>,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        let child = self.collection_child()?;
        let lazy = self.lazy;
        self.execute::<false, _, _>(
            |resolved| {
                let len = measure().map_err(PreparationError::input)?;
                if lazy && resolved.has_ancestor_words() {
                    return prepared_input(
                        resolved.prepare_map_ancestors(operation, key, len, u32::MAX, hint),
                        (),
                    );
                }
                match hint {
                    Some(hint) => prepared_input(
                        resolved.prepare_entry(operation, key, len, u32::MAX, hint),
                        (),
                    ),
                    None => prepared_input(resolved.prepare_map(operation, key, len, u32::MAX), ()),
                }
            },
            |span, bytes, _| read_removed(removed_view(span, bytes, child)?),
            |(), destination| encode(destination),
        )
    }

    pub(crate) fn list_remove_into(&mut self, index: usize, destination: &mut [u8]) -> Result<u32> {
        let child = self.collection_child()?;
        let lazy = self.lazy;
        if self.checked_list_enabled() {
            return self.execute_checked_list(
                ListRequest::Remove(index),
                || Ok(0),
                |span, bytes| {
                    extract_value(span, bytes, child, destination)?
                        .ok_or(ErrorKind::InvalidPlan.into())
                },
                |destination| assert!(destination.is_empty(), "removal has no input"),
            );
        }
        self.execute::<true, _, _>(
            |resolved| {
                let span = resolved.selection().0;
                let count = blubber_core::read_u32(resolved.inspect_resolved(), span.start)?;
                let operation = ListRequest::Remove(index).checked(count)?;
                if lazy && resolved.has_ancestor_words() {
                    prepared_input(resolved.prepare_list_ancestors(operation, 0, u32::MAX), ())
                } else {
                    prepared_input(resolved.prepare_list(operation, 0, u32::MAX), ())
                }
            },
            |span, bytes, _| {
                extract_value(span, bytes, child, destination)?.ok_or(ErrorKind::InvalidPlan.into())
            },
            |(), destination| assert!(destination.is_empty(), "removal has no input"),
        )
    }

    pub(crate) fn map_remove_into(
        &mut self,
        key: &[u8],
        destination: &mut [u8],
    ) -> Result<Option<u32>> {
        let child = self.collection_child()?;
        let lazy = self.lazy;
        if self.checked_map_enabled() {
            return self.execute_checked_map::<false, _>(
                blubber_core::raw::MapEdit::Remove,
                key,
                None,
                || Ok(0),
                |span, bytes| extract_value(span, bytes, child, destination),
                |destination| assert!(destination.is_empty(), "removal has no input"),
            );
        }
        self.execute::<false, _, _>(
            |resolved| {
                if lazy && resolved.has_ancestor_words() {
                    return prepared_input(
                        resolved.prepare_map_ancestors(
                            blubber_core::raw::MapEdit::Remove,
                            key,
                            0,
                            u32::MAX,
                            None,
                        ),
                        (),
                    );
                }
                prepared_input(
                    resolved.prepare_map(blubber_core::raw::MapEdit::Remove, key, 0, u32::MAX),
                    (),
                )
            },
            |span, bytes, _| extract_value(span, bytes, child, destination),
            |(), destination| assert!(destination.is_empty(), "removal has no input"),
        )
    }

    #[inline(always)]
    fn try_tail_remove<R>(
        &mut self,
        index: usize,
        key: Option<&[u8]>,
        read_removed: &mut Option<impl FnOnce(Option<View<'_>>) -> Result<R>>,
    ) -> Result<Option<R>> {
        // Avoid byte admission when cached geometry selects the staged path.
        if !matches!(self.fixed_offset, Offset::Tail(_)) {
            return Ok(None);
        }
        let child = self.collection_child()?;
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            return Ok(None);
        }
        self.ingress = None;
        let access = self.store.edit_access();
        let Some(removal) = blubber_core::raw::prepare_tail_remove(
            access.bytes,
            self.discriminator,
            self.minimum,
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            index,
            key,
        )
        .inspect_err(|error| {
            list_pass::poison_source_error(self.lazy, *error, access.poisoned);
        })?
        else {
            return Ok(None);
        };
        let required = removal.required_len() as usize;
        if required > access.hard_limit {
            return Err(ErrorKind::Capacity.into());
        }
        if required > access.limit {
            return Err(access.limit_error.into());
        }
        let view = removed_view(removal.removed_span(), removal.inspect(), child);
        let view = view.inspect_err(|error| {
            list_pass::poison_source_error(self.lazy, *error, access.poisoned);
        })?;
        let result =
            read_removed.take().expect("one removal reader")(view).inspect_err(|error| {
                list_pass::poison_source_error(self.lazy, *error, access.poisoned);
            })?;
        *access.poisoned = true;
        if key.is_some() {
            removal.finish_fixed_map();
        } else {
            removal.finish();
        }
        *access.poisoned = false;
        Ok(Some(result))
    }

    #[inline(always)]
    fn collection_child(&self) -> Result<Layout> {
        match self.selected.header(0) {
            7 | 8 => Ok(Layout::Flat {
                checked: self.selected.child(0, true),
            }),
            _ => Err(ErrorKind::InvalidSchema.into()),
        }
    }

    // R cannot borrow inspection bytes. I contains only input/coordinates, not
    // account references. Backend growth is the sole detached-plan boundary.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        all(target_os = "solana", not(feature = "max-depth-16")),
        inline(always)
    )]
    fn execute<'key, const LIST_FRAME: bool, I, R>(
        &mut self,
        prepare: impl for<'b> FnOnce(
            blubber_core::raw::Resolved<'b>,
        ) -> Preparation<(PreparedEdit<'b, 'key>, I)>,
        read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
        encode: impl FnOnce(I, &mut [u8]),
    ) -> Result<R> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        let mut stage = EditStage {
            input: None,
            removed: Err(ErrorKind::InvalidPlan.into()),
            encode: Some(encode),
        };
        self.ingress = None;
        let access = self.store.edit_access();
        let outcome = stage_bytes::<LIST_FRAME, _, _, _>(
            self.lazy,
            access,
            (self.discriminator, self.minimum),
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            prepare,
            &mut stage,
            read_removed,
        );
        // The exact byte loan has ended: only a detached Plan can reach growth.
        if !matches!(outcome, Ok(None)) {
            finish_growth(outcome, self.store, &mut stage)?;
        }
        stage.removed
    }

    /// Returns the checked logical account length, including the envelope.
    pub fn used_len(&self) -> Result<u32> {
        Ok(12 + self.root_view()?.bytes().len() as u32)
    }

    /// Returns the checked physical length, including spare capacity.
    pub fn storage_len(&self) -> Result<u32> {
        check_state(self.store, false)?;
        checked_u32(self.store.storage_len())
    }

    /// Ensures physical room beyond the current logical account length.
    pub fn reserve(&mut self, additional: u32) -> Result<()> {
        self.require_root()?;
        let used = self.used_len()?;
        self.ingress = None;
        self.store.reserve(used, additional as usize)
    }

    /// Attempts or schedules physical compaction to the logical account length.
    pub fn shrink_to_fit(&mut self) -> Result<bool> {
        self.require_root()?;
        let used = self.used_len()?;
        self.ingress = None;
        self.store.shrink_to_fit(used)
    }

    /// Consumes the root and completes its storage session.
    /// A failed root completion also closes the backend without settlement.
    pub fn commit(self) -> Result<CommitInfo> {
        match self.require_root().and_then(|()| self.used_len()) {
            Ok(used) => self.store.commit(used),
            Err(error) => {
                if self.root && !self.store.is_closed() {
                    // No trustworthy length remains. Poison prevents a backend
                    // from trimming or settling with a fabricated used length.
                    self.store.set_poisoned(true);
                    let _ = self.store.commit(0);
                }
                Err(error)
            }
        }
    }

    fn require_root(&self) -> Result<()> {
        check_state(self.store, false)?;
        self.check_path()?;
        if self.root {
            Ok(())
        } else {
            Err(ErrorKind::InvalidSchema.into())
        }
    }
}

#[inline(always)]
fn fixed_list_child(child: CheckedLayout<'static>) -> bool {
    matches!(child.header(0), 1..=4) || (child.header(0) == 9 && child.header(4) == 0)
}

#[inline(always)]
fn tail_walk<'a>(
    access: crate::storage::private::FixedAccess<'a>,
    lazy: &bool,
    discriminator: [u8; 8],
    minimum: u32,
    offset: u32,
    width: u32,
    layout: CheckedLayout<'static>,
) -> Result<crate::edit_handle::FixedWalk<'a>> {
    let physical = access.bytes.len();
    let (bytes, used, _count) = match blubber_core::raw::fixed_list_payload(
        access.bytes,
        discriminator,
        minimum,
        offset,
        width,
    ) {
        Ok(payload) => payload,
        Err(error) => {
            let lazy = *lazy;
            let error = match error {
                Error::Fault {
                    kind: ErrorKind::ArithmeticOverflow,
                    offset: Error::NO_OFFSET,
                } if lazy => Error::new(ErrorKind::ArithmeticOverflow, 12 + offset),
                other => other,
            };
            list_pass::poison_source_error(lazy, error, access.poisoned);
            return Err(error);
        }
    };
    // Successful kernel admission proves the complete physical length fits u32.
    let storage_len = physical as u32;
    let payload_len = bytes.len();
    Ok(crate::edit_handle::FixedWalk {
        bytes,
        poisoned: access.poisoned,
        meta: crate::edit_handle::FixedMeta {
            lazy: crate::edit_handle::WalkPolicy::Checked,
            layout,
            base: 16 + offset,
            used,
            storage_len,
            payload_len,
        },
    })
}

#[inline(never)]
fn validate_load<S: RootSchema>(store: &mut dyn Resize, discriminator: [u8; 8]) -> Result<()> {
    check_state(store, true)?;
    View::load::<S>(store.bytes(), discriminator)?;
    store.set_poisoned(false);
    Ok(())
}

#[inline(always)]
fn check_state(store: &dyn Resize, allow_poison: bool) -> Result<()> {
    if store.is_closed() {
        Err(ErrorKind::Closed.into())
    } else if store.is_poisoned() && !allow_poison {
        Err(ErrorKind::Poisoned.into())
    } else {
        Ok(())
    }
}

// These direct gateways report source header failures with a wire offset.
// With a representable source, offset-free overflow names the requested extent.
#[cold]
fn input_extent_error(error: Error, physical: usize, lazy: bool, poisoned: &mut bool) -> Error {
    if matches!(
        error,
        Error::Fault {
            kind: ErrorKind::ArithmeticOverflow,
            offset: Error::NO_OFFSET,
        }
    ) && physical <= u32::MAX as usize
    {
        return error;
    }
    list_pass::poison_source_error(lazy, error, poisoned);
    error
}

// Generic checked ingress keeps its pre-write frame; lazy fixed entries use a fallible ancestor publication.
// Preparation distinguishes caller measurement failures from source planning.
// Only error variants carry this distinction; admitted plans keep their ABI.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PreparationError {
    InputFault { kind: ErrorKind, offset: u32 },
    SourceFault { kind: ErrorKind, offset: u32 },
    Program { code: u64 },
}
pub(crate) type Preparation<T> = core::result::Result<T, PreparationError>;
// Origin must fit the old error ABI; admitted plans retain their checked frame.
const _: () = assert!(core::mem::size_of::<PreparationError>() == core::mem::size_of::<Error>());
impl From<Error> for PreparationError {
    #[inline(always)]
    fn from(error: Error) -> Self {
        if matches!(error, Error::Fault { offset, .. } if offset != Error::NO_OFFSET) {
            Self::source(error)
        } else {
            Self::input(error)
        }
    }
}
impl From<ErrorKind> for PreparationError {
    fn from(kind: ErrorKind) -> Self {
        Self::input(kind.into())
    }
}
impl PreparationError {
    #[inline(always)]
    pub(crate) fn input(error: Error) -> Self {
        match error {
            Error::Fault { kind, offset } => Self::InputFault { kind, offset },
            Error::Program { code } => Self::Program { code },
        }
    }
    #[inline(always)]
    pub(crate) fn source(error: Error) -> Self {
        match error {
            Error::Fault { kind, offset } => Self::SourceFault { kind, offset },
            Error::Program { code } => Self::Program { code },
        }
    }
    #[inline(always)]
    pub(crate) fn into_error(self) -> Error {
        match self {
            Self::InputFault { kind, offset } | Self::SourceFault { kind, offset } => {
                Error::Fault { kind, offset }
            }
            Self::Program { code } => Error::Program { code },
        }
    }
    #[cold]
    pub(crate) fn report(self, lazy: bool, poisoned: &mut bool) -> Error {
        if let Self::SourceFault { kind, offset } = self {
            let error = Error::Fault { kind, offset };
            if lazy {
                let _ = list_pass::source::<()>(Err(error), poisoned);
            }
            error
        } else {
            self.into_error()
        }
    }
}

enum PreparedEdit<'a, 'key> {
    Strict(blubber_core::raw::Prepared<'a, 'key>),
    Ancestors(blubber_core::raw::AncestorPrepared<'a, 'key>),
}
impl<'a, 'key> From<blubber_core::raw::Prepared<'a, 'key>> for PreparedEdit<'a, 'key> {
    #[inline(always)]
    fn from(p: blubber_core::raw::Prepared<'a, 'key>) -> Self {
        Self::Strict(p)
    }
}
impl<'a, 'key> From<blubber_core::raw::AncestorPrepared<'a, 'key>> for PreparedEdit<'a, 'key> {
    #[inline(always)]
    fn from(p: blubber_core::raw::AncestorPrepared<'a, 'key>) -> Self {
        Self::Ancestors(p)
    }
}
impl<'key> PreparedEdit<'_, 'key> {
    fn check_before_error(&self, poisoned: &mut bool) -> Result<()> {
        match self {
            Self::Strict(_) => Ok(()),
            Self::Ancestors(p) => p.check_before_error(poisoned),
        }
    }
    #[inline(always)]
    fn prepared_len(&self) -> u32 {
        match self {
            Self::Strict(p) => p.prepared_len(),
            Self::Ancestors(p) => p.prepared_len(),
        }
    }
    #[inline(always)]
    fn inspect_prepared(&self) -> &[u8] {
        match self {
            Self::Strict(p) => p.inspect_prepared(),
            Self::Ancestors(p) => p.inspect_prepared(),
        }
    }
    #[inline(always)]
    fn inspect_removed(&self) -> Option<Span> {
        match self {
            Self::Strict(p) => p.inspect_removed(),
            Self::Ancestors(p) => p.inspect_removed(),
        }
    }
    #[inline(always)]
    fn is_missing_fixed_remove(&self) -> bool {
        match self {
            Self::Strict(p) => p.is_missing_fixed_remove(),
            Self::Ancestors(p) => p.is_missing_fixed_remove(),
        }
    }
    #[inline(always)]
    fn detach(self) -> DetachedEdit<'key> {
        match self {
            Self::Strict(p) => DetachedEdit::Strict(p.detach()),
            Self::Ancestors(p) => DetachedEdit::Entries(p.detach()),
        }
    }
}

// Detached requests retain the existing strict gateway or fresh header replanning.
enum DetachedEdit<'key> {
    Strict(Plan<'key>),
    Subtree(blubber_core::raw::CheckedSubtreePlan),
    Entries(blubber_core::raw::AncestorPlan<'key>),
}

impl DetachedEdit<'_> {
    fn required_storage_len(&self) -> u32 {
        match self {
            Self::Strict(plan) => plan.required_storage_len(),
            Self::Subtree(plan) => plan.required_storage_len(),
            Self::Entries(plan) => plan.required_storage_len(),
        }
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
fn apply_detached(
    plan: DetachedEdit<'_>,
    store: &mut dyn Resize,
    required: usize,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    match plan {
        DetachedEdit::Strict(plan) => grow_and_apply(plan, store, required, encode),
        DetachedEdit::Entries(plan) => apply_detached_entries(plan, store, required, encode),
        DetachedEdit::Subtree(plan) => apply_detached_subtree(plan, store, required, encode),
    }
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    not(all(target_os = "solana", feature = "max-depth-16")),
    inline(always)
)]
fn apply_detached_entries(
    plan: blubber_core::raw::AncestorPlan<'_>,
    store: &mut dyn Resize,
    required: usize,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    if let Err(error) = store.resize(required) {
        let access = store.edit_access();
        plan.check_detached_error(access.bytes, access.poisoned)?;
        return Err(error);
    }
    store.set_poisoned(true);
    let access = store.edit_access();
    let apply = plan.begin(&mut access.bytes[..required], access.poisoned)?;
    let mut apply = apply.move_regions(access.poisoned).destination();
    encode(apply.bytes());
    apply.finish(access.poisoned)
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    not(all(target_os = "solana", feature = "max-depth-16")),
    inline(always)
)]
fn apply_detached_subtree(
    plan: blubber_core::raw::CheckedSubtreePlan,
    store: &mut dyn Resize,
    required: usize,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    if let Err(error) = store.resize(required) {
        let access = store.edit_access();
        plan.check_detached_error(access.bytes, access.poisoned)?;
        return Err(error);
    }
    store.set_poisoned(true);
    let access = store.edit_access();
    let write = plan.begin(&mut access.bytes[..required], access.poisoned)?;
    let mut write = write.move_regions(access.poisoned).destination();
    encode(write.bytes());
    write.finish(access.poisoned)
}

// Growth runs after detaching the exact byte loan. Keep its Plan and the
// readiness Result<Apply> out of the frame retaining wide Removed values.
#[cfg_attr(target_os = "solana", inline(never))]
fn grow_and_apply(
    plan: Plan<'_>,
    store: &mut dyn Resize,
    required: usize,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    store.resize(required)?;
    store.set_poisoned(true);
    let result = begin_unwrapped(store, plan, required, encode);
    let mut poisoned = true;
    let result = blubber_core::patch_pass::finish_publication(&mut poisoned, result);
    store.set_poisoned(poisoned);
    result
}

#[cfg_attr(target_os = "solana", inline(never))]
fn begin_unwrapped(
    store: &mut dyn Resize,
    plan: Plan<'_>,
    required: usize,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    complete_apply(
        blubber_core::raw::begin_apply(&mut store.bytes_mut()[..required], plan),
        encode,
    )
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn complete_apply(
    apply: Result<blubber_core::raw::Apply<'_, '_>>,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    let mut apply = apply?;
    apply.move_regions();
    encode(apply.destination());
    apply.finish();
    Ok(())
}

#[inline(always)]
fn removed_view(span: Option<Span>, bytes: &[u8], layout: Layout) -> Result<Option<View<'_>>> {
    span.map(|span| {
        View::inherited(
            &bytes[span.start as usize..span.end as usize],
            layout,
            span.start,
        )
    })
    .transpose()
}

fn extract_value(
    span: Option<Span>,
    bytes: &[u8],
    layout: Layout,
    destination: &mut [u8],
) -> Result<Option<u32>> {
    let Some(view) = removed_view(span, bytes, layout)? else {
        return Ok(None);
    };
    view.validate()?;
    let source = view.bytes();
    if destination.len() < source.len() {
        return Err(ErrorKind::Capacity.into());
    }
    destination[..source.len()].copy_from_slice(source);
    Ok(Some(source.len() as u32))
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn complete_prepared(
    prepared: PreparedEdit<'_, '_>,
    poisoned: &mut bool,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    match prepared {
        PreparedEdit::Strict(p) => complete_apply(p.begin_prepared(), encode),
        PreparedEdit::Ancestors(p) => complete_ancestors(p, poisoned, encode),
    }
}

// Keep the new ancestor state machine out of ordinary fixed-entry frames.
#[cfg_attr(target_os = "solana", inline(never))]
fn complete_ancestors(
    prepared: blubber_core::raw::AncestorPrepared<'_, '_>,
    poisoned: &mut bool,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    let apply = prepared.begin_prepared()?;
    let mut apply = apply.move_regions(poisoned).destination();
    encode(apply.bytes());
    apply.finish(poisoned)
}

/// Full-width typed requests, narrowed only after the single path resolution.
pub(crate) enum ListRequest {
    Push,
    Insert(usize),
    Replace(usize),
    Remove(usize),
    Truncate(usize),
    Pop,
    Clear,
}

impl ListRequest {
    fn checked(self, count: u32) -> Result<ListEdit> {
        use crate::list_bounds::{checked_list_index, truncate_len};
        Ok(match self {
            Self::Push => ListEdit::Insert(count),
            Self::Insert(index) => ListEdit::Insert(checked_list_index(index, count, true)?),
            Self::Replace(index) => ListEdit::Replace(checked_list_index(index, count, false)?),
            Self::Remove(index) => ListEdit::Remove(checked_list_index(index, count, false)?),
            Self::Truncate(len) => ListEdit::Truncate(truncate_len(len, count)),
            Self::Pop => ListEdit::Pop,
            Self::Clear => ListEdit::Clear,
        })
    }
}

// Keep Resolved/Result<Resolved> out of the frame retaining Prepared and Removed.
#[cfg_attr(target_os = "solana", inline(never))]
#[cfg_attr(not(target_os = "solana"), inline(always))]
fn prepare_bytes<'a, 'key, I>(
    bytes: &'a mut [u8],
    discriminator: [u8; 8],
    minimum: u32,
    layout: &Layout,
    path: &InlinePath,
    prepare: impl FnOnce(blubber_core::raw::Resolved<'a>) -> Preparation<(PreparedEdit<'a, 'key>, I)>,
) -> Preparation<(PreparedEdit<'a, 'key>, I)> {
    prepare_bytes_body(bytes, discriminator, minimum, layout, path, prepare)
}

#[inline(always)]
fn prepare_bytes_body<'a, 'key, I>(
    bytes: &'a mut [u8],
    discriminator: [u8; 8],
    minimum: u32,
    layout: &Layout,
    path: &InlinePath,
    prepare: impl FnOnce(blubber_core::raw::Resolved<'a>) -> Preparation<(PreparedEdit<'a, 'key>, I)>,
) -> Preparation<(PreparedEdit<'a, 'key>, I)> {
    prepare_resolved(
        blubber_core::raw::resolve_account_edit(bytes, discriminator, minimum, layout, path),
        prepare,
    )
}

// Large removed values keep preparation out of their SBF frame.
#[cfg_attr(target_os = "solana", inline(never))]
fn prepare_bytes_large<'a, 'key, I>(
    bytes: &'a mut [u8],
    discriminator: [u8; 8],
    minimum: u32,
    layout: &Layout,
    path: &InlinePath,
    prepare: impl FnOnce(blubber_core::raw::Resolved<'a>) -> Preparation<(PreparedEdit<'a, 'key>, I)>,
) -> Preparation<(PreparedEdit<'a, 'key>, I)> {
    prepare_bytes(bytes, discriminator, minimum, layout, path, prepare)
}

// Keep preparation and the outgoing Plan away from a wide Removed slot.
#[cfg_attr(target_os = "solana", inline(never))]
#[allow(clippy::too_many_arguments)] // Exclusive loan, size policy and staged encoder.
fn stage_bytes<'a, 'key, const LIST_FRAME: bool, I, R, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    access: crate::storage::private::EditAccess<'a>,
    (discriminator, minimum): ([u8; 8], u32),
    layout: &Layout,
    path: &InlinePath,
    prepare: impl FnOnce(blubber_core::raw::Resolved<'a>) -> Preparation<(PreparedEdit<'a, 'key>, I)>,
    stage: &mut EditStage<I, R, E>,
    read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<Option<DetachedEdit<'key>>> {
    // Unit map results share preparation; lists and wider results retain its frame.
    let prepared = match (
        blubber_core::MAX_DEPTH,
        core::mem::size_of::<R>(),
        LIST_FRAME,
    ) {
        (8, 0, false) => {
            prepare_bytes_body(access.bytes, discriminator, minimum, layout, path, prepare)
        }
        _ => {
            if core::mem::size_of::<R>() > 512 && !cfg!(feature = "max-depth-16") {
                prepare_bytes_large(access.bytes, discriminator, minimum, layout, path, prepare)
            } else {
                prepare_bytes(access.bytes, discriminator, minimum, layout, path, prepare)
            }
        }
    };
    if LIST_FRAME && blubber_core::MAX_DEPTH == 8 && core::mem::size_of::<R>() <= 8 {
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(error.report(lazy, access.poisoned));
            }
        };
        return stage_selected::<LIST_FRAME, _, _, _>(
            lazy,
            prepared,
            access.poisoned,
            (access.hard_limit, access.limit, access.limit_error),
            &mut LoanStage {
                data: stage,
                #[cfg(feature = "pinocchio")]
                preflight: access.preflight,
            },
            read_removed,
        );
    }
    let prepared = prepared.map_err(|error| error.report(lazy, access.poisoned));
    stage_prepared::<LIST_FRAME, _, _, _>(
        lazy,
        prepared,
        access.poisoned,
        (access.hard_limit, access.limit, access.limit_error),
        &mut LoanStage {
            data: stage,
            #[cfg(feature = "pinocchio")]
            preflight: access.preflight,
        },
        read_removed,
    )
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn stage_replacement<const SOURCE: bool, I, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    resolved: Result<blubber_core::raw::Resolved<'_>>,
    poisoned: &mut bool,
    limits: (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, (), E>,
    measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
) -> Result<Option<DetachedEdit<'static>>> {
    if let (true, Err(error)) = (lazy, &resolved) {
        list_pass::source::<()>(Err(*error), poisoned)?;
    }
    let resolved = resolved?;
    replace_selected::<SOURCE, _, _>(lazy, resolved, poisoned, limits, stage, measure)
}

#[cfg_attr(target_os = "solana", inline(never))]
fn replace_selected<const SOURCE: bool, I, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    resolved: blubber_core::raw::Resolved<'_>,
    poisoned: &mut bool,
    limits: (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, (), E>,
    measure: impl FnOnce(View<'_>) -> Result<(u32, I)>,
) -> Result<Option<DetachedEdit<'static>>> {
    let (span, layout) = resolved.selection();
    let bytes = &resolved.inspect_resolved()[span.start as usize..span.end as usize];
    let view = View::inherited_for_edit(bytes, layout, span.start)?;
    let (len, input) = match measure(view) {
        Ok(pair) => pair,
        Err(error) => {
            if SOURCE {
                return Err(PreparationError::from(error).report(lazy, poisoned));
            }
            return Err(error);
        }
    };
    if len == span.end - span.start {
        let data = &mut *stage.data;
        let destination = resolved.into_destination();
        *poisoned = true;
        data.encode.take().expect("one replacement encoder")(input, destination);
        *poisoned = false;
        data.removed = Ok(());
        return Ok(None);
    }
    replace_different(lazy, resolved, poisoned, limits, stage, len, input, span)
}

// Keep growth planning and its larger frame away from equal-size overwrites.
#[cfg_attr(target_os = "solana", inline(never))]
#[allow(clippy::too_many_arguments)] // Exclusive loan, size policy and staged encoder.
fn replace_different<I, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    resolved: blubber_core::raw::Resolved<'_>,
    poisoned: &mut bool,
    limits: (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, (), E>,
    len: u32,
    input: I,
    _span: Span,
) -> Result<Option<DetachedEdit<'static>>> {
    if lazy && resolved.has_ancestor_words() && resolved.subtree_fits(len) {
        let write = resolved.prepare_subtree_checked(len, poisoned)?;
        let required = write.required_len() as usize;
        if required > limits.0 {
            write.check_before_error(poisoned)?;
            return Err(ErrorKind::Capacity.into());
        }
        if required > limits.1 {
            write.check_before_error(poisoned)?;
            return Err(limits.2.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > _span.end - _span.start {
            if let Err(error) = stage.preflight.check(required) {
                write.check_before_error(poisoned)?;
                return Err(error);
            }
        }
        let mut write = write.move_regions(poisoned).destination();
        stage.data.encode.take().expect("one replacement encoder")(input, write.bytes());
        write.finish(poisoned)?;
        stage.data.removed = Ok(());
        return Ok(None);
    }
    if resolved.subtree_fits(len) {
        let mut write = resolved.prepare_subtree(len)?;
        let required = write.required_len() as usize;
        if required > limits.0 {
            return Err(ErrorKind::Capacity.into());
        }
        if required > limits.1 {
            return Err(limits.2.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > _span.end - _span.start {
            stage.preflight.check(required)?;
        }
        *poisoned = true;
        stage.data.encode.take().expect("one replacement encoder")(input, write.destination());
        write.finish();
        *poisoned = false;
        stage.data.removed = Ok(());
        return Ok(None);
    }
    if lazy && resolved.has_ancestor_words() {
        let required = resolved.subtree_required(len)? as usize;
        if required > limits.0 {
            resolved.check_ancestor_error(len, poisoned)?;
            return Err(ErrorKind::Capacity.into());
        }
        if required > limits.1 {
            resolved.check_ancestor_error(len, poisoned)?;
            return Err(limits.2.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > _span.end - _span.start {
            if let Err(error) = stage.preflight.check(required) {
                resolved.check_ancestor_error(len, poisoned)?;
                return Err(error);
            }
        }
        let plan = resolved.defer_subtree(len)?;
        stage.data.input = Some(input);
        stage.data.removed = Ok(());
        return Ok(Some(DetachedEdit::Subtree(plan)));
    }
    let prepared = prepare_replacement(resolved, len, input);
    stage_prepared::<false, _, _, _>(lazy, prepared, poisoned, limits, stage, |_, _, _| Ok(()))
}

// Keep byte staging separate at depth sixteen. This mirrors the callback body
// so its original default-depth ABI and small-edit CU controls stay intact.
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    not(all(target_os = "solana", feature = "max-depth-16")),
    inline(always)
)]
#[allow(clippy::too_many_arguments)] // Exclusive loan, size policy and staged encoder.
fn replace_generic_different<I, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    resolved: blubber_core::raw::Resolved<'_>,
    poisoned: &mut bool,
    limits: (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, (), E>,
    len: u32,
    input: I,
    _span: Span,
) -> Result<Option<DetachedEdit<'static>>> {
    if lazy && resolved.has_ancestor_words() && resolved.subtree_fits(len) {
        let write = resolved.prepare_subtree_checked(len, poisoned)?;
        let required = write.required_len() as usize;
        if required > limits.0 {
            write.check_before_error(poisoned)?;
            return Err(ErrorKind::Capacity.into());
        }
        if required > limits.1 {
            write.check_before_error(poisoned)?;
            return Err(limits.2.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > _span.end - _span.start {
            if let Err(error) = stage.preflight.check(required) {
                write.check_before_error(poisoned)?;
                return Err(error);
            }
        }
        let mut write = write.move_regions(poisoned).destination();
        stage.data.encode.take().expect("one replacement encoder")(input, write.bytes());
        write.finish(poisoned)?;
        stage.data.removed = Ok(());
        return Ok(None);
    }
    if resolved.subtree_fits(len) {
        let mut write = resolved.prepare_subtree(len)?;
        let required = write.required_len() as usize;
        if required > limits.0 {
            return Err(ErrorKind::Capacity.into());
        }
        if required > limits.1 {
            return Err(limits.2.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > _span.end - _span.start {
            stage.preflight.check(required)?;
        }
        *poisoned = true;
        stage.data.encode.take().expect("one replacement encoder")(input, write.destination());
        write.finish();
        *poisoned = false;
        stage.data.removed = Ok(());
        return Ok(None);
    }
    if lazy && resolved.has_ancestor_words() {
        let required = resolved.subtree_required(len)? as usize;
        if required > limits.0 {
            resolved.check_ancestor_error(len, poisoned)?;
            return Err(ErrorKind::Capacity.into());
        }
        if required > limits.1 {
            resolved.check_ancestor_error(len, poisoned)?;
            return Err(limits.2.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > _span.end - _span.start {
            if let Err(error) = stage.preflight.check(required) {
                resolved.check_ancestor_error(len, poisoned)?;
                return Err(error);
            }
        }
        let plan = resolved.defer_subtree(len)?;
        stage.data.input = Some(input);
        stage.data.removed = Ok(());
        return Ok(Some(DetachedEdit::Subtree(plan)));
    }
    let prepared = prepare_replacement(resolved, len, input);
    stage_prepared::<false, _, _, _>(lazy, prepared, poisoned, limits, stage, |_, _, _| Ok(()))
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn prepare_replacement<I>(
    resolved: blubber_core::raw::Resolved<'_>,
    len: u32,
    input: I,
) -> Result<(PreparedEdit<'_, 'static>, I)> {
    prepared_input(resolved.prepare_replace(len, u32::MAX), input)
}

// Output slots keep wide Removed values out of each preparation/apply frame.
struct EditStage<I, R, E> {
    input: Option<I>,
    removed: Result<R>,
    encode: Option<E>,
}

// This short loan ends before detached growth reborrows the backend. Keep the
// context behind the existing stage argument, including equal-size overwrites.
struct LoanStage<'s, I, R, E> {
    data: &'s mut EditStage<I, R, E>,
    #[cfg(feature = "pinocchio")]
    preflight: crate::storage::private::Preflight<'s>,
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn stage_prepared<'key, const LIST_FRAME: bool, I, R, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    result: Result<(PreparedEdit<'_, 'key>, I)>,
    poisoned: &mut bool,
    limits: (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, R, E>,
    read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<Option<DetachedEdit<'key>>> {
    stage_selected::<LIST_FRAME, _, _, _>(lazy, result?, poisoned, limits, stage, read_removed)
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    not(all(target_os = "solana", feature = "max-depth-16")),
    inline(always)
)]
fn stage_selected<'key, const LIST_FRAME: bool, I, R, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    (prepared, input): (PreparedEdit<'_, 'key>, I),
    poisoned: &mut bool,
    (hard_limit, limit, limit_error): (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, R, E>,
    read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<Option<DetachedEdit<'key>>> {
    match (
        blubber_core::MAX_DEPTH,
        core::mem::size_of::<R>(),
        LIST_FRAME,
    ) {
        (8, 0..=8, true) => match prepared {
            PreparedEdit::Strict(prepared) => stage_plain_list_unit(
                lazy,
                (prepared, input),
                poisoned,
                (hard_limit, limit, limit_error),
                stage,
                read_removed,
            ),
            PreparedEdit::Ancestors(prepared) => stage_selected_unit(
                lazy,
                (PreparedEdit::Ancestors(prepared), input),
                poisoned,
                (hard_limit, limit, limit_error),
                stage,
                read_removed,
            ),
        },
        (8, 0, false) => stage_selected_unit(
            lazy,
            (prepared, input),
            poisoned,
            (hard_limit, limit, limit_error),
            stage,
            read_removed,
        ),
        _ => stage_selected_body(
            lazy,
            (prepared, input),
            poisoned,
            (hard_limit, limit, limit_error),
            stage,
            read_removed,
        ),
    }
}

// Keep the original small-list plan direct through publication.
#[inline(always)]
fn stage_plain_list_unit<'key, I, R, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    (prepared, input): (blubber_core::raw::Prepared<'_, 'key>, I),
    poisoned: &mut bool,
    (hard_limit, limit, limit_error): (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, R, E>,
    read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<Option<DetachedEdit<'key>>> {
    let required = prepared.prepared_len() as usize;
    if required > hard_limit {
        return Err(ErrorKind::Capacity.into());
    }
    if required > limit {
        return Err(limit_error.into());
    }
    #[cfg(feature = "pinocchio")]
    if required > blubber_core::read_u32(prepared.inspect_prepared(), 8)? as usize {
        stage.preflight.check(required)?;
    }
    let stage = &mut *stage.data;
    stage.removed = read_removed(
        prepared.inspect_removed(),
        prepared.inspect_prepared(),
        &input,
    );
    list_pass::source_try!(
        lazy,
        stage.removed.as_ref().map(|_| ()).map_err(|error| *error),
        poisoned,
    );
    if prepared.is_missing_fixed_remove() {
        return Ok(None);
    }
    if required <= prepared.inspect_prepared().len() {
        *poisoned = true;
        let encode = stage.encode.take().expect("one prepared encoder");
        complete_apply(prepared.begin_prepared(), |destination| {
            encode(input, destination)
        })?;
        *poisoned = false;
        Ok(None)
    } else {
        stage.input = Some(input);
        Ok(Some(DetachedEdit::Strict(prepared.detach())))
    }
}

// Keep checked publication separate from its preparation frame.
#[cfg_attr(target_os = "solana", inline(never))]
fn stage_selected_unit<'key, I, R, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    (prepared, input): (PreparedEdit<'_, 'key>, I),
    poisoned: &mut bool,
    (hard_limit, limit, limit_error): (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, R, E>,
    read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<Option<DetachedEdit<'key>>> {
    stage_selected_body(
        lazy,
        (prepared, input),
        poisoned,
        (hard_limit, limit, limit_error),
        stage,
        read_removed,
    )
}

#[inline(always)]
fn stage_selected_body<'key, I, R, E: FnOnce(I, &mut [u8])>(
    lazy: bool,
    (prepared, input): (PreparedEdit<'_, 'key>, I),
    poisoned: &mut bool,
    (hard_limit, limit, limit_error): (usize, usize, ErrorKind),
    stage: &mut LoanStage<'_, I, R, E>,
    read_removed: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<Option<DetachedEdit<'key>>> {
    let required = prepared.prepared_len() as usize;
    if required > hard_limit {
        prepared.check_before_error(poisoned)?;
        return Err(ErrorKind::Capacity.into());
    }
    if required > limit {
        prepared.check_before_error(poisoned)?;
        return Err(limit_error.into());
    }
    #[cfg(feature = "pinocchio")]
    if required > blubber_core::read_u32(prepared.inspect_prepared(), 8)? as usize {
        if let Err(error) = stage.preflight.check(required) {
            prepared.check_before_error(poisoned)?;
            return Err(error);
        }
    }
    let stage = &mut *stage.data;
    let removed = decode_removed(&prepared, &input, &mut stage.removed, read_removed);
    if removed.is_err() {
        prepared.check_before_error(poisoned)?;
        if lazy {
            list_pass::source(removed, poisoned)?;
        } else {
            removed?;
        }
    }
    if prepared.is_missing_fixed_remove() {
        return Ok(None);
    }
    if required <= prepared.inspect_prepared().len() {
        *poisoned = true;
        let encode = stage.encode.take().expect("one prepared encoder");
        complete_prepared(prepared, poisoned, |destination| encode(input, destination))?;
        *poisoned = false;
        Ok(None)
    } else {
        stage.input = Some(input);
        Ok(Some(prepared.detach()))
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
fn finish_growth<I, R, E: FnOnce(I, &mut [u8])>(
    result: Result<Option<DetachedEdit<'_>>>,
    store: &mut dyn Resize,
    stage: &mut EditStage<I, R, E>,
) -> Result<()> {
    if let Some(plan) = result? {
        let required = plan.required_storage_len() as usize;
        let input = stage.input.take().expect("detached input");
        let encode = stage.encode.take().expect("one detached encoder");
        apply_detached(plan, store, required, |destination| {
            encode(input, destination)
        })?;
    }
    Ok(())
}

// Separate the resolver's Result, unwrapped loan and caller's preparation frame.
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn prepare_resolved<'a, 'key, I>(
    result: Result<blubber_core::raw::Resolved<'a>>,
    prepare: impl FnOnce(blubber_core::raw::Resolved<'a>) -> Preparation<(PreparedEdit<'a, 'key>, I)>,
) -> Preparation<(PreparedEdit<'a, 'key>, I)> {
    prepare_selected(result.map_err(PreparationError::source)?, prepare)
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn prepare_selected<'a, 'key, I>(
    resolved: blubber_core::raw::Resolved<'a>,
    prepare: impl FnOnce(blubber_core::raw::Resolved<'a>) -> Preparation<(PreparedEdit<'a, 'key>, I)>,
) -> Preparation<(PreparedEdit<'a, 'key>, I)> {
    prepare(resolved)
}

#[cfg_attr(target_os = "solana", inline(never))]
fn decode_removed<I, R>(
    prepared: &PreparedEdit<'_, '_>,
    input: &I,
    result: &mut Result<R>,
    read: impl FnOnce(Option<Span>, &[u8], &I) -> Result<R>,
) -> Result<()> {
    *result = read(
        prepared.inspect_removed(),
        prepared.inspect_prepared(),
        input,
    );
    result.as_ref().map(|_| ()).map_err(|error| *error)
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    all(target_os = "solana", not(feature = "max-depth-16")),
    inline(always)
)]
fn prepared_input<'a, 'key, I, P: Into<PreparedEdit<'a, 'key>>, F: From<Error>>(
    result: Result<P>,
    input: I,
) -> core::result::Result<(PreparedEdit<'a, 'key>, I), F> {
    Ok((result.map_err(F::from)?.into(), input))
}

#[cfg(all(test, feature = "derive", feature = "alloc", feature = "pinocchio"))]
#[path = "edit/tests.rs"]
mod funding_tests;

#[cfg(test)]
mod tests {
    #![forbid(unsafe_code)]

    use super::*;

    use core::cell::Cell;

    #[test]
    fn preparation_errors_preserve_origin_and_full_sdk_codes() {
        for error in [
            Error::new(ErrorKind::InvalidBoundary, 17),
            ErrorKind::ArithmeticOverflow.into(),
            Error::Program { code: 0 },
            Error::Program { code: u64::MAX },
        ] {
            for source in [false, true] {
                for lazy in [false, true] {
                    for initial_poison in [false, true] {
                        let tagged = if source {
                            PreparationError::source(error)
                        } else {
                            PreparationError::input(error)
                        };
                        assert_eq!(tagged.into_error(), error);
                        let mut poisoned = initial_poison;
                        assert_eq!(tagged.report(lazy, &mut poisoned), error);
                        assert_eq!(
                            poisoned,
                            initial_poison
                                || (source && lazy && matches!(error, Error::Fault { .. }))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn detached_readiness_failure_keeps_poison_and_old_used_length() {
        use crate::{ReadStorage, Resize as _};
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(b"patchend");
        bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
        let layout = Layout::List {
            element: &Layout::Str,
        };
        let plan = blubber_core::raw::plan_list_edit(
            &bytes,
            16,
            &layout,
            &InlinePath::new(),
            ListEdit::Insert(0),
            1,
            32,
        )
        .unwrap();
        let required = plan.required_storage_len() as usize;
        // Fault injection between detached planning and readiness.
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        let before = bytes;
        let mut store = crate::SliceStore::new(&mut bytes);
        let encoded = Cell::new(false);
        let result = grow_and_apply(plan, &mut store, required, |_| encoded.set(true));
        assert_eq!(result, Err(ErrorKind::InvalidPlan.into()));
        assert!(!encoded.get());
        assert!(store.is_poisoned());
        assert_eq!(store.bytes(), &before[..required]);
        assert_eq!(&store.bytes()[8..12], &16u32.to_le_bytes());
        assert_eq!(store.commit(16), Err(ErrorKind::Poisoned.into()));
    }

    #[test]
    fn capacity_profiles_reject_before_decoding_and_detach_at_the_exact_limit() {
        // Simulate policy limits without manufacturing native SDK account memory.
        for (hard, available, expected) in [
            (35, 34, Some(ErrorKind::Capacity)),
            (40, 35, Some(ErrorKind::GrowthLimit)),
            (36, 36, None),
        ] {
            let mut bytes = [0xa5; 32];
            bytes[8..12].copy_from_slice(&13u32.to_le_bytes());
            let before = bytes;
            let prepared =
                blubber_core::raw::resolve_edit(&mut bytes, 13, &Layout::Str, &InlinePath::new())
                    .unwrap()
                    .prepare_replace(24, u32::MAX)
                    .unwrap();
            let decoded = Cell::new(false);
            let encoded = Cell::new(false);
            let mut poisoned = false;
            let mut stage = EditStage {
                input: None,
                removed: Err::<(), _>(ErrorKind::InvalidPlan.into()),
                encode: Some(|(), _: &mut [u8]| encoded.set(true)),
            };
            let result = stage_selected::<false, _, _, _>(
                false,
                (prepared.into(), ()),
                &mut poisoned,
                (hard, available, ErrorKind::GrowthLimit),
                &mut LoanStage {
                    data: &mut stage,
                    #[cfg(feature = "pinocchio")]
                    preflight: crate::storage::private::Preflight::default(),
                },
                |_, _, _| {
                    decoded.set(true);
                    Ok(())
                },
            );
            if let Some(kind) = expected {
                assert_eq!(result.err(), Some(kind.into()));
                assert!(!decoded.get());
            } else {
                assert_eq!(result.unwrap().unwrap().required_storage_len(), 36);
                assert!(decoded.get());
                assert!(stage.input.is_some());
            }
            assert!(!encoded.get());
            assert!(!poisoned);
            assert_eq!(bytes, before);
        }
    }

    #[cfg(feature = "derive")]
    mod fast_limits {
        #![forbid(unsafe_code)]
        use crate::storage::private::{self, Write};
        use crate::storage::Backend as Resize;
        use crate::Resize as _;
        #[cfg(feature = "alloc")]
        use crate::RootSchema;
        use crate::{CommitInfo, List, Map, ReadStorage, Result, SliceStore, Str};
        use crate::{Error, ErrorKind};
        #[cfg(feature = "alloc")]
        use blubber_core::{
            raw::{InlinePath, Step},
            Layout,
        };

        struct LimitedStore<S> {
            inner: S,
            hard: usize,
            limit: usize,
        }
        impl<S: Resize> private::Sealed for LimitedStore<S> {}
        impl<S: Resize> ReadStorage for LimitedStore<S> {
            fn is_closed(&self) -> bool {
                self.inner.is_closed()
            }
            fn bytes(&self) -> &[u8] {
                self.inner.bytes()
            }
        }
        impl<S: Resize> Write for LimitedStore<S> {
            fn check_initialize_resize(&self, needed: usize) -> Result<()> {
                <Self as crate::Resize>::check_resize(self, needed)
            }

            fn fixed_access(&mut self) -> Result<private::FixedAccess<'_>> {
                self.inner.fixed_access()
            }

            fn edit_access(&mut self) -> private::EditAccess<'_> {
                let mut access = self.inner.edit_access();
                access.hard_limit = self.hard;
                access.limit = self.limit;
                access.limit_error = ErrorKind::GrowthLimit;
                access
            }
        }
        impl<S: Resize> crate::Resize for LimitedStore<S> {
            fn storage_len(&self) -> usize {
                self.inner.storage_len()
            }
            fn growth_limit(&self) -> usize {
                self.limit
            }
            fn check_resize(&self, new_len: usize) -> Result<()> {
                self.inner.check_resize(self.inner.storage_len())?;
                if new_len > self.hard {
                    return Err(ErrorKind::Capacity.into());
                }
                if new_len > self.limit {
                    return Err(ErrorKind::GrowthLimit.into());
                }
                self.inner.check_resize(new_len)
            }
            fn is_poisoned(&self) -> bool {
                self.inner.is_poisoned()
            }
            fn __backend(&mut self, _: crate::storage::Writer) -> &mut dyn crate::storage::Backend {
                self
            }
        }
        impl<S: Resize> Resize for LimitedStore<S> {
            fn bytes_mut(&mut self) -> &mut [u8] {
                self.inner.bytes_mut()
            }
            fn resize(&mut self, new_len: usize) -> Result<()> {
                self.check_resize(new_len)?;
                self.inner.resize(new_len)
            }
            fn set_poisoned(&mut self, value: bool) {
                self.inner.set_poisoned(value);
            }
            fn commit(&mut self, used: u32) -> Result<CommitInfo> {
                self.inner.commit(used)
            }
        }
        #[crate::schema(account)]
        struct Text {
            value: Str,
        }
        #[crate::schema(account)]
        struct Numbers {
            value: List<u64>,
        }
        #[crate::schema(account)]
        struct Entries {
            value: Map<u16, u64>,
        }

        #[test]
        fn tail_removal_checks_both_limits_before_removed_and_preserves_failures() {
            use core::cell::Cell;
            for map in [false, true] {
                for missing in [false, true] {
                    if missing && !map {
                        continue;
                    }
                    let used = if map {
                        if missing {
                            16
                        } else {
                            26
                        }
                    } else {
                        24
                    };
                    for hard in [used - 1, used, 64] {
                        for limit in [used - 1, used, 64] {
                            for decode_fails in [false, true] {
                                let mut bytes = [0; 64];
                                let mut inner = SliceStore::new(&mut bytes);
                                if map {
                                    Entries::initialize(
                                        &mut inner,
                                        EntriesInit {
                                            value: if missing { &[] } else { &[(1, 7)] },
                                        },
                                    )
                                    .unwrap();
                                } else {
                                    Numbers::initialize(&mut inner, NumbersInit { value: &[7] })
                                        .unwrap();
                                }
                                let mut store = LimitedStore { inner, hard, limit };
                                let before: [u8; 64] = store.bytes().try_into().unwrap();
                                let decoded = Cell::new(false);
                                let mut read = Some(|_: Option<crate::View<'_>>| {
                                    decoded.set(true);
                                    if decode_fails {
                                        Err(ErrorKind::InvalidBool.into())
                                    } else {
                                        Ok(())
                                    }
                                });
                                let mut edit = if map {
                                    crate::Edit::load::<Entries>(&mut store, Entries::DISCRIMINATOR)
                                        .unwrap()
                                } else {
                                    crate::Edit::load::<Numbers>(&mut store, Numbers::DISCRIMINATOR)
                                        .unwrap()
                                }
                                .field(0)
                                .into_account()
                                .unwrap();
                                let key = 1u16.to_le_bytes();
                                let result = if map {
                                    edit.map_tail_edit::<u64, _>(
                                        blubber_core::raw::MapEdit::Remove,
                                        &key,
                                        None,
                                        read.take().unwrap(),
                                    )
                                } else {
                                    edit.list_tail_edit::<u64, _>(
                                        super::super::ListRequest::Remove(0),
                                        None,
                                        read.take().unwrap(),
                                    )
                                };
                                let expected = if hard < used {
                                    Some(ErrorKind::Capacity)
                                } else if limit < used {
                                    Some(ErrorKind::GrowthLimit)
                                } else if decode_fails {
                                    Some(ErrorKind::InvalidBool)
                                } else {
                                    None
                                };
                                assert_eq!(result.err(), expected.map(Into::into));
                                assert_eq!(decoded.get(), hard >= used && limit >= used);
                                assert!(!store.is_poisoned());
                                if expected.is_some() || missing {
                                    assert_eq!(store.bytes(), before);
                                }
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn no_input_map_clear_retains_its_operation_and_physical_frame() {
            let mut bytes = [0; 64];
            let mut store = SliceStore::new(&mut bytes);
            Entries::initialize(
                &mut store,
                EntriesInit {
                    value: &[(1, 7), (3, 9)],
                },
            )
            .unwrap();
            let old_used = blubber_core::read_u32(store.bytes(), 8).unwrap() as usize;
            store.bytes_mut()[old_used..].fill(0xa5);
            let before: [u8; 64] = store.bytes().try_into().unwrap();
            {
                let mut edit = crate::Edit::load::<Entries>(&mut store, Entries::DISCRIMINATOR)
                    .unwrap()
                    .field(0)
                    .into_account()
                    .unwrap();
                assert_eq!(
                    edit.map_tail_edit::<u64, _>(
                        blubber_core::raw::MapEdit::Clear,
                        &[],
                        None,
                        |removed| {
                            assert!(removed.is_none());
                            Ok(())
                        }
                    ),
                    Ok(())
                );
            }
            assert_eq!(Entries::load(store.bytes()).unwrap().value().len(), 0);
            assert_eq!(blubber_core::read_u32(store.bytes(), 8), Ok(16));
            assert!(store.bytes()[16..old_used].iter().all(|byte| *byte == 0));
            assert_eq!(&store.bytes()[old_used..], &before[old_used..]);
            assert!(!store.is_poisoned());
        }

        #[test]
        fn fixed_fill_checks_limits_before_encoding() {
            for hard in [23, 24, 40] {
                for limit in [23, 24, 40] {
                    let mut bytes = [0; 40];
                    let mut inner = SliceStore::new(&mut bytes);
                    Numbers::initialize(&mut inner, NumbersInit { value: &[7] }).unwrap();
                    let mut store = LimitedStore { inner, hard, limit };
                    let before: [u8; 40] = store.bytes().try_into().unwrap();
                    let result = Numbers::load_mut(&mut store).unwrap().value_mut().fill(42);
                    let expected = if hard < 24 {
                        Some(ErrorKind::Capacity)
                    } else if limit < 24 {
                        Some(ErrorKind::GrowthLimit)
                    } else {
                        None
                    };
                    assert_eq!(result.err(), expected.map(Into::into));
                    assert!(!store.is_poisoned());
                    if expected.is_some() {
                        assert_eq!(store.bytes(), before);
                    } else {
                        assert_eq!(
                            Numbers::load(store.bytes()).unwrap().value().get(0),
                            Some(42)
                        );
                    }
                }
            }
        }

        #[test]
        fn byte_and_selected_replacements_enforce_both_limit_boundaries() {
            let input = b"01234567890123456789";
            for mode in 0..3 {
                for hard in [31, 32, 40] {
                    for limit in [31, 32, 40] {
                        let mut bytes = [0; 40];
                        let mut store = LimitedStore {
                            inner: SliceStore::new(&mut bytes),
                            hard,
                            limit,
                        };
                        Text::initialize(&mut store, TextInit { value: "" }).unwrap();
                        let before: [u8; 40] = store.bytes().try_into().unwrap();
                        let result = {
                            let mut edit =
                                crate::Edit::load::<Text>(&mut store, Text::DISCRIMINATOR)
                                    .unwrap()
                                    .field(0);
                            match mode {
                                0 => edit.replace_bytes(input),
                                1 => edit.replace_bytes_generic(input),
                                _ => edit.replace_with(
                                    |_| Ok((input.len() as u32, ())),
                                    |(), destination| destination.copy_from_slice(input),
                                ),
                            }
                        };
                        let error = if hard < 32 {
                            Some(ErrorKind::Capacity)
                        } else if limit < 32 {
                            Some(ErrorKind::GrowthLimit)
                        } else {
                            None
                        };
                        assert_eq!(
                            result.err(),
                            error.map(Into::into),
                            "mode={mode} hard={hard} limit={limit}"
                        );
                        let mut expected = before;
                        if error.is_none() {
                            expected[8..12].copy_from_slice(&32u32.to_le_bytes());
                            expected[12..32].copy_from_slice(input);
                            Text::load(store.bytes()).unwrap().validate().unwrap();
                        }
                        assert_eq!(store.bytes(), expected);
                        assert!(!store.is_poisoned());
                    }
                }
            }
        }

        #[cfg(feature = "alloc")]
        #[test]
        fn byte_replacement_grows_storage_with_exact_policy_limits() {
            for input in ["x", "01234567890123456789"] {
                let required = 12 + input.len();
                for generic in [false, true] {
                    for hard in [required - 1, required, required + 8] {
                        for limit in [required - 1, required, required + 8] {
                            let mut bytes = alloc::vec::Vec::new();
                            let mut store = LimitedStore {
                                inner: crate::VecStore::new(&mut bytes),
                                hard,
                                limit,
                            };
                            Text::initialize(&mut store, TextInit { value: "" }).unwrap();
                            let before = store.bytes().to_vec();
                            let result = {
                                let mut edit =
                                    crate::Edit::load::<Text>(&mut store, Text::DISCRIMINATOR)
                                        .unwrap()
                                        .field(0);
                                if generic {
                                    edit.replace_bytes_generic(input.as_bytes())
                                } else {
                                    edit.replace_bytes(input.as_bytes())
                                }
                            };
                            let error = if hard < required {
                                Some(ErrorKind::Capacity)
                            } else if limit < required {
                                Some(ErrorKind::GrowthLimit)
                            } else {
                                None
                            };
                            assert_eq!(result.err(), error.map(Into::into));
                            let mut expected = before;
                            if error.is_none() {
                                expected.resize(required, 0);
                                expected[8..12].copy_from_slice(&(required as u32).to_le_bytes());
                                expected[12..].copy_from_slice(input.as_bytes());
                                Text::load(store.bytes()).unwrap().validate().unwrap();
                            }
                            assert_eq!(store.bytes(), expected);
                            assert!(!store.is_poisoned());
                        }
                    }
                }
            }
        }

        #[crate::schema(account)]
        struct NestedText {
            value: Str,
            middle: Str,
            suffix: Str,
        }

        #[cfg(feature = "alloc")]
        #[test]
        fn lazy_nested_replacements_preserve_limits_and_source_priority() {
            for input in [b"x".as_slice(), b"grower".as_slice()] {
                let used = 28usize;
                let required = used.max(used - 3 + input.len());
                for physical in [used, 64] {
                    for mode in 0..3 {
                        for bad in [false, true] {
                            for hard in [required - 1, required, required + 8] {
                                for limit in [required - 1, required, required + 8] {
                                    let mut bytes = alloc::vec![0; physical];
                                    let mut inner = crate::VecStore::new(&mut bytes);
                                    NestedText::initialize(
                                        &mut inner,
                                        NestedTextInit {
                                            value: "old",
                                            middle: "m",
                                            suffix: "tail",
                                        },
                                    )
                                    .unwrap();
                                    if bad {
                                        inner.bytes_mut()[16..20]
                                            .copy_from_slice(&u32::MAX.to_le_bytes());
                                    }
                                    let mut store = LimitedStore { inner, hard, limit };
                                    let before = store.bytes().to_vec();
                                    let mut expected = before.clone();
                                    let mut path = InlinePath::new();
                                    path.push(Step::Field(0)).unwrap();
                                    let strict = blubber_core::raw::plan_replace(
                                        &before,
                                        used as u32,
                                        &Layout::Flat {
                                            checked: NestedText::__layout(),
                                        },
                                        &path,
                                        input.len() as u32,
                                        u32::MAX,
                                    );
                                    let source_error = strict.as_ref().err().copied();
                                    let quota = if hard < required {
                                        Some(ErrorKind::Capacity)
                                    } else if limit < required {
                                        Some(ErrorKind::GrowthLimit)
                                    } else {
                                        None
                                    };
                                    if !bad && quota.is_none() {
                                        let plan = strict.unwrap();
                                        if required > expected.len() {
                                            expected.resize(required, 0);
                                        }
                                        let mut apply =
                                            blubber_core::raw::begin_apply(&mut expected, plan)
                                                .unwrap();
                                        apply.move_regions();
                                        apply.destination().copy_from_slice(input);
                                        apply.finish();
                                    }
                                    let result = crate::Edit::load_trusted::<NestedText>(
                                        &mut store,
                                        NestedText::DISCRIMINATOR,
                                    )
                                    .and_then(|edit| {
                                        let mut edit = edit.field(0);
                                        match mode {
                                            0 => edit.replace_bytes(input),
                                            1 => edit.replace_bytes_generic(input),
                                            _ => edit.replace_with(
                                                |_| Ok((input.len() as u32, ())),
                                                |(), destination| {
                                                    destination.copy_from_slice(input)
                                                },
                                            ),
                                        }
                                    });
                                    let error = if bad {
                                        source_error
                                    } else {
                                        quota.map(Into::into)
                                    };
                                    assert_eq!(
                                        result.err(),
                                        error,
                                        "mode={mode} input={input:?} physical={physical} hard={hard} limit={limit} bad={bad}"
                                    );
                                    assert_eq!(
                                        store.is_poisoned(),
                                        bad && !cfg!(feature = "verify-on-load")
                                    );
                                    if quota.is_some() || (bad && cfg!(feature = "verify-on-load"))
                                    {
                                        assert_eq!(store.bytes(), before);
                                    } else if bad {
                                        assert_eq!(&store.bytes()[..12], &before[..12]);
                                        assert_ne!(&store.bytes()[12..16], &before[12..16]);
                                    } else {
                                        assert_eq!(store.bytes(), expected);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        #[crate::schema(account)]
        struct NestedFlags {
            value: List<bool>,
            middle: Str,
            suffix: Str,
        }
        #[crate::schema(account)]
        struct NestedFlagMap {
            value: Map<u8, bool>,
            middle: Str,
            suffix: Str,
        }

        #[test]
        fn lazy_fixed_entries_keep_ancestor_errors_before_limits_and_removed() {
            for map in [false, true] {
                for limited in [false, true] {
                    let mut bytes = [0; 64];
                    let mut inner = SliceStore::new(&mut bytes);
                    if map {
                        NestedFlagMap::initialize(
                            &mut inner,
                            NestedFlagMapInit {
                                value: &[(1, true)],
                                middle: "m",
                                suffix: "tail",
                            },
                        )
                        .unwrap();
                    } else {
                        NestedFlags::initialize(
                            &mut inner,
                            NestedFlagsInit {
                                value: &[true, false],
                                middle: "m",
                                suffix: "tail",
                            },
                        )
                        .unwrap();
                    }
                    inner.bytes_mut()[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
                    // Removing this invalid value must still report the older ancestor fault.
                    inner.bytes_mut()[if map { 25 } else { 24 }] = 2;
                    let used =
                        u32::from_le_bytes(inner.bytes()[8..12].try_into().unwrap()) as usize;
                    let hard = if limited { used - 1 } else { 64 };
                    let mut store = LimitedStore {
                        inner,
                        hard,
                        limit: 64,
                    };
                    let before = store.bytes().to_vec();
                    let result = if map {
                        NestedFlagMap::load_trusted_mut(&mut store)
                            .and_then(|mut edit| edit.value_mut().remove(&1).map(|_| ()))
                    } else {
                        NestedFlags::load_trusted_mut(&mut store)
                            .and_then(|mut edit| edit.value_mut().remove(0).map(|_| ()))
                    };
                    assert_eq!(result, Err(Error::new(ErrorKind::InvalidBoundary, 16)));
                    assert_eq!(store.is_poisoned(), !cfg!(feature = "verify-on-load"));
                    assert_eq!(store.bytes(), before);
                }
            }
        }

        #[cfg(feature = "alloc")]
        #[test]
        fn checked_replacements_keep_ancestor_faults_read_only() {
            for mode in 0..2 {
                for physical in [28, 64] {
                    let mut bytes = alloc::vec![0; physical];
                    let mut store = crate::VecStore::new(&mut bytes);
                    NestedText::initialize(
                        &mut store,
                        NestedTextInit {
                            value: "old",
                            middle: "m",
                            suffix: "tail",
                        },
                    )
                    .unwrap();
                    let mut edit = crate::edit::AccountEdit::load::<NestedText>(
                        &mut store,
                        NestedText::DISCRIMINATOR,
                    )
                    .unwrap()
                    .field(0);
                    // Retire the checked ingress after an external source change.
                    edit.store.bytes_mut()[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
                    edit.ingress = None;
                    let before = edit.store.bytes().to_vec();
                    let result = if mode == 0 {
                        edit.replace_bytes_generic(b"grower")
                    } else {
                        edit.replace_with(
                            |_| Ok((6, ())),
                            |(), bytes| bytes.copy_from_slice(b"grower"),
                        )
                    };
                    assert_eq!(result, Err(Error::new(ErrorKind::InvalidBoundary, 16)));
                    assert_eq!(edit.store.bytes(), before);
                    assert!(!edit.store.is_poisoned());
                }
            }
        }

        #[test]
        fn fixed_map_insert_enforces_both_limit_boundaries() {
            for hard in [25, 26, 40] {
                for limit in [25, 26, 40] {
                    let mut bytes = [0; 40];
                    let mut store = LimitedStore {
                        inner: SliceStore::new(&mut bytes),
                        hard,
                        limit,
                    };
                    Entries::initialize(&mut store, EntriesInit { value: &[] }).unwrap();
                    let before: [u8; 40] = store.bytes().try_into().unwrap();
                    let result = Entries::load_mut(&mut store)
                        .unwrap()
                        .value_mut()
                        .insert(1, 2);
                    let error = if hard < 26 {
                        Some(ErrorKind::Capacity)
                    } else if limit < 26 {
                        Some(ErrorKind::GrowthLimit)
                    } else {
                        None
                    };
                    assert_eq!(result, error.map_or(Ok(None), |kind| Err(kind.into())));
                    let mut expected = before;
                    if error.is_none() {
                        expected[8..12].copy_from_slice(&26u32.to_le_bytes());
                        expected[12..16].copy_from_slice(&1u32.to_le_bytes());
                        expected[16..18].copy_from_slice(&1u16.to_be_bytes());
                        expected[18..26].copy_from_slice(&2u64.to_le_bytes());
                        Entries::load(store.bytes()).unwrap().validate().unwrap();
                    }
                    assert_eq!(store.bytes(), expected);
                    assert!(!store.is_poisoned());
                }
            }
        }

        #[test]
        fn compact_growth_accepts_exact_hard_and_lease_limits() {
            // Initialized headroom is inside both limits, as on a real lease.
            for hard in [32, 40] {
                let mut bytes = [0; 32];
                let mut store = LimitedStore {
                    inner: SliceStore::new(&mut bytes),
                    hard,
                    limit: 32,
                };
                {
                    let mut root =
                        Text::initialize(&mut store, TextInit { value: "original" }).unwrap();
                    root.value_mut().set("01234567890123456789").unwrap();
                    assert_eq!(root.value(), "01234567890123456789");
                    root.validate().unwrap();
                }
                assert_eq!(store.bytes()[8..12], 32u32.to_le_bytes());
                assert!(!store.is_poisoned());

                let mut bytes = [0; 32];
                let mut store = LimitedStore {
                    inner: SliceStore::new(&mut bytes),
                    hard,
                    limit: 32,
                };
                {
                    let mut root =
                        Numbers::initialize(&mut store, NumbersInit { value: &[1] }).unwrap();
                    root.value_mut().push(2).unwrap();
                    assert_eq!(root.value().len(), 2);
                    assert_eq!(root.value().get(0), Some(1));
                    assert_eq!(root.value().get(1), Some(2));
                    root.validate().unwrap();
                }
                assert_eq!(store.bytes()[8..12], 32u32.to_le_bytes());
                assert!(!store.is_poisoned());
            }
        }
    }
}

#[cfg(all(test, feature = "derive"))]
mod map_ingress_controls {
    use super::*;
    crate::__root_schema!(__Map, crate::Map<u8,u8>);

    #[test]
    fn malformed_encoding_invalidates_owned_and_parent_ingress() {
        for reborrow in [false, true] {
            let mut bytes = [0; 40];
            {
                let mut store = crate::SliceStore::new(&mut bytes);
                AccountEdit::initialize::<__Map>(&mut store, *b"affineQ6", &[(1, 3), (2, 4)])
                    .unwrap()
                    .commit()
                    .unwrap();
            }
            let mut store = crate::SliceStore::new(&mut bytes);
            let mut root = AccountEdit::load::<__Map>(&mut store, *b"affineQ6").unwrap();
            let encode = |_: (), destination: &mut [u8]| {
                destination.copy_from_slice(&[2, 0, 0, 0, 1, 3, 1, 4])
            };
            if reborrow {
                root.reborrow()
                    .replace_with(|_| Ok((8, ())), encode)
                    .unwrap();
            } else {
                root.replace_with(|_| Ok((8, ())), encode).unwrap();
            }
            // A handwritten encoder can violate its functional contract; a
            // later cursor must scan those changed keys rather than reuse ingress.
            assert!(
                matches!(root.fixed_map::<u8>(1), Err(error) if error == Error::new(ErrorKind::DuplicateKey,18))
            );
        }
    }
}

#[cfg(all(test, feature = "derive"))]
mod verification_tests {
    use super::*;
    use crate::{raw, SliceStore};

    #[crate::schema(account)]
    struct Flag {
        flag: bool,
    }

    #[test]
    fn verify_preserves_poison_and_checks_closed_first() {
        let mut bytes = [0; 13];
        let mut store = SliceStore::new(&mut bytes);
        Flag::initialize(&mut store, FlagInit { flag: true }).unwrap();
        let edit =
            AccountEdit::load::<Flag>(raw::backend(&mut store), Flag::DISCRIMINATOR).unwrap();
        assert_eq!(edit.verify_root(), Ok(()));
        edit.store.set_poisoned(true);
        assert_eq!(edit.verify_root(), Err(ErrorKind::Poisoned.into()));
        assert!(edit.store.is_poisoned());
        edit.store.bytes_mut()[8..12].fill(0);
        assert_eq!(edit.verify_root(), Err(ErrorKind::Poisoned.into()));
        let _ = edit.store.commit(0);
        assert_eq!(edit.verify_root(), Err(ErrorKind::Closed.into()));
        assert!(edit.store.is_poisoned());
    }
}
