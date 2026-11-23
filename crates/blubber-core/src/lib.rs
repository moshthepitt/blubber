//! `blubber-core` is the allocator-free byte kernel for layout tables, [`validate`], encoding and byte moves. Most programs should use [`blubber`](https://docs.rs/blubber/latest/blubber/) instead, because raw kernel functions require you to uphold their layout premises.
//!
//! ## Validate bytes
//!
//! Errors include an [`ErrorKind`] and a byte offset when the fault has a byte location. [`validate_utf8`] checks a slice in constant space:
//!
//! ```
//! // docs:start core-utf8
//! use blubber_core::validate_utf8;
//!
//! validate_utf8("hello λ".as_bytes())?;
//! assert!(validate_utf8(&[b'a', 0xff]).is_err());
//! // docs:end core-utf8
//! # Ok::<(), blubber_core::Error>(())
//! ```
//!
//! Verus checks the byte operations under their stated premises. See the [crate README](https://github.com/moshthepitt/blubber/blob/main/crates/blubber-core/README.md#work-with-encoded-bytes), [`SPEC.md`](https://github.com/moshthepitt/blubber/blob/main/SPEC.md#strict-raw-gateways) and [`ASSURANCE.md`](https://github.com/moshthepitt/blubber/blob/main/ASSURANCE.md#production-proofs-and-bounds).

#![no_std]
#![forbid(unsafe_code)]

mod ancestor_pass;
mod arithmetic;
#[cfg(verus_only)]
pub mod batched_read;
pub mod concat_read;
mod edit_layout;
pub mod encoding;
mod encoding_array_source;
#[cfg(verus_only)]
pub mod encoding_graph;
pub mod encoding_leaf;
pub mod encoding_list;
#[cfg(verus_only)]
mod encoding_list_fixed_inputs;
#[cfg(verus_only)]
mod encoding_list_repeated_inputs;
#[cfg(verus_only)]
mod encoding_list_text_inputs;
#[cfg(verus_only)]
pub mod encoding_list_valid;
pub mod encoding_map;
#[cfg(verus_only)]
mod encoding_map_fixed_inputs;
#[cfg(verus_only)]
mod encoding_map_text_inputs;
#[cfg(verus_only)]
pub mod encoding_map_valid;
pub mod encoding_metadata;
pub mod encoding_option;
mod encoding_pair;
pub mod encoding_product;
pub mod encoding_publication;
pub mod encoding_replay;
#[cfg(verus_only)]
mod encoding_replay_inputs;
#[cfg(verus_only)]
pub mod encoding_struct;
pub mod encoding_text;
mod encoding_text_source;
#[cfg(verus_only)]
mod encoding_utf8;
mod error;
pub mod fill;
mod fixed_edit;
#[cfg(verus_only)]
pub mod fixed_map_fold;
mod flat_layout;
mod layout;
mod layout_head;
#[cfg(verus_only)]
pub mod lease_budget;
mod leaves;
mod list_layout;
pub mod list_read;
mod map_layout;
pub mod map_read;
pub mod map_values;
mod moves;
#[doc(hidden)]
pub mod patch_pass;
pub mod point_read;
#[cfg(verus_only)]
mod preservation;
#[cfg(feature = "program-error")]
mod program_error;
pub mod raw;
mod replace_plan;
mod search;
mod struct_layout;
mod utf8;
mod validation;
mod words;
pub(crate) use raw::{move_bytes, write_u32};

pub use arithmetic::{checked_add, checked_delta, checked_mul, checked_u32};
pub use error::{Error, ErrorKind, Result};
pub use flat_layout::{certify, CheckedLayout, RECORD_WORDS};
pub use layout::{check_layout, FixedField, KeyDomain, KeyLayout, Layout, Leaf, VariableField};
pub use layout_head::{Field, Fields, LayoutHead};
pub use leaves::{partition_option, validate_fixed};
pub use list_layout::{list_element, partition_list, ListPartition};
pub use map_layout::{map_value, partition_map, MapPartition};
#[doc(hidden)]
pub use search::search_map_local;
#[cfg(kani)]
#[doc(hidden)]
pub use search::{__local_key_order, __strict_map_partition};
pub use search::{compare_keys, map_key, search_map, validate_map_keys, Search};
pub use struct_layout::{fixed_region, partition_struct, struct_variable_field, StructPartition};
pub use utf8::validate_utf8;
pub use validation::validate;
pub use words::{checked_span, read_u32, Span};

vstd::prelude::verus! {
/// Default maximum number of descriptor nodes on a root-to-leaf path.
pub const DEFAULT_MAX_DEPTH: usize = 8;
/// Maximum descriptor depth selected by this build.
pub const MAX_DEPTH: usize = if cfg!(feature = "max-depth-16") { 16 } else { 8 };
}
