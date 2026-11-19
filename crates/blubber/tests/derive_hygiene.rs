//! Generated codecs preserve consumer types without capturing their names.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

extern crate blubber as wire;

#[path = "support/hygiene.rs"]
mod hygiene;

use blubber::{Fixed, Schema};

#[test]
fn primitive_aliases_preserve_input_types_and_exact_wire_bytes() {
    use hygiene::primitives::{Account, AccountInit, Native};
    let native = Native {
        byte: 0x1234,
        word: 0x5678_9abc,
        flag: true,
    };
    let bytes: [u8; 7] = native.encode();
    assert_eq!(bytes, [0x34, 0x12, 0xbc, 0x9a, 0x78, 0x56, 1]);
    assert_eq!(Native::decode(&bytes), Ok(native));
    let mut room = [0; 128];
    let mut store = blubber::SliceStore::new(&mut room);
    let mut root = Account::initialize(
        &mut store,
        AccountInit {
            value: native,
            count: u64::MAX,
            text: "initial",
        },
    )
    .unwrap();
    assert_eq!(root.count(), u64::MAX);
    root.value_mut().set(native).unwrap();
    root.text_mut().set("changed").unwrap();
    assert_eq!(root.value(), native);
    assert_eq!(root.text(), "changed");
    root.validate().unwrap();
}

#[test]
fn helper_names_and_suffixes_do_not_capture_consumer_const_expressions() {
    use hygiene::constants::{Native, Record, RecordInit};
    let native = Native {
        first: [1; 2],
        second: [2; 3],
    };
    let bytes: [u8; 5] = native.encode();
    assert_eq!(bytes, [1, 1, 2, 2, 2]);
    assert_eq!(Native::decode(&bytes), Ok(native));
    let init = RecordInit {
        a: [1; 1],
        b: [2; 2],
        c: [3; 3],
        d: [4; 4],
        e: [5; 5],
        f: [6; 6],
        g: [7; 7],
        text: "end",
    };
    assert_eq!(Record::measure(&init), Ok(31));
    let mut bytes = [0; 31];
    Record::encode(&init, &mut bytes);
    let view = blubber::View::new(&bytes, Record::LAYOUT, 0).unwrap();
    let record = Record::read(view).unwrap();
    assert_eq!(record.a(), [1; 1]);
    assert_eq!(record.b(), [2; 2]);
    assert_eq!(record.c(), [3; 3]);
    assert_eq!(record.d(), [4; 4]);
    assert_eq!(record.e(), [5; 5]);
    assert_eq!(record.f(), [6; 6]);
    assert_eq!(record.g(), [7; 7]);
    assert_eq!(record.text(), "end");
    record.validate().unwrap();
}

#[test]
fn concrete_projection_locals_preserve_consumer_const_expressions() {
    use hygiene::projection_locals::{Account, AccountInit};
    let mut bytes = [0; 64];
    let mut store = blubber::SliceStore::new(&mut bytes);
    let mut root = Account::initialize(
        &mut store,
        AccountInit {
            a: [1; 1],
            b: [2; 2],
            c: [3; 3],
            d: [4; 4],
            e: [5; 5],
            f: [6; 6],
        },
    )
    .unwrap();
    root.a_mut().set([7; 1]).unwrap();
    root.b_mut().set([7; 2]).unwrap();
    root.c_mut().set([7; 3]).unwrap();
    root.d_mut().set([7; 4]).unwrap();
    root.e_mut().set([7; 5]).unwrap();
    root.f_mut().set([7; 6]).unwrap();
    assert_eq!(root.a(), [7; 1]);
    assert_eq!(root.b(), [7; 2]);
    assert_eq!(root.c(), [7; 3]);
    assert_eq!(root.d(), [7; 4]);
    assert_eq!(root.e(), [7; 5]);
    assert_eq!(root.f(), [7; 6]);
    root.validate().unwrap();
}
