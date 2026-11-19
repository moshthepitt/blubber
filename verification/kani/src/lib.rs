//! Bounded checks of production typed codecs. Run through scripts/kani.sh.

#![forbid(unsafe_code)]

#[cfg(kani)]
mod arithmetic;

#[cfg(kani)]
mod leaf;

#[cfg(kani)]
mod derive_encode;

#[cfg(kani)]
mod error_codes;

#[cfg(kani)]
mod map_key;

#[cfg(kani)]
mod fixed_struct;

#[cfg(kani)]
mod record;

#[cfg(kani)]
mod typed_plan;
