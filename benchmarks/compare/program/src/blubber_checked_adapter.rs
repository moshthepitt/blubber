//! Explicit checked ingress with the unchanged comparison operations and oracles.
use super::*;
use blubber::pinocchio::{Funding, ReadStore, Store};
use blubber::Pubkey;
use blubber::{List, Map, Str};
#[blubber::schema(account)]
/// Matched format-1 account used by the speed fixture.
pub struct Numbers {
    /// Complete collection measured by this workload.
    pub value: List<u64>,
}
#[blubber::schema(account)]
/// Matched format-1 account used by the speed fixture.
pub struct PublicKeyMap {
    /// Complete collection measured by this workload.
    pub value: Map<Pubkey, u64>,
}
#[blubber::schema(account)]
/// Matched format-1 account used by the speed fixture.
pub struct Words {
    /// Complete collection measured by this workload.
    pub value: List<Str>,
}
#[blubber::schema(account)]
/// Matched format-1 account used by the speed fixture.
pub struct KeyWords {
    /// Complete collection measured by this workload.
    pub value: Map<Pubkey, Str>,
}
/// Executes a checked or lazy comparison operation with the selected keys.
pub fn run<const TRUSTED: bool>(
    id: &Address,
    state: &mut AccountView,
    w: u8,
    op: u8,
    n: usize,
) -> Result<u64, ProgramError> {
    let mut out = 0;
    if op == 0 || matches!((w, op), (1, 1 | 2) | (2, 1 | 5) | (3, 1) | (4, 1)) {
        let store = ReadStore::new(state, id)?;
        match w {
            1 => {
                let root = if TRUSTED {
                    Numbers::load_trusted(store.bytes())?
                } else {
                    Numbers::load(store.bytes())?
                };
                let v = root.value();
                if op == 1 {
                    out = v.iter().sum();
                } else if op == 2 {
                    out = v.get(n / 2).unwrap_or(0);
                }
            }
            2 => {
                let root = if TRUSTED {
                    PublicKeyMap::load_trusted(store.bytes())?
                } else {
                    PublicKeyMap::load(store.bytes())?
                };
                let v = root.value();
                if op == 1 {
                    out = v
                        .get(&Pubkey::from_bytes(middle_key(n)))
                        .unwrap_or(0);
                } else if op == 5 {
                    out = v.iter().map(|(_, x)| x).sum();
                }
            }
            3 => {
                let root = if TRUSTED {
                    Words::load_trusted(store.bytes())?
                } else {
                    Words::load(store.bytes())?
                };
                if op == 1 {
                    for x in root.value().try_iter() {
                        out += text_sum(x?);
                    }
                }
            }
            4 => {
                let root = if TRUSTED {
                    KeyWords::load_trusted(store.bytes())?
                } else {
                    KeyWords::load(store.bytes())?
                };
                if op == 1 {
                    out = root
                        .value()
                        .get(&Pubkey::from_bytes(middle_key(n)))
                        .map(text_sum)
                        .unwrap_or(0);
                }
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    } else {
        // Mutable comparison families share one idle across growing and
        // equal-width operations. Keep its explicit Max policy comparable.
        let mut store = Store::new_with_headroom(
            state,
            id,
            Funding::Prepaid,
            blubber::pinocchio::Headroom::Max,
        )?;
        match w {
            1 => {
                let mut root = if TRUSTED {
                    Numbers::load_trusted_mut(&mut store)?
                } else {
                    Numbers::load_checked_mut(&mut store)?
                };
                match op {
                    10 => {}
                    3 => {
                        root.value_mut().try_for_each_mut(|child| child.set(SET))?;
                    }
                    6 => root.value_mut().fill(SET)?,
                    4 => root.value_mut().push(SET)?,
                    5 => {
                        if n > 0 {
                            root.value_mut().remove(0)?;
                        }
                    }
                    _ => unreachable!(),
                }
                root.commit()?;
            }
            2 => {
                let mut root = if TRUSTED {
                    PublicKeyMap::load_trusted_mut(&mut store)?
                } else {
                    PublicKeyMap::load_checked_mut(&mut store)?
                };
                match op {
                    10 => {}
                    2 => {
                        root.value_mut()
                            .insert(Pubkey::from_bytes(insert_key(n)), SET)?;
                    }
                    3 => {
                        root.value_mut()
                            .remove(&Pubkey::from_bytes(middle_key(n)))?;
                    }
                    4 => root.value_mut().fill_values(SET)?,
                    _ => unreachable!(),
                }
                root.commit()?;
            }
            3 => {
                let mut root = if TRUSTED {
                    Words::load_trusted_mut(&mut store)?
                } else {
                    Words::load_checked_mut(&mut store)?
                };
                match op {
                    10 => {}
                    2 | 3 => {
                        if n > 0 {
                            root.value_mut().get_mut(n / 2)?.unwrap().set(if op == 2 {
                                EQUAL
                            } else {
                                GROWN
                            })?;
                        }
                    }
                    4 => root.value_mut().fill(GROWN)?,
                    5 => {
                        let mut staged = Vec::<[u8; 17]>::with_capacity(n);
                        for old in root.value().try_iter() {
                            staged.push(grow(old?));
                        }
                        let values: Vec<&str> = staged
                            .iter()
                            .map(|s| core::str::from_utf8(s).unwrap())
                            .collect();
                        root.value_mut().rewrite_from(&values)?;
                    }
                    _ => unreachable!(),
                }
                root.commit()?;
            }
            4 => {
                let mut root = if TRUSTED {
                    KeyWords::load_trusted_mut(&mut store)?
                } else {
                    KeyWords::load_checked_mut(&mut store)?
                };
                match op {
                    10 => {}
                    2 => {
                        root.value_mut()
                            .insert(Pubkey::from_bytes(insert_key(n)), EQUAL)?;
                    }
                    3 => {
                        let mut staged = Vec::<[u8; 17]>::with_capacity(n);
                        for item in root.value().try_iter() {
                            let (_, old) = item?;
                            staged.push(grow(old));
                        }
                        let values: Vec<&str> = staged
                            .iter()
                            .map(|s| core::str::from_utf8(s).unwrap())
                            .collect();
                        root.value_mut().rewrite_values(&values)?;
                    }
                    _ => unreachable!(),
                }
                root.commit()?;
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    }
    Ok(out)
}
