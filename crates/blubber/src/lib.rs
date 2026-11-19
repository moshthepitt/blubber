//! Typed, borrowed reads and edits over contiguous account storage.
//!
//! Use `#[blubber::schema]` for a concrete account root and nested record markers. Initialize
//! a [`SliceStore`], edit generated fields, and propagate the root's `commit()`.
//! The crate README has a complete host example; the user guide covers runtime
//! authority, growth budgets and rent. See
//! [For coding agents](https://github.com/moshthepitt/blubber/blob/v0.1.0/crates/blubber/README.md#for-coding-agents)
//! for the integration rules.
//!
//! Raw and host loads validate the complete active root. Authenticated Pinocchio
//! loads check owner, schema identity and envelope, then touched contents under
//! valid current-schema writer history. Explicit checked entry and root `verify()`
//! scan the whole root. The opt-in `verify-on-load` feature adds those scans to
//! lazy entry and initialization. Default headroom is `UpTo(0)`.
//!
//! ```
//! # #[cfg(feature = "derive")]
//! # fn main() -> blubber::Result<()> {
//! use blubber::{List, SliceStore};
//!
//! #[blubber::schema(account)]
//! struct Scores { values: List<u64> }
//!
//! let mut bytes = [0; 128];
//! let mut store = SliceStore::new(&mut bytes);
//! let mut scores = Scores::initialize(&mut store, ScoresInit { values: &[3, 4] })?;
//! scores.values_mut().push(5)?;
//! let receipt = scores.commit()?;
//! let scores = Scores::load(&bytes[..receipt.used_len as usize])?;
//! assert_eq!(scores.values().try_get(2)?, Some(5));
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "derive"))]
//! # fn main() {}
//! ```

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
// The mutation oracle retains integration modules and their shared fixtures.
#![cfg_attr(test, allow(clippy::duplicate_mod))]

extern crate self as blubber;

#[cfg(feature = "alloc")]
extern crate alloc;

#[doc(hidden)]
pub mod __private;
mod account;
mod codecs;
mod cursor;
mod edit;
mod edit_handle;
mod entry;
/// Canonical wire contracts and compile-time schema identity.
pub mod identity;
mod identity_sha256;
mod list_bounds;
mod list_bulk;
mod mutable;
#[cfg(feature = "alloc")]
mod owned;
#[cfg(feature = "pinocchio")]
pub mod pinocchio;
mod product;
mod projection;
pub mod raw;
mod readers;
mod rewrite;
mod schema;
mod schema_table;
mod storage;
mod view;

pub use account::{AccountSchema, MAX_CONSTRAINT_PREFIX_SIZE};
pub use blubber_core::{Error, ErrorKind, Result};
pub use codecs::{Fixed, MapKey, Pubkey};
pub use cursor::{ListCursorMut, MapCursorMut};
pub(crate) use edit_handle::Edit;
pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use mutable::{ListMut, MapMut, OptionMut, ScalarMut, StrMut};
pub use readers::{
    ListIter, ListRef, MapIter, MapRef, TryBatchedListIter, TryBatchedMapIter, TryListIter,
    TryMapIter, TryMapValues,
};
pub use schema::{Kind, List, Map, RootSchema, Schema, Str};
#[cfg(feature = "alloc")]
pub use storage::VecStore;
pub use storage::{CommitInfo, ReadStorage, Resize, SliceStore};
pub use view::View;

#[cfg(feature = "derive")]
pub use blubber_derive::{schema, Fixed};

#[cfg(all(test, feature = "derive"))]
mod edit_routes;

#[cfg(all(test, feature = "alloc", feature = "pinocchio"))]
#[path = "../tests/support/mutation_suite.rs"]
mod mutation_suite;
