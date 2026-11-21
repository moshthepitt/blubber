//! Generated borrowed inputs, packed metadata and exact nested encodings.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{Error, ErrorKind, Kind, List, Map, Schema, Str, View};
use blubber_core::{check_layout, validate, Layout, MAX_DEPTH};

/// A native fixed value stays a copied field in a borrowed input.
#[derive(Clone, Copy, blubber::Fixed)]
pub struct Limits {
    /// Whether edits are enabled.
    pub enabled: bool,
    /// Item limit.
    pub limit: u16,
}

/// Borrowed record declaration.
#[blubber::schema]
pub struct Record {
    /// Name bytes precede tags in the variable payload.
    pub name: Str,
    /// Counted fixed-width tags.
    pub tags: List<u32>,
}

/// Interleaved declaration order differs from fixed-prefix wire order.
#[blubber::schema]
pub struct Mixed {
    /// First variable child.
    pub name: Str,
    /// First fixed child.
    pub number: u16,
    /// Second variable child.
    pub records: List<Record>,
    /// Second fixed child.
    pub limits: Limits,
    /// Third variable child.
    pub memo: Option<Str>,
}

/// Fixed-only declarations still use schema markers and borrowed inputs.
#[blubber::schema]
pub struct FixedMarker {
    /// A flag.
    pub flag: bool,
    /// An integer.
    pub count: u16,
}

/// One variable field has no end directory.
#[blubber::schema]
pub struct One {
    /// The payload.
    pub text: Str,
}

/// Three empty fields still need two ends.
#[blubber::schema]
pub struct EmptyStrings {
    /// First field.
    pub a: Str,
    /// Second field.
    pub b: Str,
    /// Third field.
    pub c: Str,
}

/// Tour metadata and borrowed input encoding.
#[blubber::schema(account)]
pub struct MyAccount {
    /// Owner identifier.
    pub authority: blubber::Address,
    /// Counter.
    pub counter: u64,
    /// Borrowed records.
    pub records: List<Record>,
    /// Fixed values.
    pub balances: Map<blubber::Address, u64>,
    /// Variable values.
    pub labels: Map<u16, Str>,
    /// Raw UTF-8.
    pub note: Str,
    /// Counted bytes.
    pub attachment: List<u8>,
    /// Optional text.
    pub memo: Option<Str>,
    /// Copied native limits.
    pub limits: Limits,
}

fn encoded<S: Schema>(init: &S::Init<'_>) -> Vec<u8> {
    let size = S::measure(init).unwrap() as usize;
    let mut first = vec![0xa5; size + 6];
    let mut second = vec![0x5a; size];
    S::encode(init, &mut first[3..size + 3]);
    S::encode(init, &mut second);
    assert_eq!(&first[..3], &[0xa5; 3]);
    assert_eq!(&first[size + 3..], &[0xa5; 3]);
    assert_eq!(&first[3..size + 3], second);
    check_layout(S::LAYOUT, MAX_DEPTH).unwrap();
    validate(&second, S::LAYOUT, MAX_DEPTH).unwrap();
    second
}

#[test]
fn record_matches_the_hand_calculated_struct_wire_example() {
    let init = RecordInit {
        name: "A",
        tags: &[7, 9],
    };
    assert_eq!(Record::KIND, Kind::Variable);
    assert_eq!(Record::FIXED_SIZE, None);
    assert_eq!(Record::MIN_SIZE, 8);
    assert_eq!(
        encoded::<Record>(&init),
        [1, 0, 0, 0, b'A', 2, 0, 0, 0, 7, 0, 0, 0, 9, 0, 0, 0,]
    );
}

#[test]
fn interleaved_fields_have_exact_fixed_offsets_and_variable_ordinals() {
    let Layout::Struct {
        fixed_bytes,
        fixed,
        variable,
    } = Mixed::LAYOUT
    else {
        panic!()
    };
    assert_eq!(*fixed_bytes, 5);
    assert_eq!(
        fixed
            .iter()
            .map(|f| (f.declaration, f.offset))
            .collect::<Vec<_>>(),
        [(1, 0), (3, 2)]
    );
    assert_eq!(
        variable.iter().map(|f| f.declaration).collect::<Vec<_>>(),
        [0, 2, 4]
    );
    assert_eq!(Mixed::MIN_SIZE, 18);
    let bytes = encoded::<Mixed>(&MixedInit {
        name: "ab",
        number: 0x1234,
        records: &[],
        limits: Limits {
            enabled: true,
            limit: 9,
        },
        memo: Some(""),
    });
    assert_eq!(
        bytes,
        [
            0x34, 0x12, 1, 9, 0, // fixed prefix
            2, 0, 0, 0, 6, 0, 0, 0, // ends relative to payload
            b'a', b'b', 0, 0, 0, 0, 1,
        ]
    );
}

