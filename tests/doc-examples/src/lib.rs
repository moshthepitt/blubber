//! Compiled user-document examples for this repository.

#![forbid(unsafe_code)]
#![cfg_attr(doctest, doc = include_str!("../../../README.md"))]

#[cfg(doctest)]
#[doc = include_str!("../../../docs/guide.md")]
mod guide {}

#[cfg(doctest)]
#[doc = include_str!("../../../docs/recipes.md")]
mod recipes {}

#[cfg(doctest)]
#[doc = include_str!("../../../docs/trust-model.md")]
mod trust_model {}

#[cfg(doctest)]
#[doc = include_str!("../../../docs/performance.md")]
mod performance {}
