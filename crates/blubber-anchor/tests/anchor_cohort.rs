//! Compile and exercise the nominal SDK types shared by the native feature.
#![forbid(unsafe_code)]

use blubber::pinocchio;
use blubber_anchor::sdk;

const ID: sdk::Address = sdk::Address::new_from_array([7; 32]);

#[test]
fn account_view_address_and_error_are_shared() {
    // Function pointer assignments check identity without fabricating SDK memory.
    let _: fn(sdk::AccountView) -> pinocchio::AccountView = core::convert::identity;
    let _: fn(sdk::Address) -> pinocchio::Address = core::convert::identity;
    let _: fn(sdk::Error) -> ::pinocchio::error::ProgramError = core::convert::identity;
    let error = blubber::Error::from(sdk::Error::InvalidAccountData);
    assert_eq!(sdk::Error::from(error), sdk::Error::InvalidAccountData);
}

#[blubber::schema(account)]
struct CheckedState {
    flag: bool,
}
impl sdk::Owner for CheckedState {
    const OWNER: sdk::Address = ID;
}

fn checked_traits<
    T: sdk::AnchorAccount
        + sdk::AccountInitialize
        + sdk::AccountRealloc
        + sdk::AccountClose
        + sdk::IdlAccountType,
>() {
}
const _: fn() = checked_traits::<blubber_anchor::Blubber<CheckedState, blubber_anchor::Checked>>;

// The SDK derive emits its consumer's optional idl-build cfg.
#[allow(dead_code, unexpected_cfgs)]
mod checked_context {
    use super::*;
    #[derive(sdk::Accounts)]
    struct CheckedContext {
        #[account(mut, constraint = state.flag)]
        state: blubber_anchor::Blubber<CheckedState, blubber_anchor::Checked>,
    }
}
