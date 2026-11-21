//! Schema metadata, checked readers and deterministic borrowed-input encoders.

use core::marker::PhantomData;

use blubber_core::{checked_u32, Layout, Leaf, MAX_DEPTH};

use crate::{Address, Error, ErrorKind, Fixed, MapKey, Result, View};

#[cfg(test)]
#[path = "schema_work.rs"]
mod work;

/// Whether a schema occupies a fixed width or an exact parent-delimited region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Every value has the same positive encoded width.
    Fixed,
    /// Encoded width depends on the value.
    Variable,
}

/// Wire metadata, checked reads and borrowed encoding for a format-1 type.
///
/// Generic derived structs and collections can be nested in a concrete root.
/// Root entry points require [`RootSchema`], implemented by concrete derives.
/// Provided borrowed encoders are deterministic, replayable and allocation-free.
/// Owned decoding, enabled by `alloc`, reports allocation failures explicitly.
/// Handwritten implementations are outside the functional assurance claim;
/// consumers must still check ranges and validate bytes before typed reads.
pub trait Schema: Sized + 'static {
    /// Selects the encoder contract when the complete child graph opts in.
    ///
    /// A handwritten true value is a trusted implementer contract, in the same
    /// class as derive projection glue: pure deterministic measurement, exact
    /// encoding into the measured destination, schema-valid output and replay.
    /// It is not a proof about that implementation. A violation may cause
    /// errors, poison or wrong data in that program's own accounts, never
    /// undefined behavior: all library paths remain safe Rust.
    /// Built-ins are true; derives AND every child/key contract. Default false
    /// propagates the checked route to every parent before any new hook runs.
    /// Covered lazy native initialization omits the post-write full scan.
    /// Checked policy and `verify-on-load` retain it.
    #[doc(hidden)]
    const __ENCODER_AUDITED: bool = false;
    /// Permits deterministic measurement replay under the complete graph contract.
    ///
    /// A handwritten true value is a trusted implementer contract, in the same
    /// class as derive projection glue: pure deterministic measurement, exact
    /// encoding into the measured destination, schema-valid output and replay.
    /// It is not a proof about that implementation. A violation may cause
    /// errors, poison or wrong data in that program's own accounts, never
    /// undefined behavior: all library paths remain safe Rust.
    /// Measurements include every child and key comparison; fixed successful
    /// measurements return `FIXED_SIZE`. Default false propagates through
    /// parents. Covered native replay and fixed-child replay are omitted;
    /// initial measurement and variable-child sizing remain checked.
    #[doc(hidden)]
    const __MEASURE_REUSABLE: bool = false;
    /// Grants structural field projection for every declared ordinal mapping.
    ///
    /// A handwritten true value is a trusted implementer contract, in the same
    /// class as derive projection glue: pure deterministic measurement, exact
    /// encoding into the measured destination, schema-valid output and replay.
    /// It is not a proof about that implementation. A violation may cause
    /// errors, poison or wrong data in that program's own accounts, never
    /// undefined behavior: all library paths remain safe Rust.
    /// Mappings additionally preserve declaration order and field identity.
    /// This projection marker alone does not select the encoder contract route;
    /// both encoder and measurement graph markers are required. Native fixed
    /// derives use whole-value editors and keep this marker false.
    #[doc(hidden)]
    const __PROJECT_FIELDS: bool = false;
    /// Stable codec contract used by automatic account identity generation.
    ///
    /// Built-ins and derives supply this independently of optimization tables.
    /// Handwritten codecs must explicitly supply a stable `Custom` descriptor
    /// before they can be nested in an automatically identified account.
    const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor =
        &crate::identity::WireDescriptor::Unspecified;
    /// Selects fixed-stride or indexed collection storage.
    const KIND: Kind;
    /// Exact positive width for fixed kinds; `None` for variable kinds.
    const FIXED_SIZE: Option<u32>;
    /// Smallest valid encoding, including tags or count words.
    const MIN_SIZE: u32;
    /// Recursive descriptor for raw validation, generic fields and CTFE checks.
    /// Typed roots and inherited views use the certified integer table.
    #[doc(hidden)]
    const LAYOUT: &'static Layout;
    /// Total words in this subtree, used only for concrete const construction.
    #[doc(hidden)]
    const __LEN: usize;
    /// Maximum metadata depth, including inactive children.
    #[doc(hidden)]
    const __DEPTH: usize = <Self::__Children as crate::__private::SchemaList>::DEPTH + 1;
    /// CTFE-only header and declaration-ordered field ordinal/offset pairs.
    #[doc(hidden)]
    const __HEADER: &'static [u32];
    /// CTFE-only child schema sequence.
    #[doc(hidden)]
    type __Children: crate::__private::SchemaList;
    /// Copied value or immutable view returned by checked reads.
    type Read<'a>;
    /// Exclusive editor produced by consuming an account or exact-region selection.
    type Mut<'a>;
    /// Borrowed input accepted by the encoder.
    type Init<'a>;
    /// Copied old value, or `()` when removal discards its bytes.
    type Removed;

    /// Copies a fixed-kind value into its lifetime-independent initializer.
    ///
    /// Generated account metadata uses this only for fixed fields. Variable
    /// schemas and unsupported handwritten codecs return `InvalidSchema`.
    #[doc(hidden)]
    fn __read_fixed(_view: View<'_>) -> Result<Self::Init<'static>> {
        Err(ErrorKind::InvalidSchema.into())
    }

    /// Reads an exact region, checking domains before creating references.
    ///
    /// Errors already carry account-relative offsets from the supplied view.
    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>>;

    /// Reads one admitted exact chunk with the same result as [`Self::read`].
    ///
    /// The collection has checked the inherited descriptor and that `base` plus
    /// the chunk length fits `u32`. The default retains the ordinary checked
    /// view reader, including handwritten domains and borrowed result lifetimes.
    /// Implementations must preserve its errors and rebase codec faults once.
    #[doc(hidden)]
    #[inline(always)]
    fn read_region<'a>(bytes: &'a [u8], layout: Layout, base: u32) -> Result<Self::Read<'a>> {
        Self::read(View::inherited(bytes, layout, base)?)
    }

    /// Whether the region reader needs the inherited descriptor.
    ///
    /// False promises that `read_region` ignores its layout argument. The
    /// reader may then supply `Self::LAYOUT` after checking the actual stride.
    /// The default preserves descriptors for handwritten and borrowed readers.
    #[doc(hidden)]
    const __READ_REGION_NEEDS_LAYOUT: bool = true;

    /// Wraps the supplied exclusive selection without reading its bytes.
    fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a>;

    /// Copies a native fixed value, or validates a discarded encoded value.
    fn read_removed(view: View<'_>) -> Result<Self::Removed>;

    /// Checks metadata, ordering and representable sizes before any write.
    fn measure(init: &Self::Init<'_>) -> Result<u32>;

    /// Writes every byte of an exact, previously measured destination.
    ///
    /// # Panics
    ///
    /// The input must have passed [`Self::measure`] and the destination length
    /// must equal that result. Violating this precondition may panic.
    fn encode(init: &Self::Init<'_>, destination: &mut [u8]);

    /// Encodes one exact bulk element loan and reports the number of bytes written.
    ///
    /// The destination has the previously measured length. Return an error if
    /// encoding fails; report every byte written on success. Bulk edits poison
    /// the store on an error or a count differing from the loan's exact length.
    /// The default adapts the complete, infallible [`Self::encode`] contract.
    #[inline(always)]
    fn encode_slot(init: &Self::Init<'_>, destination: &mut [u8]) -> Result<usize> {
        Self::encode(init, destination);
        Ok(destination.len())
    }

    /// Independent native mirror, available with the `alloc` feature.
    #[cfg(feature = "alloc")]
    type Owned;

    /// Checks sizes and canonical map order without allocating or writing.
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32>;

    /// Encodes a borrowed owned value directly, without allocating Init arrays.
    ///
    /// # Panics
    ///
    /// The input must have passed `measure_owned`; destination must be exact.
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]);

    /// Validates and copies an exact region using fallible allocation.
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned>;
}

