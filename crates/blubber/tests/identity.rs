//! Independent canonical descriptor/hash vectors and codec identity contracts.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]
#[path = "support/identity.rs"]
mod fixtures;

use blubber::{
    identity::{account_identity, CodecIdentity, WireDescriptor, WireKey},
    AccountSchema,
};
use sha2::{Digest, Sha256};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn independent_canonical_descriptor_and_hash_vectors() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/identity-vectors.json")).unwrap();
    let rows = vectors["vectors"].as_array().unwrap();
    assert_eq!(rows.len(), fixtures::COUNT);
    for (i, row) in rows.iter().enumerate() {
        let (name, discriminator, namespace, epoch, descriptor) = fixtures::vector(i).unwrap();
        assert_eq!(name, row["name"].as_str().unwrap());
        assert_eq!(namespace, row["namespace"].as_str().unwrap());
        assert_eq!(epoch as u64, row["epoch"].as_u64().unwrap());
        assert_eq!(
            descriptor,
            unhex(row["descriptor_hex"].as_str().unwrap()),
            "{name}"
        );
        assert_eq!(
            discriminator.as_slice(),
            unhex(row["identity_hex"].as_str().unwrap()),
            "{name}"
        );
        if name != "override" {
            let mut hash = Sha256::new();
            hash.update(b"blubber\0schema\0");
            hash.update(1u32.to_le_bytes());
            hash.update((namespace.len() as u32).to_le_bytes());
            hash.update(namespace.as_bytes());
            hash.update(epoch.to_le_bytes());
            hash.update((descriptor.len() as u32).to_le_bytes());
            hash.update(descriptor);
            assert_eq!(discriminator, hash.finalize()[..8]);
        }
    }
    assert!(fixtures::vector(fixtures::COUNT).is_none());
    // Exercise every SHA-256 padding boundary across several input segments.
    // These native reference checks are separate from alias-model fixtures.
    for length in 0..=1024 {
        let namespace = "λ".repeat(length);
        for descriptor in [
            &b""[..],
            &b"descriptor"[..],
            &[0xa5; 55][..],
            &[0x99; 64][..],
            &[0; 127][..],
        ] {
            let mut hash = Sha256::new();
            hash.update(b"blubber\0schema\0");
            hash.update(1u32.to_le_bytes());
            hash.update((namespace.len() as u32).to_le_bytes());
            hash.update(namespace.as_bytes());
            hash.update(u32::MAX.to_le_bytes());
            hash.update((descriptor.len() as u32).to_le_bytes());
            hash.update(descriptor);
            assert_eq!(
                account_identity(&namespace, u32::MAX, descriptor),
                hash.finalize()[..8]
            );
        }
    }
}

