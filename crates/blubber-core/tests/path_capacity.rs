//! Path admission agrees with the existing append operation.
#![forbid(unsafe_code)]

use blubber_core::{
    raw::{InlinePath, Step},
    MAX_DEPTH,
};

#[test]
fn capacity_matches_append_through_full_and_sticky_paths() {
    let mut path = InlinePath::new();
    for index in 0..MAX_DEPTH + 2 {
        let mut extended = path;
        let result = extended.push(Step::Index(index as u32));
        assert_eq!(path.can_push(), result.is_ok());
        path = extended;
    }
}