/// A concrete schema with a compile-time certified root table.
///
/// Generic derived schemas are available as nested fields. Materialize their
/// concrete parent as a root; generic schemas have no standalone root table.
pub trait RootSchema: Schema {
    /// Returns a witness for the exact immutable concrete table.
    #[doc(hidden)]
    fn __layout() -> blubber_core::CheckedLayout<'static>;
}

/// Raw UTF-8 occupying its entire region, without a count or terminator.
#[derive(Clone, Copy, Debug)]
pub struct Str;

/// A counted list, using fixed strides or elided ends according to `T`.
///
/// Fixed byte arrays must have a positive, `u32`-representable width:
///
/// ```compile_fail
/// use blubber::{List, Schema};
/// let _ = <List<[u8; 0]> as Schema>::LAYOUT;
/// ```
pub struct List<T: Schema>(PhantomData<T>);

/// A counted map with canonical, strictly increasing keys.
///
/// With `alloc`, its owned mirror is a sorted `Vec<(K, V::Owned)>`. Measurement
/// rejects duplicates or descending keys before writing. Vector reservation
/// keeps decoding allocation failures recoverable on stable Rust.
pub struct Map<K: MapKey, V: Schema>(PhantomData<(K, V)>);

macro_rules! fixed_schema {
    ($ty:ty, $leaf:expr) => {
        impl RootSchema for $ty {
            fn __layout() -> blubber_core::CheckedLayout<'static> {
                crate::__layout_of!($ty)
            }
        }
        impl Schema for $ty {
            const __ENCODER_AUDITED: bool = <$ty as Fixed>::__ENCODER_AUDITED;
            const __MEASURE_REUSABLE: bool = true;
            const KIND: Kind = Kind::Fixed;
            const FIXED_SIZE: Option<u32> = Some(Self::SIZE);
            const MIN_SIZE: u32 = Self::SIZE;
            const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor = &match $leaf {
                Leaf::Unsigned(n) => crate::identity::WireDescriptor::Unsigned(n as u32),
                Leaf::Signed(n) => crate::identity::WireDescriptor::Signed(n as u32),
                Leaf::Bool => crate::identity::WireDescriptor::Bool,
                Leaf::Raw(n) => crate::identity::WireDescriptor::Raw(n),
            };
            const LAYOUT: &'static Layout = &Layout::Leaf($leaf);
            const __LEN: usize = 8;
            const __HEADER: &'static [u32] = &[
                match $leaf {
                    Leaf::Unsigned(_) => 1,
                    Leaf::Signed(_) => 2,
                    Leaf::Bool => 3,
                    Leaf::Raw(_) => 4,
                },
                Self::SIZE,
                1,
                0,
                0,
                0,
                0,
                0,
            ];
            type __Children = crate::__private::Nil;
            type Read<'a> = Self;
            type Mut<'a> = crate::ScalarMut<'a, Self>;

            #[inline(always)]
            fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
                crate::ScalarMut::from_edit(edit)
            }
            type Init<'a> = Self;
            type Removed = Self;

            fn __read_fixed(view: View<'_>) -> Result<Self> {
                view.read_fixed::<Self>()
            }

            #[cfg(feature = "alloc")]
            type Owned = Self;
            #[cfg(feature = "alloc")]
            fn measure_owned(value: &Self) -> Result<u32> {
                Self::measure(value)
            }
            #[cfg(feature = "alloc")]
            fn encode_owned(value: &Self, destination: &mut [u8]) {
                <Self as Schema>::encode(value, destination);
            }
            #[cfg(feature = "alloc")]
            fn decode_owned(view: View<'_>) -> Result<Self> {
                Self::read(view)
            }

            fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
                view.read_fixed::<Self>()
            }

            const __READ_REGION_NEEDS_LAYOUT: bool = false;

            #[inline(always)]
            fn read_region<'a>(
                bytes: &'a [u8],
                _layout: Layout,
                base: u32,
            ) -> Result<Self::Read<'a>> {
                <Self as Fixed>::__decode_region(bytes).map_err(|error| error.rebase(base))
            }

            fn read_removed(view: View<'_>) -> Result<Self::Removed> {
                view.read_fixed::<Self>()
            }

            fn measure(_: &Self::Init<'_>) -> Result<u32> {
                Ok(Self::SIZE)
            }

            #[inline(always)]
            fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
                blubber_core::encoding_leaf::encode_array(&Fixed::encode(*init), destination);
            }
        }
    };
}

