//! Full native ingress and genuine SDK borrow controls on fresh SBF ELFs.
#![forbid(unsafe_code)]
mod support;
use solana_instruction::error::InstructionError;
use support::*;

#[test]
fn ingress_and_prefix_refresh_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false);
        c.run(0, &7u64.to_le_bytes(), read_metas(), None);
        c.run(1, &11u64.to_le_bytes(), edit_metas(), None);
        c.assert_counter(11);
        c.run(0, &11u64.to_le_bytes(), read_metas(), None);
        assert_eq!(c.state().data.len(), 256);
        assert!(c.state().data[c.used()..].iter().all(|b| *b == 0xA5));
    }
}

#[test]
fn full_malformed_ingress_all_profiles() {
    for profile in 0..3 {
        for mutation in 0..9 {
            let mut c = Case::new(profile, false);
            let used = c.used();
            match mutation {
                0 => c.state_mut().owner = PAYER,
                1 => c.state_mut().executable = true,
                2 => c.state_mut().data[0] ^= 1,
                3 => c.state_mut().data[8..12].copy_from_slice(&257u32.to_le_bytes()),
                4 => c.state_mut().data[8..12].copy_from_slice(&11u32.to_le_bytes()),
                5 => c.state_mut().data[52] = 2,
                6 => c.state_mut().data[used - 1] = 255,
                7 => c.state_mut().data.truncate(10),
                8 => {
                    c.state_mut().data[used - 10..used - 6].copy_from_slice(&u32::MAX.to_le_bytes())
                }
                _ => unreachable!(),
            }
            // The ordinary reader visits the damaged path and must reject it.
            c.reject(0, &7u64.to_le_bytes(), read_metas());
            let constraints = vec![readonly(), signer()];
            if std::env::var("BLUBBER_NATIVE_LAZY").as_deref() == Ok("1")
                && matches!(mutation, 6 | 8)
            {
                c.run(57, &[0], constraints.clone(), None);
                c.reject(57, &[1], constraints);
            } else {
                c.reject(57, &[0], constraints);
            }
        }
    }
}

#[test]
fn duplicate_nested_remaining_and_mixed_guards_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false);
        c.run(2, &[], duplicates(), None);
        for op in 3..=6 {
            c.run(
                op,
                &[],
                duplicates(),
                Some(InstructionError::AccountBorrowFailed),
            );
        }
        c.run(7, &[0], vec![writable()], None);
        c.run(7, &[1], vec![writable()], None);
        c.run(8, &[], duplicates(), None);
        c.run(11, &[], duplicates(), None);
    }
}

#[test]
fn drop_forget_and_bounded_slots_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false);
        for mutable in [0, 1] {
            for forget in [0, 1] {
                c.run(9, &[mutable, forget], vec![writable()], None);
            }
        }
        c.run(10, &[], vec![writable()], None);
        c.assert_counter(7);
    }
}

#[test]
fn writable_provenance_and_authority_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false);
        c.run(12, &[], read_metas(), None);
        c.reject(1, &11u64.to_le_bytes(), vec![readonly(), signer()]);
        c.state_mut().data[12..44].fill(0);
        c.reject(1, &11u64.to_le_bytes(), edit_metas());
    }
}

#[test]
fn callback_error_rolls_back_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false);
        c.run(13, &[], edit_metas(), Some(InstructionError::Custom(77)));
        c.assert_counter(7);
    }
}