#[test]
fn zero_one_and_many_variable_fields_elide_the_right_boundaries() {
    assert_eq!(FixedMarker::KIND, Kind::Fixed);
    assert_eq!(FixedMarker::FIXED_SIZE, Some(3));
    assert_eq!(FixedMarker::MIN_SIZE, 3);
    assert_eq!(
        encoded::<FixedMarker>(&FixedMarkerInit {
            flag: true,
            count: 0x1234
        }),
        [1, 0x34, 0x12]
    );
    assert_eq!(One::MIN_SIZE, 0);
    assert_eq!(encoded::<One>(&OneInit { text: "" }), [0u8; 0]);
    assert_eq!(encoded::<One>(&OneInit { text: "x" }), b"x");
    assert_eq!(EmptyStrings::MIN_SIZE, 8);
    assert_eq!(
        encoded::<EmptyStrings>(&EmptyStringsInit {
            a: "",
            b: "",
            c: ""
        }),
        [0; 8]
    );
    assert_eq!(
        encoded::<EmptyStrings>(&EmptyStringsInit {
            a: "",
            b: "é",
            c: ""
        }),
        [0, 0, 0, 0, 2, 0, 0, 0, 0xc3, 0xa9]
    );
}

#[test]
fn fixed_schema_markers_select_fixed_list_and_map_strides() {
    let values = [
        FixedMarkerInit {
            flag: false,
            count: 2,
        },
        FixedMarkerInit {
            flag: true,
            count: 3,
        },
    ];
    assert_eq!(
        encoded::<List<FixedMarker>>(&values.as_slice()),
        [2, 0, 0, 0, 0, 2, 0, 1, 3, 0]
    );
    let entries = [(1u16, values[0]), (256, values[1])];
    assert_eq!(
        encoded::<Map<u16, FixedMarker>>(&entries.as_slice()),
        [2, 0, 0, 0, 0, 1, 0, 2, 0, 1, 0, 1, 3, 0]
    );
}

#[test]
fn nested_inputs_borrow_local_strings_slices_options_and_records() {
    let name = String::from("borrowed 🦀");
    let tags = vec![1u32, 256, u32::MAX];
    let records = [
        RecordInit {
            name: &name,
            tags: &tags,
        },
        RecordInit {
            name: "",
            tags: &[],
        },
    ];
    let init = MixedInit {
        name: "header",
        number: 9,
        records: &records,
        limits: Limits {
            enabled: false,
            limit: 32,
        },
        memo: Some(&name),
    };
    let bytes = encoded::<Mixed>(&init);
    let view = View::new(&bytes, Mixed::LAYOUT, 12).unwrap();
    let read = view.read::<Mixed>().unwrap();
    assert_eq!(read.name(), "header");
    let list = read.records();
    let record = list.get(0).unwrap();
    assert_eq!(record.name(), name);
    assert_eq!(record.tags().iter().collect::<Vec<_>>(), tags);
    assert_eq!(read.memo(), Some(name.as_str()));
}

#[test]
fn account_constants_and_measurement_distinguish_root_from_envelope() {
    let input = MyAccountInit {
        authority: blubber::Address::new_from_array([0; 32]),
        counter: 0,
        records: &[],
        balances: &[],
        labels: &[],
        note: "",
        attachment: &[],
        memo: None,
        limits: Limits {
            enabled: false,
            limit: 0,
        },
    };
    assert_eq!(
        MyAccount::DISCRIMINATOR,
        [47, 253, 130, 2, 67, 165, 139, 141]
    );
    assert_eq!(MyAccount::MIN_SIZE, 80);
    assert_eq!(MyAccount::MIN_ACCOUNT_LEN, 92);
    assert_eq!(MyAccount::measure(&input), Ok(80));
    assert_eq!(MyAccount::encoded_len_of(&input), Ok(92));
    assert_eq!(encoded::<MyAccount>(&input).len(), 80);
}

