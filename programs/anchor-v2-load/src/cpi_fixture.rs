//! Real caller/callee handlers and generated interfaces share these bodies.
use super::*;
use anchor_lang::pinocchio::{
    self,
    instruction::{InstructionAccount, InstructionView},
    Resize,
};
use anchor_lang::{CpiContext, CpiHandle, CpiHandleMut};

/// Marker authenticating the executable fixture program.
pub struct FixtureProgram;
impl Id for FixtureProgram {
    fn id() -> Address {
        ID
    }
}

#[derive(Accounts)]
/// Raw entry views let the handler select shared/exclusive native ingress.
pub struct CpiCall {
    #[account(mut)]
    /// State is authenticated when loaded by Blubber inside the handler.
    pub state: UncheckedAccount,
    /// Transaction signer or program-signed PDA, checked by the typed callee.
    pub authority: UncheckedAccount,
    /// Executable caller/callee program for checked self-invocation.
    pub program: Program<FixtureProgram>,
}

#[derive(Accounts)]
/// Callee deliberately bypasses the typed codec for corrupted reload controls.
pub struct CpiRaw {
    #[account(mut, owner = ID)]
    /// Current-program bytes, modified only through safe SDK operations.
    pub state: UncheckedAccount,
}

#[derive(Accounts)]
/// A transaction-readonly account uses the same scoped native CPI API.
pub struct CpiReadonlyCall {
    /// Shared SDK guard retained outside the callback.
    pub state: Blubber<State>,
    /// Executable callee, present in the transaction's account list.
    pub program: Program<FixtureProgram>,
}

pub(super) fn readonly(ctx: &mut Context<CpiReadonlyCall>) -> Result<()> {
    let old = ctx.accounts.state.counter;
    ctx.accounts
        .state
        .with_cpi_read(|handle| generated_read(handle, old))?;
    ctx.accounts.state.read(|s| s.validate())?;
    Ok(())
}

