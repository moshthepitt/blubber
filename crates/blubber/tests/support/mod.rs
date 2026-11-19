//! Handwritten schemas for integration tests.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use blubber::{ErrorKind, Kind, List, Result, Schema, Str, View};
use blubber_core::{FixedField, Layout, Leaf, VariableField};

pub const DISCRIMINATOR: [u8; 8] = *b"readers1";

pub fn encoded<S: Schema>(init: &S::Init<'_>) -> Vec<u8> {
    let mut bytes = vec![0; S::measure(init).unwrap() as usize];
    S::encode(init, &mut bytes);
    bytes
}

pub fn account<S: Schema>(init: &S::Init<'_>) -> Vec<u8> {
    let root = encoded::<S>(init);
    let used = (root.len() + 12) as u32;
    let mut bytes = DISCRIMINATOR.to_vec();
    bytes.extend_from_slice(&used.to_le_bytes());
    bytes.extend_from_slice(&root);
    bytes.extend_from_slice(&[0xff; 7]);
    bytes
}

pub fn read<S: Schema>(bytes: &[u8]) -> Result<S::Read<'_>> {
    View::new(bytes, S::LAYOUT, 0)?.read::<S>()
}

// Declaration order differs from packed wire order. This descriptor exercises
// a fixed prefix, two variable fields and a stored/implicit end pair.
#[derive(Debug)]
pub struct Record;
#[derive(Clone, Copy, Debug)]
pub struct RecordRef<'a>(pub View<'a>);

impl<'a> RecordRef<'a> {
    pub fn try_name(self) -> Result<&'a str> {
        self.0.field(0)?.read::<Str>()
    }
    pub fn name(self) -> &'a str {
        self.try_name().unwrap()
    }
    pub fn tags(self) -> blubber::ListRef<'a, u16> {
        self.0.field(2).unwrap().read::<List<u16>>().unwrap()
    }
    pub fn enabled(self) -> Result<bool> {
        self.0.field(1)?.read::<bool>()
    }
}

impl Schema for Record {
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 9;
    const LAYOUT: &'static Layout = &Layout::Struct {
        fixed_bytes: 1,
        fixed: &[FixedField {
            declaration: 1,
            offset: 0,
            layout: &Layout::Leaf(Leaf::Bool),
        }],
        variable: &[
            VariableField {
                declaration: 0,
                layout: <Str as Schema>::LAYOUT,
            },
            VariableField {
                declaration: 2,
                layout: <List<u16> as Schema>::LAYOUT,
            },
        ],
    };
    const __LEN: usize = 72;
    const __HEADER: &'static [u32] = &[9, 1, 0, 1, 2, 0, 0, 0, 0, 0, 1, 0, 2, 0];
    type __Children = blubber::__private::Cons<
        Str,
        blubber::__private::Cons<
            bool,
            blubber::__private::Cons<List<u16>, blubber::__private::Nil>,
        >,
    >;
    type Read<'a> = RecordRef<'a>;
    type Mut<'a> = blubber::raw::Edit<'a>;

    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(edit)
    }
    type Init<'a> = (&'a str, bool, &'a [u16]);
    type Removed = ();
    #[cfg(feature = "alloc")]
    type Owned = (String, bool, Vec<u16>);
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32> {
        Self::measure(&(value.0.as_str(), value.1, &value.2))
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        Self::encode(&(value.0.as_str(), value.1, &value.2), destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned> {
        view.validate()?;
        Ok((
            Str::decode_owned(view.field(0)?)?,
            bool::decode_owned(view.field(1)?)?,
            List::<u16>::decode_owned(view.field(2)?)?,
        ))
    }

    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
        Ok(RecordRef(view))
    }
    fn read_removed(view: View<'_>) -> Result<()> {
        view.validate()
    }

    fn measure(init: &Self::Init<'_>) -> Result<u32> {
        let name = Str::measure(&init.0)?;
        let tags = List::<u16>::measure(&init.2)?;
        5u32.checked_add(name)
            .and_then(|size| size.checked_add(tags))
            .ok_or(ErrorKind::ArithmeticOverflow.into())
    }

    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        destination[0] = u8::from(init.1);
        destination[1..5].copy_from_slice(&(init.0.len() as u32).to_le_bytes());
        let (name, tags) = destination[5..].split_at_mut(init.0.len());
        Str::encode(&init.0, name);
        List::<u16>::encode(&init.2, tags);
    }
}

// A native fixed-kind view schema without a Copy bound on its marker. This
// tests fixed-stride dispatch separately from native scalar return types.
#[derive(Debug)]
pub struct Pair;
impl Schema for Pair {
    const KIND: Kind = Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(3);
    const MIN_SIZE: u32 = 3;
    const LAYOUT: &'static Layout = &Layout::Struct {
        fixed_bytes: 3,
        fixed: &[
            FixedField {
                declaration: 0,
                offset: 0,
                layout: <u16 as Schema>::LAYOUT,
            },
            FixedField {
                declaration: 1,
                offset: 2,
                layout: <bool as Schema>::LAYOUT,
            },
        ],
        variable: &[],
    };
    const __LEN: usize = 40;
    const __HEADER: &'static [u32] = &[9, 3, 0, 2, 0, 0, 0, 0, 0, 0, 1, 2];
    type __Children =
        blubber::__private::Cons<u16, blubber::__private::Cons<bool, blubber::__private::Nil>>;
    type Read<'a> = View<'a>;
    type Mut<'a> = blubber::raw::Edit<'a>;

    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(edit)
    }
    type Init<'a> = (u16, bool);
    type Removed = ();
    #[cfg(feature = "alloc")]
    type Owned = (u16, bool);
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned> {
        view.validate()?;
        Ok((
            view.field(0)?.read::<u16>()?,
            view.field(1)?.read::<bool>()?,
        ))
    }

    fn read<'a>(view: View<'a>) -> Result<View<'a>> {
        Ok(view)
    }
    fn read_removed(view: View<'_>) -> Result<()> {
        view.validate()
    }
    fn measure(_: &Self::Init<'_>) -> Result<u32> {
        Ok(3)
    }
    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        destination[..2].copy_from_slice(&init.0.to_le_bytes());
        destination[2] = u8::from(init.1);
    }
}

impl blubber::RootSchema for Record {
    fn __layout() -> blubber_core::CheckedLayout<'static> {
        blubber::__layout_of!(Record)
    }
}
