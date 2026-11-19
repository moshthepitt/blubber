//! Reviewer alternate caller, with ordinary fallback for its missing rows.
//! Keep callers and closures free of code-generation attributes.

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

/// Runs the reviewer dispatch and the complete ordinary oracle inventory.
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [layout, op, lo, hi, common] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let n = u16::from_le_bytes([*lo, *hi]) as usize;
    if n > 1000 || *common > 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    // Retain the original reviewer body and its cursor loops. Supplement rows
    // it did not measure through the other ordinary dispatcher in the same ELF.
    if matches!(
        (*layout, *op),
        (0, 6 | 20 | 30..=34 | 70) | (2, 20 | 30 | 31 | 33 | 34) | (1, 63 | 64) | (3, 63)
    ) || (*layout == 3 && *op == 64 && *common == 0)
    {
        reviewer::process_instruction(id, accounts, ix)
    } else {
        super::mixed_operations::run(id, accounts, ix)
    }
}

// The original alternate-probe body, including its independent account schemas.
mod reviewer {
    extern crate alloc;
    use blubber::pinocchio::{Funding, ReadStore, Store};
    use blubber::{List, Map, Pubkey, Str};
    use core::hint::black_box;
    use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
    #[blubber::schema(account)]
    pub struct Numbers {
        pub value: List<u64>,
    }
    #[blubber::schema(account)]
    pub struct PublicKeyMap {
        pub value: Map<Pubkey, u64>,
    }
    #[blubber::schema(account)]
    pub struct Words {
        pub value: List<Str>,
    }
    #[blubber::schema(account)]
    pub struct KeyWords {
        pub value: Map<Pubkey, Str>,
    }
    const TEXTS: [&str; 18] = [
        "",
        "a",
        "ab",
        "abc",
        "abcd",
        "abcde",
        "abcdef",
        "abcdefg",
        "abcdefgh",
        "abcdefghi",
        "abcdefghij",
        "abcdefghijk",
        "abcdefghijkl",
        "abcdefghijklm",
        "abcdefghijklmn",
        "abcdefghijklmno",
        "abcdefghijklmnop",
        "abcdefghijklmnopq",
    ];
    const fn values() -> [&'static str; 1000] {
        let mut a = [""; 1000];
        let mut i = 0;
        while i < 1000 {
            a[i] = TEXTS[i % 18];
            i += 1;
        }
        a
    }
    static VALUES: [&str; 1000] = values();
    const fn pairs() -> [(Pubkey, &'static str); 1000] {
        let mut a = [(Pubkey::from_bytes([0; 32]), ""); 1000];
        let mut i = 0;
        while i < 1000 {
            let mut k = [42; 32];
            let w = (2 * (i + 1) as u32).to_be_bytes();
            let mut j = 0;
            while j < 4 {
                k[j] = w[j];
                j += 1;
            }
            a[i] = (Pubkey::from_bytes(k), TEXTS[i % 18]);
            i += 1;
        }
        a
    }
    static ENTRIES: [(Pubkey, &str); 1000] = pairs();
    fn key_sum(k: Pubkey) -> u64 {
        let b = k.to_bytes();
        let a = u64::from_le_bytes(b[..8].try_into().unwrap());
        let c = u64::from_le_bytes(b[8..16].try_into().unwrap());
        let d = u64::from_le_bytes(b[16..24].try_into().unwrap());
        let e = u64::from_le_bytes(b[24..].try_into().unwrap());
        let m = 0x00ff_00ff_00ff_00ff;
        let x = (a & m)
            .wrapping_add(c & m)
            .wrapping_add(d & m)
            .wrapping_add(e & m)
            .wrapping_add((a >> 8) & m)
            .wrapping_add((c >> 8) & m)
            .wrapping_add((d >> 8) & m)
            .wrapping_add((e >> 8) & m);
        x.wrapping_mul(0x0001_0001_0001_0001) >> 48
    }
    pub fn process_instruction(
        id: &Address,
        accounts: &mut [AccountView],
        ix: &[u8],
    ) -> ProgramResult {
        let [layout, op, lo, hi, _common] = ix else {
            return Err(ProgramError::InvalidInstructionData);
        };
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let n = u16::from_le_bytes([*lo, *hi]) as usize;
        let mut out = 0u64;
        if *op == 6 {
            let store = ReadStore::new(state, id)?;
            let root = Numbers::load(store.bytes())?;
            for x in root.value().iter() {
                out = out.wrapping_add(black_box(x));
            }
        } else {
            // Indexed fill/rewrite values can add nine bytes beyond each eight-byte input.
            let room = if matches!(*layout, 1 | 3) && matches!(*op, 63 | 64) {
                n.checked_mul(9)
                    .ok_or(blubber::Error::from(blubber::ErrorKind::ArithmeticOverflow))?
            } else {
                0
            };
            let mut store = Store::new_with_headroom(
                state,
                id,
                Funding::Prepaid,
                blubber::pinocchio::Headroom::UpTo(room),
            )?;
            match (*layout, *op) {
                (0, 20) => {
                    let root = Numbers::load_checked_mut(&mut store)?;
                    root.commit()?;
                }
                (2, 20) => {
                    let root = PublicKeyMap::load_checked_mut(&mut store)?;
                    root.commit()?;
                }

                (0, 30) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    {
                        let mut c = root.value_mut().cursor_mut()?;
                        while let Some(child) = c.next()? {
                            out = out.wrapping_add(black_box(child.try_get()?));
                        }
                    }
                    root.commit()?;
                }
                (0, 31) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    {
                        let mut c = root.value_mut().cursor_mut()?;
                        while let Some(mut child) = c.next()? {
                            child.set(123)?;
                        }
                    }
                    root.commit()?;
                }
                (0, 32) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    root.value_mut().try_for_each_mut(|child| {
                        black_box(child);
                        Ok(())
                    })?;
                    root.commit()?;
                }
                (0, 33) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    root.value_mut().try_for_each_mut(|child| {
                        out = out.wrapping_add(black_box(child.try_get()?));
                        Ok(())
                    })?;
                    root.commit()?;
                }
                (0, 34) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    root.value_mut().try_for_each_mut(|child| child.set(123))?;
                    root.commit()?;
                }
                (2, 33) => {
                    let mut root = PublicKeyMap::load_checked_mut(&mut store)?;
                    root.value_mut().try_for_each_mut(|key, child| {
                        out = out
                            .wrapping_add(key_sum(key).wrapping_add(black_box(child.try_get()?)));
                        Ok(())
                    })?;
                    root.commit()?;
                }
                (2, 34) => {
                    let mut root = PublicKeyMap::load_checked_mut(&mut store)?;
                    root.value_mut().try_for_each_mut(|key, child| {
                        black_box(key_sum(key));
                        child.set(123)
                    })?;
                    root.commit()?;
                }

                (0, 70) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    for i in 0..n {
                        root.value_mut().get_mut(i)?.unwrap().set(123)?;
                    }
                    root.commit()?;
                }
                (2, 30) => {
                    let mut root = PublicKeyMap::load_checked_mut(&mut store)?;
                    {
                        let mut cursor = root.value_mut().cursor_mut()?;
                        while let Some((key, child)) = cursor.next()? {
                            out = out.wrapping_add(
                                key_sum(key).wrapping_add(black_box(child.try_get()?)),
                            );
                        }
                    }
                    root.commit()?;
                }
                (2, 31) => {
                    let mut root = PublicKeyMap::load_checked_mut(&mut store)?;
                    {
                        let mut cursor = root.value_mut().cursor_mut()?;
                        while let Some((key, mut child)) = cursor.next()? {
                            black_box(key_sum(key));
                            child.set(123)?;
                        }
                    }
                    root.commit()?;
                }
                (1, 63) => {
                    let mut root = Words::load_checked_mut(&mut store)?;
                    root.value_mut().fill(black_box("replacement grows"))?;
                    root.commit()?;
                }
                (3, 63) => {
                    let mut root = KeyWords::load_checked_mut(&mut store)?;
                    root.value_mut()
                        .fill_values(black_box("replacement grows"))?;
                    root.commit()?;
                }
                (1, 64) => {
                    let mut root = Words::load_checked_mut(&mut store)?;
                    root.value_mut().rewrite_from(&black_box(&VALUES)[..n])?;
                    root.commit()?;
                }
                (3, 64) => {
                    let mut root = KeyWords::load_checked_mut(&mut store)?;
                    root.value_mut()
                        .rewrite_values_from(&black_box(&ENTRIES)[..n])?;
                    root.commit()?;
                }
                (3, 71) => {
                    let mut root = KeyWords::load_checked_mut(&mut store)?;
                    let entries: alloc::vec::Vec<_> = root
                        .value()
                        .iter()
                        .enumerate()
                        .map(|(i, (k, _))| (k, TEXTS[i % 18]))
                        .collect();
                    root.value_mut().rewrite_values_from(&entries)?;
                    root.commit()?;
                }
                _ => return Err(ProgramError::InvalidInstructionData),
            }
        }
        pinocchio::cpi::set_return_data(&out.to_le_bytes());
        Ok(())
    }
}
