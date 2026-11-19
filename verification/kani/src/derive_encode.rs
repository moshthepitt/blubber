//! Actual schema expansion: packed fixed and small borrowed variable inputs.

use blubber::raw::Edit;
use blubber::Resize as _;
use blubber::{ErrorKind, RootSchema, Schema, SliceStore, Str};
use blubber_core::{struct_variable_field, validate_fixed, validate_utf8, Layout, LayoutHead};

#[blubber::schema]
struct FixedFields {
    flag: bool,
    count: u16,
}

#[blubber::schema]
struct VariableFields {
    text: Str,
}

#[blubber::schema]
struct EqualWidthFields {
    first: u32,
    second: u32,
}

#[kani::proof]
#[kani::unwind(12)]
fn derive_encode_projection_swap() {
    use blubber::__private::{product, Pair};
    type Fields = Pair<product::Field<u32>, product::Field<u32>>;
    let first: u32 = kani::any();
    let second: u32 = kani::any();
    let init = EqualWidthFieldsInit { first, second };
    let exchanged = Pair {
        left: product::borrowed::<u32>(&init.second),
        right: product::borrowed::<u32>(&init.first),
    };
    let mut output = [0xA5; 10];
    assert_eq!(product::measure::<Fields>(&exchanged), Ok(8));
    product::encode::<Fields>(&exchanged, &mut output[1..9]);
    assert_eq!(output[0], 0xA5);
    assert_eq!(output[9], 0xA5);
    assert!(validate_fixed(&output[1..9], EqualWidthFields::LAYOUT).is_ok());
    let a = first.to_le_bytes();
    let b = second.to_le_bytes();
    let golden = [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]];
    assert_eq!(output[1..9] == golden, first == second);
    kani::cover!(
        first == 0 && second == u32::MAX,
        "distinct full-width fields retain validity and fail the golden oracle"
    );
}

fn checked<S: Schema>(init: S::Init<'_>, expected: &[u8], certify: impl FnOnce(&[u8])) {
    let len = S::measure(&init).unwrap() as usize;
    assert_eq!(len, expected.len());
    let mut bytes: [u8; 32] = kani::any();
    let before = bytes;
    S::encode(&init, &mut bytes[1..1 + len]);
    assert_eq!(&bytes[1..1 + len], expected);
    assert_eq!(bytes[0], before[0]);
    assert_eq!(&bytes[1 + len..], &before[1 + len..]);
    certify(&bytes[1..1 + len]);
    kani::cover!(
        true,
        "measured output matches independent encoding and validates"
    );
}

#[kani::proof]
#[kani::unwind(34)]
fn derive_encode_fixed() {
    let flag: bool = kani::any();
    let count: u16 = kani::any();
    checked::<FixedFields>(
        FixedFieldsInit { flag, count },
        &[u8::from(flag), count as u8, (count >> 8) as u8],
        |bytes| {
            let view = blubber::View::new(
                bytes,
                Layout::Flat {
                    checked: FixedFields::__layout(),
                },
                0,
            )
            .unwrap();
            let read = FixedFields::read(view).unwrap();
            assert_eq!(read.flag(), flag);
            assert_eq!(read.count(), count);
        },
    );
}

// Keep recursive validation separate from the 32-byte frame loop above.
// Both checks retain their full symbolic domains and all unwind assertions.
#[kani::proof]
#[kani::unwind(3)]
fn derive_encode_fixed_validation() {
    let bytes: [u8; 3] = kani::any();
    let layout = Layout::Flat {
        checked: FixedFields::__layout(),
    };
    assert_eq!(validate_fixed(&bytes, &layout).is_ok(), bytes[0] <= 1);
    kani::cover!(
        bytes[0] == 1 && bytes[1] == 255 && bytes[2] == 255,
        "all-bits u16 and true accepted through the concrete table"
    );
}

#[kani::proof]
#[kani::unwind(34)]
fn derive_encode_variable() {
    let texts = ["", "a", "é"];
    let index: usize = kani::any();
    kani::assume(index < 3);
    let text = texts[index];
    let init = VariableFieldsInit { text };
    checked::<VariableFields>(init, text.as_bytes(), |bytes| {
        // This schema has exactly one Str child. Certify the complete actual
        // descriptor, partition and semantic domain through production APIs.
        let layout = Layout::Flat {
            checked: VariableFields::__layout(),
        };
        let LayoutHead::Struct {
            fixed_bytes,
            fixed,
            variable,
        } = layout.head()
        else {
            panic!("derived struct descriptor");
        };
        assert_eq!(fixed_bytes, 0);
        assert!(fixed.is_empty());
        assert_eq!(variable.len(), 1);
        assert_eq!(variable.get(0).declaration, 0);
        assert!(matches!(variable.get(0).layout.head(), LayoutHead::Str));
        let span = struct_variable_field(
            bytes,
            &Layout::Flat {
                checked: VariableFields::__layout(),
            },
            0,
        )
        .unwrap();
        assert_eq!(span.start, 0);
        assert_eq!(span.end as usize, bytes.len());
        assert_eq!(
            validate_utf8(&bytes[span.start as usize..span.end as usize]),
            Ok(())
        );
    });
}

