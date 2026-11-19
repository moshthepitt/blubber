//! A borrowed handwritten domain exercises the checked default region reader.
#![forbid(unsafe_code)]
#![allow(dead_code)]

use blubber::{Error, ErrorKind, Kind, Result, Schema, View};

/// A one-byte domain that reserves 255 and returns the original byte borrow.
pub struct Domain;
impl Schema for Domain {
    const KIND: Kind = Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = 8;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a [u8];
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = u8;
    type Removed = ();
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read(view: View<'_>) -> Result<Self::Read<'_>> {
        if view.read_fixed::<u8>()? == 255 {
            return Err(Error::new(ErrorKind::InvalidTag, view.base()));
        }
        Ok(view.bytes())
    }
    fn read_removed(view: View<'_>) -> Result<()> {
        Self::read(view).map(|_| ())
    }
    fn measure(value: &u8) -> Result<u32> {
        if *value == 255 {
            return Err(ErrorKind::InvalidTag.into());
        }
        Ok(1)
    }
    fn encode(value: &u8, bytes: &mut [u8]) {
        bytes.copy_from_slice(&[*value]);
    }
    blubber::__private::with_alloc! {
        type Owned = u8;
        fn measure_owned(value: &u8) -> Result<u32> { Self::measure(value) }
        fn encode_owned(value: &u8, bytes: &mut [u8]) { Self::encode(value, bytes); }
        fn decode_owned(view: View<'_>) -> Result<u8> { Ok(Self::read(view)?[0]) }
    }
}