#[test]
fn explicit_and_raw_identifier_discriminators_are_exact() {
    #[blubber::schema(account, discriminator = [1, 2, 3, 4, 5, 6, 7, 255])]
    struct Explicit {
        value: u8,
    }
    #[allow(non_camel_case_types)]
    #[blubber::schema(account)]
    struct r#type {
        r#type: u8,
    }
    assert_eq!(Explicit::DISCRIMINATOR, [1, 2, 3, 4, 5, 6, 7, 255]);
    assert_eq!(Explicit::encoded_len_of(&ExplicitInit { value: 8 }), Ok(13));
    // D21 identity for the raw identifier namespace, computed independently.
    assert_eq!(r#type::DISCRIMINATOR, [107, 195, 33, 4, 35, 90, 51, 192]);
    assert_eq!(encoded::<r#type>(&typeInit { r#type: 1 }), [1]);
}

#[blubber::schema]
struct WithMaps {
    fixed: Map<i16, u8>,
    indexed: Map<u16, Str>,
}

#[test]
fn borrowed_maps_encode_both_value_layouts_without_sorting() {
    let init = WithMapsInit {
        fixed: &[(-1, 8), (1, 9)],
        indexed: &[(1, "a"), (256, "")],
    };
    assert_eq!(
        encoded::<WithMaps>(&init),
        [
            10, 0, 0, 0, // first child's end
            2, 0, 0, 0, 0x7f, 0xff, 8, 0x80, 1, 9, // fixed map
            2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, b'a', // indexed map
        ]
    );
}

#[test]
fn invalid_map_order_is_rejected_during_measurement_at_every_nesting_level() {
    for (entries, kind) in [
        ([(2, "a"), (2, "b")], ErrorKind::DuplicateKey),
        ([(2, "a"), (1, "b")], ErrorKind::UnsortedKeys),
    ] {
        let init = WithMapsInit {
            fixed: &[],
            indexed: &entries,
        };
        let expected = Err(Error::new(kind, Error::NO_OFFSET));
        assert_eq!(WithMaps::measure(&init), expected);
        assert_eq!(Option::<WithMaps>::measure(&Some(init)), expected);
        let values = [init];
        assert_eq!(List::<WithMaps>::measure(&values.as_slice()), expected);
    }
    let init = WithMapsInit {
        fixed: &[(1, 0), (-1, 0)],
        indexed: &[],
    };
    assert_eq!(
        WithMaps::measure(&init),
        Err(ErrorKind::UnsortedKeys.into())
    );
}

#[test]
fn empty_map_with_maximum_width_key_measures_without_unused_stride_overflow() {
    type HugeKeyMap = Map<[u8; u32::MAX as usize], Str>;
    assert_eq!(HugeKeyMap::measure(&[].as_slice()), Ok(4));
}

#[test]
fn aliases_qualified_types_and_generated_local_names_do_not_control_dispatch() {
    type Text = blubber::Str;
    type Count = u16;
    #[blubber::schema]
    struct Names {
        fixed: Text,
        directory: Count,
        payload: core::option::Option<Text>,
        end: Count,
        size: Count,
        init: Text,
        r#type: Text,
    }
    let init = NamesInit {
        fixed: "a",
        directory: 1,
        payload: None,
        end: 2,
        size: 3,
        init: "b",
        r#type: "c",
    };
    assert_eq!(
        encoded::<Names>(&init),
        [
            1, 0, 2, 0, 3, 0, // fixed fields
            1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, // directory
            b'a', 0, b'b', b'c',
        ]
    );
}

#[test]
fn removed_schema_values_validate_and_report_account_relative_faults() {
    let bytes = encoded::<FixedMarker>(&FixedMarkerInit {
        flag: false,
        count: 1,
    });
    assert_eq!(
        FixedMarker::read_removed(View::new(&bytes, FixedMarker::LAYOUT, 17).unwrap()),
        Ok(())
    );
    let bad = [2, 1, 0];
    assert_eq!(
        FixedMarker::read_removed(View::new(&bad, FixedMarker::LAYOUT, 17).unwrap()),
        Err(Error::new(ErrorKind::InvalidBool, 17))
    );
}

impl RecordInit<'_> {
    fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

#[test]
fn generated_inputs_are_copy_and_support_ordinary_inherent_helpers() {
    fn clone_input<T: Clone>(input: &T) -> T {
        input.clone()
    }

    let original = RecordInit {
        name: "x",
        tags: &[1, 2],
    };
    let copied = original;
    let cloned = clone_input(&original);
    assert_eq!(copied.tag_count(), 2);
    assert_eq!(encoded::<Record>(&original), encoded::<Record>(&cloned));
}

// A length-only zero-filled UTF-8 source exercises the full wire domain without
// allocating a multi-gigabyte input. It is not a supported application codec.
struct Zeroes;

impl Schema for Zeroes {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:zeroes",
                epoch: 0,
            },
            wire: &blubber::identity::WireDescriptor::Str,
        };
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static Layout = &Layout::Str;
    const __LEN: usize = <blubber::Str as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <blubber::Str as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a str;
    type Mut<'a> = blubber::raw::Edit<'a>;

    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(edit)
    }
    type Init<'a> = u32;
    type Removed = ();
    #[cfg(feature = "alloc")]
    type Owned = u32;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &u32) -> blubber::Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &u32, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: blubber::View<'_>) -> blubber::Result<u32> {
        view.validate()?;
        if view.bytes().iter().any(|byte| *byte != 0) {
            return Err(blubber::ErrorKind::InvalidBoundary.into());
        }
        blubber_core::checked_u32(view.bytes().len())
    }

    fn read<'a>(view: View<'a>) -> blubber::Result<Self::Read<'a>> {
        view.read::<Str>()
    }

    fn read_removed(view: View<'_>) -> blubber::Result<()> {
        view.validate()
    }

    fn measure(init: &u32) -> blubber::Result<u32> {
        Ok(*init)
    }

    fn encode(_: &u32, destination: &mut [u8]) {
        destination.fill(0);
    }
}

