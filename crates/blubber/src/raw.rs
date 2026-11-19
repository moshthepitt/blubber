//! Deliberate low-level access, outside typed schema preservation.
//!
//! Raw writes end the approved writer history. Fully validate with the matching
//! schema before resuming typed access. Backend mutation includes poison and
//! length publication; callers must follow each operation's phase contract.

pub use crate::edit_handle::Edit;
pub use crate::rewrite::{
    encode_rewrite_slot, prepare_fill, prepare_list, prepare_map, prepare_map_values,
    rewrite_collection,
};
pub use crate::storage::Backend as Resize;
pub use blubber_core::raw as core;

/// Enters the raw backend protocol, ending typed writer provenance.
pub fn backend(store: &mut dyn crate::Resize) -> &mut dyn Resize {
    store.__backend(crate::storage::WRITER)
}

/// Extracts an untyped selection, ending its schema-bound authority.
pub fn into_edit<S: crate::Schema>(selection: crate::__private::Projection<'_, S>) -> Edit<'_> {
    selection.edit
}

/// Reinterprets a raw selection under the caller's schema/codec obligation.
///
/// This performs no descriptor comparison or validation. A successful local
/// edit through the result makes no enclosing-root preservation claim.
pub fn project<S: crate::Schema>(edit: Edit<'_>) -> S::Mut<'_> {
    S::project(crate::projection::Projection::new(edit))
}

/// Reinterprets a raw selection with a handwritten fixed codec.
/// Width/domain correspondence and enclosing validity are the caller's duty.
pub fn scalar<T: crate::Fixed>(edit: Edit<'_>) -> crate::ScalarMut<'_, T> {
    crate::ScalarMut::from_raw(edit)
}
