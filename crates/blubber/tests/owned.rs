//! Recursive owned mirrors, direct borrowed insertion and checked extraction.

#![forbid(unsafe_code)]
#![cfg(all(feature = "alloc", feature = "derive"))]

#[path = "support/demo.rs"]
mod demo;

blubber::__root_schema!(__ConcreteRoot0, List<demo::Limits>);

use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, Map, Pubkey, ReadStorage, Schema, SliceStore, Str, View};
use demo::{MyAccount, MyAccountOwned, RecordOwned};

fn encoded<S: Schema>(value: &S::Owned) -> Vec<u8> {
    let mut bytes = vec![0xa5; S::measure_owned(value).unwrap() as usize];
    S::encode_owned(value, &mut bytes);
    View::new(&bytes, S::LAYOUT, 0).unwrap().validate().unwrap();
    bytes
}

#[test]
fn complete_demo_owned_mirror_matches_independent_golden_and_field_types() {
    let bytes = demo::golden();
    let root = MyAccount::load(&bytes).unwrap();
    let owned: MyAccountOwned = root.to_owned().unwrap();
    assert_eq!(owned.authority, Pubkey::from_bytes([0x11; 32]));
    assert_eq!(owned.counter, 1);
    assert_eq!(owned.records[0].name, "A");
    assert_eq!(owned.records[0].tags, [7, 9]);
    assert_eq!(owned.balances, [(Pubkey::from_bytes([0x22; 32]), 5)]);
    assert_eq!(owned.labels, [(1, String::from("x")), (256, String::new())]);
    assert_eq!(owned.note, "ok");
    assert_eq!(owned.attachment, [0xaa, 0xbb]);
    assert_eq!(owned.memo.as_deref(), Some("m"));
    assert!(owned.limits.enabled);
    assert_eq!(encoded::<MyAccount>(&owned), bytes[12..167]);
    assert_eq!(root.records().to_owned().unwrap()[0].name, "A");
    assert_eq!(root.balances().to_owned().unwrap(), owned.balances);
    assert_eq!(
        root.records().get(0).unwrap().to_owned().unwrap().tags,
        [7, 9]
    );
}

#[test]
fn recursive_builtin_owned_codecs_match_borrowed_encodings() {
    type Tree = List<Map<i16, Option<List<Str>>>>;
    let value = vec![
        vec![
            (i16::MIN, None),
            (0, Some(vec![String::new(), String::from("é🦀")])),
        ],
        vec![],
        vec![(i16::MAX, Some(vec![]))],
    ];
    let bytes = encoded::<Tree>(&value);
    let borrowed: <Tree as Schema>::Init<'_> = &[
        &[(i16::MIN, None), (0, Some(&["", "é🦀"]))],
        &[],
        &[(i16::MAX, Some(&[]))],
    ];
    let mut expected = vec![0; Tree::measure(&borrowed).unwrap() as usize];
    Tree::encode(&borrowed, &mut expected);
    assert_eq!(bytes, expected);
    assert_eq!(
        Tree::decode_owned(View::new(&bytes, Tree::LAYOUT, 40).unwrap()).unwrap(),
        value
    );
    assert_eq!(
        encoded::<List<u16>>(&vec![1, 256]),
        [2, 0, 0, 0, 1, 0, 0, 1]
    );
    assert_eq!(
        encoded::<Map<i16, bool>>(&vec![(-1, true), (256, false)]),
        [2, 0, 0, 0, 0x7f, 0xff, 1, 0x81, 0, 0]
    );
    assert_eq!(encoded::<Option<Str>>(&None), [0]);
    assert_eq!(encoded::<List<Str>>(&vec![]), [0; 4]);
    assert_eq!(encoded::<Map<u16, Str>>(&vec![]), [0; 4]);
}

#[test]
fn owned_map_measurement_rejects_bad_order_before_any_write() {
    for (values, kind) in [
        (
            vec![(2, String::from("a")), (1, String::from("b"))],
            ErrorKind::UnsortedKeys,
        ),
        (
            vec![(1, String::from("a")), (1, String::from("b"))],
            ErrorKind::DuplicateKey,
        ),
    ] {
        assert_eq!(Map::<u16, Str>::measure_owned(&values), Err(kind.into()));
    }
}