fixed_schema!(u8, Leaf::Unsigned(1));
fixed_schema!(u16, Leaf::Unsigned(2));
fixed_schema!(u32, Leaf::Unsigned(4));
fixed_schema!(u64, Leaf::Unsigned(8));
fixed_schema!(u128, Leaf::Unsigned(16));
fixed_schema!(i8, Leaf::Signed(1));
fixed_schema!(i16, Leaf::Signed(2));
fixed_schema!(i32, Leaf::Signed(4));
fixed_schema!(i64, Leaf::Signed(8));
fixed_schema!(i128, Leaf::Signed(16));
fixed_schema!(bool, Leaf::Bool);
fixed_schema!(Address, Leaf::Raw(32));

impl<const N: usize> Schema for [u8; N] {
    const __ENCODER_AUDITED: bool = true;
    const __MEASURE_REUSABLE: bool = true;
    const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor =
        &crate::identity::WireDescriptor::Raw(Self::SIZE);
    const KIND: Kind = Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(Self::SIZE);
    const MIN_SIZE: u32 = Self::SIZE;
    const LAYOUT: &'static Layout = &Layout::Leaf(Leaf::Raw(Self::SIZE));
    const __LEN: usize = 8;
    const __HEADER: &'static [u32] = &[4, Self::SIZE, 1, 0, 0, 0, 0, 0];
    type __Children = crate::__private::Nil;
    type Read<'a> = Self;
    type Mut<'a> = crate::ScalarMut<'a, Self>;

