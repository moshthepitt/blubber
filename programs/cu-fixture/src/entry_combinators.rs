//! Permanent entry combinator calls and explicit-match controls.
use blubber::Entry;
use blubber::{Map, Str};
use core::hint::black_box;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
/// Fixed-value map and a retained suffix.
#[blubber::schema(account)]
pub struct Fixed {
    /// Values under test.
    pub values: Map<Address, u64>,
    /// Suffix preserved by every edit.
    pub tail: Str,
}
/// Indexed-value map and a retained suffix.
#[blubber::schema(account)]
pub struct Indexed {
    /// Values under test.
    pub values: Map<Address, Str>,
    /// Suffix preserved by every edit.
    pub tail: Str,
}
/// Generates sorted distinct full-width keys.
pub fn key(n: u16, common: bool) -> Address {
    let mut key = [42; 32];
    if common {
        key[31] = n as u8;
    } else {
        key[..2].copy_from_slice(&n.to_be_bytes());
    }
    Address::new_from_array(key)
}
#[inline(never)]
fn make<T>(built: &mut u32, value: T) -> T {
    *built += 1;
    black_box(value)
}
macro_rules! edit {
    ($map:expr, $key:expr, $op:expr, $old:expr, $new:expr, $default:expr, $present:expr, $control:expr) => {{
        let mut values = $map;
        let mut built = 0;
        let mut modified = 0;
        match $op {
            0 => {
                values.entry($key)?.or_insert($old)?;
                values
                    .get_mut(&$key)?
                    .ok_or(blubber::Error::from(blubber::ErrorKind::MissingKey))?
                    .set($new)?;
            }
            1 => {
                let entry = values.entry($key)?;
                let value = if $control {
                    match entry {
                        Entry::Occupied(e) => e.into_mut(),
                        Entry::Vacant(e) => e.insert(make(&mut built, $new))?,
                    }
                } else {
                    entry.or_insert_with(|| make(&mut built, $new))?
                };
                black_box(value);
                if built != u32::from(!$present) {
                    return Err(ProgramError::InvalidInstructionData);
                }
            }
            2 => {
                let entry = values.entry($key)?;
                let value = if $control {
                    match entry {
                        Entry::Occupied(e) => e.into_mut(),
                        Entry::Vacant(e) => e.insert($default)?,
                    }
                } else {
                    entry.or_default()?
                };
                black_box(value);
            }
            3 => {
                let entry = values.entry($key)?;
                let value = if $control {
                    match entry {
                        Entry::Occupied(e) => {
                            let mut v = e.into_mut();
                            modified += 1;
                            v.set($new)?;
                            v
                        }
                        Entry::Vacant(e) => e.insert(make(&mut built, $old))?,
                    }
                } else {
                    entry
                        .and_modify(|v| {
                            modified += 1;
                            v.set($new)
                        })?
                        .or_insert_with(|| make(&mut built, $old))?
                };
                black_box(value);
                if modified != u32::from($present) || built != u32::from(!$present) {
                    return Err(ProgramError::InvalidInstructionData);
                }
            }
            4 => {
                black_box(values.insert($key, $new)?);
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    }};
}
/// Runs a checked result and callback-count fixture.
pub fn run(owner: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [layout, op, present, lo, hi, common, grow, control] = *data else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let state = accounts
        .first_mut()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let n = u16::from_le_bytes([lo, hi]);
    let k = key(2 * (n / 2) + if present != 0 { 2 } else { 1 }, common != 0);
    let mut store = blubber::pinocchio::Store::new_with_headroom(
        state,
        owner,
        blubber::pinocchio::Funding::Prepaid,
        blubber::pinocchio::Headroom::Max,
    )?;
    match (layout, control != 0) {
        (0, false) => fixed::<false>(&mut store, k, op, present != 0),
        (0, true) => fixed::<true>(&mut store, k, op, present != 0),
        (_, false) => indexed::<false>(&mut store, k, op, present != 0, grow != 0),
        (_, true) => indexed::<true>(&mut store, k, op, present != 0, grow != 0),
    }
}
#[inline(never)]
fn fixed<const CONTROL: bool>(
    store: &mut blubber::pinocchio::Store<'_>,
    k: Address,
    op: u8,
    present: bool,
) -> ProgramResult {
    let mut root = Fixed::load_trusted_mut(store)?;
    edit!(root.values_mut(), k, op, 17, 123, 0, present, CONTROL);
    root.commit()?;
    Ok(())
}
#[inline(never)]
fn indexed<const CONTROL: bool>(
    store: &mut blubber::pinocchio::Store<'_>,
    k: Address,
    op: u8,
    present: bool,
    grow: bool,
) -> ProgramResult {
    let mut root = Indexed::load_trusted_mut(store)?;
    edit!(
        root.values_mut(),
        k,
        op,
        "original",
        if grow {
            "replacement grows"
        } else {
            "replaced"
        },
        "",
        present,
        CONTROL
    );
    root.commit()?;
    Ok(())
}
