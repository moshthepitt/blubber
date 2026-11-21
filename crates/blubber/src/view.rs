//! Exact borrowed regions and shared, lazy read selection.

use blubber_core::{check_layout, checked_u32, Layout, LayoutHead, Span, MAX_DEPTH};

use crate::schema::{checked_add, mul as checked_mul};
use crate::{Error, ErrorKind, Fixed, Result, RootSchema, Schema};

/// An exact immutable subtree, its descriptor, and its account-relative offset.
///
/// Construction checks metadata and representable bounds. Reads check the
/// selected path; [`Self::validate`] certifies the complete subtree. Copies
/// borrow the original bytes, so references survive temporary parent views.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    bytes: &'a [u8],
    layout: Layout,
    base: u32,
}

// Read the source offset only after header arithmetic has failed.
macro_rules! count_try {
    ($view:expr, $result:expr $(,)?) => {
        match $result {
            Ok(value) => value,
            Err(error) => return Err($crate::view::View::count_error(error, $view.base()).into()),
        }
    };
}
pub(crate) use count_try;

// Share header checks and keep each caller's arithmetic within its CU bounds.
// Both extent forms enforce the u32 wire bound on 32-bit and 64-bit hosts.
macro_rules! read_list_header {
    ($view:ident, $wide:literal) => {{
        let element = if let Layout::Flat { checked } = $view.layout {
            if checked.header(0) != 7 {
                return Err(ErrorKind::InvalidSchema.into());
            }
            Layout::Flat {
                checked: checked.child(0, true),
            }
        } else {
            match $view.layout.head() {
                LayoutHead::List { element } => element,
                _ => return Err(ErrorKind::InvalidSchema.into()),
            }
        };
        let count = $view.word(0)?;
        let width = width(&element);
        let payload = match width {
            Some(width) => {
                $view.exact(
                    if $wide {
                        let end = u64::from(count) * u64::from(width) + 4;
                        if end > u64::from(u32::MAX) {
                            return Err(Error::new(ErrorKind::ArithmeticOverflow, $view.base()));
                        }
                        end as u32
                    } else {
                        count_try!(
                            $view,
                            checked_mul(count, width).and_then(|n| checked_add(4, n))
                        )
                    },
                    4,
                )?;
                4
            }
            None => {
                let payload = if $wide {
                    let end = u64::from(count.saturating_sub(1)) * 4 + 4;
                    if end > u64::from(u32::MAX) {
                        return Err(Error::new(ErrorKind::ArithmeticOverflow, $view.base()));
                    }
                    end as u32
                } else {
                    count_try!(
                        $view,
                        checked_mul(count.saturating_sub(1), 4).and_then(|n| checked_add(4, n))
                    )
                };
                if payload as usize > $view.bytes.len() {
                    return Err($view.fault(
                        ErrorKind::Truncated,
                        4 + ($view.bytes.len() as u32 - 4) / 4 * 4,
                    ));
                }
                if count == 0 {
                    $view.exact(4, 4)?;
                }
                payload
            }
        };
        Ok(ListHeader {
            count,
            width,
            payload,
            element,
        })
    }};
}

impl<'a> View<'a> {
    /// Borrows an exact subtree at `base`, without validating its content.
    ///
    /// The exclusive end must fit `u32`; metadata must fit [`MAX_DEPTH`].
    pub fn new(bytes: &'a [u8], layout: impl Into<Layout>, base: u32) -> Result<Self> {
        let layout = layout.into();
        check_layout(&layout, MAX_DEPTH)?;
        checked_add(base, checked_u32(bytes.len())?)?;
        Ok(Self {
            bytes,
            layout,
            base,
        })
    }

    pub(crate) fn inherited(bytes: &'a [u8], layout: Layout, base: u32) -> Result<Self> {
        checked_add(base, checked_u32(bytes.len())?)?;
        Ok(Self {
            bytes,
            layout,
            base,
        })
    }

    #[inline(always)]
    pub(crate) fn inherited_for_edit(bytes: &'a [u8], layout: Layout, base: u32) -> Result<Self> {
        checked_add(base, checked_u32(bytes.len())?)?;
        Ok(Self {
            bytes,
            layout,
            base,
        })
    }

