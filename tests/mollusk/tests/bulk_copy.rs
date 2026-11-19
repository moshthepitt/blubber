//! Proved production SBF movement against an independent directional oracle.
#![forbid(unsafe_code)]

#[path = "../../../crates/blubber-core/tests/support/moves.rs"]
mod support;

use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[test]
fn checked_moves_match_naive_loop_on_sbf() {
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    let mut vm = Mollusk::default();
    vm.add_program(&owner, "blubber_cu_fixture");
    let mut executed = 0;
    support::cases(|start, end, dest| {
        let before = support::initial();
        let mut expected = before;
        support::naive(&mut expected, start, end, dest);
        let mut data = vec![21];
        for coordinate in [start, end, dest] {
            data.extend_from_slice(&(coordinate as u32).to_le_bytes());
        }
        let accounts = [(
            state,
            Account {
                lamports: 1_000_000_000,
                data: before.to_vec(),
                owner,
                executable: false,
                rent_epoch: 0,
            },
        )];
        let ix = Instruction {
            program_id: owner,
            accounts: vec![AccountMeta::new(state, false)],
            data,
        };
        let result = vm.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
        let account = result.get_account(&state).unwrap();
        assert_eq!(account.data, expected, "{start}..{end} -> {dest}");
        assert_eq!(account.lamports, accounts[0].1.lamports);
        executed += 1;
    });
    assert_eq!(executed, 450);
}

#[test]
fn directory_passes_match_independent_words_and_report_late_faults_on_sbf() {
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    let mut vm = Mollusk::default();
    vm.add_program(&owner, "blubber_cu_fixture");
    for stride in [4usize, 36] {
        for (delta, bad, payload, fault) in [
            (-7i64, None, 4096, None),
            (0, None, 4096, None),
            (11, None, 4096, None),
            (
                1,
                Some(4097),
                4096,
                Some(blubber::ErrorKind::InvalidBoundary),
            ),
            (
                1,
                Some(u32::MAX),
                u32::MAX,
                Some(blubber::ErrorKind::ArithmeticOverflow),
            ),
            (
                -1,
                Some(0),
                u32::MAX,
                Some(blubber::ErrorKind::ArithmeticOverflow),
            ),
        ] {
            let mut before = vec![0xa5; 1 + 100 * stride + 17];
            for i in 0..100 {
                let at = 1 + i * stride;
                before[at..at + 4].copy_from_slice(&(10 + 3 * i as u32).to_le_bytes());
            }
            let last = 1 + 99 * stride;
            if let Some(word) = bad {
                before[last..last + 4].copy_from_slice(&word.to_le_bytes());
            }
            let mut expected = before.clone();
            if fault.is_none() {
                for i in 0..100 {
                    let at = 1 + i * stride;
                    let word = u32::try_from(10 + 3 * i as i64 + delta).unwrap();
                    expected[at..at + 4].copy_from_slice(&word.to_le_bytes());
                }
            }
            let mut data = vec![22];
            for word in [1, stride as u32, 100] {
                data.extend_from_slice(&word.to_le_bytes());
            }
            data.extend_from_slice(&delta.to_le_bytes());
            data.extend_from_slice(&payload.to_le_bytes());
            let ix = Instruction {
                program_id: owner,
                accounts: vec![AccountMeta::new(state, false)],
                data,
            };
            let accounts = [(
                state,
                Account {
                    lamports: 1_000_000_000,
                    data: before,
                    owner,
                    executable: false,
                    rent_epoch: 0,
                },
            )];
            let check = fault.map_or_else(Check::success, |kind| {
                Check::err(blubber::Error::new(kind, last as u32).into())
            });
            let result = vm.process_and_validate_instruction(&ix, &accounts, &[check]);
            assert_eq!(result.get_account(&state).unwrap().data, expected);
        }
    }
}
