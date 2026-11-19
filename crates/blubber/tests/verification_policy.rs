//! Full-root verification, local validation and explicit ingress policies.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{Error, ErrorKind, List, Map, ReadStorage, Resize, Schema, SliceStore, Str, View};

#[blubber::schema(account)]
pub struct Child {
    pub flag: bool,
}

#[blubber::schema(account, namespace = "tests:verification-policy")]
pub struct State {
    pub child: Child,
    pub text: Str,
    pub entries: Map<u8, bool>,
    pub flags: List<bool>,
}

fn bytes() -> [u8; 96] {
    let mut bytes = [0; 96];
    State::initialize(
        &mut SliceStore::new(&mut bytes),
        StateInit {
            child: ChildInit { flag: true },
            text: "ok",
            entries: &[(1, true), (2, false)],
            flags: &[true],
        },
    )
    .unwrap();
    bytes
}

#[test]
fn root_verify_excludes_spare_and_keeps_subtree_validation_local() {
    let mut bytes = bytes();
    bytes[95] = 0xff;
    let loaded = State::load(&bytes).unwrap();
    loaded.verify().unwrap();
    assert_eq!(
        loaded.child().verify(),
        Err(ErrorKind::InvalidBoundary.into())
    );
    let view = View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap();
    let bare = State::read(view).unwrap();
    bare.validate().unwrap();
    assert_eq!(bare.verify(), Err(ErrorKind::InvalidBoundary.into()));
    let mut store = SliceStore::new(&mut bytes);
    let mut root = State::load_checked_mut(&mut store).unwrap();
    root.child_mut().validate().unwrap();
    assert_eq!(
        root.child_mut().verify(),
        Err(ErrorKind::InvalidBoundary.into())
    );
    root.verify().unwrap();
    root.text_mut().set("changed").unwrap();
    root.verify().unwrap();
    root.commit().unwrap();
}

#[test]
fn trusted_feature_policy_and_full_scan_have_the_same_fault() {
    let mut bytes = bytes();
    // The root packs one bool then two ends. Its final List<bool> is implicit.
    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    bytes[used - 1] = 9;
    let expected = Error::new(ErrorKind::InvalidBool, (used - 1) as u32);
    assert!(matches!(State::load(&bytes), Err(error) if error == expected));
    let trusted = State::load_trusted(&bytes);
    if cfg!(feature = "verify-on-load") {
        assert!(matches!(trusted, Err(error) if error == expected));
    } else {
        let root = trusted.unwrap();
        root.child().validate().unwrap();
        assert_eq!(root.verify(), Err(expected));
    }
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    assert!(matches!(State::load_mut(&mut store), Err(error) if error == expected));
    assert!(matches!(State::load_checked_mut(&mut store), Err(error) if error == expected));
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    let trusted = State::load_trusted_mut(&mut store);
    if cfg!(feature = "verify-on-load") {
        assert!(matches!(trusted, Err(error) if error == expected));
    } else {
        let mut root = trusted.unwrap();
        root.child_mut().validate().unwrap();
        assert_eq!(root.verify(), Err(expected));
    }
}

#[test]
fn complete_scan_rejects_distant_duplicate_keys() {
    let mut bytes = bytes();
    let view = View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap();
    let map = view.field(2).unwrap();
    let second_key = map.base() as usize + 6;
    bytes[second_key] = 1;
    assert!(matches!(
        State::load(&bytes),
        Err(Error::Fault {
            kind: ErrorKind::DuplicateKey,
            ..
        })
    ));
    #[cfg(not(feature = "verify-on-load"))]
    {
        let root = State::load_trusted(&bytes).unwrap();
        root.child().validate().unwrap();
        assert!(root.verify().is_err());
    }
}