    /// Loads a root after checking its envelope, discriminator and minimum size.
    ///
    /// With `verify-on-load`, scans the full root before returning. Otherwise
    /// work is constant in account content. Spare storage is excluded from the
    /// returned view. An all-zero expected discriminator is `InvalidSchema`.
    /// Short envelopes return `Truncated` at zero, mismatches `Discriminator`
    /// at zero, and invalid used lengths `UsedLength` at eight.
    ///
    /// Trust requires bytes written by this program through blubber, with owner
    /// and discriminator checked. Reads and edits check only touched structure;
    /// malformed directories can alias siblings within the account.
    #[inline(always)]
    pub fn load_trusted<S: RootSchema>(bytes: &'a [u8], discriminator: [u8; 8]) -> Result<Self> {
        let view = Self::load_envelope::<S>(bytes, discriminator)?;
        #[cfg(feature = "verify-on-load")]
        view.validate()?;
        Ok(view)
    }

    #[inline(always)]
    fn load_envelope<S: RootSchema>(bytes: &'a [u8], discriminator: [u8; 8]) -> Result<Self> {
        let used = blubber_core::raw::account_used(bytes, discriminator, S::MIN_SIZE);
        #[cfg(all(feature = "pinocchio", any(target_os = "solana", target_arch = "bpf")))]
        let used = used.map_err(|error| identity_admission_error(&(discriminator, bytes), error));
        let used = used?;
        // The proved envelope check establishes every View bound here.
        Ok(Self {
            bytes: &bytes[12..used as usize],
            layout: Layout::Flat {
                checked: S::__layout(),
            },
            base: 12,
        })
    }

    pub(crate) fn load_layout(
        bytes: &'a [u8],
        discriminator: [u8; 8],
        layout: Layout,
        minimum: u32,
    ) -> Result<Self> {
        if discriminator == [0; 8] {
            return Err(ErrorKind::InvalidSchema.into());
        }
        checked_u32(bytes.len())?;
        if bytes.len() < 12 {
            return Err(Error::new(ErrorKind::Truncated, 0));
        }
        if bytes[..8] != discriminator {
            return Err(Error::new(ErrorKind::Discriminator, 0));
        }
        let minimum = checked_add(12, minimum)?;
        let used = blubber_core::read_u32(bytes, 8)?;
        if used < minimum || used as usize > bytes.len() {
            return Err(Error::new(ErrorKind::UsedLength, 8));
        }
        Self::inherited(&bytes[12..used as usize], layout, 12)
    }

    /// Loads an envelope and validates every root byte before returning.
    #[inline(always)]
    pub fn load<S: RootSchema>(bytes: &'a [u8], discriminator: [u8; 8]) -> Result<Self> {
        let view = Self::load_envelope::<S>(bytes, discriminator)?;
        view.validate()?;
        Ok(view)
    }

    /// Returns the exact encoded subtree, excluding any envelope or spare.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Returns this subtree's account-relative start.
    pub const fn base(self) -> u32 {
        self.base
    }

    /// Returns this subtree's schema descriptor.
    pub const fn layout(self) -> Layout {
        self.layout
    }

    /// Checks every boundary, leaf domain, child and map key in this subtree.
    pub fn validate(self) -> Result<()> {
        self.relative(blubber_core::validate(self.bytes, &self.layout, MAX_DEPTH))
    }