#[test]
fn aliases_padding_names_and_resolved_generics() {
    #[blubber::schema(account, namespace = "qa:reorder")]
    struct Before {
        debit: u64,
        credit: u64,
    }
    #[blubber::schema(account, namespace = "qa:reorder")]
    struct Reordered {
        credit: u64,
        debit: u64,
    }
    #[blubber::schema(account, namespace = "qa:reorder", semantic_epoch = 1)]
    struct Converted {
        credit: u64,
        debit: u64,
    }
    assert_eq!(
        Before::WIRE_DESCRIPTOR_BYTES,
        Reordered::WIRE_DESCRIPTOR_BYTES
    );
    assert_eq!(Before::DISCRIMINATOR, Reordered::DISCRIMINATOR);
    assert_ne!(Before::DISCRIMINATOR, Converted::DISCRIMINATOR);
    let mut bytes = [0; 28];
    Before::initialize(
        &mut blubber::SliceStore::new(&mut bytes),
        BeforeInit {
            debit: 11,
            credit: 22,
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    let read = Reordered::load(&bytes).unwrap();
    assert_eq!((read.credit(), read.debit()), (11, 22));
    let unsigned = fixtures::vector(1).unwrap();
    for i in [13, 17] {
        let v = fixtures::vector(i).unwrap();
        assert_eq!(unsigned.1, v.1);
        assert_eq!(unsigned.4, v.4);
    }
    assert_eq!(
        fixtures::vector(18).unwrap().1,
        fixtures::vector(19).unwrap().1
    );
    assert_eq!(
        fixtures::vector(21).unwrap().1,
        fixtures::vector(22).unwrap().1
    );
    assert_eq!(
        fixtures::vector(23).unwrap().1,
        fixtures::vector(24).unwrap().1
    );
    assert!(core::mem::size_of::<fixtures::Padded>() > 9);
    for i in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 20] {
        assert_ne!(unsigned.1, fixtures::vector(i).unwrap().1);
    }
    assert_ne!(
        fixtures::vector(6).unwrap().1,
        fixtures::vector(7).unwrap().1
    );
    assert_ne!(
        fixtures::vector(8).unwrap().1,
        fixtures::vector(9).unwrap().1
    );
    assert_ne!(
        fixtures::vector(8).unwrap().1,
        fixtures::vector(11).unwrap().1
    );
    assert_eq!(
        <fixtures::DefaultState as AccountSchema>::DISCRIMINATOR_OVERRIDE,
        None
    );
    assert_eq!(
        <fixtures::Override as AccountSchema>::DISCRIMINATOR_OVERRIDE,
        Some([1, 2, 3, 4, 5, 6, 7, 255])
    );
}

#[test]
fn custom_codec_and_key_epochs_enter_the_preimage() {
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Key(u16);
    impl blubber::Fixed for Key {
        const SIZE: u32 = 2;
        type Encoded = [u8; 2];
        fn decode(bytes: &[u8]) -> blubber::Result<Self> {
            <u16 as blubber::Fixed>::decode(bytes).map(Self)
        }
        fn encode(self) -> Self::Encoded {
            <u16 as blubber::Fixed>::encode(self.0)
        }
    }
    impl blubber::MapKey for Key {
        const KEY_LAYOUT: blubber_core::KeyLayout = <u16 as blubber::MapKey>::KEY_LAYOUT;
        const KEY_DESCRIPTOR: Option<WireKey> = Some(WireKey {
            layout: Self::KEY_LAYOUT,
            codec: Some(CodecIdentity {
                namespace: "test:key",
                epoch: 0,
            }),
        });
        type KeyBytes = [u8; 2];
        fn encode_key(self) -> Self::KeyBytes {
            <u16 as blubber::MapKey>::encode_key(self.0)
        }
        fn decode_key(bytes: &[u8]) -> blubber::Result<Self> {
            <u16 as blubber::MapKey>::decode_key(bytes).map(Self)
        }
    }
    #[blubber::schema(account, namespace = "custom")]
    struct CustomMap {
        values: blubber::Map<Key, blubber::Str>,
    }
    assert!(CustomMap::WIRE_DESCRIPTOR_BYTES
        .windows(8)
        .any(|part| part == b"test:key"));
    let mut bytes = [0; 16];
    let mut store = blubber::SliceStore::new(&mut bytes);
    CustomMap::initialize(&mut store, CustomMapInit { values: &[] })
        .unwrap()
        .commit()
        .unwrap();
    CustomMap::load(&bytes).unwrap().validate().unwrap();

    const WIRE: WireDescriptor = WireDescriptor::Custom {
        codec: CodecIdentity {
            namespace: "test:units",
            epoch: 0,
        },
        wire: &WireDescriptor::Unsigned(8),
    };
    const NEXT: WireDescriptor = WireDescriptor::Custom {
        codec: CodecIdentity {
            namespace: "test:units",
            epoch: 1,
        },
        wire: &WireDescriptor::Unsigned(8),
    };
    const BYTES: [u8; WIRE.encoded_len()] = WIRE.encode();
    const NEXT_BYTES: [u8; NEXT.encoded_len()] = NEXT.encode();
    assert_ne!(
        account_identity("test", 0, &BYTES),
        account_identity("test", 0, &NEXT_BYTES)
    );
    let map = |epoch| WireDescriptor::Map {
        key: Some(WireKey {
            layout: blubber_core::KeyLayout {
                width: 2,
                domain: blubber_core::KeyDomain::Unsigned,
            },
            codec: Some(CodecIdentity {
                namespace: "test:key",
                epoch,
            }),
        }),
        value: &WireDescriptor::Str,
    };
    assert_ne!(map(0).encode::<37>(), map(1).encode::<37>());
}

#[test]
fn unspecified_and_invalid_metadata_fail_closed() {
    assert!(std::panic::catch_unwind(|| WireDescriptor::Unspecified.encoded_len()).is_err());
    assert!(std::panic::catch_unwind(|| WireDescriptor::Map {
        key: None,
        value: &WireDescriptor::Str
    }
    .encoded_len())
    .is_err());
    assert!(std::panic::catch_unwind(|| WireDescriptor::Raw(0).encoded_len()).is_err());
    assert!(std::panic::catch_unwind(|| WireDescriptor::Unsigned(3).encoded_len()).is_err());
    assert!(std::panic::catch_unwind(|| WireDescriptor::Bool.encode::<1>()).is_err());
}

#[test]
fn explicit_discriminator_overrides_automatic_identity() {
    #[blubber::schema(account, namespace = "test:override")]
    struct Automatic {
        value: u16,
    }
    #[blubber::schema(account, namespace = "test:override", discriminator = [1,2,3,4,5,6,7,8])]
    struct Explicit {
        value: u16,
    }
    assert_eq!(
        Automatic::WIRE_DESCRIPTOR_BYTES,
        Explicit::WIRE_DESCRIPTOR_BYTES
    );
    assert_eq!(Explicit::DISCRIMINATOR, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_ne!(Automatic::DISCRIMINATOR, Explicit::DISCRIMINATOR);
    let mut bytes = [0; 14];
    Explicit::initialize(
        &mut blubber::SliceStore::new(&mut bytes),
        ExplicitInit { value: 513 },
    )
    .unwrap()
    .commit()
    .unwrap();
    assert_eq!(&bytes[..8], &Explicit::DISCRIMINATOR);
    assert_eq!(Explicit::load(&bytes).unwrap().value(), 513);
    assert!(matches!(
        Automatic::load(&bytes),
        Err(blubber::Error::Fault {
            kind: blubber::ErrorKind::Discriminator,
            offset: 0
        })
    ));
    let payload = bytes[8..].to_vec();
    bytes[..8].copy_from_slice(&Automatic::DISCRIMINATOR);
    assert_eq!(&bytes[8..], payload);
    assert_eq!(Automatic::load(&bytes).unwrap().value(), 513);
    assert!(matches!(
        Explicit::load(&bytes),
        Err(blubber::Error::Fault {
            kind: blubber::ErrorKind::Discriminator,
            offset: 0
        })
    ));
}