#[test]
fn poison_recovery_is_explicit_and_closed_precedes_poison_and_envelope() {
    let mut bytes = bytes();
    let mut store = SliceStore::new(&mut bytes);
    blubber::raw::backend(&mut store).set_poisoned(true);
    assert!(
        matches!(State::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
    );
    State::load_checked_mut(&mut store)
        .unwrap()
        .verify()
        .unwrap();
    assert!(!store.is_poisoned());
    State::load_mut(&mut store).unwrap().commit().unwrap();
    assert!(
        matches!(State::load_checked_mut(&mut store), Err(error) if error == ErrorKind::Closed.into())
    );
}

#[blubber::schema(account)]
pub struct LazyMap {
    pub prefix: u16,
    pub values: Map<bool, bool>,
    pub suffix: Str,
}

fn lazy_map_bytes() -> [u8; 64] {
    let mut bytes = [0; 64];
    LazyMap::initialize(
        &mut SliceStore::new(&mut bytes),
        LazyMapInit {
            prefix: 0x1234,
            values: &[(false, false), (true, true)],
            suffix: "tail",
        },
    )
    .unwrap();
    bytes[40..].fill(0xa5);
    bytes
}

#[test]
fn lazy_fixed_map_values_frame_corrupted_keys_count_suffix_and_spare() {
    for keys in [[0, 0], [1, 0], [0, 2], [0, 1]] {
        for operation in 0..4 {
            let mut bytes = lazy_map_bytes();
            // Fixed prefix + one end precede the eight-byte fixed map.
            bytes[22] = keys[0];
            bytes[24] = keys[1];
            let before = bytes;
            let mut store = SliceStore::new(&mut bytes);
            let loaded = LazyMap::load_trusted_mut(&mut store);
            if cfg!(feature = "verify-on-load") && keys != [0, 1] {
                assert!(loaded.is_err());
                assert_eq!(store.bytes(), before);
                continue;
            }
            let mut root = loaded.unwrap();
            let mut expected = before;
            match operation {
                0 => {
                    root.values_mut().fill_values(true).unwrap();
                    expected[23] = 1;
                    expected[25] = 1;
                }
                1 => {
                    root.values_mut().rewrite_values(&[true, false]).unwrap();
                    expected[23] = 1;
                    expected[25] = 0;
                }
                2 => {
                    let mut cursor = root.values_mut().cursor_mut().unwrap();
                    for i in 0..2 {
                        if keys[i] > 1 {
                            assert!(
                                matches!(cursor.next(), Err(e) if e == Error::new(ErrorKind::InvalidBool,24))
                            );
                            assert!(cursor.next().unwrap().is_none());
                            break;
                        }
                        let (key, mut slot) = cursor.next().unwrap().unwrap();
                        assert_eq!(key, keys[i] != 0);
                        slot.set(true).unwrap();
                        expected[23 + 2 * i] = 1;
                    }
                }
                _ => {
                    let mut visits = 0;
                    let result = root.values_mut().try_for_each_mut(|key, slot| {
                        assert_eq!(key, keys[visits] != 0);
                        slot.set(false)?;
                        expected[23 + 2 * visits] = 0;
                        visits += 1;
                        Ok(())
                    });
                    assert_eq!(result.is_err(), keys[1] > 1);
                }
            }
            assert_eq!(root.verify().is_ok(), keys == [0, 1]);
            let poisoned = operation >= 2 && keys[1] > 1;
            if poisoned {
                assert_eq!(root.commit(), Err(ErrorKind::Poisoned.into()));
            } else {
                root.commit().unwrap();
            }
            assert_eq!(store.is_poisoned(), poisoned);
            assert_eq!(bytes, expected);
        }
    }
}

#[test]
fn lazy_fixed_map_geometry_rejects_before_writes_and_checked_ingress_scans() {
    for count in [0, 1, 3, u32::MAX] {
        let mut bytes = lazy_map_bytes();
        bytes[18..22].copy_from_slice(&count.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        if let Ok(mut root) = LazyMap::load_trusted_mut(&mut store) {
            assert!(root.values_mut().fill_values(true).is_err());
            assert!(root.values_mut().rewrite_values(&[true, false]).is_err());
            assert!(root.values_mut().cursor_mut().is_err());
            assert!(root
                .values_mut()
                .try_for_each_mut(|_, _| panic!("bad geometry reached callback"))
                .is_err());
        }
        assert_eq!(store.bytes(), before);
        assert_eq!(store.is_poisoned(), !cfg!(feature = "verify-on-load"));
    }
    for keys in [[0, 0], [1, 0], [0, 2]] {
        let mut bytes = lazy_map_bytes();
        bytes[22] = keys[0];
        bytes[24] = keys[1];
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        assert!(LazyMap::load_checked_mut(&mut store).is_err());
        assert!(LazyMap::load_mut(&mut store).is_err());
        assert_eq!(store.bytes(), before);
    }
}

#[test]
fn lazy_fixed_map_values_repair_only_visited_domains_and_keep_keyed_preflight() {
    let mut bytes = lazy_map_bytes();
    bytes[23] = 7;
    bytes[25] = 9;
    let mut store = SliceStore::new(&mut bytes);
    let loaded = LazyMap::load_trusted_mut(&mut store);
    if cfg!(feature = "verify-on-load") {
        assert!(loaded.is_err());
        return;
    }
    let mut root = loaded.unwrap();
    {
        let mut cursor = root.values_mut().cursor_mut().unwrap();
        let (_, mut value) = cursor.next().unwrap().unwrap();
        assert_eq!(value.try_get(), Err(Error::new(ErrorKind::InvalidBool, 23)));
        value.set(true).unwrap();
    }
    assert_eq!(root.verify(), Err(Error::new(ErrorKind::InvalidBool, 25)));
    assert!(root
        .values_mut()
        .rewrite_values_from(&[(true, false), (false, true)])
        .is_err());
    root.values_mut().fill_values(false).unwrap();
    root.verify().unwrap();
    root.values_mut()
        .rewrite_values_from(&[(false, true), (true, false)])
        .unwrap();
    root.verify().unwrap();
}

blubber::__root_schema!(__ScanFreeMap, Map<u8,u8>);

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn lazy_fixed_map_no_global_scan_work() {
    for count in [0, 1, 2, 4, if cfg!(miri) { 4 } else { 1024 }] {
        let used = 16 + 2 * count;
        let mut bytes = vec![0xa5; used + 11];
        bytes[..8].copy_from_slice(b"walkscan");
        bytes[8..12].copy_from_slice(&(used as u32).to_le_bytes());
        bytes[12..16].copy_from_slice(&(count as u32).to_le_bytes());
        // Repeated/descending u8 keys are allowed by lazy geometry admission.
        for i in 0..count {
            bytes[16 + 2 * i] = (count - i) as u8;
        }
        let mut expected = bytes.clone();
        let values = vec![9; count];
        {
            let mut store = SliceStore::new(&mut bytes);
            let edit = blubber::raw::Edit::load_trusted::<__ScanFreeMap>(&mut store, *b"walkscan")
                .unwrap();
            let mut map = blubber::raw::project::<Map<u8, u8>>(edit);
            map.fill_values(7).unwrap();
            map.rewrite_values(&values).unwrap();
            let pairs: Vec<_> = (0..count).map(|i| ((count - i) as u8, 11)).collect();
            map.rewrite_values_from(&pairs).unwrap();
            {
                let mut cursor = map.reborrow().cursor_mut().unwrap();
                for i in 0..count {
                    let (key, mut value) = cursor.next().unwrap().unwrap();
                    assert_eq!(key, (count - i) as u8);
                    value.set(13).unwrap();
                }
                assert!(cursor.next().unwrap().is_none());
            }
            let mut visits = 0;
            map.try_for_each_mut(|key, value| {
                assert_eq!(key, (count - visits) as u8);
                visits += 1;
                value.set(17)
            })
            .unwrap();
            assert_eq!(visits, count);
            assert!(!store.is_poisoned());
        }
        for i in 0..count {
            expected[17 + 2 * i] = 17;
        }
        assert_eq!(bytes, expected);
    }
}