#[test]
fn owned_list_edits_borrow_source_and_return_independent_values() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = MyAccount::initialize(&mut store, demo::input()).unwrap();
    let mut value = RecordOwned {
        name: String::from("owned é"),
        tags: vec![10, 20],
    };
    let mut records = root.records_mut();
    records.push_owned(&value).unwrap();
    records.insert_owned(0, &value).unwrap();
    records.replace_owned(1, &value).unwrap();
    value.name.clear(); // insertion borrowed the source without retaining it.
    assert_eq!(records.get(1).unwrap().name(), "owned é");
    let old: RecordOwned = records.remove_owned(1).unwrap();
    assert_eq!(old.name, "owned é");
    assert_eq!(old.tags, [10, 20]);
    assert_eq!(records.pop_owned().unwrap().unwrap().name, "owned é");
    records.clear().unwrap();
    assert!(records.pop_owned().unwrap().is_none());
    root.validate().unwrap();
    assert_eq!(root.note(), "ok");
}

#[test]
fn wide_variable_owned_removal_preserves_values_and_poison_frames() {
    #[blubber::schema]
    struct WideValue {
        bytes: [u8; 1024],
        label: Str,
    }
    #[blubber::schema(account)]
    struct WideList {
        values: List<WideValue>,
    }
    let mut bytes = vec![0; 4096];
    {
        let mut store = SliceStore::new(&mut bytes);
        WideList::initialize(
            &mut store,
            WideListInit {
                values: &[
                    WideValueInit {
                        bytes: [0x41; 1024],
                        label: "first",
                    },
                    WideValueInit {
                        bytes: [0x42; 1024],
                        label: "second",
                    },
                ],
            },
        )
        .unwrap();
        let mut root = WideList::load_trusted_mut(&mut store).unwrap();
        let removed = root.values_mut().remove_owned(0).unwrap();
        assert_eq!(removed.bytes, [0x41; 1024]);
        assert_eq!(removed.label, "first");
        assert_eq!(root.values().len(), 1);
        assert_eq!(root.values().get(0).unwrap().label(), "second");
        let remaining = root.values_mut().pop_owned().unwrap().unwrap();
        assert_eq!(remaining.bytes, [0x42; 1024]);
        assert_eq!(remaining.label, "second");
        assert!(root.values_mut().pop_owned().unwrap().is_none());
        root.validate().unwrap();
    }
    #[cfg(not(feature = "verify-on-load"))]
    {
        bytes.fill(0);
        {
            let mut store = SliceStore::new(&mut bytes);
            WideList::initialize(
                &mut store,
                WideListInit {
                    values: &[WideValueInit {
                        bytes: [0x43; 1024],
                        label: "invalid",
                    }],
                },
            )
            .unwrap();
        }
        let list = View::load_trusted::<WideList>(&bytes, WideList::DISCRIMINATOR)
            .unwrap()
            .field(0)
            .unwrap();
        // A singleton indexed list has no stored ends after its count word.
        let label = View::new(
            &list.bytes()[4..],
            WideValue::LAYOUT,
            list.base().checked_add(4).unwrap(),
        )
        .unwrap()
        .field(1)
        .unwrap()
        .base();
        bytes[label as usize] = 0xff;
        let before = bytes.clone();
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = WideList::load_trusted_mut(&mut store).unwrap();
            assert_eq!(
                root.values_mut().remove_owned(0).map(|_| ()),
                Err(Error::new(ErrorKind::InvalidUtf8, label))
            );
            assert_eq!(root.values_mut().try_len(), Err(ErrorKind::Poisoned.into()));
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn owned_map_edits_preserve_old_value_semantics_for_both_layouts() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = MyAccount::initialize(&mut store, demo::input()).unwrap();
    let text = String::from("borrowed owned text");
    assert_eq!(root.labels_mut().insert_owned(2, &text), Ok(None));
    assert_eq!(root.labels_mut().insert_owned(1, &text), Ok(Some(())));
    assert_eq!(root.labels_mut().remove_owned(&1), Ok(Some(text.clone())));
    assert_eq!(root.labels_mut().remove_owned(&9), Ok(None));
    let key = Pubkey::from_bytes([0x22; 32]);
    assert_eq!(root.balances_mut().insert_owned(key, &99), Ok(Some(5)));
    assert_eq!(root.balances_mut().remove_owned(&key), Ok(Some(99)));
    assert_eq!(root.labels().to_owned().unwrap()[0].0, 2);
    root.validate().unwrap();
}

#[test]
fn generated_fixed_kind_mirrors_keep_native_fixed_values_and_schema_views_distinct() {
    #[blubber::schema]
    struct Flag {
        flag: bool,
        bytes: [u8; 3],
    }
    let value = FlagOwned {
        flag: true,
        bytes: [7, 8, 9],
    };
    assert_eq!(encoded::<Flag>(&value), [1, 7, 8, 9]);
    let copy = Flag::decode_owned(View::new(&[1, 7, 8, 9], Flag::LAYOUT, 50).unwrap()).unwrap();
    assert!(copy.flag);
    assert_eq!(copy.bytes, [7, 8, 9]);
    let value = demo::Limits {
        enabled: true,
        limit: 42,
    };
    assert_eq!(encoded::<demo::Limits>(&value), [1, 42, 0]);
    let raw = [1u8, 2, 3];
    assert_eq!(encoded::<[u8; 3]>(&raw), raw);
}

#[test]
fn failed_owned_insertion_preserves_storage_capacity_poison_and_input() {
    let used = MyAccount::encoded_len_of(&demo::input()).unwrap() as usize;
    let mut bytes = vec![0; used];
    let mut store = SliceStore::new(&mut bytes);
    MyAccount::initialize(&mut store, demo::input()).unwrap();
    let before = store.bytes().to_vec();
    let value = RecordOwned {
        name: "growing".into(),
        tags: vec![100],
    };
    {
        let mut root = MyAccount::load_mut(&mut store).unwrap();
        assert_eq!(
            root.records_mut().push_owned(&value),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(
            root.records_mut().insert_owned(usize::MAX, &value),
            Err(ErrorKind::IndexOutOfBounds.into())
        );
        assert_eq!(
            root.records_mut().replace_owned(1, &value),
            Err(ErrorKind::IndexOutOfBounds.into())
        );
        assert_eq!(
            root.labels_mut().insert_owned(9, &"new".into()),
            Err(ErrorKind::Capacity.into())
        );
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    assert_eq!(value.name, "growing");
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn corrupt_owned_decode_and_extraction_report_absolute_offsets_without_compaction() {
    let mut bytes = demo::golden();
    let view = View::load_trusted::<MyAccount>(&bytes, MyAccount::DISCRIMINATOR).unwrap();
    // One record: count word, then its stored name end and UTF-8 payload.
    let base = view.field(2).unwrap().base() + 8;
    bytes[base as usize] = 0xff;
    let before = bytes.clone();
    for pop in [false, true] {
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = MyAccount::load_trusted_mut(&mut store).unwrap();
            assert!(
                matches!(root.to_owned(), Err(e) if e == Error::new(ErrorKind::InvalidUtf8, base))
            );
            assert_eq!(root.records_mut().try_len(), Ok(1));
            let result = if pop {
                root.records_mut().pop_owned().map(|_| ())
            } else {
                root.records_mut().remove_owned(0).map(|_| ())
            };
            assert_eq!(result, Err(Error::new(ErrorKind::InvalidUtf8, base)));
            assert_eq!(
                root.records_mut().try_len(),
                Err(ErrorKind::Poisoned.into())
            );
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn owned_values_survive_vec_relocation_and_nested_edits() {
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = MyAccount::initialize(&mut store, demo::input()).unwrap();
    let value = RecordOwned {
        name: "long".repeat(128),
        tags: vec![1, 2, 3],
    };
    let mut records = root.records_mut();
    records.push_owned(&value).unwrap();
    records
        .reborrow()
        .at_mut(1)
        .unwrap()
        .tags_mut()
        .push(4)
        .unwrap();
    let old = records.pop_owned().unwrap().unwrap();
    assert_eq!(old.name, value.name);
    assert_eq!(old.tags, [1, 2, 3, 4]);
    assert_eq!(root.to_owned().unwrap().records[0].name, "A");
    root.validate().unwrap();
}

#[test]
fn native_owned_list_edits_and_maximum_depth_values_round_trip() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut edit =
        blubber::raw::Edit::initialize::<__ConcreteRoot0>(&mut store, *b"ownfixed", &[]).unwrap();
    let value = demo::Limits {
        enabled: true,
        limit: 256,
    };
    let mut list = blubber::raw::project::<List<demo::Limits>>(edit.reborrow());
    list.push_owned(&value).unwrap();
    list.insert_owned(0, &value).unwrap();
    list.replace_owned(1, &value).unwrap();
    assert_eq!(list.remove_owned(0), Ok(value));
    assert_eq!(list.pop_owned(), Ok(Some(value)));
    assert_eq!(list.pop_owned(), Ok(None));
    edit.validate().unwrap();
    type Deep = Option<Option<Option<Option<Option<Option<Option<Str>>>>>>>;
    let value = Some(Some(Some(Some(Some(Some(Some(String::from("é"))))))));
    let bytes = encoded::<Deep>(&value);
    assert_eq!(
        Deep::decode_owned(View::new(&bytes, Deep::LAYOUT, 123).unwrap()),
        Ok(value)
    );
}

#[test]
fn owned_decoding_rejects_unvisited_map_order_and_nested_corruption() {
    let bytes = [2, 0, 0, 0, 2, 1, 1, 0];
    assert!(
        Map::<u8, bool>::decode_owned(View::new(&bytes, Map::<u8, bool>::LAYOUT, 30).unwrap())
            .is_err()
    );
    let bytes = [1, 0, 0, 0, 2];
    assert_eq!(
        List::<bool>::decode_owned(View::new(&bytes, List::<bool>::LAYOUT, 30).unwrap()),
        Err(Error::new(ErrorKind::InvalidBool, 34))
    );
}

#[test]
#[cfg(not(miri))]
fn renamed_alloc_dependency_generates_owned_items_without_consumer_feature_flags() {
    use std::{fs, path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = root.join(".tmp/owned-consumer");
    fs::create_dir_all(fixture.join("src")).unwrap();
    fs::write(fixture.join("Cargo.toml"), format!(
        "[package]\nname = \"owned-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
        [workspace]\n[dependencies]\nwire = {{ package = \"blubber\", path = {:?}, features = [\"alloc\"] }}\n",
        root.join("crates/blubber"),
    )).unwrap();
    fs::write(
        fixture.join("src/lib.rs"),
        r#"
        #![no_std]
        #![forbid(unsafe_code)]
        #![deny(warnings, missing_docs)]
        //! Renamed alloc dependency, no matching consumer feature.
        extern crate alloc;
        /// Variable schema from a nested module.
        pub mod nested {
            /// A child mirror with a raw identifier and an accessor override.
            #[wire::schema]
            pub struct Child {
                /// Counted UTF-8 payload.
                #[blubber(accessor = "payload")]
                pub r#type: wire::Str,
            }
        }
        /// Native fixed input preserves its own layout.
        #[derive(Clone, Copy, wire::Fixed)]
        pub struct Native { /// Value.
            pub value: u16,
        }
        /// Alias exercises trait-based recursion.
        pub type Children = wire::List<nested::Child>;
        /// An owned account with renamed dependency paths.
        #[wire::schema(account)]
        pub struct Account {
            /// Child list.
            pub children: Children,
            /// Scalar native struct.
            pub fixed: Native,
            /// Sorted map.
            pub labels: wire::Map<u16, wire::Str>,
        }
        /// Construct mirrors using the original field names.
        pub fn input() -> AccountOwned {
            AccountOwned {
                children: alloc::vec![nested::ChildOwned { r#type: alloc::string::String::new() }],
                fixed: Native { value: 7 },
                labels: alloc::vec::Vec::new(),
            }
        }
        /// Every owned method is inherent or part of an explicit schema bound.
        pub fn use_it(root: &mut AccountMut<'_>) -> wire::Result<AccountOwned> {
            let value = input();
            root.children_mut().push_owned(&value.children[0])?;
            root.children().get(0).unwrap().to_owned()?;
            root.children().to_owned()?;
            root.labels().to_owned()?;
            root.to_owned()
        }
    "#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--color", "never", "--manifest-path"])
        .arg(fixture.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    let raw = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(fixture.join("check.log"), &raw).unwrap();
    assert!(output.status.success(), "{raw}");
    assert!(!raw.contains("warning:"), "{raw}");
}
