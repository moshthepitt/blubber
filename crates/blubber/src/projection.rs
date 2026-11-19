//! Schema-bound authority for downstream generated editors.

use core::marker::PhantomData;

use crate::{AccountSchema, CommitInfo, Edit, Resize, Result, Schema, View};

/// An exclusive selection bound to its schema and enclosing storage loan.
///
/// Only root admission and matching typed child selection create this value.
/// There is no conversion from an untyped raw edit or caller-supplied layout.
pub struct Projection<'a, S: Schema> {
    pub(crate) edit: Edit<'a>,
    marker: PhantomData<fn() -> S>,
}

/// Generated declaration-to-wire mapping for one field of its owning schema.
/// Handwritten schema implementations must uphold the same codec contract.
pub trait ProjectField<const INDEX: u16>: Schema {
    /// Exact schema declared for this field.
    type Field: Schema;
    /// Fixed wire ordinal and packed offset, or a variable field.
    const FIXED: Option<(u16, u32)>;
    /// Variable wire ordinal.
    const VARIABLE: u16;
    /// Generic schemas use declaration traversal without concrete table hints.
    const GENERIC: bool = false;
    /// Concrete certified parent table; generic schemas use checked traversal.
    const PARENT: Option<blubber_core::CheckedLayout<'static>> = None;
}

impl<'a, S: Schema> Projection<'a, S> {
    #[inline(always)]
    pub(crate) fn new(edit: Edit<'a>) -> Self {
        Self {
            edit,
            marker: PhantomData,
        }
    }

    /// Reborrows this exact schema selection.
    #[inline(always)]
    pub fn reborrow(&mut self) -> Projection<'_, S> {
        Projection::new(self.edit.reborrow())
    }

    /// Selects the schema and coordinates declared for a generated field.
    #[inline(always)]
    pub fn field<const INDEX: u16>(&mut self) -> Projection<'_, <S as ProjectField<INDEX>>::Field>
    where
        S: ProjectField<INDEX>,
    {
        const {
            assert!(
                S::__PROJECT_FIELDS,
                "schema does not grant field projections"
            );
            assert!(S::__HEADER[0] == 9, "field projection requires a struct");
            assert!(
                (INDEX as usize) < S::__HEADER[3] as usize + S::__HEADER[4] as usize,
                "field ordinal is outside schema"
            );
        }
        let fixed = const { S::FIXED };
        let variable = const { S::VARIABLE };
        let edit = if S::GENERIC {
            self.edit.reborrow().field(INDEX)
        } else if let Some(parent) = const { S::PARENT } {
            let (index, is_variable) = match fixed {
                Some((index, _)) => (index, false),
                None => (variable, true),
            };
            let selected = parent.child(index as usize, is_variable);
            match fixed {
                Some((index, offset)) => self
                    .edit
                    .field_reborrow_fixed(INDEX, index, offset, selected),
                None => self.edit.field_reborrow_certified(
                    INDEX,
                    None,
                    selected,
                    if parent.header(4) == 1 {
                        Some(parent.header(1))
                    } else {
                        None
                    },
                ),
            }
        } else {
            self.edit.field_reborrow_static(INDEX, fixed, variable)
        };
        Projection::new(edit)
    }

    /// Borrows the selected region for checked reading.
    #[inline(always)]
    pub fn view(&self) -> Result<View<'_>> {
        self.edit.view()
    }
    /// Validates the selected subtree.
    pub fn validate(&self) -> Result<()> {
        self.edit.validate()
    }
    /// Returns the enclosing account's logical length.
    pub fn used_len(&self) -> Result<u32> {
        self.edit.used_len()
    }
    /// Returns the enclosing backend's exposed length.
    pub fn storage_len(&self) -> Result<u32> {
        self.edit.storage_len()
    }
    /// Reserves room relative to the checked enclosing used length.
    pub fn reserve(&mut self, additional: u32) -> Result<()> {
        self.edit.reserve(additional)
    }
    /// Requests compaction to the enclosing used length.
    pub fn shrink_to_fit(&mut self) -> Result<bool> {
        self.edit.shrink_to_fit()
    }
    /// Completes the enclosing session with its checked used length.
    #[inline(always)]
    pub fn commit(self) -> Result<CommitInfo> {
        self.edit.commit()
    }
}

impl<'a, S: AccountSchema> Projection<'a, S> {
    /// Scans the enclosing account root without clearing poison or minting evidence.
    pub fn verify(&self) -> Result<()> {
        self.edit.verify()
    }

    /// Opens an authenticated SDK root lazily, or fully validates a host root.
    ///
    /// SDK callers guarantee initialization or checked migration for this schema
    /// followed only by approved writers. Touched checks do not certify global
    /// map order or disjoint directories. Use `load_checked` for untrusted contents.
    /// `verify-on-load` also scans authenticated roots, without recovering poison.
    #[inline(always)]
    pub fn load(store: &'a mut dyn Resize) -> Result<Self> {
        let backend = store.__backend(crate::storage::WRITER);
        if backend.authenticated() {
            Edit::load_trusted::<S>(backend, S::DISCRIMINATOR).map(Self::new)
        } else {
            Edit::load::<S>(backend, S::DISCRIMINATOR).map(Self::new)
        }
    }
    /// Always scans the complete root, recovering poison only after success.
    #[inline(always)]
    pub fn load_checked(store: &'a mut dyn Resize) -> Result<Self> {
        Edit::load::<S>(store.__backend(crate::storage::WRITER), S::DISCRIMINATOR).map(Self::new)
    }
    /// Opens a root under the explicit valid-current-schema caller obligation.
    #[inline(always)]
    pub fn load_trusted(store: &'a mut dyn Resize) -> Result<Self> {
        Edit::load_trusted::<S>(store.__backend(crate::storage::WRITER), S::DISCRIMINATOR)
            .map(Self::new)
    }
    /// Encodes an owned measurement token using this account's schema identity.
    #[inline(always)]
    pub fn initialize_measured(
        store: &'a mut dyn Resize,
        measured: crate::__private::MeasuredInit<'_, S>,
    ) -> Result<Self> {
        Edit::initialize_measured::<S>(
            store.__backend(crate::storage::WRITER),
            S::DISCRIMINATOR,
            measured,
        )
        .map(Self::new)
    }

    /// Encodes a matching borrowed initializer and publishes its envelope last.
    #[inline(always)]
    pub fn initialize(store: &'a mut dyn Resize, init: S::Init<'_>) -> Result<Self> {
        Edit::initialize::<S>(
            store.__backend(crate::storage::WRITER),
            S::DISCRIMINATOR,
            init,
        )
        .map(Self::new)
    }
}
