//! Account envelope and complete worked-demo golden fixtures.

#![forbid(unsafe_code)]

mod support;

use blubber_model::{Error, Value};
use support::*;

#[test]
fn entire_demo_matches_shared_golden_without_production_codecs() {
    let schema = demo_schema();
    let value = demo_value();
    let golden = demo_hex();
    assert_eq!(golden.len(), 171);
    assert_eq!(schema.encode(&value).unwrap().len(), 155);
    assert_eq!(
        schema.encode_account(DISCRIMINATOR, &value, 171),
        Ok(golden.clone())
    );
    let account = schema.decode_account(DISCRIMINATOR, &golden).unwrap();
    assert_eq!(account.value, value);
    assert_eq!((account.used_len, account.storage_len), (167, 171));
    assert_eq!(account.discriminator, DISCRIMINATOR);
    assert_eq!(&golden[52..55], hex("01 02 00"));
    assert_eq!(
        &golden[55..75],
        hex("15 00 00 00 41 00 00 00 52 00 00 00 54 00 00 00 5a 00 00 00")
    );
    assert_eq!(schema.decode(&golden[12..167]), Ok(account.value));
}

#[test]
fn minimum_demo_is_92_account_bytes_and_80_root_bytes() {
    let schema = demo_schema();
    let mut value = demo_value();
    let Value::Struct(fields) = &mut value else {
        panic!()
    };
    fields[2] = Value::List(vec![]);
    fields[3] = Value::Map(Default::default());
    fields[4] = Value::Map(Default::default());
    fields[5] = string("");
    fields[6] = Value::List(vec![]);
    fields[7] = Value::Option(None);
    let bytes = schema.encode_account(DISCRIMINATOR, &value, 92).unwrap();
    assert_eq!(schema.minimum_size(8), Ok(80));
    assert_eq!(bytes.len(), 92);
    assert_eq!(&bytes[8..12], [92, 0, 0, 0]);
    assert_eq!(
        &bytes[55..75],
        hex("04 00 00 00 08 00 00 00 0c 00 00 00 0c 00 00 00 10 00 00 00")
    );
    assert_eq!(&bytes[75..92], [0; 17]);
    assert_eq!(
        schema.decode_account(DISCRIMINATOR, &bytes).unwrap().value,
        value
    );
}

#[test]
fn spare_capacity_is_excluded_and_reencoding_zeroes_it() {
    let schema = demo_schema();
    let mut bytes = demo_hex();
    bytes[167..].fill(0xff);
    let account = schema.decode_account(DISCRIMINATOR, &bytes).unwrap();
    assert_eq!(account.value, demo_value());
    assert_eq!(
        schema.encode_account(DISCRIMINATOR, &account.value, account.storage_len),
        Ok(demo_hex())
    );
    assert_eq!(schema.decode(&bytes[12..]), Err(Error::InvalidUtf8));
}

#[test]
fn account_header_discriminator_and_capacity_rejections() {
    let schema = demo_schema();
    let value = demo_value();
    assert_eq!(
        schema.encode_account(DISCRIMINATOR, &value, 166),
        Err(Error::Capacity)
    );
    assert_eq!(
        schema.encode_account([0; 8], &value, 171),
        Err(Error::Discriminator)
    );
    assert_eq!(
        schema.decode_account([0; 8], &demo_hex()),
        Err(Error::Discriminator)
    );
    assert_eq!(
        schema.decode_account([1; 8], &demo_hex()),
        Err(Error::Discriminator)
    );
    for length in 0..12 {
        assert!(schema
            .decode_account(DISCRIMINATOR, &demo_hex()[..length])
            .is_err());
    }
    for used in [0u32, 11, 12, 172, u32::MAX] {
        let mut bytes = demo_hex();
        bytes[8..12].copy_from_slice(&used.to_le_bytes());
        assert!(
            schema.decode_account(DISCRIMINATOR, &bytes).is_err(),
            "used {used}"
        );
    }
    // The final option's Some("") needs only its tag. Reducing used_len by
    // one is a valid new logical account; its former "m" byte becomes spare.
    let mut bytes = demo_hex();
    bytes[8..12].copy_from_slice(&166u32.to_le_bytes());
    let mut expected = demo_value();
    let Value::Struct(fields) = &mut expected else {
        panic!()
    };
    fields[7] = Value::Option(Some(Box::new(string(""))));
    assert_eq!(
        schema.decode_account(DISCRIMINATOR, &bytes).unwrap().value,
        expected
    );
    if usize::BITS > 32 {
        assert_eq!(
            schema.encode_account(DISCRIMINATOR, &value, u32::MAX as usize + 1),
            Err(Error::Overflow)
        );
    }
}
