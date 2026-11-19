//! Fuzz commands shared by ASan replay and genuine SDK account sessions.
use blubber::{Entry, List, Map, Str};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

/// Mixed fixed/indexed root with independently corruptible distant children.
#[blubber::schema(account, namespace = "tests:d21-corrupt")]
pub struct State {
    /// Checked fixed domain.
    pub flag: bool,
    /// Fixed geometry with domain-constrained keys and values.
    pub fixed: Map<bool, bool>,
    /// Indexed UTF-8 values.
    pub items: List<Str>,
    /// Indexed map with canonical byte keys.
    pub entries: Map<u8, Str>,
    /// Unselected suffix and frame sentinel.
    pub tail: Str,
}

/// Applies one fallible command. Full-width indices are never narrowed.
/// The input is exactly eight bytes; at most 32 commands run per invocation.
pub fn edit(root: &mut StateMut<'_>, command: &[u8; 8]) -> blubber::Result<()> {
    let index = u32::from_le_bytes(command[4..8].try_into().expect("four bytes")) as usize;
    let text = ["", "x", "new", "λ"][command[3] as usize % 4];
    match command[0] % 16 {
        0 => root.flag_mut().set(command[1] & 1 != 0),
        1 => root.items_mut().at_mut(index)?.set(text),
        // Byte 2 selects the list route; zero keeps the original command.
        2 => grow(root, command, index, text),
        3 => shrink(root, command[2], index),
        // Byte 2 selects the map route; zero keeps the original command.
        4 => put(root, command, text),
        5 => take(root, command),
        6 => root.fixed_mut().fill_values(command[1] & 1 != 0),
        7 => {
            let map = root.fixed_mut();
            let mut cursor = map.cursor_mut()?;
            for _ in 0..4 {
                let Some((_, mut slot)) = cursor.next()? else {
                    break;
                };
                slot.set(command[1] & 1 != 0)?;
            }
            Ok(())
        }
        8 => root
            .items_mut()
            .rewrite_from(&[text; 4][..command[1] as usize % 5]),
        9 => walk(root, command[2], text),
        10 => root.verify(),
        // The session runs foreign identity, interrupts, checked loads and reopens.
        _ => Ok(()),
    }
}

// Separate frames keep each list route within the 4,096-byte SBF stack frame.
#[inline(never)]
fn grow(
    root: &mut StateMut<'_>,
    command: &[u8; 8],
    index: usize,
    text: &str,
) -> blubber::Result<()> {
    let run = &[text; 2][..command[1] as usize % 3];
    match command[2] % 4 {
        0 => push(root, text),
        1 => insert(root, index, text),
        2 => extend(root, run),
        _ => insert_run(root, index, run),
    }
}

#[inline(never)]
fn push(root: &mut StateMut<'_>, text: &str) -> blubber::Result<()> {
    root.items_mut().push(text)
}

#[inline(never)]
fn insert(root: &mut StateMut<'_>, index: usize, text: &str) -> blubber::Result<()> {
    root.items_mut().insert(index, text)
}

#[inline(never)]
fn extend(root: &mut StateMut<'_>, run: &[&str]) -> blubber::Result<()> {
    root.items_mut().extend_from_slice(run)
}

#[inline(never)]
fn insert_run(root: &mut StateMut<'_>, index: usize, run: &[&str]) -> blubber::Result<()> {
    root.items_mut().insert_from_slice(index, run)
}

#[inline(never)]
fn shrink(root: &mut StateMut<'_>, route: u8, index: usize) -> blubber::Result<()> {
    match route % 4 {
        0 => remove(root, index),
        1 => clear(root),
        2 => truncate(root, index),
        _ => pop(root),
    }
}

#[inline(never)]
fn remove(root: &mut StateMut<'_>, index: usize) -> blubber::Result<()> {
    root.items_mut().remove(index).map(|_| ())
}

#[inline(never)]
fn clear(root: &mut StateMut<'_>) -> blubber::Result<()> {
    root.items_mut().clear()
}

#[inline(never)]
fn truncate(root: &mut StateMut<'_>, len: usize) -> blubber::Result<()> {
    root.items_mut().truncate(len)
}

#[inline(never)]
fn pop(root: &mut StateMut<'_>) -> blubber::Result<()> {
    root.items_mut().pop().map(|_| ())
}

#[inline(never)]
fn put(root: &mut StateMut<'_>, command: &[u8; 8], text: &str) -> blubber::Result<()> {
    let key = command[1];
    match command[2] % 4 {
        0 => insert_entry(root, key, text),
        1 => or_insert(root, key, text),
        2 => entry_insert(root, key, text),
        _ => replace_value(root, key, text),
    }
}

#[inline(never)]
fn insert_entry(root: &mut StateMut<'_>, key: u8, text: &str) -> blubber::Result<()> {
    root.entries_mut().insert(key, text).map(|_| ())
}

#[inline(never)]
fn or_insert(root: &mut StateMut<'_>, key: u8, text: &str) -> blubber::Result<()> {
    root.entries_mut().entry(key)?.or_insert(text).map(|_| ())
}

