//! Low-level streamed rewrites with ordinary validated ingress and settlement.
use crate::{
    mark,
    schemas::{KeyWords, Words},
    Funding, Store,
};
use blubber::raw::Resize;
use blubber::{AccountSchema, ReadStorage, Schema, Str};
use blubber_core::raw::{InlinePath, RewritePlan};
use blubber_core::Layout;
use core::hint::black_box;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

#[inline(never)]
fn execute<S: AccountSchema>(
    state: &mut AccountView,
    id: &Address,
    op: u8,
    n: usize,
) -> ProgramResult {
    // Streamed replacements can grow every indexed value beyond exact-size inputs.
    let mut store = Store::new_with_headroom(
        state,
        id,
        Funding::Prepaid,
        blubber::pinocchio::Headroom::Max,
    )?;
    mark(1);
    {
        let root = blubber::raw::Edit::load::<S>(&mut store, S::DISCRIMINATOR)?;
        black_box(&root);
    }
    mark(2);
    if op == 62 {
        let layout = Layout::Flat {
            checked: S::__layout(),
        };
        const PATH: InlinePath = InlinePath::root_field(0);
        let path = &PATH;
        blubber::raw::rewrite_collection(
            &mut store,
            &layout,
            path,
            (S::DISCRIMINATOR, S::MIN_SIZE),
            #[inline(always)]
            |view| {
                let mut total = 0usize;
                for _ in 0..n {
                    total = blubber_core::checked_add(
                        total,
                        Str::measure(black_box(&"replacement grows"))? as usize,
                    )?;
                }
                Ok((
                    RewritePlan::check_admitted(
                        view.bytes(),
                        &Layout::Flat {
                            checked: S::__layout().child(0, true),
                        },
                        n as u32,
                        blubber_core::checked_u32(total)?,
                    )?,
                    (),
                ))
            },
            #[inline(always)]
            |(), stream| {
                for _ in 0..n {
                    let len = Str::measure(black_box(&"replacement grows"))?;
                    let slot = {
                        stream.prepare_slot(len)?;
                        stream.destination()
                    };
                    let written = Str::encode_slot(&"replacement grows", slot)?;
                    stream.advance(written)?;
                }
                Ok(())
            },
        )?;
    } else if op == 21 {
        let layout = Layout::Flat {
            checked: S::__layout(),
        };
        const PATH: InlinePath = InlinePath::root_field(0);
        let path = &PATH;
        let target = blubber_core::raw::resolve_rewrite_tail(
            store.bytes(),
            S::DISCRIMINATOR,
            S::MIN_SIZE,
            &layout,
            path,
        )?
        .ok_or(ProgramError::InvalidAccountData)?;
        let (span, _) = target.selection();
        let mut total = 0usize;
        for _ in 0..n {
            total = blubber_core::checked_add(
                total,
                Str::measure(black_box(&"replacement grows"))? as usize,
            )?;
        }
        black_box(RewritePlan::check_admitted(
            &store.bytes()[span.start as usize..span.end as usize],
            &Layout::Flat {
                checked: S::__layout().child(0, true),
            },
            n as u32,
            blubber_core::checked_u32(total)?,
        )?);
    } else if op != 20 {
        return Err(ProgramError::InvalidInstructionData);
    }
    mark(3);
    let used = blubber_core::read_u32(store.bytes(), 8)?;
    store.commit(used)?;
    mark(4);
    pinocchio::cpi::set_return_data(&0u64.to_le_bytes());
    Ok(())
}

/// Measures list/map 8-to-17-byte rewrites at the standard five counts.
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [layout, op, lo, hi, _common] = ix else {
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
    match layout {
        1 => execute::<Words>(state, id, *op, n),
        3 => execute::<KeyWords>(state, id, *op, n),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
