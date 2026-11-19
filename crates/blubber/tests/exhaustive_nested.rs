//! nested finite geometries; the Miri lane shares the same byte oracle.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/exhaustive_nested.rs"]
mod exhaustive;

#[test]
fn all_nested_geometries() {
    if cfg!(miri) {
        exhaustive::miri_subset();
    } else {
        exhaustive::all();
    }
}
