//! Metadata, source validation and measurement-route controls.
use super::*;
use core::cell::Cell;

std::thread_local! {
    static MAP_WIDTH: Cell<Option<Option<u32>>> = const { Cell::new(None) };
    static MEASURE_CALLS: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn record_map_width(width: Option<u32>) {
    MAP_WIDTH.with(|observed| observed.set(Some(width)));
}

#[test]
fn schema_metadata_depth_counts_every_container() {
    assert_eq!(Str::__DEPTH, 1);
    assert_eq!(crate::List::<Str>::__DEPTH, 2);
    assert_eq!(crate::List::<crate::List<Str>>::__DEPTH, 3);
    assert_eq!(crate::Map::<u8, crate::List<Str>>::__DEPTH, 3);
}

#[test]
fn schema_depth_limit_rejects_deeper_containers() {
    type N1 = crate::List<Str>;
    type N2 = crate::List<N1>;
    type N3 = crate::List<N2>;
    type N4 = crate::List<N3>;
    type N5 = crate::List<N4>;
    type N6 = crate::List<N5>;
    type N7 = crate::List<N6>;
    type N8 = crate::List<N7>;
    type N9 = crate::List<N8>;
    type N10 = crate::List<N9>;
    type N11 = crate::List<N10>;
    type N12 = crate::List<N11>;
    type N13 = crate::List<N12>;
    type N14 = crate::List<N13>;
    type N15 = crate::List<N14>;
    type N16 = crate::List<N15>;
    assert_eq!(check_depth::<N7>(), Ok(()));
    assert_eq!(N16::__DEPTH, 17);
    assert_eq!(check_depth::<N16>(), Err(ErrorKind::InvalidSchema.into()));
}

#[test]
fn variable_schema_rejects_fixed_initializer_read() {
    let view = View::new(b"valid", Str::LAYOUT, 31).unwrap();
    assert_eq!(
        Str::__read_fixed(view),
        Err(ErrorKind::InvalidSchema.into())
    );
}

#[test]
fn removed_list_validates_source_values() {
    for (byte, expected) in [
        (1, Ok(())),
        (2, Err(Error::new(ErrorKind::InvalidBool, 45))),
    ] {
        let bytes = [1, 0, 0, 0, byte];
        let view = View::new(&bytes, crate::List::<bool>::LAYOUT, 41).unwrap();
        assert_eq!(crate::List::<bool>::read_removed(view), expected);
    }
}

#[test]
fn removed_map_validates_source_values() {
    for (byte, expected) in [
        (1, Ok(())),
        (2, Err(Error::new(ErrorKind::InvalidBool, 46))),
    ] {
        let bytes = [1, 0, 0, 0, 1, byte];
        let view = View::new(&bytes, crate::Map::<u8, bool>::LAYOUT, 41).unwrap();
        assert_eq!(crate::Map::<u8, bool>::read_removed(view), expected);
    }
    let bytes = [1, 0, 0, 0, 2, 7];
    let view = View::new(&bytes, crate::Map::<bool, u8>::LAYOUT, 41).unwrap();
    assert_eq!(
        crate::Map::<bool, u8>::read_removed(view),
        Err(Error::new(ErrorKind::InvalidBool, 45))
    );
}

#[test]
fn map_measure_keeps_primitive_fixed_width_route() {
    MAP_WIDTH.with(|width| width.set(None));
    assert_eq!(
        crate::Map::<u8, u16>::measure(&&[(1, 7), (3, 9)][..]),
        Ok(10)
    );
    MAP_WIDTH.with(|width| assert_eq!(width.get(), Some(Some(2))));
}

// This handwritten fixed codec validates its caller's u8 initializer. Its
// default-false reusable marker keeps those calls even at a constant wire size.
struct MeasuredBool;

impl Schema for MeasuredBool {
    const KIND: Kind = Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static Layout = <bool as Schema>::LAYOUT;
    const __LEN: usize = <bool as Schema>::__LEN;
    const __HEADER: &'static [u32] = <bool as Schema>::__HEADER;
    type __Children = crate::__private::Nil;
    type Read<'a> = bool;
    type Mut<'a> = ();
    type Init<'a> = u8;
    type Removed = bool;

    fn project(_: crate::__private::Projection<'_, Self>) {}
    fn read(view: View<'_>) -> Result<bool> {
        <bool as Schema>::read(view)
    }
    fn read_removed(view: View<'_>) -> Result<bool> {
        <bool as Schema>::read_removed(view)
    }
    fn measure(value: &u8) -> Result<u32> {
        MEASURE_CALLS.with(|calls| calls.set(calls.get() + 1));
        if *value > 1 {
            Err(ErrorKind::InvalidBool.into())
        } else {
            Ok(1)
        }
    }
    fn encode(value: &u8, destination: &mut [u8]) {
        destination[0] = *value;
    }
    #[cfg(feature = "alloc")]
    type Owned = u8;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &u8) -> Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &u8, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<u8> {
        Self::read(view).map(u8::from)
    }
}

#[test]
fn map_measure_visits_nonreusable_fixed_values() {
    for (value, expected) in [(1, Ok(6)), (2, Err(ErrorKind::InvalidBool.into()))] {
        MEASURE_CALLS.with(|calls| calls.set(0));
        MAP_WIDTH.with(|width| width.set(None));
        assert_eq!(
            crate::Map::<u8, MeasuredBool>::measure(&&[(1, value)][..]),
            expected
        );
        MEASURE_CALLS.with(|calls| assert_eq!(calls.get(), 1));
        MAP_WIDTH.with(|width| assert_eq!(width.get(), Some(None)));
    }
}

#[test]
fn collection_markers_format_names_without_child_traits() {
    let list = List::<MeasuredBool>(PhantomData);
    let map = Map::<u8, MeasuredBool>(PhantomData);
    assert!(std::format!("{list:?}").contains("List"));
    assert!(std::format!("{map:?}").contains("Map"));
}