    #[inline(always)]
    fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        crate::ScalarMut::from_edit(edit)
    }
    type Init<'a> = Self;
    type Removed = Self;

    fn __read_fixed(view: View<'_>) -> Result<Self> {
        view.read_fixed::<Self>()
    }

    #[cfg(feature = "alloc")]
    type Owned = Self;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self) -> Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self, destination: &mut [u8]) {
        <Self as Schema>::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self> {
        Self::read(view)
    }

    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
        view.read_fixed::<Self>()
    }

    const __READ_REGION_NEEDS_LAYOUT: bool = false;

    #[inline(always)]
    fn read_region<'a>(bytes: &'a [u8], _layout: Layout, base: u32) -> Result<Self::Read<'a>> {
        <Self as Fixed>::__decode_region(bytes).map_err(|error| error.rebase(base))
    }

    fn read_removed(view: View<'_>) -> Result<Self::Removed> {
        view.read_fixed::<Self>()
    }

    fn measure(_: &Self::Init<'_>) -> Result<u32> {
        Ok(Self::SIZE)
    }

    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        let _ = Self::SIZE;
        blubber_core::encoding_leaf::encode_array(init, destination);
    }
}

impl Schema for Str {
    const __ENCODER_AUDITED: bool = true;
    const __MEASURE_REUSABLE: bool = true;
    const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor =
        &crate::identity::WireDescriptor::Str;
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static Layout = &Layout::Str;
    const __LEN: usize = 8;
    const __HEADER: &'static [u32] = &[5, 0, 1, 0, 0, 0, 0, 0];
    type __Children = crate::__private::Nil;
    type Read<'a> = &'a str;
    type Mut<'a> = crate::StrMut<'a>;

    #[inline(always)]
    fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        crate::StrMut::new(edit.edit)
    }
    type Init<'a> = &'a str;
    type Removed = ();

    #[cfg(feature = "alloc")]
    type Owned = alloc::string::String;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32> {
        Self::measure(&value.as_str())
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        blubber_core::encoding_text::encode_text(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned> {
        crate::owned::string(Self::read(view)?)
    }

    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
        view.read_str()
    }

    const __READ_REGION_NEEDS_LAYOUT: bool = false;

    #[inline(always)]
    fn read_region<'a>(bytes: &'a [u8], _layout: Layout, base: u32) -> Result<Self::Read<'a>> {
        core::str::from_utf8(bytes).map_err(|error| {
            crate::Error::new(ErrorKind::InvalidUtf8, error.valid_up_to() as u32).rebase(base)
        })
    }

    fn read_removed(view: View<'_>) -> Result<Self::Removed> {
        view.validate()
    }

    #[inline(always)]
    fn measure(init: &Self::Init<'_>) -> Result<u32> {
        checked_u32(init.len())
    }

    #[inline(always)]
    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        blubber_core::encoding_text::encode_text(init, destination);
    }
}

impl<T: Schema> Schema for Option<T> {
    const __ENCODER_AUDITED: bool = T::__ENCODER_AUDITED;
    const __MEASURE_REUSABLE: bool = T::__MEASURE_REUSABLE;
    const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor =
        &crate::identity::WireDescriptor::Option(T::WIRE_DESCRIPTOR);
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static Layout = &Layout::Option { value: T::LAYOUT };
    const __LEN: usize = T::__LEN + 16;
    const __HEADER: &'static [u32] = &[6, 0, 0, 0, 1, 0, 0, 0, 0, 0];
    type __Children = crate::__private::Cons<T, crate::__private::Nil>;
    type Read<'a> = Option<T::Read<'a>>;
    type Mut<'a> = crate::OptionMut<'a, T>;

