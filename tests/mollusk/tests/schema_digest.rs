//! Host/SBF canonical schema words, independent of native enum bytes and addresses.
#![forbid(unsafe_code)]
use blubber_cu_fixture::digests;
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[test]
fn every_representative_schema_has_the_same_digest_on_host_and_sbf() {
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    let mut vm = Mollusk::default();
    let artifact = std::env::var("BLUBBER_DIGEST_ARTIFACT").unwrap_or_else(|_| {
        if cfg!(feature = "depth16") {
            "blubber_cu_fixture_16".into()
        } else {
            "blubber_cu_fixture".into()
        }
    });
    vm.add_program(&owner, &artifact);
    let directory = std::env::var_os("BLUBBER_SCHEMA_WORD_DIR").map(std::path::PathBuf::from);
    if let Some(directory) = &directory {
        std::fs::create_dir_all(directory).unwrap();
    }
    for index in 0..digests::count() {
        let (name, table) = digests::table(index).unwrap();
        if let Some(directory) = &directory {
            let bytes: Vec<_> = table
                .words()
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect();
            std::fs::write(directory.join(format!("{name}.bin")), bytes).unwrap();
        }
        for seed in [0, 1, 0x99, 0xff] {
            let accounts = [(
                state,
                Account {
                    lamports: 1_000_000_000,
                    data: vec![0; 8],
                    owner,
                    executable: false,
                    rent_epoch: 0,
                },
            )];
            let ix = Instruction {
                program_id: owner,
                accounts: vec![AccountMeta::new(state, false)],
                data: vec![20, index as u8, seed],
            };
            let result = vm.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
            assert_eq!(
                result.get_account(&state).unwrap().data,
                digests::digest(table, seed).to_le_bytes(),
                "{name}; seed {seed}"
            );
            assert_eq!(
                result.get_account(&state).unwrap().lamports,
                accounts[0].1.lamports
            );
        }
    }
}
