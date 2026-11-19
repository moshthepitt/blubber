//! Untyped writes and destination loans, outside typed schema preservation.
//! Follow the bounds, readiness, poison and publication contracts of each operation.

//! Preparation, movement, destination issuance and publication consume their states.
//! The destination is borrowed through `bytes()` after issuance.
//!
//! ```compile_fail,E0599
//! use blubber_core::raw::CheckedSubtreeReady;
//! fn premature(write: CheckedSubtreeReady<'_>, poison: &mut bool) {
//!     write.finish(poison);
//! }
//! ```
//! ```compile_fail,E0382
//! use blubber_core::raw::CheckedSubtreeReady;
//! fn repeated(write: CheckedSubtreeReady<'_>, poison: &mut bool) {
//!     let moved = write.move_regions(poison);
//!     let _first = moved.destination();
//!     let _second = moved.destination();
//! }
//! ```
//! ```compile_fail,E0599
//! use blubber_core::raw::CheckedListReady;
//! fn premature(write: CheckedListReady<'_, '_>, poison: &mut bool) {
//!     write.finish();
//! }
//! ```
//! ```compile_fail,E0382
//! use blubber_core::raw::CheckedListReady;
//! fn repeated(write: CheckedListReady<'_, '_>, poison: &mut bool) {
//!     let moved = write.move_regions(poison).unwrap();
//!     let _first = moved.destination();
//!     let _second = moved.destination();
//! }
//! ```
//! ```compile_fail,E0599
//! use blubber_core::raw::AncestorReady;
//! fn premature(write: AncestorReady<'_, '_>, poison: &mut bool) {
//!     write.finish(poison);
//! }
//! ```
//! ```compile_fail,E0382
//! use blubber_core::raw::AncestorReady;
//! fn repeated(write: AncestorReady<'_, '_>, poison: &mut bool) {
//!     let moved = write.move_regions(poison);
//!     let _first = moved.destination();
//!     let _second = moved.destination();
//! }
//! ```

#[doc(hidden)]
pub use crate::fixed_edit::fixed_destination;
pub use crate::moves::move_bytes;
#[doc(hidden)]
pub use crate::replace_plan::{
    account_used, adjust_directory, fixed_chunk_base, fixed_chunk_end, fixed_list_payload,
    fixed_read_payload, fixed_slot_region, indexed_coordinate, indexed_map_key, indexed_next_end,
    indexed_read_word, indexed_slot_region, indexed_string_window, prepare_byte_replacement,
    prepare_checked_byte_replacement, prepare_fixed_insert, prepare_string_write,
    prepare_tail_append, prepare_tail_remove, resolve_account_edit, resolve_edit, resolve_local,
    resolve_rewrite_tail, split_fixed_chunk, split_fixed_map_entry, split_fixed_slot,
    AncestorApply, AncestorMoved, AncestorPlan, AncestorPrepared, AncestorReady, BulkLayout,
    BulkListPlan, BulkListSource, BulkSlots, CheckedListApply, CheckedListMoved, CheckedListPlan,
    CheckedListPrepared, CheckedListReady, CheckedSubtreeMoved, CheckedSubtreePlan,
    CheckedSubtreeReady, CheckedSubtreeWrite, FixedInsert, HeaderBulkLayout, HeaderBulkPlan,
    HeaderBulkSlots, HeaderBulkSource, Prepared, Resolved, RewritePlan, RewriteStream, RewriteTail,
    StringWrite, SubtreeWrite, TailAppend, TailRemove,
};
pub use crate::replace_plan::{
    begin_apply, plan_list_edit, plan_map_edit, plan_replace, resolve, Apply, InlinePath, ListEdit,
    MapEdit, Plan, Step,
};
pub use crate::words::write_u32;

#[cfg(kani)]
#[doc(hidden)]
pub use crate::moves::fixed_removal_move_model;

#[cfg(kani)]
#[doc(hidden)]
pub use crate::replace_plan::patch_directory_suffix as kani_patch_suffix;

#[cfg(kani)]
#[doc(hidden)]
pub use crate::replace_plan::{__list_insert_model, __list_splice_model, __map_patch_model};

#[cfg(kani)]
pub use crate::replace_plan::__ancestor_pass_model;
