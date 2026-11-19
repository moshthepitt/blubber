//! Executable staging, verification and identity-last migration recipe.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]
use blubber::{raw::backend, Schema, SliceStore};

#[blubber::schema(account, namespace = "example:ledger", semantic_epoch = 0)]
struct OldLedger {
    debit: u64,
    credit: u64,
}
#[blubber::schema(account, namespace = "example:ledger", semantic_epoch = 1)]
struct NewLedger {
    credit: u64,
    debit: u64,
}

#[test]
fn convert_verify_then_publish_identity_last() {
    // On-chain callers first authenticate the owner and application authority.
    let mut destination = [0; 28];
    OldLedger::initialize(
        &mut SliceStore::new(&mut destination),
        OldLedgerInit {
            debit: 11,
            credit: 22,
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    let old = OldLedger::load(&destination).unwrap(); // Complete old-schema scan.
    let input = NewLedgerInit {
        credit: old.credit(),
        debit: old.debit(),
    };

    // Stage separately. An interrupted stage cannot publish a new identity.
    let mut staging = [0; 28];
    NewLedger::initialize(&mut SliceStore::new(&mut staging), input)
        .unwrap()
        .commit()
        .unwrap();
    NewLedger::load(&staging).unwrap().verify().unwrap();
    assert_eq!(&destination[..8], &OldLedger::DISCRIMINATOR);
    assert!(NewLedger::load(&destination).is_err());

    // Deliberate raw migration: hold exclusion through verification/publication.
    let mut store = SliceStore::new(&mut destination);
    let bytes = backend(&mut store).bytes_mut();
    bytes[8..].copy_from_slice(&staging[8..]);
    let payload = blubber::View::new(&bytes[12..], NewLedger::LAYOUT, 12).unwrap();
    payload.validate().unwrap();
    assert_eq!(&bytes[..8], &OldLedger::DISCRIMINATOR);
    bytes[..8].copy_from_slice(&NewLedger::DISCRIMINATOR); // Final publication.

    let reopened = NewLedger::load_checked_mut(&mut store).unwrap();
    assert_eq!((reopened.credit(), reopened.debit()), (22, 11));
    reopened.verify().unwrap();
}
