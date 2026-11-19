//! Independent reference encodings and edit oracles for host tests.
//!
//! The recursive [`Schema`] codec uses owned [`Value`] trees and native
//! [`std::collections::BTreeMap`] ordering. [`Model`] edits clone the logical
//! tree and rebuild its complete encoding; they share no offsets, descriptors,
//! moves, validators, or codecs with the production crates. Errors describe
//! reference-model rejection, without promising production fault precedence
//! or byte offsets. Allocation exhaustion follows the standard collections'
//! host behavior; it is not simulated as a recoverable codec failure.
//!
//! ```
//! use blubber_model::{Edit, Model, Schema, Value};
//!
//! let mut model = Model::new(
//!     Schema::List(Box::new(Schema::Str)),
//!     Value::List(vec![]),
//! )?;
//! model.apply(&[], Edit::ListInsert {
//!     index: 0,
//!     values: vec![Value::Str("a".into()), Value::Str("".into())],
//! })?;
//! assert_eq!(model.encode()?, [2, 0, 0, 0, 1, 0, 0, 0, b'a']);
//! # Ok::<(), blubber_model::Error>(())
//! ```

#![forbid(unsafe_code)]

mod fixed_list;

pub use fixed_list::{FixedList, FixedListEdit, FixedListError};

mod fixed_map;
pub use fixed_map::{FixedMap, FixedMapEdit, FixedMapError};

mod indexed_map;
pub use indexed_map::IndexedMap;

mod schema;
pub use schema::{Error, IntWidth, Key, KeySchema, Schema, Value};

mod codec;
pub use codec::Account;

mod edit;
pub use edit::{Edit, Model, PathSegment};

mod api;
pub use api::{ApiEdit, ApiOutcome, ApiReturn, Callbacks, EntryMode, Patch, Selector};
