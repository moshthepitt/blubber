//! Fresh SBF identity and canonical descriptor agreement with pinned host vectors.
#![forbid(unsafe_code)]
use blubber_cu_fixture::identity;
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[test]
fn all_identity_vectors_match_host_on_every_runtime_mapping() {
    // Ordinary workspace tests use the tour artifact, which has no identity
    // opcode. The cumulative D21 lane supplies its dedicated fresh ELF.
    let Some(directory) = std::env::var_os("BLUBBER_IDENTITY_DIR") else {
        return;
    };
    let artifact = std::path::PathBuf::from(directory).join("blubber_cu_fixture");
    assert!(artifact.with_extension("so").is_file());
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap());
        for index in 0..identity::COUNT {
            let (name, discriminator, _, _, descriptor) = identity::vector(index).unwrap();
            let expected: Vec<_> = discriminator.iter().chain(descriptor).copied().collect();
            let accounts = [(
                state,
                Account {
                    lamports: 1_000_000_000,
                    data: vec![0xa5; expected.len()],
                    owner,
                    ..Account::default()
                },
            )];
            let instruction = Instruction {
                program_id: owner,
                accounts: vec![AccountMeta::new(state, false)],
                data: vec![32, index as u8],
            };
            let result =
                vm.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
            assert_eq!(
                result.get_account(&state).unwrap().data,
                expected,
                "{name}; profile {profile}"
            );
        }
    }
}