#[kani::proof]
#[kani::unwind(34)]
fn derive_encode_capacity() {
    let flag: bool = kani::any();
    let count: u16 = kani::any();
    let mut bytes = [0; 14]; // The derived fixed root needs 12 + 3 bytes.
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    let result =
        Edit::initialize::<FixedFields>(&mut store, *b"encode33", FixedFieldsInit { flag, count });
    assert!(matches!(result, Err(error) if error == ErrorKind::Capacity.into()));
    assert!(!store.is_poisoned());
    assert_eq!(bytes, before);
    kani::cover!(true, "capacity rejection precedes writes");
}

#[blubber::schema]
struct UnrestrictedFields {
    count: u16,
}

#[kani::proof]
#[kani::unwind(9)]
fn derive_encode_unrestricted_validation() {
    let bits: [u8; 2] = kani::any();
    let checked = UnrestrictedFields::__layout();
    assert!(checked.is_unrestricted());
    assert!(!FixedFields::__layout().is_unrestricted());
    let bytes = [1, 0, 0, 0, bits[0], bits[1]];
    let layout = Layout::Flat {
        checked: blubber::__layout_of!(blubber::List<UnrestrictedFields>),
    };
    assert!(blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).is_ok());
    kani::cover!(
        true,
        "unrestricted record list accepts arbitrary value bits"
    );
}

#[blubber::schema(account, discriminator = [1, 2, 3, 4, 5, 6, 7, 8])]
struct PolicyFlag {
    flag: bool,
}

#[kani::proof]
#[kani::unwind(17)]
fn derive_encode_policy_identity() {
    let flag: u8 = kani::any();
    let identity: u8 = kani::any();
    let mut bytes = [0; 13];
    bytes[..8].copy_from_slice(&PolicyFlag::DISCRIMINATOR);
    bytes[0] = identity;
    bytes[8] = 13;
    bytes[12] = flag;
    // Verify envelope/identity admission separately from symbolic leaf scans.
    // Their combined public-load composition exceeded D14's 300-second cap.
    let result =
        blubber_core::raw::account_used(&bytes, PolicyFlag::DISCRIMINATOR, PolicyFlag::MIN_SIZE);
    assert_eq!(result.is_ok(), identity == 1);
    if identity != 1 {
        assert!(
            matches!(result, Err(error) if error == blubber::Error::new(ErrorKind::Discriminator, 0))
        );
    }
    kani::cover!(
        identity == 1 && flag == 1,
        "matching identity admits exact envelope independently of bool contents"
    );
}

#[kani::proof]
#[kani::unwind(17)]
fn derive_encode_policy_trusted() {
    let flag: u8 = kani::any();
    let mut bytes = [0; 13];
    bytes[..8].copy_from_slice(&PolicyFlag::DISCRIMINATOR);
    bytes[8] = 13;
    bytes[12] = flag;
    let result = PolicyFlag::load_trusted(&bytes);
    if cfg!(feature = "verify-on-load") {
        assert_eq!(result.is_ok(), flag <= 1);
    } else {
        let root = result.unwrap();
        assert_eq!(root.verify().is_ok(), flag <= 1);
    }
    kani::cover!(
        flag == 255,
        "corrupted bool reaches policy admission and verification"
    );
}

// D14 supporting checks isolate changed writer bytes, frames and measurement.
// Complete recursive validity is proved in Verus and exercised in Rust/Miri.
fn fixed_list_bytes<const N: usize>(values: &[u8; 3]) {
    use blubber::List;
    let input = &values[..N];
    let length = 4 + N;
    assert_eq!(List::<u8>::measure(&input), Ok(length as u32));
    assert_eq!(List::<u8>::measure(&input), Ok(length as u32));
    let mut bytes: [u8; 9] = kani::any();
    let before = bytes;
    List::<u8>::encode(&input, &mut bytes[1..1 + length]);
    assert_eq!(&bytes[1..5], &[N as u8, 0, 0, 0]);
    assert_eq!(&bytes[5..1 + length], input);
    assert_eq!(bytes[0], before[0]);
    assert_eq!(&bytes[1 + length..], &before[1 + length..]);
}

