//! Generated readers, account lifecycle, lazy faults and initialization atomicity.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/demo.rs"]
mod demo;

#[cfg(not(feature = "verify-on-load"))]
use blubber::raw::Resize;
use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, Map, ReadStorage, Schema, SliceStore, Str, View};
use demo::{MyAccount, Record, RecordInit, RecordRef};

#[blubber::schema]
struct Nested {
    record: Record,
    optional: Option<Record>,
    map: Map<u16, Record>,
    fixed: List<Flag>,
}

#[blubber::schema]
struct Flag {
    enabled: bool,
}

#[test]
fn accessor_overrides_preserve_init_names_raw_identifiers_and_alias_dispatch() {
    type Text = Str;
    #[blubber::schema]
    struct Renamed {
        #[blubber(accessor = "checked")]
        validate: Text,
        r#type: u16,
    }
    let init = RenamedInit {
        validate: "renamed",
        r#type: 256,
    };
    let mut bytes = vec![0; Renamed::measure(&init).unwrap() as usize];
    Renamed::encode(&init, &mut bytes);
    let read = View::new(&bytes, Renamed::LAYOUT, 0)
        .unwrap()
        .read::<Renamed>()
        .unwrap();
    assert_eq!(read.checked(), "renamed");
    assert_eq!(read.try_checked(), Ok("renamed"));
    assert_eq!(read.r#type(), 256);
    assert_eq!(read.try_type(), Ok(256));
    read.validate().unwrap();
}

#[test]
fn bare_account_schema_reads_report_lengths_without_claiming_outer_spare() {
    let bytes = demo::golden();
    let read = View::new(&bytes[12..167], MyAccount::LAYOUT, 40)
        .unwrap()
        .read::<MyAccount>()
        .unwrap();
    assert_eq!(read.used_len(), 167);
    assert_eq!(read.storage_len(), 167);
    assert_eq!(read.note(), "ok");
    let mut root = bytes[12..167].to_vec();
    root[40] = 2;
    let read = View::new(&root, MyAccount::LAYOUT, 40)
        .unwrap()
        .read::<MyAccount>()
        .unwrap();
    assert_eq!(
        read.try_limits(),
        Err(Error::new(ErrorKind::InvalidBool, 80))
    );
}

#[test]
fn local_readers_compose_through_fields_options_both_maps_and_lists() {
    let record = RecordInit {
        name: "borrowed",
        tags: &[7, 9],
    };
    let init = NestedInit {
        record,
        optional: Some(record),
        map: &[(1, record)],
        fixed: &[FlagInit { enabled: true }],
    };
    let mut bytes = vec![0; Nested::measure(&init).unwrap() as usize];
    Nested::encode(&init, &mut bytes);
    let read = View::new(&bytes, Nested::LAYOUT, 40)
        .unwrap()
        .read::<Nested>()
        .unwrap();
    fn name(record: RecordRef<'_>) -> &str {
        record.name()
    }
    assert_eq!(name(read.record()), "borrowed");
    assert_eq!(read.optional().unwrap().tags().get(1), Some(9));
    assert_eq!(read.map().get(&1).unwrap().name(), "borrowed");
    assert!(read.fixed().get(0).unwrap().enabled());
    read.validate().unwrap();
    assert!(Flag::read_removed(View::new(&[2], Flag::LAYOUT, 99).unwrap()).is_err());
}

impl<'a> RecordRef<'a> {
    fn label(self) -> &'a str {
        self.name()
    }
}