#[inline(never)]
fn entry_insert(root: &mut StateMut<'_>, key: u8, text: &str) -> blubber::Result<()> {
    match root.entries_mut().entry(key)? {
        Entry::Occupied(entry) => entry.insert(text).map(|_| ()),
        Entry::Vacant(entry) => entry.insert(text).map(|_| ()),
    }
}

#[inline(never)]
fn replace_value(root: &mut StateMut<'_>, key: u8, text: &str) -> blubber::Result<()> {
    match root.entries_mut().get_mut(&key)? {
        Some(mut value) => value.set(text),
        None => Ok(()),
    }
}

#[inline(never)]
fn take(root: &mut StateMut<'_>, command: &[u8; 8]) -> blubber::Result<()> {
    match command[2] % 3 {
        0 => remove_entry(root, command[1]),
        1 => clear_entries(root),
        _ => entry_remove(root, command[1]),
    }
}

#[inline(never)]
fn remove_entry(root: &mut StateMut<'_>, key: u8) -> blubber::Result<()> {
    root.entries_mut().remove(&key).map(|_| ())
}

#[inline(never)]
fn clear_entries(root: &mut StateMut<'_>) -> blubber::Result<()> {
    root.entries_mut().clear()
}

#[inline(never)]
fn entry_remove(root: &mut StateMut<'_>, key: u8) -> blubber::Result<()> {
    match root.entries_mut().entry(key)? {
        Entry::Occupied(entry) => entry.remove().map(|_| ()),
        Entry::Vacant(_) => Ok(()),
    }
}

#[inline(never)]
fn walk(root: &mut StateMut<'_>, route: u8, text: &str) -> blubber::Result<()> {
    if route.is_multiple_of(2) {
        return root
            .entries_mut()
            .try_for_each_mut(|_, slot| slot.set(text));
    }
    walk_cursor(root, text)
}

// A cursor sets at most four values; each set completes before the next visit.
#[inline(never)]
fn walk_cursor(root: &mut StateMut<'_>, text: &str) -> blubber::Result<()> {
    let mut cursor = root.entries_mut().cursor_mut()?;
    for _ in 0..4 {
        let Some((_, mut slot)) = cursor.next()? else {
            break;
        };
        slot.set(text)?;
    }
    Ok(())
}

/// Exercises fallible selections, domains and fused iteration on corrupt bytes.
/// Returns a digest of the observed value: present text, a miss or visited pairs.
pub fn read(root: StateRef<'_>, command: &[u8; 8]) -> blubber::Result<u64> {
    let index = u32::from_le_bytes(command[4..8].try_into().expect("four bytes")) as usize;
    match command[0] % 6 {
        0 => root.try_flag().map(u64::from),
        1 => Ok(text(root.try_items()?.try_get(index)?)),
        2 => Ok(text(root.try_entries()?.try_get(&command[1])?)),
        3 => {
            let mut iter = root.try_fixed()?.try_iter();
            let mut hash = SEED;
            for _ in 0..32 {
                match iter.next() {
                    Some(Ok((key, value))) => hash = mix(hash, &[u8::from(key), u8::from(value)]),
                    Some(Err(error)) => {
                        assert!(iter.next().is_none());
                        assert!(iter.next().is_none());
                        return Err(error);
                    }
                    None => {
                        assert!(iter.next().is_none());
                        break;
                    }
                }
            }
            Ok(hash >> 1)
        }
        4 => root.try_tail().map(|tail| digest(tail.as_bytes())),
        _ => root.verify().map(|()| 0),
    }
}

/// Read digest of an absent selection; real digests clear the top bit.
pub const MISSING: u64 = u64::MAX;
const SEED: u64 = 0xcbf2_9ce4_8422_2325;