#[kani::proof]
#[kani::unwind(12)]
fn derive_encode_fixed_list() {
    let count: u8 = kani::any();
    kani::assume(count <= 3);
    let values: [u8; 3] = kani::any();
    match count {
        0 => fixed_list_bytes::<0>(&values),
        1 => fixed_list_bytes::<1>(&values),
        2 => fixed_list_bytes::<2>(&values),
        3 => fixed_list_bytes::<3>(&values),
        _ => unreachable!(),
    }
    kani::cover!(
        count == 3 && values[2] == u8::MAX,
        "maximum count retains unrestricted element bytes"
    );
}

fn indexed_list_input(present: [bool; 2], text: [bool; 2]) -> [Option<&'static str>; 2] {
    [
        present[0].then_some(if text[0] { "\u{80}" } else { "" }),
        present[1].then_some(if text[1] { "\u{80}" } else { "" }),
    ]
}

fn indexed_list_oracle<const N: usize>(values: &[Option<&str>; 2]) -> ([u8; 14], usize) {
    let mut golden = [0u8; 14];
    golden[0] = N as u8;
    let payload = if N == 2 { 8 } else { 4 };
    let mut end = payload;
    for (index, value) in values[..N].iter().enumerate() {
        golden[end] = u8::from(value.is_some());
        end += 1;
        if let Some(value) = value {
            for byte in value.as_bytes() {
                golden[end] = *byte;
                end += 1;
            }
        }
        if index == 0 && N == 2 {
            golden[4] = (end - payload) as u8;
        }
    }
    (golden, end)
}

fn indexed_list_bytes<const N: usize>(values: &[Option<&str>; 2]) {
    use blubber::List;
    let input = &values[..N];
    let (golden, end) = indexed_list_oracle::<N>(values);
    assert_eq!(List::<Option<Str>>::measure(&input), Ok(end as u32));
    assert_eq!(List::<Option<Str>>::measure(&input), Ok(end as u32));
    let mut first = [0xA5; 16];
    let mut second = [0x5A; 16];
    List::<Option<Str>>::encode(&input, &mut first[1..1 + end]);
    List::<Option<Str>>::encode(&input, &mut second[1..1 + end]);
    assert_eq!(&first[1..1 + end], &golden[..end]);
    assert_eq!(&second[1..1 + end], &golden[..end]);
    assert_eq!((first[0], second[0]), (0xA5, 0x5A));
    assert!(first[1 + end..].iter().all(|b| *b == 0xA5));
    assert!(second[1 + end..].iter().all(|b| *b == 0x5A));
}

#[kani::proof]
#[kani::unwind(18)]
fn derive_encode_indexed_list() {
    let count: u8 = kani::any();
    kani::assume(count <= 2);
    let present: [bool; 2] = kani::any();
    let text: [bool; 2] = kani::any();
    let values = indexed_list_input(present, text);
    match count {
        0 => indexed_list_bytes::<0>(&values),
        1 => indexed_list_bytes::<1>(&values),
        2 => indexed_list_bytes::<2>(&values),
        _ => unreachable!(),
    }
    kani::cover!(
        count == 2 && present[0] && !text[0] && !present[1],
        "empty present text and absent terminal child have exact partitions"
    );
}

#[kani::proof]
#[kani::unwind(14)]
fn derive_encode_fixed_map() {
    use blubber::Map;
    let keys: [u8; 2] = kani::any();
    let values: [u8; 2] = kani::any();
    let pairs = [(keys[0], values[0]), (keys[1], values[1])];
    let input = pairs.as_slice();
    let first = Map::<u8, u8>::measure(&input);
    assert_eq!(first, Map::<u8, u8>::measure(&input));
    if keys[0] < keys[1] {
        assert_eq!(first, Ok(8));
        let mut bytes: [u8; 10] = kani::any();
        let before = bytes;
        Map::<u8, u8>::encode(&input, &mut bytes[1..9]);
        assert_eq!(
            &bytes[1..9],
            &[2, 0, 0, 0, keys[0], values[0], keys[1], values[1]]
        );
        assert_eq!(bytes[0], before[0]);
        assert_eq!(bytes[9], before[9]);
    } else {
        let kind = if keys[0] == keys[1] {
            ErrorKind::DuplicateKey
        } else {
            ErrorKind::UnsortedKeys
        };
        assert_eq!(first, Err(kind.into()));
    }
    kani::cover!(
        keys == [0, 255] && values == [255, 0],
        "ordered full-width keys and values encode"
    );
    kani::cover!(keys[0] == keys[1], "duplicate keys fail preflight");
}

