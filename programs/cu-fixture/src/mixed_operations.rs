//! Ordinary mixed-operation caller, based on the reviewer probe.
//! Keep callers and closures free of code-generation attributes.

extern crate alloc;

use crate::schemas::{KeyWords, Numbers, PublicKeyMap, Words};
use blubber::pinocchio::{Funding, ReadStore, Store};
use blubber::ErrorKind;
use core::hint::black_box;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

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
    let mut values = [""; 1000];
    let mut i = 0;
    while i < values.len() {
        values[i] = TEXTS[i % 18];
        i += 1;
    }
    values
}

const fn entries(common: bool) -> [(Address, &'static str); 1000] {
    let mut entries = [(Address::new_from_array([0; 32]), ""); 1000];
    let mut i = 0;
    while i < entries.len() {
        let mut key = [42; 32];
        let word = (2 * (i + 1) as u32).to_be_bytes();
        let at = if common { 28 } else { 0 };
        let mut j = 0;
        while j < 4 {
            key[at + j] = word[j];
            j += 1;
        }
        entries[i] = (Address::new_from_array(key), TEXTS[i % 18]);
        i += 1;
    }
    entries
}

static VALUES: [&str; 1000] = values();
static ENTRIES: [(Address, &str); 1000] = entries(false);
static COMMON_ENTRIES: [(Address, &str); 1000] = entries(true);

fn key_sum(key: Address) -> u64 {
    let b = key.to_bytes();
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

fn text_sum(x: &str) -> u64 {
    let bytes = black_box(x).as_bytes();
    if let Ok(word) = <&[u8; 8]>::try_from(bytes) {
        let word = u64::from_le_bytes(*word);
        let lanes =
            (word & 0x00ff_00ff_00ff_00ff).wrapping_add((word >> 8) & 0x00ff_00ff_00ff_00ff);
        return lanes.wrapping_mul(0x0001_0001_0001_0001) >> 48;
    }
    let (chunks, tail) = bytes.as_chunks::<8>();
    let mut sum = 0u64;
    for chunk in chunks {
        let word = u64::from_le_bytes(*chunk);
        let lanes =
            (word & 0x00ff_00ff_00ff_00ff).wrapping_add((word >> 8) & 0x00ff_00ff_00ff_00ff);
        sum = sum.wrapping_add(lanes.wrapping_mul(0x0001_0001_0001_0001) >> 48);
    }
    for byte in tail {
        sum = sum.wrapping_add(u64::from(*byte));
    }
    sum
}

/// Dispatches all operations from one unannotated program with validated loads.
fn ordinary(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [layout, op, lo, hi, common] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = u16::from_le_bytes([*lo, *hi]) as usize;
    if n > 1000 || *common > 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut out = 0u64;
    if matches!(*op, 0 | 6 | 7) {
        let store = ReadStore::new(state, id)?;
        match *layout {
            0 => {
                let root = Numbers::load(store.bytes())?;
                if *op == 6 {
                    for value in root.value().iter() {
                        out = out.wrapping_add(black_box(value));
                    }
                } else if *op == 7 {
                    for value in root.value().try_iter() {
                        out = out.wrapping_add(black_box(value?));
                    }
                }
            }
            2 => {
                let root = PublicKeyMap::load(store.bytes())?;
                if *op == 6 {
                    for (key, value) in root.value().iter() {
                        out = out.wrapping_add(key_sum(key).wrapping_add(black_box(value)));
                    }
                } else if *op == 7 {
                    for item in root.value().try_iter() {
                        let (key, value) = item?;
                        out = out.wrapping_add(key_sum(key).wrapping_add(black_box(value)));
                    }
                }
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    } else {
        // Indexed values start at eight bytes and replacements are at most
        // seventeen bytes, including the recipe built from old values.
        let room = if matches!(*layout, 1 | 3) && matches!(*op, 63 | 64 | 66 | 71..=73) {
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
        if matches!(*layout, 1 | 3) {
            out = edit_indexed(&mut store, *layout, *op, n, *common)?;
        } else {
            match (*layout, *op) {
                (0, 26 | 27) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    if *op == 26 {
                        for value in root.value_mut().iter() {
                            out = out.wrapping_add(black_box(value));
                        }
                    } else {
                        for value in root.value_mut().try_iter()? {
                            out = out.wrapping_add(black_box(value?));
                        }
                    }
                    root.commit()?;
                }
                (2, 26 | 27) => {
                    let mut root = PublicKeyMap::load_checked_mut(&mut store)?;
                    if *op == 26 {
                        for (key, value) in root.value_mut().iter() {
                            out = out.wrapping_add(key_sum(key).wrapping_add(black_box(value)));
                        }
                    } else {
                        for item in root.value_mut().try_iter()? {
                            let (key, value) = item?;
                            out = out.wrapping_add(key_sum(key).wrapping_add(black_box(value)));
                        }
                    }
                    root.commit()?;
                }

                (0, 20) => {
                    Numbers::load_checked_mut(&mut store)?.commit()?;
                }
                (2, 20) => {
                    PublicKeyMap::load_checked_mut(&mut store)?.commit()?;
                }

                (0, 30) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    {
                        let mut cursor = root.value_mut().cursor_mut()?;
                        while let Some(child) = cursor.next()? {
                            out = out.wrapping_add(black_box(child.try_get()?));
                        }
                    }
                    root.commit()?;
                }
                (0, 31) => {
                    let mut root = Numbers::load_checked_mut(&mut store)?;
                    {
                        let mut cursor = root.value_mut().cursor_mut()?;
                        while let Some(mut child) = cursor.next()? {
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
                (2, 32) => {
                    let mut root = PublicKeyMap::load_checked_mut(&mut store)?;
                    root.value_mut().try_for_each_mut(|key, child| {
                        black_box(key_sum(key));
                        black_box(child);
                        Ok(())
                    })?;
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

                _ => return Err(ProgramError::InvalidInstructionData),
            }
        }
    }
    pinocchio::cpi::set_return_data(&out.to_le_bytes());
    Ok(())
}

// Indexed traversal and batch replacements share an ordinary caller frame.
fn edit_indexed(
    store: &mut Store<'_>,
    layout: u8,
    op: u8,
    n: usize,
    common: u8,
) -> Result<u64, ProgramError> {
    let mut out = 0u64;
    match (layout, op) {
        (1, 30) => {
            let mut root = Words::load_checked_mut(store)?;
            {
                let mut cursor = root.value_mut().cursor_mut()?;
                while let Some(child) = cursor.next()? {
                    out = out.wrapping_add(text_sum(child.try_get()?));
                }
            }
            root.commit()?;
        }
        (1, 31) => {
            let mut root = Words::load_checked_mut(store)?;
            {
                let mut cursor = root.value_mut().cursor_mut()?;
                while let Some(mut child) = cursor.next()? {
                    child.set("sameSIZE")?;
                }
            }
            root.commit()?;
        }
        (1, 33) => {
            let mut root = Words::load_checked_mut(store)?;
            root.value_mut().try_for_each_mut(|child| {
                out = out.wrapping_add(text_sum(child.try_get()?));
                Ok(())
            })?;
            root.commit()?;
        }
        (1, 34) => {
            let mut root = Words::load_checked_mut(store)?;
            root.value_mut()
                .try_for_each_mut(|child| child.set("sameSIZE"))?;
            root.commit()?;
        }
        (3, 30) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            {
                let mut cursor = root.value_mut().cursor_mut()?;
                while let Some((key, child)) = cursor.next()? {
                    out = out.wrapping_add(key_sum(key));
                    out = out.wrapping_add(text_sum(child.try_get()?));
                }
            }
            root.commit()?;
        }
        (3, 31) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            {
                let mut cursor = root.value_mut().cursor_mut()?;
                while let Some((key, mut child)) = cursor.next()? {
                    black_box(key_sum(key));
                    child.set("sameSIZE")?;
                }
            }
            root.commit()?;
        }
        (3, 33) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            root.value_mut().try_for_each_mut(|key, child| {
                out = out.wrapping_add(key_sum(key));
                out = out.wrapping_add(text_sum(child.try_get()?));
                Ok(())
            })?;
            root.commit()?;
        }
        (3, 34) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            root.value_mut().try_for_each_mut(|key, child| {
                black_box(key_sum(key));
                child.set("sameSIZE")
            })?;
            root.commit()?;
        }
        (1, 20) => {
            Words::load_checked_mut(store)?.commit()?;
        }
        (3, 20) => {
            KeyWords::load_checked_mut(store)?.commit()?;
        }
        (1, 63) => {
            let mut root = Words::load_checked_mut(store)?;
            root.value_mut().fill(black_box("replacement grows"))?;
            root.commit()?;
        }
        (3, 63) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            root.value_mut()
                .fill_values(black_box("replacement grows"))?;
            root.commit()?;
        }
        (1, 64) => {
            let mut root = Words::load_checked_mut(store)?;
            root.value_mut().rewrite_from(&black_box(&VALUES)[..n])?;
            root.commit()?;
        }
        (3, 64) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            let entries = if common == 1 {
                &COMMON_ENTRIES
            } else {
                &ENTRIES
            };
            root.value_mut()
                .rewrite_values_from(&black_box(entries)[..n])?;
            root.commit()?;
        }
        (3, 66) => {
            let mut root = KeyWords::load_checked_mut(store)?;
            root.value_mut().rewrite_values(&black_box(&VALUES)[..n])?;
            root.commit()?;
        }
        (3, 71..=73) => {
            grow_from_old_values(store, op, n)?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(out)
}

// Per-value callback rows retained for comparison with the batched recipe.
fn grow_from_old_values(store: &mut Store<'_>, op: u8, n: usize) -> ProgramResult {
    use alloc::{string::String, vec::Vec};
    let mut root = KeyWords::load_checked_mut(store)?;
    let keyed = op == 72;
    let ends_needed = op != 73;
    let mut buf = String::with_capacity(17 * n);
    let mut ends = Vec::with_capacity(if ends_needed { n } else { 0 });
    let mut keys = Vec::with_capacity(if keyed { n } else { 0 });
    root.value_mut().try_for_each_mut(|key, child| {
        let old = child.try_get()?;
        buf.push_str(old);
        buf.push_str(old);
        buf.push('!');
        if ends_needed {
            ends.push(buf.len());
        }
        if keyed {
            keys.push(key);
        }
        Ok(())
    })?;
    // All account reads end before the mutable rewrite loan.
    let mut a = 0;
    let values = (0..n).map(|i| {
        let b = if ends_needed { ends[i] } else { 17 * (i + 1) };
        let value = &buf[a..b];
        a = b;
        value
    });
    if keyed {
        let entries: Vec<_> = keys.into_iter().zip(values).collect();
        root.value_mut().rewrite_values_from(&entries)?;
    } else {
        let values: Vec<&str> = values.collect();
        root.value_mut().rewrite_values(&values)?;
    }
    root.commit()?;
    Ok(())
}

fn grow_from_old_values_batched(store: &mut Store<'_>, op: u8, n: usize) -> blubber::Result<()> {
    use alloc::vec::Vec;

    fn grow(old: &str) -> blubber::Result<[u8; 17]> {
        let old: &[u8; 8] = old
            .as_bytes()
            .try_into()
            .map_err(|_| ErrorKind::InvalidBoundary)?;
        let mut value = [b'!'; 17];
        value[..8].copy_from_slice(old);
        value[8..16].copy_from_slice(old);
        Ok(value)
    }

    let mut root = KeyWords::load_checked_mut(store)?;
    if op == 74 {
        let mut staged = Vec::with_capacity(n);
        for item in root.value().try_iter_batched() {
            let (_, old) = item?;
            staged.push(grow(old)?);
        }
        let text =
            core::str::from_utf8(staged.as_flattened()).map_err(|_| ErrorKind::InvalidUtf8)?;
        let mut values = Vec::with_capacity(n);
        for i in 0..n {
            values.push(
                text.get(17 * i..17 * (i + 1))
                    .ok_or(ErrorKind::InvalidUtf8)?,
            );
        }
        root.value_mut().rewrite_values(&values)?;
    } else {
        root.value_mut()
            .rewrite_values_with(17 * n, |old, output| {
                let old: &[u8; 8] = old
                    .as_bytes()
                    .try_into()
                    .map_err(|_| ErrorKind::InvalidBoundary)?;
                let output = output.first_chunk_mut::<17>().ok_or(ErrorKind::Capacity)?;
                output[..8].copy_from_slice(old);
                output[8..16].copy_from_slice(old);
                output[16] = b'!';
                Ok(17)
            })?;
    }
    root.commit()?;
    Ok(())
}

// Order-sensitive consumption of every returned key and value byte.
fn iteration_bytes(mut result: u64, bytes: &[u8]) -> u64 {
    for &byte in black_box(bytes) {
        result = (result ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    result
}

/// Dispatches the ordinary inventory, including indexed shared iteration.
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    if let [3, op @ (74 | 75), lo, hi, common] = ix {
        let n = u16::from_le_bytes([*lo, *hi]) as usize;
        if n > 1000 || *common > 1 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let mut store = Store::new_with_headroom(
            state,
            id,
            Funding::Prepaid,
            blubber::pinocchio::Headroom::UpTo(9 * n),
        )?;
        grow_from_old_values_batched(&mut store, *op, n)?;
        pinocchio::cpi::set_return_data(&0u64.to_le_bytes());
        return Ok(());
    }
    if let [layout @ (1 | 3), op @ (0 | 6 | 7 | 26 | 27), lo, hi, common] = ix {
        if u16::from_le_bytes([*lo, *hi]) > 1000 || *common > 1 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let out = if matches!(*op, 0 | 6 | 7) {
            let store = ReadStore::new(state, id)?;
            iterate_indexed_ref(&store, *layout, *op)?
        } else {
            let mut store = Store::new(state, id, Funding::Prepaid)?;
            iterate_indexed_mut(&mut store, *layout, *op)?
        };
        pinocchio::cpi::set_return_data(&out.to_le_bytes());
        return Ok(());
    }
    ordinary(id, accounts, ix)
}

fn iterate_indexed_ref(store: &ReadStore<'_>, layout: u8, op: u8) -> Result<u64, ProgramError> {
    let mut out = 0u64;
    match layout {
        1 => {
            let root = Words::load(store.bytes())?;
            if op == 6 {
                for value in root.value().iter() {
                    out = iteration_bytes(out, value.as_bytes());
                }
            } else if op == 7 {
                for value in root.value().try_iter() {
                    out = iteration_bytes(out, value?.as_bytes());
                }
            }
        }
        3 => {
            let root = KeyWords::load(store.bytes())?;
            if op == 6 {
                for (key, value) in root.value().iter() {
                    out = iteration_bytes(iteration_bytes(out, &key.to_bytes()), value.as_bytes());
                }
            } else if op == 7 {
                for item in root.value().try_iter() {
                    let (key, value) = item?;
                    out = iteration_bytes(iteration_bytes(out, &key.to_bytes()), value.as_bytes());
                }
            }
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(out)
}

// Ordinary shared iteration from an exclusive session; no child editor escapes.
fn iterate_indexed_mut(store: &mut Store<'_>, layout: u8, op: u8) -> Result<u64, ProgramError> {
    let mut out = 0u64;
    match layout {
        1 => {
            let mut root = Words::load_checked_mut(store)?;
            if op == 26 {
                for value in root.value_mut().iter() {
                    out = iteration_bytes(out, value.as_bytes());
                }
            } else {
                for value in root.value_mut().try_iter()? {
                    out = iteration_bytes(out, value?.as_bytes());
                }
            }
            root.commit()?;
        }
        3 => {
            let mut root = KeyWords::load_checked_mut(store)?;
            if op == 26 {
                for (key, value) in root.value_mut().iter() {
                    out = iteration_bytes(iteration_bytes(out, &key.to_bytes()), value.as_bytes());
                }
            } else {
                for item in root.value_mut().try_iter()? {
                    let (key, value) = item?;
                    out = iteration_bytes(iteration_bytes(out, &key.to_bytes()), value.as_bytes());
                }
            }
            root.commit()?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(out)
}
