//! Real SBF execution of probing and guard lifecycle checks.

#![forbid(unsafe_code)]

use blubber::ErrorKind;
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const INITIAL: usize = 128;
const MAX: usize = 10_485_760;

fn execute(
    opcode: u8,
    len: usize,
    n: Option<u32>,
    writable: bool,
    wrong_owner: bool,
    executable: bool,
    error: Option<ErrorKind>,
) -> Account {
    let program_id = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let mut mollusk = Mollusk::default();
    if (20..=34).contains(&opcode) {
        mollusk.feature_set.virtual_address_space_adjustments = n != Some(0);
        mollusk.feature_set.account_data_direct_mapping = n == Some(2);
    }
    mollusk.program_cache = mollusk_svm::program::ProgramCache::new(
        &mollusk.feature_set,
        &mollusk.compute_budget,
        false,
    );
    let artifact = if (20..=31).contains(&opcode) || opcode == 34 {
        "blubber_borrow_resize_alloc"
    } else {
        "blubber_borrow_resize"
    };
    mollusk.add_program(&program_id, artifact);
    let mut data = vec![0x19; len];
    data[..8].copy_from_slice(b"LEASE001");
    data[8..12].copy_from_slice(&(len as u32).to_le_bytes());
    if opcode == 10 {
        data[8..12].copy_from_slice(&16u32.to_le_bytes());
    }
    let account = Account {
        lamports: 100_000_000_000,
        data,
        owner: if wrong_owner {
            Pubkey::new_from_array([9; 32])
        } else {
            program_id
        },
        executable,
        rent_epoch: 0,
    };
    let mut ix_data = vec![opcode];
    if let Some(n) = n {
        ix_data.extend_from_slice(&n.to_le_bytes());
    }
    let ix = Instruction {
        program_id,
        accounts: vec![if writable {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        }],
        data: ix_data,
    };
    let mut ix = ix;
    if opcode == 16 || (20..=31).contains(&opcode) || opcode == 34 {
        ix.accounts.push(AccountMeta::new(key, false));
    }
    let mut accounts = vec![(key, account)];
    if [32, 33].contains(&opcode) {
        ix.accounts
            .push(AccountMeta::new_readonly(program_id, false));
        accounts.push((
            program_id,
            mollusk_svm::program::create_program_account_loader_v2(&[]),
        ));
    }
    let checks = if let Some(kind) = error {
        let error: blubber::Error = kind.into();
        vec![
            Check::err(error.into()),
            Check::account(&key).data(&accounts[0].1.data).build(),
        ]
    } else {
        vec![Check::success()]
    };
    let result = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
    result
        .resulting_accounts
        .into_iter()
        .find(|(address, _)| *address == key)
        .unwrap()
        .1
}

fn run(opcode: u8) -> Account {
    execute(opcode, INITIAL, None, true, false, false, None)
}

#[test]
fn first_open_discovers_exact_window_and_commit_closes() {
    let account = run(0);
    assert_eq!(account.data.len(), INITIAL);
    assert_eq!(account.data[12], 0x1a);
}

#[test]
fn second_session_after_growth_finds_exact_remaining_window() {
    let account = run(1);
    assert_eq!(account.data.len(), INITIAL);
    assert_eq!(account.data[12], 99);
}

#[test]
fn reopen_after_shrink_is_conservative_and_returns_growth_limit() {
    assert_eq!(run(2).data.len(), INITIAL - 37);
}

#[test]
fn up_to_never_exceeds_requested_additional_headroom() {
    for n in [0, 1, 17, 10_239, 10_240, 10_241, u32::MAX] {
        let account = execute(3, INITIAL, Some(n), true, false, false, None);
        assert_eq!(account.data.len(), INITIAL);
    }
}

#[test]
fn up_to_after_growth_obeys_sdk_ceiling() {
    assert_eq!(run(15).data.len(), INITIAL);
}