#[kani::proof]
#[kani::unwind(18)]
fn derive_encode_indexed_map() {
    use blubber::Map;
    let nonempty: [bool; 2] = kani::any();
    let text = [
        if nonempty[0] { "x" } else { "" },
        if nonempty[1] { "x" } else { "" },
    ];
    let pairs = [(false, text[0]), (true, text[1])];
    let input = pairs.as_slice();
    let length = 14 + usize::from(nonempty[0]) + usize::from(nonempty[1]);
    assert_eq!(Map::<bool, Str>::measure(&input), Ok(length as u32));
    assert_eq!(Map::<bool, Str>::measure(&input), Ok(length as u32));
    let mut bytes: [u8; 18] = kani::any();
    let before = bytes;
    Map::<bool, Str>::encode(&input, &mut bytes[1..1 + length]);
    let first = u8::from(nonempty[0]);
    let last = first + u8::from(nonempty[1]);
    assert_eq!(
        &bytes[1..15],
        &[2, 0, 0, 0, 0, first, 0, 0, 0, 1, last, 0, 0, 0]
    );
    assert!(bytes[15..1 + length].iter().all(|byte| *byte == b'x'));
    assert_eq!(bytes[0], before[0]);
    assert_eq!(&bytes[1 + length..], &before[1 + length..]);
    kani::cover!(
        nonempty == [true, false],
        "terminal empty value retains the exact stored end"
    );
    kani::cover!(
        nonempty == [false, false],
        "empty values retain equal cumulative ends"
    );
}

#[kani::proof]
#[kani::unwind(8)]
fn derive_encode_prepared_replay() {
    use blubber_core::encoding_replay::{write, Prepared, Projected};
    let source: [u8; 2] = kani::any();
    let before: [u8; 6] = kani::any();
    let retained = Prepared::new(Projected {
        value: blubber_core::encoding_leaf::raw(&source),
    })
    .unwrap();
    let elided = Prepared::new(Projected {
        value: blubber_core::encoding_leaf::raw(&source),
    })
    .unwrap();
    let (retained, a) = retained.replay().unwrap();
    let (elided, b) = elided.into_measured();
    assert_eq!(a, 2);
    assert_eq!(a, b);
    let mut first = before;
    let mut second = before;
    write(&retained, &mut first, 2, a, true).unwrap();
    write(&elided, &mut second, 2, b, false).unwrap();
    assert_eq!(first, second);
    assert_eq!(&first[2..4], &source);
    assert_eq!(&first[..2], &before[..2]);
    assert_eq!(&first[4..], &before[4..]);
    kani::cover!(
        source == [0, 255],
        "full-width source survives the prepared handoff"
    );
}

#[kani::proof]
#[kani::unwind(32)]
fn derive_encode_init_publication() {
    use blubber_core::encoding_publication::{publish, settle, withdraw};
    let failed: [bool; 3] = [kani::any(), kani::any(), kani::any()];
    let offset: u32 = kani::any();
    let errors = [
        blubber::Error::new(ErrorKind::InvalidBoundary, offset),
        blubber::Error::new(ErrorKind::BorrowConflict, offset),
        blubber::Error::new(ErrorKind::InvalidBool, offset),
    ];
    let encoded = if failed[0] { Err(errors[0]) } else { Ok(23u32) };
    let acquired = if failed[1] { Err(errors[1]) } else { Ok(()) };
    let admitted = if failed[2] { Err(errors[2]) } else { Ok(7u32) };
    let mut new = true;
    let mut prefix = Some(5u32);
    withdraw(&mut new, &mut prefix);
    assert!(!new && prefix.is_none());
    let result = settle(encoded, acquired).and_then(|receipt| {
        publish(&mut prefix, admitted)?;
        Ok(receipt)
    });
    let expected = if failed[0] {
        Err(errors[0])
    } else if failed[1] {
        Err(errors[1])
    } else if failed[2] {
        Err(errors[2])
    } else {
        Ok(23)
    };
    assert_eq!(result, expected);
    assert_eq!(prefix.is_some(), !failed[0] && !failed[1] && !failed[2]);
    kani::cover!(failed == [false; 3], "all stages admit publication");
    kani::cover!(failed == [true, true, false], "earlier encoder error wins");
    kani::cover!(
        failed == [false, false, true],
        "admission fault keeps prefix withdrawn"
    );
}

#[kani::proof]
#[kani::unwind(2)]
fn derive_encode_init_policy() {
    use blubber_core::encoding_product::Markers;
    use blubber_core::encoding_publication::init_policy;
    let encoder: bool = kani::any();
    let replay: bool = kani::any();
    let checked: bool = kani::any();
    let verify: bool = kani::any();
    let policy = init_policy(Markers { encoder, replay }, checked, verify);
    let covered = encoder && replay;
    assert_eq!(policy.scan, checked || verify || !covered);
    assert_eq!(policy.replay, !covered);
    kani::cover!(
        covered && !checked && !verify,
        "covered lazy initialization"
    );
    kani::cover!(covered && checked && !verify, "checked scan retained");
    kani::cover!(covered && verify, "feature scan retained");
    kani::cover!(!encoder && replay, "opaque encoder retains both checks");
    kani::cover!(encoder && !replay, "opaque measurement retains both checks");
}
