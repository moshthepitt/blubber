//! finite variable map and entry splices with overflow checks enabled.

#![forbid(unsafe_code)]

#[path = "support/exhaustive_typed_plan.rs"]
mod exhaustive;

#[test]
fn all_typed_plan_geometries() {
    exhaustive::all();
}
