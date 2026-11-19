//! Shared host/MSRV/SBF source-compatibility exercise for fixed loans.
#![forbid(unsafe_code)]

use blubber::raw::Edit;
use blubber::{List, ListCursorMut, ListMut, Result, ScalarMut, Schema};

#[blubber::schema]
pub struct WalkRow {
    /// Copied packed field.
    pub value: u64,
}

#[blubber::schema(account)]
pub struct WalkRows {
    /// Rows share the `List<u64>` wire representation used by the CU oracle.
    pub value: List<WalkRow>,
}

/// Calls the existing public callback signature from a generic consumer.
pub fn generic<S: Schema>(
    mut list: ListMut<'_, S>,
    f: impl for<'s> FnMut(&mut S::Mut<'s>) -> Result<()>,
) -> Result<()> {
    list.try_for_each_mut(f)
}

/// Exercises explicit handles and temporary chains while retaining the store guard.
/// The inactive form has the same validated ingress, commit and return shape.
#[inline(never)]
pub fn exercise(
    store: &mut dyn blubber::raw::Resize,
    discriminator: [u8; 8],
    active: bool,
    mut mark: impl FnMut(u64),
) -> Result<u64> {
    let edit = Edit::load::<WalkRows>(store, discriminator)?;
    let mut root: WalkRowsMut<'_> = blubber::raw::project::<WalkRows>(edit);
    mark(2);
    let mut sum = 0u64;
    if active {
        let rows: ListMut<'_, WalkRow> = root.value_mut();
        let mut cursor: ListCursorMut<'_, WalkRow> = rows.cursor_mut()?;
        while let Some(mut row) = cursor.next()? {
            let typed: &mut WalkRowMut<'_> = &mut row;
            let mut value: ScalarMut<'_, u64> = typed.value_mut();
            let old = value.try_get()?;
            sum = sum.wrapping_add(old);
            value.set(old)?;
        }
        generic(root.value_mut(), |row| {
            let old = row.value();
            row.value_mut().set(old)
        })?;
        if let Some(row) = root.value().get(0) {
            let old = row.value();
            root.value_mut().at_mut(0)?.value_mut().set(old)?;
        }
        root.validate()?;
    }
    mark(3);
    root.commit()?;
    mark(4);
    Ok(sum)
}

/// Measures one generated fixed projection operation with identical ingress.
#[inline(never)]
pub fn slots(
    store: &mut dyn blubber::raw::Resize,
    discriminator: [u8; 8],
    operation: u8,
    mut mark: impl FnMut(u64),
) -> Result<u64> {
    let edit = Edit::load::<WalkRows>(store, discriminator)?;
    let mut root = blubber::raw::project::<WalkRows>(edit);
    mark(2);
    let mut sum = 0u64;
    match operation {
        55 => {}
        56 => {
            core::hint::black_box(root.value_mut().cursor_mut()?);
        }
        57 | 58 => {
            let mut cursor = root.value_mut().cursor_mut()?;
            while let Some(mut row) = cursor.next()? {
                if operation == 57 {
                    sum = sum.wrapping_add(core::hint::black_box(row.value_mut().try_get()?));
                } else {
                    row.value_mut().set(123)?;
                }
            }
        }
        59 => root.value_mut().try_for_each_mut(|row| {
            core::hint::black_box(row);
            Ok(())
        })?,
        60 => root.value_mut().try_for_each_mut(|row| {
            sum = sum.wrapping_add(core::hint::black_box(row.value_mut().try_get()?));
            Ok(())
        })?,
        61 => root
            .value_mut()
            .try_for_each_mut(|row| row.value_mut().set(123))?,
        _ => return Err(blubber::ErrorKind::InvalidBoundary.into()),
    }
    mark(3);
    root.commit()?;
    mark(4);
    Ok(sum)
}
