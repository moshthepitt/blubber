//! Shared field representations for derived encoding and metadata.

use crate::identity::WireDescriptor;
use crate::{Fixed, Kind, Result, Schema};
use blubber_core::encoding_product::{self as kernel, Fields, Pair, Source};
use blubber_core::{FixedField, VariableField};
use core::marker::PhantomData;

/// One schema type in a declaration-ordered field representation.
pub struct Field<S>(PhantomData<S>);

/// Reads the complete monomorphized encoder and replay contracts.
#[inline(always)]
pub(crate) const fn markers<S: Schema>() -> kernel::Markers {
    kernel::Markers {
        encoder: S::__ENCODER_AUDITED,
        replay: S::__MEASURE_REUSABLE,
    }
}

use blubber_core::encoding_metadata::Metadata as FieldMetadata;
/// Shared field metadata with the public identity descriptor as its wire view.
pub type Metadata = FieldMetadata<WireDescriptor>;

/// Balanced field types, independent of any particular source lifetime.
pub trait Types {
    /// Field metadata; grouping does not add wire nodes.
    const META: Metadata;
    /// Borrowed projections in exactly this type order.
    type Input<'v, 'i>: Fields
    where
        'i: 'v;
    /// Owned projections in exactly this type order.
    #[cfg(feature = "alloc")]
    type Owned<'v>: Fields;
}

/// Fixed codec projections over the same schema field representation.
pub trait NativeTypes: Types {
    /// Native fixed projections in declaration order.
    type Native<'v>: Fields;
    /// Conjunction of the native child codec contracts.
    const AUDITED: bool;
    /// Compile-time native width, schema and depth checks.
    const CHECK: ();
}

impl<S: Schema + Fixed> NativeTypes for Field<S> {
    type Native<'v> = kernel::Field<Native<'v, S>>;
    const AUDITED: bool = <S as Fixed>::__ENCODER_AUDITED;
    const CHECK: () = {
        assert!(S::SIZE > 0, "Fixed field must have positive fixed width");
        assert!(
            matches!(S::KIND, Kind::Fixed),
            "Fixed field must have fixed kind"
        );
        assert!(
            matches!(S::FIXED_SIZE, Some(width) if width == S::SIZE),
            "Fixed field has inconsistent Schema width"
        );
        assert!(
            S::MIN_SIZE == S::SIZE,
            "Fixed field has inconsistent Schema width"
        );
        crate::__private::assert_fixed_layout(S::LAYOUT, S::SIZE, blubber_core::MAX_DEPTH - 1);
    };
}

impl<L: NativeTypes, R: NativeTypes> NativeTypes for Pair<L, R> {
    type Native<'v> = Pair<L::Native<'v>, R::Native<'v>>;
    const AUDITED: bool = L::AUDITED && R::AUDITED;
    const CHECK: () = {
        let () = L::CHECK;
        let () = R::CHECK;
        assert!(
            L::META
                .fixed_bytes
                .checked_add(R::META.fixed_bytes)
                .is_some(),
            "Fixed wire width exceeds u32"
        );
    };
}

impl<S: Schema> Types for Field<S> {
    const META: Metadata = blubber_core::encoding_metadata::leaf(
        S::LAYOUT,
        S::WIRE_DESCRIPTOR,
        matches!(S::KIND, Kind::Fixed),
        blubber_core::encoding_metadata::field_width(matches!(S::KIND, Kind::Fixed), S::FIXED_SIZE),
        S::MIN_SIZE,
        markers::<S>(),
    );
    type Input<'v, 'i>
        = kernel::Field<Borrowed<'v, 'i, S>>
    where
        'i: 'v;
    #[cfg(feature = "alloc")]
    type Owned<'v> = kernel::Field<Owned<'v, S>>;
}

impl<L: Types, R: Types> Types for Pair<L, R> {
    const META: Metadata = blubber_core::encoding_metadata::pair(&L::META, &R::META);
    type Input<'v, 'i>
        = Pair<L::Input<'v, 'i>, R::Input<'v, 'i>>
    where
        'i: 'v;
    #[cfg(feature = "alloc")]
    type Owned<'v> = Pair<L::Owned<'v>, R::Owned<'v>>;
}

/// Borrowed child adapter; encoding remains the child's public Schema operation.
pub struct Borrowed<'v, 'i, S: Schema> {
    value: &'v S::Init<'i>,
}

impl<S: Schema> Source for Borrowed<'_, '_, S> {
    #[inline(always)]
    fn is_fixed(&self) -> bool {
        matches!(S::KIND, Kind::Fixed)
    }
    #[inline(always)]
    fn measure(&self) -> Result<u32> {
        S::measure(self.value)
    }
    #[inline(always)]
    fn encode(&self, destination: &mut [u8]) {
        S::encode(self.value, destination);
    }
}

