//! SDK-independent account metadata and detached constraint field copies.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{AccountSchema, Error, ErrorKind, Schema, SliceStore, Str, View};

type Counter = u64;
type Text = Str;

/// Native fixed values preserve their native type and padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
pub struct Native {
    /// Validated boolean.
    pub enabled: bool,
    /// Packed integer.
    pub count: u32,
}

/// Fixed schema markers copy into their borrowed input type at `'static`.
#[blubber::schema]
pub struct FixedChild<T> {
    /// A generic fixed child.
    pub value: T,
    /// Another copied child.
    pub code: u16,
}

/// Interleaved fields exercise semantic kind selection through aliases.
#[blubber::schema(account)]
pub struct State {
    /// Variable aliases do not become constraint fields.
    pub text: Text,
    /// Integer alias.
    pub counter: Counter,
    /// Native fixed type.
    pub native: Native,
    /// Fixed generic marker.
    pub nested: FixedChild<u8>,
    /// Raw key bytes.
    pub authority: blubber::Pubkey,
    /// Variable collection.
    pub values: blubber::List<u16>,
    /// Absent values are still variable kind.
    pub maybe: Option<u8>,
}

/// Explicit identity and an empty, opted-out prefix.
#[blubber::schema(account, discriminator = [1, 2, 3, 4, 5, 6, 7, 8], constraint_fields = false)]
pub struct OptOut {
    /// Wider than the copied prefix cap, still readable through ordinary views.
    pub wide: [u8; 2048],
    /// Variable content is unaffected by the opt-out.
    pub text: Str,
}

/// Native padding, rather than wire width alone, determines the cap.
#[blubber::schema(account)]
pub struct AtLimit {
    /// Forces padding in the native copied representation.
    pub flag: bool,
    /// Aligned copied integer.
    pub code: u64,
    /// Rounds the complete prefix to exactly 1,024 native bytes.
    pub wide: [u8; 1001],
    /// Variable data takes no prefix space.
    pub text: Str,
}

/// Accounts containing only variable data have an empty prefix.
#[blubber::schema(account)]
pub struct VariableOnly {
    /// Uncopied text.
    pub text: Str,
}

fn input() -> StateInit<'static> {
    StateInit {
        text: "borrowed",
        counter: 19,
        native: Native {
            enabled: true,
            count: 73,
        },
        nested: FixedChildInit { value: 7, code: 9 },
        authority: blubber::Pubkey::from_bytes([3; 32]),
        values: &[1, 4],
        maybe: None,
    }
}

fn encoded() -> Vec<u8> {
    let mut bytes = vec![0; State::encoded_len_of(&input()).unwrap() as usize + 11];
    let mut store = SliceStore::new(&mut bytes);
    State::initialize(&mut store, input())
        .unwrap()
        .commit()
        .unwrap();
    bytes
}

#[test]
fn default_prefix_copies_aliases_native_values_and_nested_fixed_markers() {
    let bytes = encoded();
    let view = View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap();
    let prefix: StatePrefix = State::read_prefix(view).unwrap();
    assert_eq!(prefix.counter, 19);
    assert_eq!(prefix.native, input().native);
    assert_eq!(prefix.nested.value, 7);
    assert_eq!(prefix.nested.code, 9);
    assert_eq!(prefix.authority, input().authority);
}

#[test]
fn prefix_is_copy_and_outlives_the_encoded_storage() {
    let prefix = {
        let bytes = encoded();
        State::read_prefix(View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap()).unwrap()
    };
    let copy = prefix;
    assert_eq!(prefix.native, copy.native);
    assert_eq!(prefix.nested.value, 7);
}

#[test]
fn prefix_can_be_refreshed_after_a_checked_local_edit() {
    let mut bytes = encoded();
    let old =
        State::read_prefix(View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap()).unwrap();
    let mut store = SliceStore::new(&mut bytes);
    let mut root = State::load_mut(&mut store).unwrap();
    root.counter_mut().set(42).unwrap();
    root.native_mut()
        .set(Native {
            enabled: false,
            count: 8,
        })
        .unwrap();
    root.commit().unwrap();
    let fresh =
        State::read_prefix(View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap()).unwrap();
    assert_eq!(old.counter, 19);
    assert_eq!(fresh.counter, 42);
    assert_eq!(
        fresh.native,
        Native {
            enabled: false,
            count: 8
        }
    );
}

#[test]
fn metadata_retains_hash_identity_and_explicit_override() {
    // D21 canonical descriptor/hash vector, computed independently.
    assert_eq!(State::DISCRIMINATOR, [135, 44, 222, 198, 28, 253, 95, 206]);
    assert_eq!(
        <State as AccountSchema>::DISCRIMINATOR,
        State::DISCRIMINATOR
    );
    assert_eq!(State::DISCRIMINATOR_OVERRIDE, None);
    assert_eq!(
        OptOut::DISCRIMINATOR_OVERRIDE,
        Some([1, 2, 3, 4, 5, 6, 7, 8])
    );
    assert_eq!(
        <OptOut as AccountSchema>::DISCRIMINATOR,
        OptOut::DISCRIMINATOR
    );
    assert_eq!(
        <State as AccountSchema>::MIN_ACCOUNT_LEN,
        State::MIN_ACCOUNT_LEN
    );
}

