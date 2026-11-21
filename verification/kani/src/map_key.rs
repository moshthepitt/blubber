//! Full-domain pairs for each built-in key, with explicit array instantiations.

use blubber::{Address, Fixed, MapKey};
use core::fmt::Debug;

fn pair<T: MapKey + Debug>(left: T, right: T) {
    let left_bytes = left.encode_key();
    let right_bytes = right.encode_key();
    assert_eq!(left_bytes.as_ref().len(), T::SIZE as usize);
    assert_eq!(right_bytes.as_ref().len(), T::SIZE as usize);
    assert_eq!(
        left.cmp(&right),
        left_bytes.as_ref().cmp(right_bytes.as_ref())
    );
    assert_eq!(left == right, left_bytes.as_ref() == right_bytes.as_ref());
    assert_eq!(T::decode_key(left_bytes.as_ref()), Ok(left));
    assert_eq!(T::decode_key(right_bytes.as_ref()), Ok(right));
    assert_eq!(T::decode(left.encode().as_ref()), Ok(left));
    assert_eq!(T::decode(right.encode().as_ref()), Ok(right));
}

macro_rules! numeric {
    ($name:ident, $ty:ty) => {
        #[kani::proof]
        #[kani::unwind(18)]
        fn $name() {
            let left: $ty = kani::any();
            let right: $ty = kani::any();
            pair(left, right);
            assert_eq!(left.encode(), left.to_le_bytes());
        }
    };
}

numeric!(map_key_u8, u8);
numeric!(map_key_u16, u16);
numeric!(map_key_u32, u32);
numeric!(map_key_u64, u64);
numeric!(map_key_u128, u128);
numeric!(map_key_i8, i8);
numeric!(map_key_i16, i16);
numeric!(map_key_i32, i32);
numeric!(map_key_i64, i64);
numeric!(map_key_i128, i128);

#[kani::proof]
#[kani::unwind(3)]
fn map_key_bool() {
    pair(kani::any::<bool>(), kani::any::<bool>());
    let byte: u8 = kani::any();
    let decoded = bool::decode_key(&[byte]);
    assert_eq!(decoded.is_ok(), byte <= 1);
    if let Ok(value) = decoded {
        assert_eq!(value.encode_key(), [byte]);
    }
}

#[kani::proof]
#[kani::unwind(34)]
fn map_key_pubkey() {
    pair(
        Address::new_from_array(kani::any()),
        Address::new_from_array(kani::any()),
    );
}

#[kani::proof]
#[kani::unwind(34)]
fn map_key_address_projection() {
    let a_bytes: [u8; 32] = kani::any();
    let b_bytes: [u8; 32] = kani::any();
    let a = pinocchio::Address::new_from_array(a_bytes);
    let b = pinocchio::Address::new_from_array(b_bytes);
    assert_eq!(*a.as_array(), a_bytes);
    assert_eq!(*b.as_array(), b_bytes);
    assert_eq!(a.to_bytes(), a_bytes);
    assert_eq!(pinocchio::Address::new_from_array(*a.as_array()), a);
    assert_eq!(a.cmp(&b), a_bytes.cmp(&b_bytes));
    assert_eq!(a == b, a_bytes == b_bytes);
    assert_eq!(a.encode(), a_bytes);
    assert_eq!(a.encode_key(), a_bytes);
    let schema_key: Address = a;
    assert_eq!(schema_key.to_bytes(), a_bytes);
    assert_eq!(schema_key, a);
    kani::cover!(a_bytes == b_bytes, "equal SDK values");
    kani::cover!(
        a_bytes[..31] == b_bytes[..31] && a_bytes[31] < b_bytes[31],
        "SDK later-byte less"
    );
    kani::cover!(
        a_bytes[..31] == b_bytes[..31] && a_bytes[31] > b_bytes[31],
        "SDK later-byte greater"
    );
}

#[kani::proof]
#[kani::unwind(34)]
fn map_key_pubkey_projection() {
    let a_bytes: [u8; 32] = kani::any();
    let b_bytes: [u8; 32] = kani::any();
    let a = Address::new_from_array(a_bytes);
    let b = Address::new_from_array(b_bytes);
    assert_eq!(AsRef::<[u8]>::as_ref(&a), &a_bytes);
    assert_eq!(AsRef::<[u8]>::as_ref(&b), &b_bytes);
    assert_eq!(a.to_bytes(), a_bytes);
    assert_eq!(Address::new_from_array(a.to_bytes()), a);
    assert_eq!(a.cmp(&b), a_bytes.cmp(&b_bytes));
    assert_eq!(a == b, a_bytes == b_bytes);
    assert_eq!(a.encode(), a_bytes);
    assert_eq!(a.encode_key(), a_bytes);
    kani::cover!(a_bytes == b_bytes, "equal library keys");
    kani::cover!(
        a_bytes[..31] == b_bytes[..31] && a_bytes[31] < b_bytes[31],
        "library later-byte less"
    );
    kani::cover!(
        a_bytes[..31] == b_bytes[..31] && a_bytes[31] > b_bytes[31],
        "library later-byte greater"
    );
}

macro_rules! array {
    ($name:ident, $width:literal, $unwind:literal) => {
        #[kani::proof]
        #[kani::unwind($unwind)]
        fn $name() {
            let left: [u8; $width] = kani::any();
            let right: [u8; $width] = kani::any();
            pair(left, right);
            assert_eq!(left.encode_key(), left);
            assert_eq!(right.encode_key(), right);
        }
    };
}

array!(map_key_array_1, 1, 3);
array!(map_key_array_2, 2, 4);
array!(map_key_array_3, 3, 5);
array!(map_key_array_4, 4, 6);
array!(map_key_array_16, 16, 18);
array!(map_key_array_31, 31, 33);
array!(map_key_array_32, 32, 34);
array!(map_key_array_33, 33, 35);
array!(map_key_array_64, 64, 66);