#[test]
fn max_and_up_to_enforce_ten_mib_cap() {
    assert_eq!(
        execute(0, MAX - 7, None, true, false, false, None)
            .data
            .len(),
        MAX - 7
    );
    assert_eq!(
        execute(0, MAX, None, true, false, false, None).data.len(),
        MAX
    );
    assert_eq!(
        execute(3, MAX - 7, Some(500), true, false, false, None)
            .data
            .len(),
        MAX - 7
    );
}

#[test]
fn reserve_retains_extent_and_shrink_to_fit_clears_it() {
    assert_eq!(run(4).data.len(), INITIAL);
}

#[test]
fn drop_trims_and_preserves_completed_edits() {
    let account = run(5);
    assert_eq!(account.data.len(), INITIAL);
    assert_eq!(account.data[12], 42);
}

#[test]
fn drop_ignores_invalid_envelope_and_releases_guard() {
    for opcode in [6, 17, 18] {
        assert_eq!(run(opcode).data.len(), INITIAL);
    }
}

#[test]
fn forget_keeps_guard_and_pregrown_storage() {
    assert_eq!(run(7).data.len(), INITIAL + 10_240);
}

#[test]
fn shared_and_exclusive_duplicate_borrows_are_checked() {
    assert_eq!(run(8).data.len(), INITIAL);
    assert_eq!(run(16).data.len(), INITIAL);
}

#[test]
fn sdk_resize_canary_accepts_boundary_and_rejects_next_byte_without_writes() {
    let account = run(9);
    assert_eq!(account.data.len(), INITIAL);
    assert!(account.data.iter().all(|byte| *byte == 0xa5));
}

#[test]
fn generated_root_edits_and_commits_through_adapter() {
    let account = run(10);
    assert_eq!(account.data.len(), 16);
    assert_eq!(&account.data[12..], &77u32.to_le_bytes());
}

#[test]
fn poison_and_invalid_commit_close_and_release_guards() {
    assert_eq!(run(11).data.len(), INITIAL);
    assert_eq!(run(12).data.len(), INITIAL);
}

#[test]
fn authentication_and_writability_fail_before_resize() {
    for (opcode, writable, wrong_owner, executable, error) in [
        (13, true, true, false, ErrorKind::WrongOwner),
        (14, true, true, false, ErrorKind::WrongOwner),
        (13, false, false, false, ErrorKind::NotWritable),
        (13, true, false, true, ErrorKind::Unauthorized),
        (14, false, false, true, ErrorKind::Unauthorized),
    ] {
        assert_eq!(
            execute(
                opcode,
                INITIAL,
                None,
                writable,
                wrong_owner,
                executable,
                Some(error)
            )
            .data
            .len(),
            INITIAL
        );
    }
    assert_eq!(
        execute(14, INITIAL, None, false, false, false, None)
            .data
            .len(),
        INITIAL
    );
}

#[cfg(feature = "alloc")]
#[test]
fn on_demand_lifecycle_in_all_three_mapping_profiles() {
    for profile in 0..3 {
        for (opcode, expected) in [
            (20, 64),
            (21, 72),
            (22, 464),
            (23, 64),
            (24, 64),
            (25, 64),
            (26, 64),
            (27, 64),
            (28, 88),
            (29, 416),
            (30, 64),
            (31, 64),
        ] {
            let account = execute(opcode, 64, Some(profile), true, false, false, None);
            assert_eq!(
                account.data.len(),
                expected,
                "profile={profile}, opcode={opcode}"
            );
            if [20, 21, 24, 25, 26].contains(&opcode) {
                assert_eq!(account.data[12], 42);
            }
        }
    }
}

#[test]
fn cpi_before_open_preserves_invocation_allowance_after_grow_and_shrink() {
    for profile in 0..3 {
        for opcode in [32, 33] {
            assert_eq!(
                execute(opcode, INITIAL, Some(profile), true, false, false, None)
                    .data
                    .len(),
                INITIAL
            );
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn on_demand_caps_growth_at_ten_mib_in_all_profiles() {
    for profile in 0..3 {
        for initial in [MAX - 7, MAX] {
            assert_eq!(
                execute(34, initial, Some(profile), true, false, false, None)
                    .data
                    .len(),
                initial
            );
        }
    }
}
