//! Fallible recursive mirrors. Maps retain canonical order in sorted vectors.

use alloc::{string::String, vec::Vec};

use crate::{ErrorKind, Kind, List, Map, MapKey, Result, Schema, View};

// Every growing allocation in owned decoding passes this point. Tests inject
// failures here without replacing the allocator or using unsafe code.
fn allocation() -> Result<()> {
    #[cfg(test)]
    tests::allocation()?;
    Ok(())
}

fn vector<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    if count > 0 && core::mem::size_of::<T>() > 0 {
        allocation()?;
    }
    values
        .try_reserve_exact(count)
        .map_err(|_| ErrorKind::Allocation)?;
    Ok(values)
}

pub(crate) fn string(value: &str) -> Result<String> {
    let mut owned = String::new();
    if !value.is_empty() {
        allocation()?;
    }
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| ErrorKind::Allocation)?;
    owned.push_str(value);
    Ok(owned)
}

pub(crate) fn measure_list<T: Schema>(values: &[T::Owned]) -> Result<u32> {
    crate::schema::check_depth::<List<T>>()?;
    blubber_core::encoding_list::measure(
        &crate::product::owned_list::<T>(values),
        T::KIND == Kind::Variable,
        None,
    )
}

pub(crate) fn encode_list<T: Schema>(values: &[T::Owned], destination: &mut [u8]) {
    let width = if T::KIND == Kind::Fixed {
        Some(T::FIXED_SIZE.expect("fixed schema width"))
    } else {
        None
    };
    blubber_core::encoding_list::encode(
        &crate::product::owned_list::<T>(values),
        destination,
        width,
        false,
    );
}

// Keep wide element temporaries in their own SBF frame. Separate view parts
// also keep this boundary distinct from the always-inlined small-value loop.
#[inline(always)]
pub(crate) fn decode_list<T: Schema>(view: View<'_>) -> Result<Vec<T::Owned>> {
    if core::mem::size_of::<T::Owned>() > 128 {
        return decode_list_wide::<T>(view.bytes(), view.layout(), view.base());
    }
    view.validate()?;
    let header = view.list_header()?;
    let mut values = vector(header.count as usize)?;
    for index in 0..header.count {
        values.push(T::decode_owned(header.element(view, index)?)?);
    }
    Ok(values)
}

#[inline(never)]
fn decode_list_wide<T: Schema>(
    bytes: &[u8],
    layout: blubber_core::Layout,
    base: u32,
) -> Result<Vec<T::Owned>> {
    let view = View::inherited(bytes, layout, base)?;
    view.validate()?;
    let header = view.list_header()?;
    let mut values = vector(header.count as usize)?;
    for index in 0..header.count {
        values.push(T::decode_owned(header.element(view, index)?)?);
    }
    Ok(values)
}

pub(crate) fn measure_map<K: MapKey, V: Schema>(values: &[(K, V::Owned)]) -> Result<u32> {
    crate::schema::check_depth::<Map<K, V>>()?;
    blubber_core::encoding_map::measure(
        &crate::product::owned_map::<K, V>(values),
        K::SIZE,
        V::KIND == Kind::Variable,
        None,
    )
}

pub(crate) fn encode_map<K: MapKey, V: Schema>(values: &[(K, V::Owned)], destination: &mut [u8]) {
    let width = if V::KIND == Kind::Fixed {
        Some(V::FIXED_SIZE.unwrap_or(0))
    } else {
        None
    };
    blubber_core::encoding_map::encode(
        &crate::product::owned_map::<K, V>(values),
        destination,
        K::SIZE,
        width,
        true,
    );
}

pub(crate) fn decode_map<K: MapKey, V: Schema>(view: View<'_>) -> Result<Vec<(K, V::Owned)>> {
    view.validate()?;
    let header = view.map_header()?;
    let mut values = vector(header.count as usize)?;
    for index in 0..header.count {
        let (key, base) = header.key(view, index)?;
        let key = K::decode_key(key).map_err(|error| error.rebase(base))?;
        values.push((key, V::decode_owned(header.value(view, index)?)?));
    }
    Ok(values)
}

#[cfg(test)]
mod tests;
