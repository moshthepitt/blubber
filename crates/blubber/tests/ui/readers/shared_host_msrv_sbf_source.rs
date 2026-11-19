#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
/// Renamed dependency for the shared source.
pub extern crate wire as blubber;
/// The identical host and SBF fixture.
#[path = "../../fixtures/walk_contract.rs"] pub mod contract;
use foreign as _;