/// Projects a borrowed child without measuring it or invoking its codec.
#[inline(always)]
pub fn borrowed<'v, 'i, S: Schema>(value: &'v S::Init<'i>) -> kernel::Field<Borrowed<'v, 'i, S>> {
    kernel::Field {
        source: Borrowed { value },
    }
}

/// Owned child adapter, sharing the borrowed representation's metadata.
#[cfg(feature = "alloc")]
pub struct Owned<'v, S: Schema> {
    value: &'v S::Owned,
}

#[cfg(feature = "alloc")]
impl<S: Schema> Source for Owned<'_, S> {
    #[inline(always)]
    fn is_fixed(&self) -> bool {
        matches!(S::KIND, Kind::Fixed)
    }
    #[inline(always)]
    fn measure(&self) -> Result<u32> {
        S::measure_owned(self.value)
    }
    #[inline(always)]
    fn encode(&self, destination: &mut [u8]) {
        S::encode_owned(self.value, destination);
    }
}

/// Projects an owned child without invoking its codec.
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn owned<S: Schema>(value: &S::Owned) -> kernel::Field<Owned<'_, S>> {
    kernel::Field {
        source: Owned { value },
    }
}

/// Native fixed child projection; no native memory layout enters the encoding.
pub struct Native<'v, S: Fixed> {
    value: &'v S,
}

impl<S: Fixed> Source for Native<'_, S> {
    #[inline(always)]
    fn is_fixed(&self) -> bool {
        true
    }
    #[inline(always)]
    fn measure(&self) -> Result<u32> {
        Ok(S::SIZE)
    }
    #[inline(always)]
    fn encode(&self, destination: &mut [u8]) {
        destination.copy_from_slice(S::encode(*self.value).as_ref());
    }
}

/// Projects a native fixed field without invoking its encoder.
#[inline(always)]
pub fn native<S: Fixed>(value: &S) -> kernel::Field<Native<'_, S>> {
    kernel::Field {
        source: Native { value },
    }
}

/// Measures a borrowed product in declaration order, retaining all child calls.
#[inline(always)]
pub fn measure<'v, 'i: 'v, R: Types>(input: &R::Input<'v, 'i>) -> Result<u32> {
    kernel::measure_fields(input, directory::<R>())
}

/// Writes a borrowed product into an exact, previously measured destination.
///
/// # Panics
/// The destination must have the measured extent and children must satisfy
/// their public encoding contracts.
#[inline(always)]
pub fn encode<'v, 'i: 'v, R: Types>(input: &R::Input<'v, 'i>, destination: &mut [u8]) {
    encode_fields(input, destination, R::META.fixed_bytes, directory::<R>());
}

/// Measures an owned product in declaration order.
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn measure_owned<R: Types>(input: &R::Owned<'_>) -> Result<u32> {
    kernel::measure_fields(input, directory::<R>())
}

/// Writes an owned product through the same generic packing algorithm.
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn encode_owned<R: Types>(input: &R::Owned<'_>, destination: &mut [u8]) {
    encode_fields(input, destination, R::META.fixed_bytes, directory::<R>());
}

/// Packs native fixed field projections through the common field writer.
#[inline(always)]
pub fn pack<R: NativeTypes>(input: &R::Native<'_>, destination: &mut [u8]) {
    encode_fields(input, destination, destination.len() as u32, 0);
}

#[inline(always)]
fn encode_fields<F: Fields>(input: &F, destination: &mut [u8], fixed: u32, directory: u32) {
    let cursor = kernel::cursor(destination.len(), fixed as usize, directory as usize);
    let cursor = kernel::write_fields(input, destination, cursor);
    kernel::finish(cursor, destination.len());
}

const fn sum(left: u32, right: u32) -> u32 {
    match left.checked_add(right) {
        Some(value) => value,
        None => panic!("schema wire width exceeds u32"),
    }
}

/// Number of fixed fields, computed from the field representation.
pub const fn fixed_count<R: Types>() -> usize {
    R::META.fixed_count
}

/// Number of variable fields, computed from the field representation.
pub const fn variable_count<R: Types>() -> usize {
    R::META.count - R::META.fixed_count
}

/// Exact packed prefix width.
pub const fn fixed_bytes<R: Types>() -> u32 {
    R::META.fixed_bytes
}

/// Elided-terminal-end directory width.
pub const fn directory<R: Types>() -> u32 {
    variable_count::<R>().saturating_sub(1) as u32 * 4
}

/// Minimum extent including the directory.
pub const fn minimum<R: Types>() -> u32 {
    sum(R::META.minimum, directory::<R>())
}

/// Canonical identity children in declaration order.
pub const fn wire<R: Types, const N: usize>() -> [&'static WireDescriptor; N] {
    blubber_core::encoding_metadata::wire::<_, N>(&R::META, &WireDescriptor::Unspecified)
}

/// Packed fixed field metadata from the shared type.
pub const fn fixed<R: Types, const N: usize>() -> [FixedField; N] {
    blubber_core::encoding_metadata::fixed::<_, N>(&R::META)
}