fn mix(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// FNV-1a over observed bytes, shifted so it never equals [`MISSING`].
pub fn digest(bytes: &[u8]) -> u64 {
    mix(SEED, bytes) >> 1
}

fn text(value: Option<&str>) -> u64 {
    value.map_or(MISSING, |value| digest(value.as_bytes()))
}

/// Trace bytes per command: result code, read digest and poison flag.
pub const RECORD: usize = 17;
/// Most commands per invocation.
pub const COMMANDS: usize = 32;
/// Trace bytes before the snapshot: records, then the final account length.
pub const TRACE: usize = COMMANDS * RECORD + 4;
/// Mode value for the self-invoked callee of a CPI reopen.
pub const CALLEE: u8 = 4;

/// Executes authenticated lazy or checked ingress under real SDK guards.
/// Expected library failures are returned as trace words, so partial bytes and
/// poison can be inspected before transaction rollback. VM faults fail the run.
///
/// Edit commands 14 and 15 first complete the session. Drop trims to a
/// well-formed used length, and volatile poison does not survive. Command 15
/// also invokes this program on the account before the new session opens.
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    use blubber::{pinocchio::ReadStore, ReadStorage, Resize};
    let [mode, commands @ ..] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    if *mode == CALLEE {
        return callee(id, accounts);
    }
    let [account, trace] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let mut output = trace.try_borrow_mut()?;
    if output.len() != TRACE + account.data_len() || commands.len() > COMMANDS * 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (records, snapshot) = output.split_at_mut(COMMANDS * RECORD);
    let (records, _) = records.as_chunks_mut::<RECORD>();
    if mode & 1 == 0 {
        let store = ReadStore::new(account, id)?;
        for (command, out) in commands.as_chunks::<8>().0.iter().zip(records.iter_mut()) {
            let result = if mode & 2 == 0 {
                store.load::<State>()
            } else {
                store.load_checked::<State>()
            }
            .and_then(|root| read(root, command));
            record(out, result, false);
        }
        publish(snapshot, store.bytes());
    } else {
        let mut store = open(account, id)?;
        for (command, out) in commands.as_chunks::<8>().0.iter().zip(records.iter_mut()) {
            let op = command[0] % 16;
            if op == 14 || op == 15 {
                drop(store);
                if op == 15 {
                    reenter(id, account)?;
                }
                store = open(account, id)?;
            }
            let result = if op == 11 {
                store.load::<super::Numbers>().map(|_| ())
            } else if op == 12 {
                interrupt(blubber::raw::backend(&mut store))
            } else if op == 13 {
                store.load_checked::<State>().map(|_| ())
            } else {
                let loaded = if mode & 2 == 0 {
                    store.load::<State>()
                } else {
                    store.load_checked::<State>()
                };
                loaded.and_then(|mut root| edit(&mut root, command))
            };
            record(out, result.map(|()| 0), store.is_poisoned());
        }
        // Observe physical spare before the ordinary best-effort Drop trim.
        publish(snapshot, store.bytes());
    }
    Ok(())
}

fn open<'a>(
    account: &'a mut AccountView,
    id: &Address,
) -> blubber::Result<blubber::pinocchio::Store<'a>> {
    use blubber::pinocchio::{Funding, Headroom, Store};
    Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::UpTo(0))
}

// Self-CPI with the account writable; the runtime syncs it across the call.
fn reenter(id: &Address, account: &AccountView) -> ProgramResult {
    use pinocchio::instruction::{InstructionAccount, InstructionView};
    pinocchio::cpi::invoke::<1, _>(
        &InstructionView {
            program_id: id,
            accounts: &[InstructionAccount::writable(account.address())],
            data: &[0x77, CALLEE],
        },
        &[account],
    )
}

// Lazy typed access inside the callee. Its result is dropped: an error would
// abort the caller's whole history, and host replay has no callee to mirror.
fn callee(id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let store = blubber::pinocchio::ReadStore::new(account, id)?;
    let _ = store.load::<State>().and_then(|root| root.verify());
    Ok(())
}

fn record(out: &mut [u8], result: blubber::Result<u64>, poisoned: bool) {
    let value = result.as_ref().map_or(0, |value| *value);
    out[..8].copy_from_slice(&code(result.map(|_| ())).to_le_bytes());
    out[8..16].copy_from_slice(&value.to_le_bytes());
    out[16] = u8::from(poisoned);
}

// Final length word, then the exposed bytes; a trimmed account leaves zeros.
fn publish(snapshot: &mut [u8], bytes: &[u8]) {
    let (len, rest) = snapshot.split_at_mut(4);
    len.copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    rest[..bytes.len()].copy_from_slice(bytes);
}

/// Stable trace encoding; zero denotes success, errors use the SDK conversion.
pub fn code(result: blubber::Result<()>) -> u64 {
    result.map_or_else(|error| u64::from(ProgramError::from(error)), |()| 0)
}

/// Simulates a returned custom encoder error after its first exact slot write.
/// Raw publication ends approved history; only explicit checked recovery may
/// admit it again. No panic or unwind is intercepted by the corrupt target.
pub fn interrupt(store: &mut dyn blubber::raw::Resize) -> blubber::Result<()> {
    use blubber_core::raw::{InlinePath, RewritePlan};
    blubber::raw::rewrite_collection(
        store,
        &blubber_core::Layout::Flat {
            checked: <State as blubber::RootSchema>::__layout(),
        },
        &InlinePath::root_field(2),
        (State::DISCRIMINATOR, <State as blubber::Schema>::MIN_SIZE),
        |view| {
            let count = u32::from_le_bytes(
                view.bytes()
                    .get(..4)
                    .ok_or(blubber::ErrorKind::Truncated)?
                    .try_into()
                    .expect("four bytes"),
            );
            if count == 0 || count > 4 {
                return Err(blubber::ErrorKind::InvalidBoundary.into());
            }
            Ok((
                RewritePlan::check(view.bytes(), &view.layout(), count, 3 * count)?,
                (),
            ))
        },
        |(), stream| {
            stream.prepare_slot(3)?;
            stream.destination().copy_from_slice(b"new");
            Err(blubber::ErrorKind::InvalidBoundary.into())
        },
    )
}
