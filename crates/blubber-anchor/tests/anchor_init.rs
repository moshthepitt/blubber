//! Native init capability and borrowed inputs; actual SDK views run in SBF.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{List, Str};
use blubber_anchor::{
    sdk::{AccountInitialize, Address, Owner},
    Blubber,
};

#[blubber::schema(account)]
pub struct State {
    pub authority: Address,
    pub values: List<Str>,
}
impl Owner for State {
    const OWNER: Address = Address::new_from_array([1; 32]);
}

#[test]
fn native_init_has_no_owned_or_static_input_requirement() {
    fn accepts_hook<T: AccountInitialize>() {}
    accepts_hook::<Blubber<State>>();
    let _: <Blubber<State> as AccountInitialize>::Params<'_> = Default::default();
    fn borrowed(account: &mut Blubber<State>, text: &str) {
        let _ = account.initialize(StateInit {
            authority: State::OWNER,
            values: &[text],
        });
        let _ = account.initialize_if_new(StateInit {
            authority: State::OWNER,
            values: &[text],
        });
    }
    let _ = borrowed;
}

#[test]
fn minimum_space_includes_the_envelope_and_empty_list() {
    assert_eq!(State::space(0).unwrap(), 48);
    assert_eq!(State::MIN_ACCOUNT_LEN, 48);
    assert_eq!(State::space(100).unwrap(), 148);
    assert!(State::space(u32::MAX as usize).is_err());
}