#[test]
fn readers_are_copy_and_borrow_original_bytes_through_temporary_parents() {
    let bytes = demo::golden();
    let name = MyAccount::load(&bytes)
        .unwrap()
        .records()
        .get(0)
        .unwrap()
        .label();
    assert_eq!(name, "A");
    assert_eq!(name.as_ptr(), bytes[83..].as_ptr());
    let account = MyAccount::load(&bytes).unwrap();
    let copy = account;
    assert_eq!(copy.note(), account.note());
    assert_eq!(copy.try_authority(), Ok(copy.authority()));
    assert_eq!(copy.try_counter(), Ok(1));
    assert_eq!(copy.try_memo(), Ok(Some("m")));
    assert_eq!(copy.try_limits(), Ok(copy.limits()));
    copy.try_records().unwrap().validate().unwrap();
    copy.try_balances().unwrap().validate().unwrap();
    copy.try_labels().unwrap().validate().unwrap();
    copy.try_attachment().unwrap().validate().unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn lazy_load_and_try_getters_report_absolute_leaf_faults() {
    let mut bytes = demo::golden();
    bytes[157] = 0xff;
    let read = MyAccount::load_trusted(&bytes).unwrap();
    assert_eq!(read.counter(), 1);
    let expected = Error::new(ErrorKind::InvalidUtf8, 157);
    assert_eq!(read.try_note(), Err(expected));
    assert_eq!(read.validate(), Err(expected));
    assert_eq!(MyAccount::load(&bytes).unwrap_err(), expected);
    assert!(std::panic::catch_unwind(|| read.note()).is_err());
    bytes[157] = b'o';
    bytes[52] = 2;
    assert_eq!(
        MyAccount::load_trusted(&bytes).unwrap().try_limits(),
        Err(Error::new(ErrorKind::InvalidBool, 52))
    );
    bytes[52] = 1;
    bytes[83] = 0xff;
    let record = MyAccount::load_trusted(&bytes)
        .unwrap()
        .records()
        .get(0)
        .unwrap();
    assert_eq!(
        record.try_name(),
        Err(Error::new(ErrorKind::InvalidUtf8, 83))
    );
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn lazy_load_checks_only_selected_boundaries_and_full_validation_checks_all() {
    let mut bytes = demo::golden();
    bytes[63..67].copy_from_slice(&u32::MAX.to_le_bytes());
    let read = MyAccount::load_trusted(&bytes).unwrap();
    assert_eq!(read.counter(), 1);
    assert_eq!(read.records().get(0).unwrap().name(), "A");
    assert_eq!(
        read.try_note(),
        Err(Error::new(ErrorKind::InvalidBoundary, 63))
    );
    assert_eq!(
        read.validate(),
        Err(Error::new(ErrorKind::InvalidBoundary, 63))
    );
}

#[test]
fn account_constructors_preserve_envelope_fault_precedence_and_ignore_spare() {
    let golden = demo::golden();
    for len in 0..12 {
        assert_eq!(
            MyAccount::load(&golden[..len]).unwrap_err(),
            Error::new(ErrorKind::Truncated, 0)
        );
    }
    let mut bytes = golden.clone();
    bytes[0] ^= 1;
    assert_eq!(
        MyAccount::load(&bytes).unwrap_err(),
        Error::new(ErrorKind::Discriminator, 0)
    );
    for used in [0, 12, 91, 172, u32::MAX] {
        bytes.copy_from_slice(&golden);
        bytes[8..12].copy_from_slice(&used.to_le_bytes());
        assert_eq!(
            MyAccount::load(&bytes).unwrap_err(),
            Error::new(ErrorKind::UsedLength, 8)
        );
    }
    bytes.copy_from_slice(&golden);
    bytes[167..].fill(0xff);
    MyAccount::load(&bytes).unwrap();
    assert_eq!(MyAccount::load(&bytes[..167]).unwrap().storage_len(), 167);
}

#[test]
fn mutable_root_reads_commit_and_closed_loads_use_the_same_contract() {
    let mut bytes = demo::golden();
    let mut store = SliceStore::new(&mut bytes);
    let root = MyAccount::load_mut(&mut store).unwrap();
    assert_eq!(
        root.authority(),
        blubber::Address::new_from_array([0x11; 32])
    );
    assert_eq!(root.counter(), 1);
    assert_eq!(root.note(), "ok");
    assert_eq!(root.records().get(0).unwrap().name(), "A");
    assert_eq!(root.labels().get(&256), Some(""));
    assert_eq!(root.balances().len(), 1);
    assert_eq!(root.memo(), Some("m"));
    assert_eq!(root.attachment().len(), 2);
    assert!(root.limits().enabled);
    root.validate().unwrap();
    assert_eq!(root.commit().unwrap().used_len, 167);
    assert!(
        matches!(MyAccount::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Closed.into())
    );
    assert!(
        matches!(MyAccount::load_mut(&mut store), Err(error) if error == ErrorKind::Closed.into())
    );
    assert!(
        matches!(MyAccount::initialize(&mut store, demo::input()), Err(error) if error == ErrorKind::Closed.into())
    );
}

#[test]
fn fixed_room_reserve_failures_and_shrink_regrowth_preserve_the_logical_root() {
    let mut bytes = demo::golden();
    let mut store = SliceStore::new(&mut bytes);
    let mut account = MyAccount::load_mut(&mut store).unwrap();
    assert_eq!(account.reserve(5), Err(ErrorKind::Capacity.into()));
    assert_eq!(account.storage_len(), 171);
    assert_eq!(account.used_len(), 167);
    assert_eq!(account.try_note(), Ok("ok"));
    assert!(account.shrink_to_fit().unwrap());
    assert_eq!(account.storage_len(), 167);
    account.reserve(4).unwrap();
    assert_eq!(account.storage_len(), 171);
    account.commit().unwrap();
    assert_eq!(bytes, demo::golden());
}

#[test]
fn initialization_preflight_never_changes_existing_bytes_or_length() {
    let mut bytes = demo::golden();
    let original = bytes.clone();
    let mut store = SliceStore::new(&mut bytes);
    assert!(
        matches!(MyAccount::initialize(&mut store, demo::input()), Err(error)
        if error == Error::new(ErrorKind::AlreadyInitialized, 0))
    );
    assert_eq!(store.bytes(), original);
    let mut input = demo::input();
    input.labels = &[(1, "x"), (1, "y")];
    assert!(
        matches!(MyAccount::initialize(&mut store, input), Err(error)
        if error == ErrorKind::DuplicateKey.into())
    );
    assert_eq!(store.bytes(), original);
    assert_eq!(store.storage_len(), 171);
    assert!(!store.is_poisoned());
    for len in [0, 4, 8, 12, 92, 166] {
        let mut room = vec![0; len];
        let mut store = SliceStore::new(&mut room);
        assert!(
            matches!(MyAccount::initialize(&mut store, demo::input()), Err(error)
            if error == ErrorKind::Capacity.into())
        );
        assert_eq!(store.bytes(), vec![0; len]);
        assert_eq!(store.storage_len(), len);
        assert!(!store.is_poisoned());
    }
}

#[test]
fn initialized_identity_is_checked_even_with_a_partial_envelope() {
    for len in 1..12 {
        let mut bytes = vec![0; len];
        bytes[len.min(8) - 1] = 1;
        let original = bytes.clone();
        let mut store = SliceStore::new(&mut bytes);
        assert!(
            matches!(MyAccount::initialize(&mut store, demo::input()), Err(error)
            if error == Error::new(ErrorKind::AlreadyInitialized, 0))
        );
        assert_eq!(store.bytes(), original);
    }
}

#[test]
fn explicit_discriminator_and_zero_length_schema_payload_initialize() {
    #[blubber::schema(account, discriminator = [1,2,3,4,5,6,7,8])]
    struct Empty {
        text: Str,
    }
    let mut bytes = [0; 12];
    let mut store = SliceStore::new(&mut bytes);
    let account = Empty::initialize(&mut store, EmptyInit { text: "" }).unwrap();
    assert_eq!(account.text(), "");
    assert_eq!(account.used_len(), 12);
    account.commit().unwrap();
    assert_eq!(bytes, [1, 2, 3, 4, 5, 6, 7, 8, 12, 0, 0, 0]);
    Empty::load(&bytes).unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn mutable_load_validation_and_poison_recovery_require_complete_validity() {
    let mut bytes = demo::golden();
    bytes[157] = 0xff;
    let mut store = SliceStore::new(&mut bytes);
    assert!(MyAccount::load_trusted_mut(&mut store)
        .unwrap()
        .try_note()
        .is_err());
    store.set_poisoned(true);
    assert!(
        matches!(MyAccount::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
    );
    assert!(
        matches!(MyAccount::initialize(&mut store, demo::input()), Err(error) if error == ErrorKind::Poisoned.into())
    );
    assert!(MyAccount::load_mut(&mut store).is_err());
    assert!(store.is_poisoned());
    store.bytes_mut()[157] = b'o';
    MyAccount::load_mut(&mut store).unwrap().validate().unwrap();
    assert!(!store.is_poisoned());
}

// Unsupported handwritten codec used only to inject an interrupted encoder.
struct Interrupted;
impl Schema for Interrupted {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:schema_read:Interrupted",
                epoch: 0,
            },
            wire: <bool as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <bool as Schema>::LAYOUT;
    const __LEN: usize = <bool as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <bool as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = bool;
    type Mut<'a> = blubber::raw::Edit<'a>;

    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(edit)
    }
    type Init<'a> = ();
    type Removed = bool;
    #[cfg(feature = "alloc")]
    type Owned = bool;
    #[cfg(feature = "alloc")]
    fn measure_owned(_: &bool) -> blubber::Result<u32> {
        Self::measure(&())
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(_: &bool, destination: &mut [u8]) {
        Self::encode(&(), destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }

    fn read<'a>(view: View<'a>) -> blubber::Result<bool> {
        view.read::<bool>()
    }
    fn read_removed(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }
    fn measure(_: &()) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(_: &(), bytes: &mut [u8]) {
        bytes[0] = 2;
        panic!("injected encoder panic")
    }
}

#[test]
fn encoder_unwind_keeps_poison_set_and_never_publishes_identity() {
    #[blubber::schema(account)]
    struct Panics {
        value: Interrupted,
    }
    let mut bytes = [0; 32];
    let mut store = SliceStore::new(&mut bytes);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = Panics::initialize(&mut store, PanicsInit { value: () });
    }))
    .is_err());
    assert!(store.is_poisoned());
    assert_eq!(&store.bytes()[..12], &[0; 12]);
    assert_eq!(store.bytes()[12], 2);
    assert!(
        matches!(Panics::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
    );
    assert!(Panics::load_mut(&mut store).is_err());
    assert!(store.is_poisoned());
}

#[cfg(feature = "alloc")]
#[test]
fn initialize_from_empty_vec_and_reads_after_reserve_shrink_and_regrowth() {
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = MyAccount::initialize(&mut store, demo::input()).unwrap();
    assert_eq!(root.storage_len(), 167);
    assert_eq!(root.note(), "ok");
    root.reserve(10_000).unwrap();
    assert_eq!(root.storage_len(), 10_167);
    assert_eq!(root.records().get(0).unwrap().name(), "A");
    assert!(root.shrink_to_fit().unwrap());
    assert_eq!(root.storage_len(), 167);
    root.reserve(4).unwrap();
    assert_eq!(root.storage_len(), 171);
    root.commit().unwrap();
    assert_eq!(bytes, demo::golden());
}
