//! Native adapter metadata and error identity; real SDK guards run under SBF.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]
use blubber_anchor::{
    sdk::{Address, AnchorAccount, Owner},
    Blubber, HANDLE_SLOT_BYTES,
};
#[blubber::schema(account)]
pub struct State {
    pub counter: u64,
}
impl Owner for State {
    const OWNER: Address = Address::new_from_array([1; 32]);
}
#[test]
fn native_metadata_uses_account_contract() {
    assert_eq!(
        Blubber::<State>::MIN_DATA_LEN,
        State::MIN_ACCOUNT_LEN as usize
    );
    const { assert!(!Blubber::<State>::RELAX_READONLY_CPI_BORROW_FROM_MUT) };
    assert_eq!(HANDLE_SLOT_BYTES, 8);
}