    /// Decodes this region using a schema's checked reader.
    pub fn read<S: Schema>(self) -> Result<S::Read<'a>> {
        S::read(self)
    }

    /// Decodes an exact fixed value and rebases a codec fault once.
    pub fn read_fixed<T: Fixed>(self) -> Result<T> {
        self.relative(T::decode(self.bytes))
    }

    /// Borrows UTF-8 only after Rust's safe UTF-8 check succeeds.
    pub fn read_str(self) -> Result<&'a str> {
        core::str::from_utf8(self.bytes).map_err(|error| {
            Error::new(ErrorKind::InvalidUtf8, error.valid_up_to() as u32).rebase(self.base)
        })
    }

    /// Selects a struct field by declaration ordinal, checking its two ends.
    ///
    /// Other directory words and child contents remain lazy. Missing ordinals
    /// return `IndexOutOfBounds`; a nonstruct descriptor returns `InvalidSchema`.
    #[doc(hidden)]
    pub fn field(self, declaration: u16) -> Result<Self> {
        if let Layout::Flat { checked } = self.layout {
            return self.field_flat(checked, declaration);
        }
        let (fixed_bytes, fixed, variable) = match self.layout.head() {
            LayoutHead::Struct {
                fixed_bytes,
                fixed,
                variable,
            } => (fixed_bytes, fixed, variable),
            _ => return Err(ErrorKind::InvalidSchema.into()),
        };
        let directory_bytes = checked_mul(variable.len().saturating_sub(1) as u32, 4)?;
        let payload = checked_add(fixed_bytes, directory_bytes)?;
        self.require(fixed_bytes, ErrorKind::Truncated, 0)?;
        if payload as usize > self.bytes.len() {
            let at = fixed_bytes + (self.bytes.len() as u32 - fixed_bytes) / 4 * 4;
            return Err(self.fault(ErrorKind::Truncated, at));
        }
        if variable.is_empty() {
            self.exact(payload, 0)?;
        }
        if let Some(field) = (0..fixed.len())
            .map(|i| fixed.get(i))
            .find(|field| field.declaration == declaration)
        {
            let end = checked_add(
                field.offset,
                width(&field.layout).ok_or(ErrorKind::InvalidSchema)?,
            )?;
            return self.child(
                Span {
                    start: field.offset,
                    end,
                },
                field.layout,
            );
        }
        let (index, field) = (0..variable.len())
            .map(|i| (i, variable.get(i)))
            .find(|(_, field)| field.declaration == declaration)
            .ok_or(ErrorKind::IndexOutOfBounds)?;
        let span = self.indexed_span(
            payload,
            fixed_bytes,
            4,
            variable.len() as u32,
            index as u32,
            false,
        )?;
        self.child(span, field.layout)
    }

    /// Selects a fixed field using an account schema's certified descriptor.
    ///
    /// Used by copied constraint metadata. The supplied View's descriptor is
    /// replaced, while its exact bytes and error base are retained. Checks the
    /// selected partition without validating unrelated variable children.
    #[doc(hidden)]
    #[inline(always)]
    pub fn __account_fixed_field<S: RootSchema>(self, declaration: u16) -> Result<Self> {
        let checked = S::__layout();
        checked_add(self.base, checked_u32(self.bytes.len())?)?;
        if checked.header(0) != 9 {
            return Err(ErrorKind::InvalidSchema.into());
        }
        let fixed = checked.header(1);
        if self.bytes.len() < fixed as usize {
            return Err(self.fault(ErrorKind::Truncated, 0));
        }
        if checked.header(4) == 0 && self.bytes.len() != fixed as usize {
            return Err(self.fault(ErrorKind::InvalidBoundary, fixed));
        }
        let wire = checked
            .declaration_index(declaration)
            .ok_or(ErrorKind::IndexOutOfBounds)?;
        if wire >= checked.header(3) as usize {
            return Err(ErrorKind::InvalidSchema.into());
        }
        let (_, offset, child) = checked.field(wire, false);
        let width = child.header(1);
        let end = checked_add(offset, width)?;
        // The certified fixed partition puts this field below `fixed`, already
        // checked against bytes.len(). Rust indexing retains its own bounds check.
        Ok(Self {
            bytes: &self.bytes[offset as usize..end as usize],
            layout: Layout::Flat { checked: child },
            base: checked_add(self.base, offset)?,
        })
    }

    /// Selects a concrete schema's sole variable tail, retaining foreign-layout fallback.
    ///
    /// The immutable table identity binds the generated ordinal to this View.
    /// Other fields and inherited tables use the ordinary checked selector.
    #[doc(hidden)]
    #[inline(always)]
    pub fn field_tail<S: RootSchema>(self, declaration: u16) -> Result<Self> {
        let parent = S::__layout();
        if parent.header(0) == 9 && parent.header(4) == 1 && parent.field(0, true).0 == declaration
        {
            if let Layout::Flat { checked } = self.layout {
                if core::ptr::eq(checked.words(), parent.words()) && checked.root() == parent.root()
                {
                    let prefix = parent.header(1);
                    self.require(prefix, ErrorKind::Truncated, 0)?;
                    let child_layout = checked.child(0, true);
                    // Preserve the actual certificate, including its budget.
                    // A contained suffix preserves the admitted parent end.
                    return Ok(Self {
                        bytes: &self.bytes[prefix as usize..],
                        layout: Layout::Flat {
                            checked: child_layout,
                        },
                        base: checked_add(self.base, prefix)?,
                    });
                }
            }
        }
        self.field(declaration)
    }

    #[inline(always)]
    fn field_flat(
        self,
        checked: blubber_core::CheckedLayout<'static>,
        declaration: u16,
    ) -> Result<Self> {
        if checked.header(0) != 9 {
            return Err(ErrorKind::InvalidSchema.into());
        }
        let fixed_bytes = checked.header(1);
        let nf = checked.header(3) as usize;
        let nv = checked.header(4) as usize;
        let directory_bytes = checked_mul((nv as u32).saturating_sub(1), 4)?;
        let payload = checked_add(fixed_bytes, directory_bytes)?;
        self.require(fixed_bytes, ErrorKind::Truncated, 0)?;
        if payload as usize > self.bytes.len() {
            let at = fixed_bytes + (self.bytes.len() as u32 - fixed_bytes) / 4 * 4;
            return Err(self.fault(ErrorKind::Truncated, at));
        }
        if nv == 0 {
            self.exact(payload, 0)?;
        }
        let wire = checked
            .declaration_index(declaration)
            .ok_or(ErrorKind::IndexOutOfBounds)?;
        // The sole variable child has two implicit ends. The prefix check
        // above and the certified declaration table establish its exact span.
        if wire >= nf && nv == 1 {
            return Self::inherited_for_edit(
                &self.bytes[fixed_bytes as usize..],
                Layout::Flat {
                    checked: checked.child(0, true),
                },
                checked_add(self.base, fixed_bytes)?,
            );
        }
        if wire < nf {
            let (_, offset, child) = checked.field(wire, false);
            let end = checked_add(offset, child.header(1))?;
            self.child(Span { start: offset, end }, Layout::Flat { checked: child })
        } else {
            let index = wire - nf;
            let span =
                self.indexed_span(payload, fixed_bytes, 4, nv as u32, index as u32, false)?;
            self.child(
                span,
                Layout::Flat {
                    checked: checked.child(index, true),
                },
            )
        }
    }

    pub(crate) fn relative<T>(self, result: Result<T>) -> Result<T> {
        result.map_err(|error| error.rebase(self.base))
    }

    pub(crate) fn fault(self, kind: ErrorKind, offset: u32) -> Error {
        Error::new(kind, offset).rebase(self.base)
    }

    #[inline(always)]
    pub(crate) fn word(self, offset: u32) -> Result<u32> {
        self.relative(blubber_core::read_u32(self.bytes, offset))
    }

    fn require(self, end: u32, kind: ErrorKind, offset: u32) -> Result<()> {
        if end as usize > self.bytes.len() {
            Err(self.fault(kind, offset))
        } else {
            Ok(())
        }
    }

    fn exact(self, end: u32, short_offset: u32) -> Result<()> {
        self.require(end, ErrorKind::Truncated, short_offset)?;
        if end as usize != self.bytes.len() {
            return Err(self.fault(ErrorKind::InvalidBoundary, end));
        }
        Ok(())
    }

    pub(crate) fn child(self, span: Span, layout: Layout) -> Result<Self> {
        let len = span
            .end
            .checked_sub(span.start)
            .ok_or_else(|| self.fault(ErrorKind::InvalidBoundary, span.start))?;
        self.relative(blubber_core::checked_span(
            span.start,
            len,
            self.bytes.len() as u32,
        ))?;
        let base = checked_add(self.base, span.start)?;
        Self::inherited(
            &self.bytes[span.start as usize..span.end as usize],
            layout,
            base,
        )
    }

    // Selection-only callers need the same geometry and error frame without
    // transporting an unused descriptor and borrowed child view.
    #[inline(always)]
    fn check_selection(self, span: Span) -> Result<()> {
        let len = span
            .end
            .checked_sub(span.start)
            .ok_or_else(|| self.fault(ErrorKind::InvalidBoundary, span.start))?;
        self.relative(blubber_core::checked_span(
            span.start,
            len,
            self.bytes.len() as u32,
        ))?;
        let base = checked_add(self.base, span.start)?;
        checked_add(base, len)?;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn option_present(self) -> Result<bool> {
        if !matches!(self.layout.head(), LayoutHead::Option { .. }) {
            return Err(ErrorKind::InvalidSchema.into());
        }
        match self.relative(blubber_core::partition_option(self.bytes, &self.layout))? {
            Some(span) => {
                self.check_selection(span)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub(crate) fn option(self) -> Result<Option<Self>> {
        let value = match self.layout.head() {
            LayoutHead::Option { value } => value,
            _ => return Err(ErrorKind::InvalidSchema.into()),
        };
        self.relative(blubber_core::partition_option(self.bytes, &self.layout))?
            .map(|span| self.child(span, value))
            .transpose()
    }

    // Checks just the selected adjacent stored/implicit ends. Directory extent
    // has already been checked, so every word calculation stays in that extent.
    pub(crate) fn indexed_span(
        self,
        payload: u32,
        directory: u32,
        stride: u32,
        count: u32,
        index: u32,
        terminal: bool,
    ) -> Result<Span> {
        let payload_len = checked_u32(self.bytes.len())? - payload;
        let previous = if index == 0 {
            None
        } else {
            Some(checked_add(directory, checked_mul(index - 1, stride)?)?)
        };
        let current = if !terminal && index + 1 == count {
            None
        } else {
            Some(checked_add(directory, checked_mul(index, stride)?)?)
        };
        let start = match previous {
            Some(at) => self.word(at)?,
            None => 0,
        };
        if start > payload_len {
            return Err(self.fault(ErrorKind::InvalidBoundary, previous.unwrap_or(directory)));
        }
        let end = match current {
            Some(at) => self.word(at)?,
            None => payload_len,
        };
        if end < start
            // control: view-end-bound
            || end > payload_len
            || (terminal && index + 1 == count && end != payload_len)
        {
            return Err(self.fault(ErrorKind::InvalidBoundary, current.unwrap_or(directory)));
        }
        Ok(Span {
            start: checked_add(payload, start)?,
            end: checked_add(payload, end)?,
        })
    }

    #[inline(always)]
    pub(crate) fn list_element_layout(self) -> Result<Layout> {
        Ok(if let Layout::Flat { checked } = self.layout {
            if checked.header(0) != 7 {
                return Err(ErrorKind::InvalidSchema.into());
            }
            Layout::Flat {
                checked: checked.child(0, true),
            }
        } else {
            match self.layout.head() {
                LayoutHead::List { element } => element,
                _ => return Err(ErrorKind::InvalidSchema.into()),
            }
        })
    }

    #[inline(always)]
    pub(crate) fn list_width_for<S: Schema>(self) -> Result<Option<u32>> {
        if let Some(expected) = S::FIXED_SIZE {
            let element = self.list_element_layout()?;
            if width_for_region(&element) == Some(expected) {
                self.relative(
                    blubber_core::raw::fixed_read_payload(self.bytes, expected)
                        .map_err(|error| Self::count_error(error, 0)),
                )?;
                return Ok(Some(expected));
            }
        }
        Ok(self.walk_list_header()?.width)
    }

    #[cold]
    #[inline(never)]
    pub(crate) fn count_error(error: Error, base: u32) -> Error {
        match error {
            Error::Fault {
                kind: ErrorKind::ArithmeticOverflow,
                ..
            } => Error::new(ErrorKind::ArithmeticOverflow, base),
            other => other,
        }
    }

    pub(crate) fn list_header(self) -> Result<ListHeader> {
        read_list_header!(self, true)
    }

    #[inline(always)]
    pub(crate) fn walk_list_header(self) -> Result<ListHeader> {
        read_list_header!(self, false)
    }

    #[inline(always)]
    pub(crate) fn map_header(self) -> Result<MapHeader> {
        let (key_width, value) = if let Layout::Flat { checked } = self.layout {
            if checked.header(0) != 8 {
                return Err(ErrorKind::InvalidSchema.into());
            }
            (
                checked.header(6),
                Layout::Flat {
                    checked: checked.child(0, true),
                },
            )
        } else {
            match self.layout.head() {
                LayoutHead::Map { key, value } => (key.width, value),
                _ => return Err(ErrorKind::InvalidSchema.into()),
            }
        };
        let count = self.word(0)?;
        let width = width(&value);
        if count == 0 {
            self.exact(4, 4)?;
            return Ok(MapHeader {
                count,
                key_width,
                width,
                stride: 0,
                payload: 4,
                value,
            });
        }
        let stride = count_try!(self, checked_add(key_width, width.unwrap_or(4)));
        let extent = count_try!(
            self,
            checked_mul(count, stride).and_then(|n| checked_add(4, n))
        );
        let payload = if width.is_some() {
            self.exact(extent, 4)?;
            4
        } else {
            if extent as usize > self.bytes.len() {
                let at = 4 + (self.bytes.len() as u32 - 4) / stride * stride;
                return Err(self.fault(ErrorKind::Truncated, at));
            }
            extent
        };
        Ok(MapHeader {
            count,
            key_width,
            width,
            stride,
            payload,
            value,
        })
    }
}

// Descriptor checks belong to View construction; this only classifies a head.
#[inline(always)]
fn width(layout: &Layout) -> Option<u32> {
    width_for_region(layout)
}

#[inline(always)]
pub(crate) fn width_for_region(layout: &Layout) -> Option<u32> {
    if let Layout::Flat { checked } = layout {
        return match checked.header(0) {
            1..=4 => Some(checked.header(1)),
            9 if checked.header(4) == 0 => Some(checked.header(1)),
            _ => None,
        };
    }
    match layout.head() {
        LayoutHead::Leaf(leaf) => Some(match leaf {
            blubber_core::Leaf::Unsigned(width) | blubber_core::Leaf::Signed(width) => {
                u32::from(width)
            }
            blubber_core::Leaf::Bool => 1,
            blubber_core::Leaf::Raw(width) => width,
        }),
        LayoutHead::Struct {
            fixed_bytes,
            variable,
            ..
        } if variable.is_empty() => Some(fixed_bytes),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ListHeader {
    pub count: u32,
    pub(crate) width: Option<u32>,
    pub(crate) payload: u32,
    pub(crate) element: Layout,
}

impl ListHeader {
    #[inline(always)]
    pub fn checked_span(self, view: View<'_>, index: u32) -> Result<Span> {
        let span = match self.width {
            Some(width) => {
                let start = checked_add(4, checked_mul(index, width)?)?;
                Span {
                    start,
                    end: checked_add(start, width)?,
                }
            }
            None => view.indexed_span(self.payload, 4, 4, self.count, index, false)?,
        };
        view.check_selection(span)?;
        Ok(span)
    }

    #[inline(always)]
    pub fn check_element(self, view: View<'_>, index: u32) -> Result<()> {
        self.checked_span(view, index).map(|_| ())
    }

    pub fn element<'a>(self, view: View<'a>, index: u32) -> Result<View<'a>> {
        let span = match self.width {
            Some(width) => {
                let start = checked_add(4, checked_mul(index, width)?)?;
                Span {
                    start,
                    end: checked_add(start, width)?,
                }
            }
            None => view.indexed_span(self.payload, 4, 4, self.count, index, false)?,
        };
        view.child(span, self.element)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MapHeader {
    pub count: u32,
    pub(crate) key_width: u32,
    pub(crate) width: Option<u32>,
    pub(crate) stride: u32,
    pub(crate) payload: u32,
    pub(crate) value: Layout,
}

impl MapHeader {
    pub fn key<'a>(self, view: View<'a>, index: u32) -> Result<(&'a [u8], u32)> {
        let start = checked_add(4, checked_mul(index, self.stride)?)?;
        let end = checked_add(start, self.key_width)?;
        view.require(end, ErrorKind::Truncated, start)?;
        Ok((
            &view.bytes[start as usize..end as usize],
            checked_add(view.base, start)?,
        ))
    }

    #[inline(always)]
    pub fn checked_span(self, view: View<'_>, index: u32) -> Result<Span> {
        let span = match self.width {
            Some(width) => {
                let start = checked_add(
                    checked_add(4, checked_mul(index, self.stride)?)?,
                    self.key_width,
                )?;
                Span {
                    start,
                    end: checked_add(start, width)?,
                }
            }
            None => view.indexed_span(
                self.payload,
                checked_add(4, self.key_width)?,
                self.stride,
                self.count,
                index,
                true,
            )?,
        };
        view.check_selection(span)?;
        Ok(span)
    }

    #[inline(always)]
    pub fn check_value(self, view: View<'_>, index: u32) -> Result<()> {
        self.checked_span(view, index).map(|_| ())
    }

    pub fn value<'a>(self, view: View<'a>, index: u32) -> Result<View<'a>> {
        let span = match self.width {
            Some(width) => {
                let start = checked_add(
                    checked_add(4, checked_mul(index, self.stride)?)?,
                    self.key_width,
                )?;
                Span {
                    start,
                    end: checked_add(start, width)?,
                }
            }
            None => view.indexed_span(
                self.payload,
                checked_add(4, self.key_width)?,
                self.stride,
                self.count,
                index,
                true,
            )?,
        };
        view.child(span, self.value)
    }
}

// Error classification and context stay outside successful envelope admission.
#[cfg(all(feature = "pinocchio", any(target_os = "solana", target_arch = "bpf")))]
#[cold]
#[inline(never)]
fn identity_admission_error((expected, bytes): &([u8; 8], &[u8]), error: Error) -> Error {
    if matches!(
        error,
        Error::Fault {
            kind: ErrorKind::Discriminator,
            ..
        }
    ) {
        report_identity_mismatch(*expected, &bytes[..8]);
    }
    error
}

// Error-path formatting and logging stay out of successful adapter admission.
#[cfg(all(feature = "pinocchio", any(target_os = "solana", target_arch = "bpf")))]
#[cold]
#[inline(never)]
fn report_identity_mismatch(expected: [u8; 8], found: &[u8]) {
    let message = identity_mismatch_message(expected, found);
    if let Ok(text) = core::str::from_utf8(&message) {
        solana_program_log::log(text);
    }
}

#[cfg(all(
    feature = "pinocchio",
    any(test, target_os = "solana", target_arch = "bpf")
))]
/// Formats the schema identity diagnostic without reading outside the found prefix.
fn identity_mismatch_message(expected: [u8; 8], found: &[u8]) -> [u8; 65] {
    let mut message = *b"schema identity: expected=0000000000000000 found=0000000000000000";
    let hex = b"0123456789abcdef";
    for (start, bytes) in [(26, expected.as_slice()), (49, found)] {
        for (index, byte) in bytes.iter().take(8).enumerate() {
            message[start + index * 2] = hex[(byte >> 4) as usize];
            message[start + index * 2 + 1] = hex[(byte & 15) as usize];
        }
    }
    message
}

#[cfg(all(test, feature = "pinocchio"))]
mod identity_diagnostic_tests {
    #[test]
    fn mismatch_names_expected_and_found_in_wire_order() {
        assert_eq!(
            &super::identity_mismatch_message([0x12; 8], &[0xab; 8]),
            b"schema identity: expected=1212121212121212 found=abababababababab"
        );
    }
}