#[test]
fn measurement_rejects_full_width_root_and_account_overflow_before_encoding() {
    #[blubber::schema]
    struct Large {
        first: Zeroes,
        second: Zeroes,
    }
    #[blubber::schema(account)]
    struct LargeAccount {
        text: Zeroes,
    }
    let expected = Err(ErrorKind::ArithmeticOverflow.into());
    assert_eq!(
        Large::measure(&LargeInit {
            first: u32::MAX - 4,
            second: 1
        }),
        expected
    );
    assert_eq!(
        Large::measure(&LargeInit {
            first: u32::MAX - 4,
            second: 0
        }),
        Ok(u32::MAX)
    );
    assert_eq!(
        LargeAccount::measure(&LargeAccountInit { text: u32::MAX }),
        Ok(u32::MAX)
    );
    assert_eq!(
        LargeAccount::encoded_len_of(&LargeAccountInit { text: u32::MAX }),
        expected
    );
    assert_eq!(
        LargeAccount::encoded_len_of(&LargeAccountInit {
            text: u32::MAX - 12
        }),
        Ok(u32::MAX)
    );
}

#[test]
fn list_and_option_of_borrowed_records_use_exact_elided_extents() {
    let a = RecordInit {
        name: "",
        tags: &[],
    };
    let b = RecordInit {
        name: "x",
        tags: &[7],
    };
    assert_eq!(encoded::<Option<Record>>(&None), [0]);
    assert_eq!(
        encoded::<Option<Record>>(&Some(a)),
        [1, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(encoded::<List<Record>>(&[].as_slice()), [0; 4]);
    let one = [a];
    let two = [a, b];
    assert_eq!(
        encoded::<List<Record>>(&one.as_slice()),
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        encoded::<List<Record>>(&two.as_slice()),
        [
            2, 0, 0, 0, 8, 0, 0, 0, // count, first record end
            0, 0, 0, 0, 0, 0, 0, 0, // empty record
            1, 0, 0, 0, b'x', 1, 0, 0, 0, 7, 0, 0, 0, // nonempty record
        ]
    );
}

#[blubber::schema]
struct RouteWrapper<T: Schema> {
    value: T,
}

static ROUTE_MEASUREMENTS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

macro_rules! route_codec {
    ($name:ident, $measurement:block, {$($marker:item)*}) => {
        struct $name;
        impl Schema for $name {
            $($marker)*
            const KIND: Kind = Kind::Fixed;
            const FIXED_SIZE: Option<u32> = Some(4);
            const MIN_SIZE: u32 = 4;
            const LAYOUT: &'static Layout = <u32 as Schema>::LAYOUT;
            const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
                <u32 as Schema>::WIRE_DESCRIPTOR;
            const __LEN: usize = <u32 as Schema>::__LEN;
            const __HEADER: &'static [u32] = <u32 as Schema>::__HEADER;
            type __Children = blubber::__private::Nil;
            type Read<'a> = u32;
            type Mut<'a> = blubber::raw::Edit<'a>;
            type Init<'a> = u32;
            type Removed = u32;
            fn project<'a>(value: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
                blubber::raw::into_edit(value)
            }
            fn read(view: View<'_>) -> blubber::Result<u32> { view.read_fixed::<u32>() }
            fn read_removed(view: View<'_>) -> blubber::Result<u32> { Self::read(view) }
            fn measure(_: &u32) -> blubber::Result<u32> { $measurement }
            fn encode(value: &u32, destination: &mut [u8]) {
                destination.copy_from_slice(&value.to_le_bytes());
            }
            blubber::__private::with_alloc! {
                type Owned = u32;
                fn measure_owned(value: &u32) -> blubber::Result<u32> { Self::measure(value) }
                fn encode_owned(value: &u32, destination: &mut [u8]) {
                    Self::encode(value, destination);
                }
                fn decode_owned(view: View<'_>) -> blubber::Result<u32> { Self::read(view) }
            }
        }
    };
}
route_codec!(
    DefaultRouteCodec,
    {
        ROUTE_MEASUREMENTS.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        Ok(4)
    },
    {}
);
route_codec!(ContractRouteCodec, { Ok(4) }, {
    const __ENCODER_AUDITED: bool = true;
    const __MEASURE_REUSABLE: bool = true;
});
route_codec!(EncoderOnlyRouteCodec, { Ok(4) }, {
    const __ENCODER_AUDITED: bool = true;
});

route_codec!(MeasurementOnlyRouteCodec, { Ok(4) }, {
    const __MEASURE_REUSABLE: bool = true;
});

macro_rules! route_key {
    ($name:ident, {$($fixed:item)*}, {$($key:item)*}) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        struct $name(u32);
        impl blubber::Fixed for $name {
            $($fixed)*
            const SIZE: u32 = 4;
            type Encoded = [u8; 4];
            fn decode(bytes: &[u8]) -> blubber::Result<Self> {
                <u32 as blubber::Fixed>::decode(bytes).map(Self)
            }
            fn encode(self) -> Self::Encoded { self.0.to_le_bytes() }
        }
        impl blubber::MapKey for $name {
            $($key)*
            const KEY_LAYOUT: blubber_core::KeyLayout =
                <u32 as blubber::MapKey>::KEY_LAYOUT;
            type KeyBytes = [u8; 4];
            fn encode_key(self) -> Self::KeyBytes { self.0.to_be_bytes() }
            fn decode_key(bytes: &[u8]) -> blubber::Result<Self> {
                <u32 as blubber::MapKey>::decode_key(bytes).map(Self)
            }
        }
    };
}
route_key!(DefaultRouteKey, {}, {});
route_key!(
    ContractRouteKey,
    {
        const __ENCODER_AUDITED: bool = true;
    },
    {
        const __KEY_ENCODER_AUDITED: bool = true;
        const __ORDER_REUSABLE: bool = true;
    }
);

route_key!(EncoderOnlyRouteKey, {}, {
    const __KEY_ENCODER_AUDITED: bool = true;
});
route_key!(OrderOnlyRouteKey, {}, {
    const __ORDER_REUSABLE: bool = true;
});

#[test]
fn marker_routes_follow_generic_wrappers_aliases_and_keys_before_hooks() {
    use blubber::__private::{
        encoder_route,
        EncoderRoute::{Checked, Contract},
    };
    fn generic<T: Schema>() -> blubber::__private::EncoderRoute {
        encoder_route::<T>()
    }
    type Alias = u32;
    type Graph<T> = RouteWrapper<List<Option<RouteWrapper<T>>>>;
    const ALIAS_ROUTE: blubber::__private::EncoderRoute = encoder_route::<Alias>();
    assert_eq!(ALIAS_ROUTE, Contract);
    assert_eq!(generic::<Graph<Alias>>(), Contract);
    assert_eq!(generic::<Graph<Limits>>(), Contract);
    assert_eq!(generic::<RouteWrapper<Map<u8, Graph<Alias>>>>(), Contract);
    ROUTE_MEASUREMENTS.store(0, core::sync::atomic::Ordering::SeqCst);
    assert_eq!(generic::<DefaultRouteCodec>(), Checked);
    assert_eq!(generic::<Graph<DefaultRouteCodec>>(), Checked);
    assert_eq!(
        generic::<RouteWrapper<Map<u8, DefaultRouteCodec>>>(),
        Checked
    );
    assert_eq!(generic::<Graph<EncoderOnlyRouteCodec>>(), Checked);
    assert_eq!(generic::<Graph<MeasurementOnlyRouteCodec>>(), Checked);
    assert_eq!(generic::<Map<EncoderOnlyRouteKey, Alias>>(), Checked);
    assert_eq!(generic::<Map<OrderOnlyRouteKey, Alias>>(), Checked);
    assert_eq!(generic::<Map<DefaultRouteKey, Alias>>(), Checked);
    assert_eq!(
        ROUTE_MEASUREMENTS.load(core::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(generic::<ContractRouteCodec>(), Contract);
    assert_eq!(generic::<Graph<ContractRouteCodec>>(), Contract);
    assert_eq!(
        generic::<Map<ContractRouteKey, ContractRouteCodec>>(),
        Contract
    );
    let mut output = [0; 4];
    ContractRouteCodec::encode(&42, &mut output);
    assert_eq!(ContractRouteCodec::measure(&42).unwrap(), 4);
    assert_eq!(output, [42, 0, 0, 0]);
}

#[test]
fn borrowed_encoder_composition_and_loans_preserve_sources_and_frames() {
    use blubber_core::encoding::{bytes, encode_frame, encode_pair};
    let data = [3u8, 5, 7, 11];
    for left_len in 0..=data.len() {
        for right_len in 0..=data.len() {
            let left = bytes(&data[..left_len]);
            let right = bytes(&data[..right_len]);
            let mut first = [0xA5; 8];
            let mut second = [0x5A; 8];
            let n = left_len + right_len;
            encode_pair(&left, &right, &mut first[..n]);
            encode_pair(&left, &right, &mut second[..n]);
            let expected = [&data[..left_len], &data[..right_len]].concat();
            assert_eq!(&first[..n], expected);
            assert_eq!(&second[..n], expected);
            assert!(first[n..].iter().all(|b| *b == 0xA5));
            assert!(second[n..].iter().all(|b| *b == 0x5A));
            for start in 0..=8 - left_len {
                let mut framed = [0xC3; 8];
                let mut oracle = framed;
                oracle[start..start + left_len].copy_from_slice(&data[..left_len]);
                encode_frame(&left, &mut framed, start);
                assert_eq!(framed, oracle);
            }
        }
    }
    assert_eq!(data, [3, 5, 7, 11]);
}

#[blubber::schema]
struct EqualWidthFields {
    first: u32,
    second: u32,
}

#[test]
fn exchanged_equal_width_projections_validate_but_fail_the_golden_oracle() {
    use blubber::__private::{product, Pair};
    type Fields = Pair<product::Field<u32>, product::Field<u32>>;
    let init = EqualWidthFieldsInit {
        first: 0x0102_0304,
        second: 0xA1B2_C3D4,
    };
    let golden = [4, 3, 2, 1, 0xD4, 0xC3, 0xB2, 0xA1];
    let mut normal = [0; 8];
    EqualWidthFields::encode(&init, &mut normal);
    assert_eq!(normal, golden);

    // Both projections have the same type and descriptor. Validation checks
    // wire validity; the independent oracle checks declaration-to-slot mapping.
    let exchanged = Pair {
        left: product::borrowed::<u32>(&init.second),
        right: product::borrowed::<u32>(&init.first),
    };
    let mut output = [0x19; 8];
    assert_eq!(product::measure::<Fields>(&exchanged), Ok(8));
    product::encode::<Fields>(&exchanged, &mut output);
    assert_eq!(
        validate(&output, EqualWidthFields::LAYOUT, MAX_DEPTH),
        Ok(())
    );
    assert_ne!(output, golden);
    assert_eq!(output, [0xD4, 0xC3, 0xB2, 0xA1, 4, 3, 2, 1]);
    const DESCRIPTOR: blubber::identity::WireDescriptor =
        blubber::identity::WireDescriptor::Struct(&product::wire::<Fields, 2>());
    assert_eq!(
        EqualWidthFields::WIRE_DESCRIPTOR.encode::<65>(),
        DESCRIPTOR.encode::<65>()
    );
}

struct ChangingWidth;
struct ChangingWidthState {
    measured: core::cell::Cell<u8>,
    encoded: core::cell::Cell<u8>,
    fresh_error: Option<Error>,
}

impl Schema for ChangingWidth {
    const KIND: Kind = Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static Layout = &Layout::Leaf(blubber_core::Leaf::Raw(1));
    const __LEN: usize = 8;
    const __HEADER: &'static [u32] = &[4, 1, 1, 0, 0, 0, 0, 0];
    type __Children = blubber::__private::Nil;
    type Read<'a> = u8;
    type Mut<'a> = ();
    type Init<'a> = &'a ChangingWidthState;
    type Removed = u8;
    fn read<'a>(view: View<'a>) -> blubber::Result<u8> {
        view.read_fixed::<u8>()
    }
    fn read_removed(view: View<'_>) -> blubber::Result<u8> {
        view.read_fixed::<u8>()
    }
    fn project(_: blubber::__private::Projection<'_, Self>) {}
    fn measure(value: &Self::Init<'_>) -> blubber::Result<u32> {
        let calls = value.measured.get();
        value.measured.set(calls + 1);
        if calls == 0 {
            Ok(1)
        } else if let Some(error) = value.fresh_error {
            Err(error)
        } else {
            Ok(2)
        }
    }
    fn encode(value: &Self::Init<'_>, destination: &mut [u8]) {
        value.encoded.set(value.encoded.get() + 1);
        destination.fill(0xFE);
    }
    blubber::__private::with_alloc! {
        type Owned = ChangingWidthState;
        fn measure_owned(value: &Self::Owned) -> blubber::Result<u32> { Self::measure(&value) }
        fn encode_owned(value: &Self::Owned, destination: &mut [u8]) { Self::encode(&value, destination); }
        fn decode_owned(_: View<'_>) -> blubber::Result<Self::Owned> { Err(ErrorKind::InvalidSchema.into()) }
    }
}

#[blubber::schema]
struct FixedLoanFrame {
    first: ChangingWidth,
    tail: Str,
}

#[test]
fn opaque_fresh_fixed_width_rejects_before_codec_calls_or_frame_changes() {
    for error in [None, Some(Error::new(ErrorKind::InvalidBoundary, 7))] {
        let state = ChangingWidthState {
            measured: core::cell::Cell::new(0),
            encoded: core::cell::Cell::new(0),
            fresh_error: error,
        };
        assert_eq!(
            blubber::__private::encoder_route::<FixedLoanFrame>(),
            blubber::__private::EncoderRoute::Checked
        );
        assert_eq!(state.measured.get(), 0);
        let input = FixedLoanFrameInit {
            first: &state,
            tail: "abcd",
        };
        assert_eq!(FixedLoanFrame::measure(&input), Ok(5));
        let mut output = [0xA5; 7];
        let before = output;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            FixedLoanFrame::encode(&input, &mut output[1..6]);
        }))
        .expect_err("the fresh fixed child cannot exceed its prefix loan");
        assert_eq!(state.measured.get(), 2);
        assert_eq!(state.encoded.get(), 0);
        assert_eq!(output, before);
        if let Some(error) = error {
            assert_eq!(
                panic.downcast_ref::<String>().unwrap(),
                &format!("previously measured schema input: {error:?}")
            );
        }
    }
}

#[test]
fn list_writers_preserve_exact_oracles_frames_and_repeated_measurements() {
    fn check<S: Schema>(input: &S::Init<'_>, expected: &[u8]) {
        let length = expected.len();
        assert_eq!(S::measure(input), Ok(length as u32));
        assert_eq!(S::measure(input), Ok(length as u32));
        for history in [0xA5, 0x5A] {
            let mut bytes = vec![history; length + 6];
            S::encode(input, &mut bytes[3..3 + length]);
            assert_eq!(&bytes[3..3 + length], expected);
            assert!(bytes[..3]
                .iter()
                .chain(&bytes[3 + length..])
                .all(|b| *b == history));
            assert_eq!(
                validate(&bytes[3..3 + length], S::LAYOUT, MAX_DEPTH),
                Ok(())
            );
        }
    }
    let values: Vec<u16> = (0..17).map(|n| n * 257).collect();
    for count in 0..=values.len() {
        let mut expected = (count as u32).to_le_bytes().to_vec();
        for value in &values[..count] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        check::<List<u16>>(&&values[..count], &expected);
    }
    let values = ["", "\u{80}", "", "abc"];
    for count in 0..=values.len() {
        let mut expected = (count as u32).to_le_bytes().to_vec();
        let mut end = 0u32;
        for value in values.iter().take(count.saturating_sub(1)) {
            end += value.len() as u32;
            expected.extend_from_slice(&end.to_le_bytes());
        }
        for value in &values[..count] {
            expected.extend_from_slice(value.as_bytes());
        }
        check::<List<Str>>(&&values[..count], &expected);
        #[cfg(feature = "alloc")]
        {
            let owned = values[..count].iter().map(|v| (*v).to_owned()).collect();
            assert_eq!(
                List::<Str>::measure_owned(&owned),
                Ok(expected.len() as u32)
            );
            let mut bytes = vec![0xA5; expected.len() + 2];
            List::<Str>::encode_owned(&owned, &mut bytes[1..1 + expected.len()]);
            assert_eq!(&bytes[1..1 + expected.len()], expected);
            assert_eq!((bytes[0], bytes[1 + expected.len()]), (0xA5, 0xA5));
        }
    }
}

#[blubber::schema]
struct GenericList<T: Schema> {
    values: List<T>,
}

#[test]
fn nested_generic_list_writers_retain_depth_and_empty_child_partitions() {
    type Alias = GenericList<GenericList<Str>>;
    let first = GenericListInit { values: &["", "x"] };
    let second = GenericListInit { values: &[] };
    let input = GenericListInit {
        values: &[first, second],
    };
    let expected = [
        2, 0, 0, 0, 9, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, b'x', 0, 0, 0, 0,
    ];
    assert_eq!(Alias::measure(&input), Ok(expected.len() as u32));
    let mut bytes = [0xA5; 23];
    Alias::encode(&input, &mut bytes[1..22]);
    assert_eq!(&bytes[1..22], expected);
    assert_eq!((bytes[0], bytes[22]), (0xA5, 0xA5));
    assert_eq!(validate(&bytes[1..22], Alias::LAYOUT, MAX_DEPTH), Ok(()));
}

#[test]
fn opaque_list_first_error_preserves_unvisited_calls_and_element_frames() {
    for error in [None, Some(Error::new(ErrorKind::InvalidBoundary, 7))] {
        let states = [0, 1].map(|_| ChangingWidthState {
            measured: core::cell::Cell::new(0),
            encoded: core::cell::Cell::new(0),
            fresh_error: error,
        });
        let values = [&states[0], &states[1]];
        let input = &values[..];
        assert_eq!(
            blubber::__private::encoder_route::<GenericList<ChangingWidth>>(),
            blubber::__private::EncoderRoute::Checked
        );
        assert!(states.iter().all(|s| s.measured.get() == 0));
        assert_eq!(List::<ChangingWidth>::measure(&input), Ok(6));
        let mut bytes = [0xA5; 8];
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            List::<ChangingWidth>::encode(&input, &mut bytes[1..7]);
        }))
        .expect_err("fresh child measurement must reject before its encoding");
        assert_eq!(bytes, [0xA5, 2, 0, 0, 0, 0xA5, 0xA5, 0xA5]);
        assert_eq!((states[0].measured.get(), states[1].measured.get()), (2, 1));
        assert!(states.iter().all(|s| s.encoded.get() == 0));
        if let Some(error) = error {
            assert_eq!(
                panic.downcast_ref::<String>().unwrap(),
                &format!("previously measured list input: {error:?}")
            );
        }
    }
}