#[test]
fn space_is_const_checked_envelope_plus_reservation_without_input() {
    const EMPTY: usize = match State::space(0) {
        Ok(n) => n,
        Err(_) => panic!("empty space"),
    };
    assert_eq!(EMPTY, 12 + State::MIN_SIZE as usize);
    let empty = StateInit {
        text: "",
        values: &[],
        ..input()
    };
    assert_eq!(EMPTY, State::encoded_len_of(&empty).unwrap() as usize);
    assert_eq!(State::space(237).unwrap(), EMPTY + 237);
    assert_eq!(<State as AccountSchema>::space(237), State::space(237));
    let room = u32::MAX as usize - EMPTY;
    assert_eq!(State::space(room), Ok(u32::MAX as usize));
    let overflow = Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    assert_eq!(State::space(room + 1), overflow);
    assert_eq!(State::space(usize::MAX), overflow);
    assert_eq!(VariableOnly::space(0), Ok(12));
    assert_eq!(OptOut::space(0), Ok(2060));
}

#[test]
fn native_padding_counts_toward_the_prefix_cap() {
    assert_eq!(core::mem::size_of::<AtLimitPrefix>(), 1024);
    assert_eq!(AtLimit::MIN_SIZE, 1010);
    assert_eq!(core::mem::size_of::<OptOutPrefix>(), 0);
    assert_eq!(core::mem::size_of::<VariableOnlyPrefix>(), 0);
}

#[test]
fn opt_out_changes_only_constraint_metadata() {
    let init = OptOutInit {
        wide: [7; 2048],
        text: "visible",
    };
    let mut bytes = vec![0; OptOut::encoded_len_of(&init).unwrap() as usize];
    let mut store = SliceStore::new(&mut bytes);
    OptOut::initialize(&mut store, init)
        .unwrap()
        .commit()
        .unwrap();
    let view = View::load::<OptOut>(&bytes, OptOut::DISCRIMINATOR).unwrap();
    assert!(
        matches!(<OptOut as Schema>::__read_fixed(view), Err(e) if e == ErrorKind::InvalidSchema.into())
    );
    let (): OptOutPrefix = OptOut::read_prefix(view).unwrap();
    let read = OptOut::load(&bytes).unwrap();
    assert_eq!(read.wide(), [7; 2048]);
    assert_eq!(read.text(), "visible");
}

#[test]
fn copying_fixed_domains_retains_account_relative_faults() {
    let mut bytes = encoded();
    // Fixed prefix order: counter[8], native[5], nested[3], authority[32].
    bytes[12 + 8] = 2;
    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let view = View::new(&bytes[12..used], State::LAYOUT, 12).unwrap();
    assert!(
        matches!(State::read_prefix(view), Err(e) if e == Error::new(ErrorKind::InvalidBool, 20))
    );
}

#[test]
fn prefix_uses_its_own_schema_and_validated_ingress_checks_variable_bytes() {
    let bytes = encoded();
    let expected = View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap();
    let foreign = View::new(expected.bytes(), blubber::List::<u8>::LAYOUT, 300).unwrap();
    assert_eq!(State::read_prefix(foreign).unwrap().counter, 19);
    assert_eq!(
        foreign.__account_fixed_field::<State>(0).unwrap_err(),
        Error::from(ErrorKind::InvalidSchema)
    );
    assert_eq!(
        foreign.__account_fixed_field::<State>(99).unwrap_err(),
        Error::from(ErrorKind::IndexOutOfBounds)
    );
    let short = View::new(&[], blubber::List::<u8>::LAYOUT, 300).unwrap();
    assert_eq!(
        short.__account_fixed_field::<State>(1).unwrap_err(),
        Error::new(ErrorKind::Truncated, 300)
    );
    let mut corrupt = bytes;
    // Replace the first byte of text, after fixed prefix and two directory ends.
    corrupt[12 + 48 + 8] = 0xff;
    assert!(View::load::<State>(&corrupt, State::DISCRIMINATOR).is_err());
}

#[test]
fn sdk_address_is_a_native_copied_constraint_field() {
    #[blubber::schema(account)]
    struct AddressState {
        authority: blubber::pinocchio::Address,
        note: Str,
    }
    let authority = blubber::pinocchio::Address::new_from_array([5; 32]);
    let init = AddressStateInit {
        authority,
        note: "",
    };
    let mut bytes = vec![0; AddressState::encoded_len_of(&init).unwrap() as usize];
    let mut store = SliceStore::new(&mut bytes);
    AddressState::initialize(&mut store, init)
        .unwrap()
        .commit()
        .unwrap();
    let prefix = AddressState::read_prefix(
        View::load::<AddressState>(&bytes, AddressState::DISCRIMINATOR).unwrap(),
    )
    .unwrap();
    assert_eq!(prefix.authority, authority);
}
