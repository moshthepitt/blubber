//! bounded-exhaustive native replacement for variable-record Kani edits.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/exhaustive_record.rs"]
mod exhaustive;
#[path = "support/record.rs"]
mod record;

#[test]
fn all_327_record_geometries_and_controls() {
    exhaustive::run_all();
}
