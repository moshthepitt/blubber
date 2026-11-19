//! Kani installation smoke test; this is not a production arithmetic proof.

#[kani::proof]
#[kani::unwind(1)]
fn arithmetic_checked_add() {
    let left: usize = kani::any();
    let right: usize = kani::any();
    let mathematical = left as u128 + right as u128;
    match left.checked_add(right) {
        Some(sum) => {
            assert_eq!(sum as u128, mathematical);
        }
        None => {
            assert!(mathematical > usize::MAX as u128);
        }
    }
}
