//! Compile and run accessor collision cases in ordinary feature configurations.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

extern crate blubber as wire;
#[path = "support/review_hygiene.rs"]
mod cases;

#[test]
fn generated_code_survives_review_collisions() {
    cases::bindings::exercise();
    cases::self_and_hrtb::exercise();
    cases::table::exercise();
    cases::type_macros::exercise();
    cases::hidden_macro_constants::exercise();
}