    #[inline(always)]
    fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        crate::OptionMut::new(edit.edit)
    }
    type Init<'a> = Option<T::Init<'a>>;
    type Removed = ();

    #[cfg(feature = "alloc")]
    type Owned = Option<T::Owned>;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32> {
        check_depth::<Self>()?;
        let source = value
            .as_ref()
            .map(|value| crate::product::owned::<T>(value).source);
        blubber_core::encoding_option::measure(source.as_ref())
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        let source = value
            .as_ref()
            .map(|value| crate::product::owned::<T>(value).source);
        blubber_core::encoding_option::encode(source.as_ref(), destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned> {
        view.option()?.map(T::decode_owned).transpose()
    }

    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
        view.option()?.map(T::read).transpose()
    }

    fn read_removed(view: View<'_>) -> Result<Self::Removed> {
        view.validate()
    }

    fn measure(init: &Self::Init<'_>) -> Result<u32> {
        check_depth::<Self>()?;
        let source = init
            .as_ref()
            .map(|value| crate::product::borrowed::<T>(value).source);
        blubber_core::encoding_option::measure(source.as_ref())
    }

    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        let source = init
            .as_ref()
            .map(|value| crate::product::borrowed::<T>(value).source);
        blubber_core::encoding_option::encode(source.as_ref(), destination);
    }
}

impl<T: Schema> Schema for List<T> {
    const __ENCODER_AUDITED: bool = T::__ENCODER_AUDITED;
    const __MEASURE_REUSABLE: bool = T::__MEASURE_REUSABLE;
    const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor =
        &crate::identity::WireDescriptor::List(T::WIRE_DESCRIPTOR);
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 4;
    const LAYOUT: &'static Layout = &Layout::List { element: T::LAYOUT };
    const __LEN: usize = T::__LEN + 16;
    const __HEADER: &'static [u32] = &[7, 0, 0, 0, 1, 0, 0, 0, 0, 0];
    type __Children = crate::__private::Cons<T, crate::__private::Nil>;
    type Read<'a> = crate::ListRef<'a, T>;
    type Mut<'a> = crate::ListMut<'a, T>;