/// Variable field metadata from the shared type.
pub const fn variable<R: Types, const N: usize>() -> [VariableField; N] {
    blubber_core::encoding_metadata::variable::<_, N>(&R::META)
}

/// Struct header from the shared field types.
pub const fn header<R: Types, const N: usize>() -> [u32; N] {
    blubber_core::encoding_metadata::header::<_, N>(&R::META)
}

/// Positive packed offsets for native fixed field codecs.
pub const fn offsets<R: Types, const N: usize>() -> [u32; N] {
    blubber_core::encoding_metadata::offsets::<_, N>(&R::META)
}

/// Borrowed list input; indexing only projects an existing initializer reference.
pub struct BorrowedList<'v, 'i, S: Schema> {
    values: &'v [S::Init<'i>],
}

impl<'v, 'i, S: Schema> blubber_core::encoding_list::Elements for BorrowedList<'v, 'i, S> {
    type Child = Borrowed<'v, 'i, S>;
    #[inline(always)]
    fn count(&self) -> usize {
        self.values.len()
    }
    #[inline(always)]
    fn at(&self, index: usize) -> Self::Child {
        Borrowed {
            value: &self.values[index],
        }
    }
}

/// Projects a borrowed list without allocating or running any child callback.
#[inline(always)]
pub fn borrowed_list<'v, 'i, S: Schema>(values: &'v [S::Init<'i>]) -> BorrowedList<'v, 'i, S> {
    BorrowedList { values }
}

/// Owned list input; children borrow the existing owned mirror.
#[cfg(feature = "alloc")]
pub struct OwnedList<'v, S: Schema> {
    values: &'v [S::Owned],
}

#[cfg(feature = "alloc")]
impl<'v, S: Schema> blubber_core::encoding_list::Elements for OwnedList<'v, S> {
    type Child = Owned<'v, S>;
    #[inline(always)]
    fn count(&self) -> usize {
        self.values.len()
    }
    #[inline(always)]
    fn at(&self, index: usize) -> Self::Child {
        Owned {
            value: &self.values[index],
        }
    }
}

/// Projects an owned list without allocating or running any child callback.
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn owned_list<S: Schema>(values: &[S::Owned]) -> OwnedList<'_, S> {
    OwnedList { values }
}

/// Borrowed map projections; keys keep their original MapKey operations.
pub struct BorrowedMap<'v, 'i, K: crate::MapKey, S: Schema> {
    values: &'v [(K, S::Init<'i>)],
}

impl<'v, 'i, K: crate::MapKey, S: Schema> blubber_core::encoding_map::Entries
    for BorrowedMap<'v, 'i, K, S>
{
    type Child = Borrowed<'v, 'i, S>;
    #[inline(always)]
    fn count(&self) -> usize {
        self.values.len()
    }
    #[inline(always)]
    fn value(&self, index: usize) -> Self::Child {
        Borrowed {
            value: &self.values[index].1,
        }
    }
    #[inline(always)]
    fn compare(&self, index: usize) -> core::cmp::Ordering {
        self.values[index - 1].0.cmp(&self.values[index].0)
    }
    #[inline(always)]
    fn write_key(&self, index: usize, destination: &mut [u8]) {
        destination.copy_from_slice(self.values[index].0.encode_key().as_ref());
    }
}

/// Projects a borrowed map without allocating or invoking a key/value callback.
#[inline(always)]
pub fn borrowed_map<'v, 'i, K: crate::MapKey, S: Schema>(
    values: &'v [(K, S::Init<'i>)],
) -> BorrowedMap<'v, 'i, K, S> {
    BorrowedMap { values }
}

/// Owned map projections through the same generic map driver.
#[cfg(feature = "alloc")]
pub struct OwnedMap<'v, K: crate::MapKey, S: Schema> {
    values: &'v [(K, S::Owned)],
}

#[cfg(feature = "alloc")]
impl<'v, K: crate::MapKey, S: Schema> blubber_core::encoding_map::Entries for OwnedMap<'v, K, S> {
    type Child = Owned<'v, S>;
    #[inline(always)]
    fn count(&self) -> usize {
        self.values.len()
    }
    #[inline(always)]
    fn value(&self, index: usize) -> Self::Child {
        Owned {
            value: &self.values[index].1,
        }
    }
    #[inline(always)]
    fn compare(&self, index: usize) -> core::cmp::Ordering {
        self.values[index - 1].0.cmp(&self.values[index].0)
    }
    #[inline(always)]
    fn write_key(&self, index: usize, destination: &mut [u8]) {
        destination.copy_from_slice(self.values[index].0.encode_key().as_ref());
    }
}

/// Projects an owned map without allocating or invoking callbacks.
#[cfg(feature = "alloc")]
#[inline(always)]
pub fn owned_map<K: crate::MapKey, S: Schema>(values: &[(K, S::Owned)]) -> OwnedMap<'_, K, S> {
    OwnedMap { values }
}
