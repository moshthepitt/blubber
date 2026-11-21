//! Copied account constraint prefixes at the native size cap, without a heap.

use super::*;
use blubber::{AccountSchema, Result, SliceStore, Str, View};

#[blubber::schema(account)]
struct Wide {
    flag: bool,
    code: u64,
    payload: [u8; 1001],
    text: Str,
}

#[blubber::schema(account, constraint_fields = false)]
struct OptOut {
    payload: [u8; 4096],
    text: Str,
}

const _: () = assert!(core::mem::size_of::<WidePrefix>() == 1024);
const _: () = assert!(core::mem::size_of::<OptOutPrefix>() == 0);

// Passing two live prefixes exercises both Copy and a wider caller frame. Each
// callee inspects every copied byte so LLVM cannot discard the wide payload.
#[inline(never)]
fn check_copies(first: WidePrefix, second: WidePrefix, code: u64) -> ProgramResult {
    let pair = core::hint::black_box((first, second));
    for prefix in [&pair.0, &pair.1] {
        if !prefix.flag || prefix.code != code {
            return Err(ProgramError::InvalidAccountData);
        }
        for (index, byte) in prefix.payload.iter().enumerate() {
            if *byte != (index % 251) as u8 {
                return Err(ProgramError::InvalidAccountData);
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn read_and_check(bytes: &[u8], code: u64) -> ProgramResult {
    let view = View::load::<Wide>(bytes, Wide::DISCRIMINATOR)?;
    let prefix = Wide::read_prefix(view)?;
    check_copies(prefix, prefix, code)
}

#[inline(never)]
fn update(bytes: &mut [u8]) -> Result<()> {
    let mut store = SliceStore::new(bytes);
    let mut root = Wide::load_mut(&mut store)?;
    root.code_mut().set(42)?;
    root.commit()?;
    Ok(())
}

pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !state.owned_by(id) || !state.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    let mut data = state.try_borrow_mut()?;
    match ix {
        [0] => {
            read_and_check(&data, 19)?;
            update(&mut data)?;
            read_and_check(&data, 42)
        }
        [1] => {
            let view = View::load::<OptOut>(&data, OptOut::DISCRIMINATOR)?;
            let (): OptOutPrefix = OptOut::read_prefix(view)?;
            // Ordinary scoped views remain available for opted-out accounts.
            let read = OptOut::load(&data)?;
            let payload = view.field(0)?.bytes();
            if payload.iter().any(|byte| *byte != 7) || read.text() != "wide" {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(())
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