    #[inline(always)]
    fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        crate::ListMut::new(edit.edit)
    }
    type Init<'a> = &'a [T::Init<'a>];
    type Removed = ();

    #[cfg(feature = "alloc")]
    type Owned = alloc::vec::Vec<T::Owned>;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32> {
        crate::owned::measure_list::<T>(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        crate::owned::encode_list::<T>(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned> {
        crate::owned::decode_list::<T>(view)
    }

    #[inline(always)]
    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
        crate::ListRef::from_view(view)
    }

    fn read_removed(view: View<'_>) -> Result<Self::Removed> {
        view.validate()
    }

    fn measure(init: &Self::Init<'_>) -> Result<u32> {
        check_depth::<Self>()?;
        let stride = if T::KIND == Kind::Fixed && T::__MEASURE_REUSABLE {
            Some(T::FIXED_SIZE.expect("fixed schema width"))
        } else {
            None
        };
        blubber_core::encoding_list::measure(
            &crate::product::borrowed_list::<T>(init),
            T::KIND == Kind::Variable,
            stride,
        )
    }

    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        let width = if T::KIND == Kind::Fixed {
            Some(T::FIXED_SIZE.expect("fixed schema width"))
        } else {
            None
        };
        if let Some(width) = width
            .filter(|_| blubber_core::encoding_product::contract(crate::product::markers::<Self>()))
        {
            blubber_core::encoding_list::encode_fixed(
                &crate::product::borrowed_list::<T>(init),
                destination,
                width,
            );
            return;
        }
        blubber_core::encoding_list::encode(
            &crate::product::borrowed_list::<T>(init),
            destination,
            width,
            true,
        );
    }
}

impl<K: MapKey, V: Schema> Schema for Map<K, V> {
    const __ENCODER_AUDITED: bool = K::__KEY_ENCODER_AUDITED && V::__ENCODER_AUDITED;
    const __MEASURE_REUSABLE: bool = K::__ORDER_REUSABLE && V::__MEASURE_REUSABLE;
    const WIRE_DESCRIPTOR: &'static crate::identity::WireDescriptor =
        &crate::identity::WireDescriptor::Map {
            key: K::KEY_DESCRIPTOR,
            value: V::WIRE_DESCRIPTOR,
        };
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 4;
    const LAYOUT: &'static Layout = &Layout::Map {
        key: K::KEY_LAYOUT,
        value: V::LAYOUT,
    };
    const __LEN: usize = V::__LEN + 16;
    const __HEADER: &'static [u32] = &[
        8,
        0,
        0,
        0,
        1,
        0,
        K::KEY_LAYOUT.width,
        match K::KEY_LAYOUT.domain {
            blubber_core::KeyDomain::Unsigned => 1,
            blubber_core::KeyDomain::Signed => 2,
            blubber_core::KeyDomain::Bool => 3,
            blubber_core::KeyDomain::Raw => 4,
        },
        0,
        0,
    ];
    type __Children = crate::__private::Cons<V, crate::__private::Nil>;
    type Read<'a> = crate::MapRef<'a, K, V>;
    type Mut<'a> = crate::MapMut<'a, K, V>;

    #[inline(always)]
    fn project<'a>(edit: crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        crate::MapMut::new(edit.edit)
    }
    type Init<'a> = &'a [(K, V::Init<'a>)];
    type Removed = ();

    #[cfg(feature = "alloc")]
    type Owned = alloc::vec::Vec<(K, V::Owned)>;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> Result<u32> {
        crate::owned::measure_map::<K, V>(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        crate::owned::encode_map::<K, V>(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> Result<Self::Owned> {
        crate::owned::decode_map::<K, V>(view)
    }

    #[inline(always)]
    fn read<'a>(view: View<'a>) -> Result<Self::Read<'a>> {
        crate::MapRef::from_view(view)
    }

    fn read_removed(view: View<'_>) -> Result<Self::Removed> {
        view.validate()
    }

    fn measure(init: &Self::Init<'_>) -> Result<u32> {
        check_depth::<Self>()?;
        let fixed_width = if V::KIND == Kind::Fixed && V::__MEASURE_REUSABLE && K::__ORDER_REUSABLE
        {
            V::FIXED_SIZE
        } else {
            None
        };
        #[cfg(test)]
        work::record_map_width(fixed_width);
        blubber_core::encoding_map::measure(
            &crate::product::borrowed_map::<K, V>(init),
            K::SIZE,
            V::KIND == Kind::Variable,
            fixed_width,
        )
    }

    fn encode(init: &Self::Init<'_>, destination: &mut [u8]) {
        if V::KIND == Kind::Fixed && V::FIXED_SIZE.is_none() {
            let (count, _) = destination.split_at_mut(4);
            count.copy_from_slice(&(init.len() as u32).to_le_bytes());
            panic!("fixed schema width");
        }
        let width = if V::KIND == Kind::Fixed {
            Some(V::FIXED_SIZE.expect("fixed schema width"))
        } else {
            None
        };
        if let Some(width) = width
            .filter(|_| blubber_core::encoding_product::contract(crate::product::markers::<Self>()))
        {
            blubber_core::encoding_map::encode_fixed(
                &crate::product::borrowed_map::<K, V>(init),
                destination,
                K::SIZE,
                width,
            );
            return;
        }
        blubber_core::encoding_map::encode(
            &crate::product::borrowed_map::<K, V>(init),
            destination,
            K::SIZE,
            width,
            false,
        );
    }
}

/// Adds encoded lengths, rejecting an extent outside the wire word domain.
#[inline(always)]
pub fn checked_add(left: u32, right: u32) -> Result<u32> {
    left.checked_add(right)
        .ok_or(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
}

pub(crate) fn mul(left: u32, right: u32) -> Result<u32> {
    left.checked_mul(right)
        .ok_or(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
}

#[doc(hidden)]
pub fn check_depth<S: Schema>() -> Result<()> {
    if S::__DEPTH > MAX_DEPTH {
        Err(ErrorKind::InvalidSchema.into())
    } else {
        Ok(())
    }
}

// Concrete roots exercised by blubber's internal acceptance and replay fixtures.
macro_rules! concrete_root {
    ($($ty:ty),* $(,)?) => { $(impl RootSchema for $ty {
        fn __layout() -> blubber_core::CheckedLayout<'static> { crate::__layout_of!($ty) }
    })* };
}
concrete_root!(
    Str,
    [u8; 3],
    List<u8>,
    List<u16>,
    List<u32>,
    List<Str>,
    Option<Str>
);

impl<T: Schema> Copy for List<T> {}
impl<T: Schema> Clone for List<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Schema> core::fmt::Debug for List<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(core::any::type_name::<Self>())
    }
}

impl<K: MapKey, V: Schema> Copy for Map<K, V> {}
impl<K: MapKey, V: Schema> Clone for Map<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: MapKey, V: Schema> core::fmt::Debug for Map<K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(core::any::type_name::<Self>())
    }
}
