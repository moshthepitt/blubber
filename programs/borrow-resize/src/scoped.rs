//! Generated scoped sessions and signer/PDA creation with SDK Address fields.

use blubber::{
    pinocchio::{EditOptions, Funding, Headroom},
    Map, Str,
};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

#[blubber::schema(account)]
struct Book {
    authority: Address,
    balances: Map<Address, u64>,
    text: Str,
}

#[blubber::schema(account)]
struct Numbers {
    values: blubber::List<u64>,
}

pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [70, action, bump] = *ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state, payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if (17..=23).contains(&action) {
        if action == 17 {
            Numbers::create(
                state,
                payer,
                system_program,
                id,
                NumbersInit { values: &[1] },
            )?;
        } else if action == 20 || action == 21 {
            // Default capacity uses only opening spare; reject before any write.
            let alias = *state;
            let opening = state.data_len();
            let mut before = [0; 128];
            before[..opening].copy_from_slice(&state.try_borrow()?);
            Numbers::with_edit(state, id, Funding::Prepaid, |root| {
                assert_eq!(alias.data_len(), opening);
                let result = root.values_mut().push(2);
                if action == 20 {
                    assert_eq!(result, Err(blubber::ErrorKind::Capacity.into()));
                    // A capacity error leaves the session usable and unpoisoned.
                    root.values_mut()
                        .get_mut(0)?
                        .ok_or(blubber::ErrorKind::InvalidBoundary)?
                        .set(1)?;
                    root.verify()?;
                    assert_eq!(root.values().len(), 1);
                } else {
                    result?;
                    assert_eq!(root.values().len(), 2);
                }
                Ok(())
            })?;
            if action == 20 {
                assert_eq!(&*state.try_borrow()?, &before[..opening]);
            }
        } else if action == 22 {
            // One numeric push needs eight bytes beyond the exact creation size.
            Numbers::with_edit_checked_options(
                state,
                id,
                EditOptions {
                    funding: Funding::Prepaid,
                    headroom: Headroom::UpTo(8),
                    refund: Some(payer),
                },
                |root| root.values_mut().push(2),
            )?;
        } else if action == 23 {
            let result =
                Numbers::with_edit_checked_options(state, id, Funding::Prepaid.into(), |root| {
                    root.values_mut()
                        .get_mut(0)?
                        .ok_or(blubber::ErrorKind::InvalidBoundary)?
                        .set(2)?;
                    Err::<(), _>(blubber::ErrorKind::Unauthorized.into())
                });
            assert_eq!(result, Err(blubber::ErrorKind::Unauthorized.into()));
            Numbers::with_read_checked(state, id, |root| {
                assert_eq!(root.values().get(0), Some(2));
                Ok(())
            })?;
        } else {
            Numbers::with_edit_options(
                state,
                id,
                EditOptions {
                    funding: Funding::TopUp {
                        payer,
                        system_program,
                    },
                    headroom: Headroom::UpTo(if action == 18 { 0 } else { 8 }),
                    refund: None,
                },
                |root| {
                    if action == 18 {
                        root.values_mut().get_mut(0)?.unwrap().set(2)
                    } else {
                        root.values_mut().push(2)
                    }
                },
            )?;
        }
        return Ok(());
    }
    let authority = *payer.address();
    let init = BookInit {
        authority,
        balances: &[],
        text: "seed",
    };
    if action <= 4 {
        let bump = [bump];
        let seeds = [&b"scoped"[..], &bump];
        let receipt = match action {
            0 => Book::create(state, payer, system_program, id, init)?,
            2 => Book::create_pda(state, payer, system_program, id, &seeds, init)?,
            3 => Book::allocate_account(state, payer, system_program, id, init)?,
            4 => Book::allocate_pda(state, payer, system_program, id, &seeds, init)?,
            _ => return Err(ProgramError::InvalidInstructionData),
        };
        assert_eq!(receipt.used_len, Book::encoded_len_of(&init)?);
        assert_eq!(receipt.used_len, receipt.storage_len);
        assert_eq!(receipt.topped_up, 0);
        assert!(!state.is_borrowed());
    } else if action == 5 {
        // The map insertion and longer text need capacity beyond exact creation.
        let (value, receipt) = Book::with_edit_options(
            state,
            id,
            EditOptions {
                funding: Funding::TopUp {
                    payer,
                    system_program,
                },
                headroom: Headroom::UpTo(45), // Fixed 40-byte pair + five-byte text growth.
                refund: None,
            },
            |book| {
                assert_eq!(book.authority(), authority);
                book.balances_mut().insert(authority, 42)?;
                book.text_mut().set("committed")?;
                Ok(123u64)
            },
        )?;
        assert_eq!(value, 123);
        assert_eq!(receipt.storage_len, receipt.used_len);
        assert!(!state.is_borrowed());
    } else if action == 6 || action == 7 {
        let balance = state.lamports();
        // Retain the completed longer text when the callback returns an error.
        let result = Book::with_edit_options(
            state,
            id,
            EditOptions {
                funding: Funding::TopUp {
                    payer,
                    system_program,
                },
                headroom: Headroom::UpTo(36), // Replacement text exceeds the four opening bytes.
                refund: None,
            },
            |book| {
                book.text_mut()
                    .set("callback error leaves this complete edit")?;
                Err::<(), _>(blubber::ErrorKind::Unauthorized.into())
            },
        );
        assert_eq!(result, Err(blubber::ErrorKind::Unauthorized.into()));
        assert!(!state.is_borrowed());
        assert_eq!(
            state.lamports(),
            balance,
            "callback Err must not settle rent"
        );
        if action == 7 {
            result?;
        }
    } else if action == 8 {
        // Invalid input must be rejected before creating or funding state.
        let entries = [(authority, 1), (authority, 2)];
        let init = BookInit {
            authority,
            balances: &entries,
            text: "bad map",
        };
        assert!(Book::create(state, payer, system_program, id, init).is_err());
        assert_eq!(state.lamports(), 0);
        assert!(state.is_data_empty());
        assert!(!state.is_borrowed());
        return Ok(());
    } else if action == 16 {
        use blubber::{
            pinocchio::{Headroom, Store},
            ReadStorage, Resize,
        };
        let opening = state.data_len();
        let mut before = [0u8; 256];
        before[..opening].copy_from_slice(&state.try_borrow()?);
        let store = Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::UpTo(0))?;
        assert_eq!(
            store.check_resize(opening + 8),
            Err(blubber::ErrorKind::Capacity.into())
        );
        assert_eq!(store.bytes(), &before[..opening]);
        drop(store);
        return Ok(());
    } else if (11..=15).contains(&action) {
        use blubber::pinocchio::{EditOptions, Headroom};
        let alias = *state;
        let opening = state.data_len();
        let mut before = [0u8; 256];
        before[..opening].copy_from_slice(&state.try_borrow()?);
        let room = match action {
            11 => 0,
            13 => 7,
            14 => 16,
            15 => usize::MAX,
            _ => 8,
        };
        let result = Book::with_edit_options(
            state,
            id,
            EditOptions {
                funding: Funding::TopUp {
                    payer,
                    system_program,
                },
                headroom: Headroom::UpTo(room),
                refund: None,
            },
            |book| {
                assert!(alias.is_borrowed_mut());
                assert_eq!(alias.data_len(), opening + room);
                book.text_mut()
                    .set(if action == 11 { "same" } else { "twelve bytes" })?;
                if action == 14 {
                    book.text_mut().set("twenty bytes of text")?;
                }
                Ok(())
            },
        );
        if action == 13 || action == 15 {
            let expected = if action == 13 {
                blubber::ErrorKind::Capacity
            } else {
                blubber::ErrorKind::ArithmeticOverflow
            };
            assert_eq!(result, Err(expected.into()));
            assert_eq!(&*state.try_borrow()?, &before[..opening]);
            assert_eq!(state.data_len(), opening);
        } else {
            let (_, receipt) = result?;
            assert_eq!(receipt.used_len as usize, opening + room);
            assert_eq!(receipt.storage_len, receipt.used_len);
        }
        assert!(!state.is_borrowed());
        return Ok(());
    } else if action == 10 {
        let denied: blubber::Result<()> = Book::with_read_checked(state, id, |_| {
            panic!("invalid ingress reached read callback")
        });
        assert!(denied.is_err());
        assert!(!state.is_borrowed());
        let denied = Book::with_edit_checked::<()>(state, id, Funding::Prepaid, |_| {
            panic!("invalid ingress reached edit callback")
        });
        assert!(denied.is_err());
        assert!(!state.is_borrowed());
        let mut called = false;
        let denied = Book::with_edit_checked_options(state, id, Funding::Prepaid.into(), |_| {
            called = true;
            Ok(())
        });
        assert!(denied.is_err());
        assert!(!called);
        assert!(!state.is_borrowed());
        Book::with_read(state, id, |book| {
            assert!(book.try_text().is_err());
            Ok(())
        })?;
        Book::with_edit(state, id, Funding::Prepaid, |book| {
            book.text_mut().set("okay")?;
            Ok(())
        })?;
        Book::with_read(state, id, |book| {
            assert_eq!(book.text(), "okay");
            Ok(())
        })?;
        assert!(!state.is_borrowed());
        return Ok(());
    } else if action != 9 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let alias = *state;
    let length = state.data_len();
    let value = Book::with_read(state, id, |book| {
        assert!(alias.is_borrowed());
        assert_eq!(book.authority(), authority);
        if action == 5 {
            assert_eq!(book.balances().get(&authority), Some(42));
        }
        book.validate()?;
        Ok(book.text().len())
    })?;
    assert!(value > 0);
    assert_eq!(state.data_len(), length);
    assert!(!state.is_borrowed());
    assert_eq!(
        Book::with_read(state, id, |_| Err::<(), _>(
            blubber::ErrorKind::Unauthorized.into()
        )),
        Err(blubber::ErrorKind::Unauthorized.into())
    );
    assert!(!state.is_borrowed());
    Ok(())
}
