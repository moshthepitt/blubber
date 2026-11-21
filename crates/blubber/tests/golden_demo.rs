//! Byte-identical account construction and the complete worked reader fixture.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/demo.rs"]
mod demo;

use blubber::{Address, Schema, SliceStore};
use demo::{Limits, MyAccount};

#[test]
fn initialize_matches_every_byte_of_the_worked_demo() {
    let expected = demo::golden();
    assert_eq!(expected.len(), 171);
    let mut bytes = [0xa5; 171];
    bytes[..8].fill(0);
    let mut store = SliceStore::new(&mut bytes);
    let account = MyAccount::initialize(&mut store, demo::input()).unwrap();
    assert_eq!(account.used_len(), 167);
    assert_eq!(account.storage_len(), 171);
    account.validate().unwrap();
    let receipt = account.commit().unwrap();
    assert_eq!((receipt.used_len, receipt.storage_len), (167, 171));
    assert_eq!(bytes.as_slice(), expected);
    assert_eq!(MyAccount::measure(&demo::input()), Ok(155));
    assert_eq!(MyAccount::encoded_len_of(&demo::input()), Ok(167));
    assert_eq!(MyAccount::MIN_ACCOUNT_LEN, 92);
}

#[test]
fn complete_local_readers_decode_the_independent_golden_bytes() {
    let bytes = demo::golden();
    let account = MyAccount::load(&bytes).unwrap();
    assert_eq!(account.used_len(), 167);
    assert_eq!(account.storage_len(), 171);
    assert_eq!(account.authority(), Address::new_from_array([0x11; 32]));
    assert_eq!(account.counter(), 1);
    let record = account.records().get(0).unwrap();
    assert_eq!(record.name(), "A");
    assert_eq!(record.tags().iter().collect::<Vec<_>>(), [7, 9]);
    assert_eq!(
        account.balances().get(&Address::new_from_array([0x22; 32])),
        Some(5)
    );
    assert_eq!(
        account.labels().iter().collect::<Vec<_>>(),
        [(1, "x"), (256, "")]
    );
    assert_eq!(account.note(), "ok");
    assert_eq!(
        account.attachment().iter().collect::<Vec<_>>(),
        [0xaa, 0xbb]
    );
    assert_eq!(account.memo(), Some("m"));
    assert_eq!(
        account.limits(),
        Limits {
            enabled: true,
            limit: 2
        }
    );
}