#[program(interface, program_id = ID)]
pub mod callee {
    use super::*;
    #[discrim = 35]
    /// Checks both copied metadata and fully typed data after release.
    pub fn cpi_read(ctx: &mut Context<Read>, expected: u64) -> Result<()> {
        require!(
            ctx.accounts.state.counter == expected,
            Error::InvalidAccountData
        );
        require!(
            ctx.accounts.state.read(|s| Ok(s.counter()))? == expected,
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 36]
    /// The signer/address constraints authenticate this completed edit.
    pub fn cpi_write(ctx: &mut Context<Modify>, expected: u64) -> Result<()> {
        require!(
            ctx.accounts.state.counter == expected,
            Error::InvalidAccountData
        );
        ctx.accounts.state.edit(|s| s.counter_mut().set(29))?;
        Ok(())
    }
    #[discrim = 37]
    /// Changes physical geometry or invalidates a specific ingress contract.
    pub fn cpi_raw(ctx: &mut Context<CpiRaw>, mode: u8, size: u32) -> Result<()> {
        let mut account = *ctx.accounts.state.account();
        match mode {
            0 => account.resize(size as usize)?,
            1 => account.try_borrow_mut()?[0] ^= 1,
            2 => account.try_borrow_mut()?[8..12].copy_from_slice(&u32::MAX.to_le_bytes()),
            // Fixed fields precede the variable directory.
            3 => account.try_borrow_mut()?[52] = 2,
            4 => {
                let mut data = account.try_borrow_mut()?;
                let used = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
                data[used - 1] = 255;
            }
            // Safe close changes ownership as well as clearing data. Restore
            // the balance so the runtime's balance invariant still holds.
            5 => {
                let balance = account.lamports();
                account.close()?;
                account.set_lamports(balance);
            }
            6 => return Err(Error::Custom(91)),
            _ => return Err(Error::InvalidArgument),
        }
        Ok(())
    }
}

fn generated_read(handle: CpiHandle<'_>, expected: u64) -> Result<()> {
    cpi::cpi_read(
        CpiContext::new(&ID, cpi::accounts::Read { state: handle }),
        expected,
    )
}

fn generated_write(
    handle: CpiHandleMut<'_>,
    authority: CpiHandle<'_>,
    signed: bool,
    wrong_bump: bool,
) -> Result<()> {
    let (_, bump) = Address::find_program_address(&[b"cpi-authority"], &ID);
    let bump = [if wrong_bump {
        bump.wrapping_add(1)
    } else {
        bump
    }];
    let seeds: &[&[u8]] = &[b"cpi-authority", &bump];
    let signers: &[&[&[u8]]] = &[seeds];
    let context = CpiContext::new(
        &ID,
        cpi::accounts::Modify {
            state: handle,
            authority,
        },
    );
    cpi::cpi_write(
        if signed {
            context.with_signer(signers)
        } else {
            context
        },
        13,
    )
}

fn raw(handle: CpiHandleMut<'_>, mode: u8, size: u32) -> Result<()> {
    cpi::cpi_raw(
        CpiContext::new(&ID, cpi::accounts::CpiRaw { state: handle }),
        mode,
        size,
    )
}

fn checked_sdk(account: &AccountView, opcode: u8, extra: &[u8], writable: bool) -> Result<()> {
    let metas = [InstructionAccount::new(account.address(), writable, false)];
    let mut data = vec![opcode];
    data.extend_from_slice(extra);
    pinocchio::cpi::invoke::<1, _>(
        &InstructionView {
            program_id: &ID,
            accounts: &metas,
            data: &data,
        },
        &[account],
    )
}

pub(super) fn call(ctx: &mut Context<CpiCall>, scenario: u8, arg: u32) -> Result<()> {
    let account = *ctx.accounts.state.account();
    if scenario == 19 {
        return wide(account, arg);
    }
    let mut state = if scenario == 0 || scenario == 16 {
        Blubber::<State>::load(account)?
    } else {
        Blubber::<State>::load_mut(account)?
    };
    let old = state.counter;
    match scenario {
        0 | 1 => state.with_cpi_read(|handle| generated_read(handle, old))?,
        2 | 3 => {
            state.edit(|s| s.counter_mut().set(13))?;
            state.with_cpi(|handle| {
                generated_write(
                    handle,
                    ctx.accounts.authority.cpi_handle(),
                    scenario == 3,
                    arg != 0,
                )
            })?;
            require!(state.counter == 29, Error::InvalidAccountData);
        }
        4 => state.with_cpi(|_| {
            let mut data = vec![0];
            data.extend_from_slice(&arg.to_le_bytes());
            checked_sdk(&account, 37, &data, true)
        })?,
        5 => state.with_cpi_read(|handle| {
            let ix = solana_instruction::Instruction {
                program_id: ID,
                accounts: vec![
                    solana_instruction::AccountMeta::new_readonly(*handle.address(), false),
                    solana_instruction::AccountMeta::new_readonly(*handle.address(), false),
                ],
                data: vec![2],
            };
            anchor_lang::solana_program::program::invoke(&ix, &[handle, handle])
        })?,
        6 => {
            return state.with_cpi(|handle| {
                let h: CpiHandle<'_> = handle.into();
                let ix = solana_instruction::Instruction {
                    program_id: ID,
                    accounts: vec![
                        solana_instruction::AccountMeta::new(*h.address(), false),
                        solana_instruction::AccountMeta::new(*h.address(), false),
                    ],
                    data: vec![5],
                };
                anchor_lang::solana_program::program::invoke(&ix, &[h, h])
            });
        }
        7 => {
            let result = state.with_cpi_read(|handle| {
                generated_read(handle, old)?;
                Err::<(), _>(Error::Custom(91))
            });
            require!(result == Err(Error::Custom(91)), Error::InvalidAccountData);
            require!(state.counter == old, Error::InvalidAccountData);
            let result = state.with_cpi_read(|handle| {
                let ix = solana_instruction::Instruction {
                    program_id: ID,
                    accounts: vec![solana_instruction::AccountMeta::new_readonly(
                        *handle.address(),
                        true,
                    )],
                    data: vec![35],
                };
                anchor_lang::solana_program::program::invoke(&ix, &[handle])
            });
            require!(
                result == Err(Error::MissingRequiredSignature),
                Error::InvalidAccountData
            );
            require!(state.counter == old, Error::InvalidAccountData);
        }
        8 | 15 => {
            let result = state.with_cpi(|handle| {
                raw(handle, arg as u8, 0)?;
                if scenario == 15 {
                    Err(Error::Custom(92))
                } else {
                    Ok(())
                }
            });
            let error = result.expect_err("corrupted callee must fail reload");
            if scenario == 15 {
                require!(error == Error::Custom(92), Error::InvalidAccountData);
            }
            if arg == 5 && scenario == 8 {
                require!(error == Error::IllegalOwner, Error::InvalidAccountData);
            }
            require!(
                state.read(|_| Ok(())).is_err() && state.exit().is_err(),
                Error::InvalidAccountData
            );
            return Err(error);
        }
        9 | 10 => {
            let result = state.with_cpi(|_| {
                core::mem::forget(Blubber::<State>::load_mut(account)?);
                if scenario == 10 {
                    Err(Error::Custom(92))
                } else {
                    Ok(())
                }
            });
            let expected = if scenario == 10 {
                Error::Custom(92)
            } else {
                Error::AccountBorrowFailed
            };
            require!(result == Err(expected), Error::InvalidAccountData);
            require!(
                state.read(|_| Ok(())).is_err() && state.exit().is_err(),
                Error::InvalidAccountData
            );
            return result;
        }
        11 => {
            state.with_cpi(|handle| raw(handle, 0, arg))?;
            state.with_cpi(|handle| raw(handle, 0, 256))?;
            state.with_cpi(|handle| raw(handle, 0, arg))?;
            drop(state);
            let mut reopened = Blubber::<State>::load_mut(account)?;
            let mut alias = account;
            let result = reopened.with_cpi(|_| alias.resize(10_497));
            require!(
                result == Err(Error::InvalidRealloc),
                Error::InvalidAccountData
            );
            reopened.read(|s| s.validate())?;
            return Ok(());
        }
        12 => {
            // A nested child cannot enlarge this ancestor's opening window.
            let mut data = vec![38];
            data.extend_from_slice(&arg.to_le_bytes());
            let metas = [
                InstructionAccount::new(account.address(), true, false),
                InstructionAccount::new(ctx.accounts.authority.address(), false, false),
                InstructionAccount::new(&ID, false, false),
            ];
            state.with_cpi(|_| {
                pinocchio::cpi::invoke::<3, _>(
                    &InstructionView {
                        program_id: &ID,
                        accounts: &metas,
                        data: &data,
                    },
                    &[
                        &account,
                        ctx.accounts.authority.account(),
                        ctx.accounts.program.account(),
                    ],
                )
            })?;
        }
        13 => {
            require!(
                generated_read(state.cpi_handle(), old) == Err(Error::AccountBorrowFailed),
                Error::InvalidAccountData
            );
            let result = raw(state.cpi_handle_mut(), 0, arg);
            require!(
                result == Err(Error::AccountBorrowFailed),
                Error::InvalidAccountData
            );
            state.with_cpi_read(|handle| generated_read(handle, old))?;
        }
        14 => {
            state.edit(|s| s.counter_mut().set(13))?;
            state.with_cpi_read(|handle| generated_read(handle, 13))?;
        }
        16 => {
            require!(
                state.with_cpi(|_| Ok(())) == Err(anchor_lang::ErrorCode::ConstraintMut.into()),
                Error::InvalidAccountData
            );
        }
        17 => return state.with_cpi(|handle| raw(handle, 6, 0)),
        _ => return Err(Error::InvalidArgument),
    }
    state.read(|s| s.validate())?;
    let mut alias = *state.account();
    require!(alias.try_borrow_mut().is_err(), Error::InvalidAccountData);
    state.exit()
}

pub(super) fn nested(ctx: &mut Context<CpiCall>, size: u32) -> Result<()> {
    let mut state = Blubber::<State>::load_mut(*ctx.accounts.state.account())?;
    state.with_cpi(|handle| raw(handle, 0, 5_000))?;
    state.with_cpi(|handle| raw(handle, 0, size))?;
    state.read(|s| s.validate())?;
    Ok(())
}

// Keep the two full-width prefix copies in a separate SBF frame.
#[inline(never)]
fn wide(account: AccountView, size: u32) -> Result<()> {
    let mut state = Blubber::<Wide>::load_mut(account)?;
    let before = state.prefix;
    state.with_cpi(|handle| raw(handle, 0, size))?;
    require!(state.prefix == before, Error::InvalidAccountData);
    state.read(|s| s.validate())?;
    Ok(())
}
