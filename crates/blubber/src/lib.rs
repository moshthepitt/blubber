//! Blubber stores lists, strings and maps in account data and edits them in place. Account data is the byte slice where a program's state lives. A borrowed view reads those bytes while the borrow is live.
//!
//! ## Try it on the host
//!
//! Host storage runs off-chain in tests and tools. [`SliceStore`] wraps a mutable byte slice. [`VecStore`](https://docs.rs/blubber/0.1.0/blubber/struct.VecStore.html) wraps a growable vector when you enable `alloc`.
//!
//! ```
//! # #[cfg(feature = "derive")]
//! # fn main() -> blubber::Result<()> {
//! // docs:start host-round-trip
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
//! // docs:end host-round-trip
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "derive"))]
//! # fn main() {}
//! ```
//!
//! `ScoresInit` is the initializer for the new account and borrows the initial list items. `Scores::initialize` writes the initial bytes, `push` appends without decoding into a `Vec`, and `commit` finishes the edit and returns [`CommitInfo`] with the final lengths. `Scores::load` and `Scores::load_checked_mut` scan the used bytes against the schema before returning. `Scores::load_trusted` checks the header, then checks fields as you read them with methods such as `try_get`. It requires valid encoding from approved writers. With `verify-on-load`, it scans the full used encoding before returning.
//!
//! ## Use it in a Solana program
//!
//! The crate is `no_std`, keeps allocation opt-in, and requires Rust 1.89.0 or newer. `derive` is on by default and re-exports [`#[blubber::schema]`](https://docs.rs/blubber-derive/0.1.0/blubber_derive/attr.schema.html) and [`#[derive(blubber::Fixed)]`](https://docs.rs/blubber-derive/0.1.0/blubber_derive/derive.Fixed.html). Enable `pinocchio` for the [`pinocchio`](https://docs.rs/blubber/0.1.0/blubber/pinocchio/index.html) module's account guards, rent top-ups, sizing options and creation helpers. Adding `max-depth-16` raises the nested schema walk limit from 8 nodes to 16 with the same byte layout. A Blubber fault carries an [`ErrorKind`] and a byte offset when the fault has a byte location. [`Error`] can also carry an SDK error code.
//!
//! See the [crate README](https://github.com/moshthepitt/blubber/blob/main/crates/blubber/README.md#check-the-handler), the [guide](https://github.com/moshthepitt/blubber/blob/main/docs/guide.md#create-an-account) and the [trust model](https://github.com/moshthepitt/blubber/blob/main/docs/trust-model.md#choose-a-load).

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
pub use codecs::{Fixed, MapKey};
pub use cursor::{ListCursorMut, MapCursorMut};
pub(crate) use edit_handle::Edit;
pub use entry::{Entry, OccupiedEntry, VacantEntry};
pub use mutable::{ListMut, MapMut, OptionMut, ScalarMut, StrMut};
pub use readers::{
    ListIter, ListRef, MapIter, MapRef, TryBatchedListIter, TryBatchedMapIter, TryListIter,
    TryMapIter, TryMapValues,
};
pub use schema::{Kind, List, Map, RootSchema, Schema, Str};
/// A 32-byte Solana account address, usable as a scalar or a map key.
pub use solana_address::Address;
#[cfg(feature = "alloc")]
pub use storage::VecStore;
pub use storage::{CommitInfo, ReadStorage, Resize, SliceStore};
pub use view::View;

/// The SDK used by the account runtime adapter, including the entrypoint macros.
#[cfg(feature = "pinocchio")]
pub use ::pinocchio as pinocchio_sdk;

#[cfg(feature = "derive")]
pub use blubber_derive::{schema, Fixed};

/// Example expansion of the [schema attribute](macro@schema) for a concrete account.
#[cfg(all(doc, docsrs, feature = "derive"))]
pub mod example_generated {
    use crate::{Address, List};

    /// An append-only queue of numbers.
    #[crate::schema(account)]
    pub struct Queue {
        /// Signer allowed to append.
        pub authority: Address,
        /// Values in insertion order.
        pub values: List<u64>,
    }
}

#[cfg(all(test, feature = "derive"))]
mod edit_routes;

#[cfg(all(test, feature = "alloc", feature = "pinocchio"))]
#[path = "../tests/support/mutation_suite.rs"]
mod mutation_suite;
