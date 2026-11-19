//! Exact-count API observations with full validated ingress and real commit.
use crate::{
    mark,
    schemas::{KeyWords, Words},
    Funding, Store,
};
use blubber::{AccountSchema, Pubkey};
use core::hint::black_box;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

const fn texts() -> [&'static str; 1000] {
    let mut out = [""; 1000];
    let mut i = 0;
    while i < 1000 {
        out[i] = crate::traversal::INPUTS[i % 18];
        i += 1;
    }
    out
}
const VALUES: [&str; 1000] = texts();
const fn pairs(common: bool) -> [(Pubkey, &'static str); 1000] {
    let mut out = [(Pubkey::from_bytes([0; 32]), ""); 1000];
    let mut i = 0;
    while i < 1000 {
        let mut key = [42; 32];
        let at = if common { 28 } else { 0 };
        let word = (2 * (i + 1) as u32).to_be_bytes();
        let mut j = 0;
        while j < 4 {
            key[at + j] = word[j];
            j += 1;
        }
        out[i] = (Pubkey::from_bytes(key), VALUES[i]);
        i += 1;
    }
    out
}
static PLAIN: [(Pubkey, &str); 1000] = pairs(false);
static COMMON: [(Pubkey, &str); 1000] = pairs(true);

// These borrowed inputs are supplied as static program data. Their storage is
// outside the account and the operation uses no allocator or length scratch.
#[inline(never)]
fn execute(
    state: &mut AccountView,
    id: &Address,
    layout: u8,
    op: u8,
    n: usize,
    common: bool,
) -> ProgramResult {
    // Streamed replacements can grow every indexed value beyond exact-size inputs.
    let mut store = Store::new_with_headroom(
        state,
        id,
        Funding::Prepaid,
        blubber::pinocchio::Headroom::Max,
    )?;
    mark(1);
    let inputs = &black_box(&VALUES)[..n];
    let entries = &black_box(if common { &COMMON } else { &PLAIN })[..n];
    match (layout, op) {
        (1, 63) => list_fill(&mut store)?,
        (1, 64) => list_rewrite(&mut store, inputs)?,
        (3, 63) => map_fill(&mut store)?,
        (3, 64) => map_rewrite(&mut store, entries)?,
        (3, 66) => map_rewrite_values(&mut store, inputs)?,
        (_, 65) => combined(&mut store, layout, inputs, entries)?,
        (1, 20..=22) => idle::<Words>(&mut store, op, inputs, entries)?,
        (3, 20..=23) => idle::<KeyWords>(&mut store, op, inputs, entries)?,
        _ => return Err(ProgramError::InvalidInstructionData),
    }

    mark(4);
    pinocchio::cpi::set_return_data(&0u64.to_le_bytes());
    Ok(())
}

// Keep both concrete roots and all four public calls in one ordinary caller.
// This is a frame/correctness contract, separate from the single-call CU rows.
#[inline(never)]
fn combined(
    store: &mut dyn blubber::raw::Resize,
    layout: u8,
    inputs: &[&str],
    entries: &[(Pubkey, &str)],
) -> ProgramResult {
    if layout == 1 {
        let mut root = Words::load_checked_mut(store)?;
        mark(2);
        root.value_mut().fill("replacement grows")?;
        root.value_mut().rewrite_from(inputs)?;
        mark(3);
        root.commit()?;
    } else if layout == 3 {
        let mut root = KeyWords::load_checked_mut(store)?;
        mark(2);
        root.value_mut().fill_values("replacement grows")?;
        root.value_mut().rewrite_values_from(entries)?;
        root.value_mut().rewrite_values(inputs)?;
        mark(3);
        root.commit()?;
    } else {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(())
}

#[inline(never)]
fn list_fill(store: &mut dyn blubber::raw::Resize) -> ProgramResult {
    let mut root = Words::load_checked_mut(store)?;
    mark(2);
    root.value_mut().fill(black_box("replacement grows"))?;
    mark(3);
    root.commit()?;
    Ok(())
}
#[inline(never)]
fn list_rewrite(store: &mut dyn blubber::raw::Resize, inputs: &[&str]) -> ProgramResult {
    let mut root = Words::load_checked_mut(store)?;
    mark(2);
    root.value_mut().rewrite_from(inputs)?;
    mark(3);
    root.commit()?;
    Ok(())
}
#[inline(never)]
fn map_fill(store: &mut dyn blubber::raw::Resize) -> ProgramResult {
    let mut root = KeyWords::load_checked_mut(store)?;
    mark(2);
    root.value_mut()
        .fill_values(black_box("replacement grows"))?;
    mark(3);
    root.commit()?;
    Ok(())
}
#[inline(never)]
fn map_rewrite(store: &mut dyn blubber::raw::Resize, entries: &[(Pubkey, &str)]) -> ProgramResult {
    let mut root = KeyWords::load_checked_mut(store)?;
    mark(2);
    root.value_mut().rewrite_values_from(entries)?;
    mark(3);
    root.commit()?;
    Ok(())
}

#[inline(never)]
fn map_rewrite_values(store: &mut dyn blubber::raw::Resize, inputs: &[&str]) -> ProgramResult {
    let mut root = KeyWords::load_checked_mut(store)?;
    mark(2);
    root.value_mut().rewrite_values(inputs)?;
    mark(3);
    root.commit()?;
    Ok(())
}

#[inline(always)]
fn idle<S: AccountSchema>(
    store: &mut dyn blubber::raw::Resize,
    op: u8,
    inputs: &[&str],
    entries: &[(Pubkey, &str)],
) -> ProgramResult {
    let edit = blubber::raw::Edit::load::<S>(store, S::DISCRIMINATOR)?;
    mark(2);
    if op != 20 {
        let view = edit.view()?.field(0)?;
        if op == 21 {
            black_box(blubber::raw::prepare_fill::<blubber::Str>(
                view,
                true,
                view.layout().tag() == 8,
                if view.layout().tag() == 8 { 32 } else { 0 },
                &"replacement grows",
            )?);
        } else if op == 23 {
            black_box(blubber::raw::prepare_map_values::<Pubkey, blubber::Str>(
                view, true, inputs,
            )?);
        } else if view.layout().tag() == 8 {
            black_box(blubber::raw::prepare_map::<Pubkey, blubber::Str>(
                view, true, entries,
            )?);
        } else {
            black_box(blubber::raw::prepare_list::<blubber::Str>(
                view, true, inputs,
            )?);
        }
    }
    mark(3);
    edit.commit()?;
    Ok(())
}

/// Measures growing fill and varied rewrite for indexed lists and copied-key maps.
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [layout, op, lo, hi, common] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = usize::from(u16::from_le_bytes([*lo, *hi]));
    if n > 1000 {
        return Err(ProgramError::InvalidInstructionData);
    }
    mark(10);
    mark(11);
    mark(0);
    execute(state, id, *layout, *op, n, *common != 0)
}
