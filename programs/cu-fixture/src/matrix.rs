//! Dynamic collection geometries for the existing SBF benchmark dispatcher.
//!
//! The host constructs each input and its expected complete encoding independently.
//! Read operations retain their result through `black_box`; mutable operations use
//! fresh leases, with explicit Max for operations that may grow. Instrumented builds place the same calibrated phase markers
//! around open, load, operation and commit as the original fixtures.

use crate::{mark, schemas::*};
use blubber::raw::Resize;
use blubber::{pinocchio::ReadStore, ReadStorage, Result};
use core::hint::black_box;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

/// Layout codes: fixed/indexed list, fixed/indexed Address map, fixed/indexed Address map.
pub const LAYOUTS: [&str; 6] = [
    "list_fixed",
    "list_indexed",
    "map_fixed",
    "map_indexed",
    "address_fixed",
    "address_indexed",
];
/// Operations shared by each collection layout; map-only cases follow list cases.
pub const OPERATIONS: [&str; 49] = [
    "load_trusted",
    "late_read",
    "validate",
    "load",
    "mutable_baseline",
    "insert",
    "replace",
    "remove",
    "remove_into",
    "clear",
    "at_mut",
    "cursor_first",
    "for_each_read",
    "push",
    "pop",
    "truncate",
    "extend",
    "insert_bulk",
    "lookup_hit",
    "lookup_miss",
    "entry_occupied",
    "entry_vacant",
    "string_push",
    "string_truncate",
    "string_clear",
    "option_set",
    "option_some",
    "get_mut",
    "read_methods",
    "remove_missing",
    "entry_insert",
    "entry_remove",
    "entry_direct",
    "cursor_all",
    "for_each_edit",
    "get_mut_missing",
    "extract_short",
    "insert_owned",
    "remove_owned",
    "pop_owned",
    "replace_owned",
    "push_owned",
    "ref_methods",
    "to_owned",
    "option_get_mut",
    "option_absent",
    "option_missing_error",
    "replace_owned",
    "remove_owned_missing",
];
/// Original payload used for all indexed collection elements.
pub const ORIGINAL: &str = "original";
/// Longer replacement, exercising growth and ancestor patching.
pub const REPLACEMENT: &str = "replacement grows";

/// Ordered numeric keys; common-prefix keys place the number in the last bytes.
pub fn matrix_key(number: u32, common_prefix: bool) -> [u8; 32] {
    let mut bytes = [42; 32];
    let at = if common_prefix { 28 } else { 0 };
    bytes[at..at + 4].copy_from_slice(&number.to_be_bytes());
    bytes
}

// Macros specialize leaf operations without changing the library or introducing
// an alternative edit engine. Results from remove/entry/extraction stay live.
macro_rules! collection {
    ($root:ty, $kind:ident, $state:ident, $id:ident, $op:ident, $index:ident, $key:ident, $value:expr, $old:expr) => {{
        if $op < 4 || matches!($op, 18 | 19) {
            let store = ReadStore::new($state, $id)?;
            mark(1);
            let root = if $op == 3 { <$root>::load(store.bytes())? } else { <$root>::load_trusted(store.bytes())? };
            mark(2);
            match $op {
                1 | 18 => { assert_eq!(black_box(collection!(@read $kind, root, $index, $key)), Some($old)); }
                2 => root.validate()?,
                19 => { assert_eq!(black_box(collection!(@read $kind, root, $index, $key)), None); }
                _ => { black_box(&root); }
            }
            mark(3);
            drop(store);
        } else {
            // Keep the explicit growing lease on its own branch so its fixed
            // Max policy can be folded at compile time.
            let mut store = if matches!($op, 4 | 7 | 8 | 9 | 12 | 14 | 15 | 23 | 24 | 28 | 29 | 31 | 35 | 36 | 38 | 39 | 45 | 46 | 48) {
                super::Store::new($state, $id, super::Funding::Prepaid)?
            } else {
                // Inserts and variable-width replacements exceed exact-size inputs.
                super::Store::new_with_headroom($state, $id, super::Funding::Prepaid, super::Headroom::Max)?
            };
            mark(1);
            let mut root = <$root>::load_trusted_mut(&mut store)?;
            mark(2);
            if $op != 4 {
                let mut values = root.value_mut();
                match $op {
                    5 => { black_box(collection!(@insert $kind, values, $index, $key, $value)?); }
                    6 => { black_box(collection!(@replace $kind, values, $index, $key, $value)?); }
                    7 => { black_box(collection!(@remove $kind, values, $index, $key)?); }
                    8 => {
                        let mut output = [0; 64];
                        black_box(collection!(@extract $kind, values, $index, $key, output)?);
                        black_box(output);
                    }
                    9 => values.clear()?,
                    10 => collection!(@at $kind, values, $index, $key)?.set($value)?,
                    11 => {
                        let mut cursor = values.cursor_mut()?;
                        if let Some(mut child) = cursor.next()? { collection!(@child $kind, child).set($value)?; }
                    }
                    12 => collection!(@each $kind, values, $value)?,
                    _ => collection!(@extra $kind, values, $op, $index, $key, $value, $old),
                }
            }
            mark(3);
            root.commit()?;
        }
    }};
    (@read list, $r:ident, $i:ident, $k:ident) => { $r.value().get($i) };
    (@read map, $r:ident, $i:ident, $k:ident) => { $r.value().get(&$k) };
    (@insert list, $r:ident, $i:ident, $k:ident, $v:expr) => { $r.insert($i, $v) };
    (@insert map, $r:ident, $i:ident, $k:ident, $v:expr) => { $r.insert($k, $v) };
    (@replace list, $r:ident, $i:ident, $k:ident, $v:expr) => { $r.replace($i, $v) };
    (@replace map, $r:ident, $i:ident, $k:ident, $v:expr) => { $r.insert($k, $v) };
    (@remove list, $r:ident, $i:ident, $k:ident) => { $r.remove($i) };
    (@remove map, $r:ident, $i:ident, $k:ident) => { $r.remove(&$k) };
    (@extract list, $r:ident, $i:ident, $k:ident, $out:ident) => { $r.remove_into($i, &mut $out) };
    (@extract map, $r:ident, $i:ident, $k:ident, $out:ident) => { $r.remove_into(&$k, &mut $out) };
    (@at list, $r:ident, $i:ident, $k:ident) => { $r.at_mut($i) };
    (@at map, $r:ident, $i:ident, $k:ident) => { $r.at_mut(&$k) };
    (@child list, $c:ident) => { $c };
    (@child map, $c:ident) => { $c.1 };
    (@each list, $r:ident, $v:expr) => { $r.try_for_each_mut(|child| { black_box(child.try_get()?); Ok(()) }) };
    (@each map, $r:ident, $v:expr) => { $r.try_for_each_mut(|_, child| { black_box(child.try_get()?); Ok(()) }) };
    (@extra list, $r:ident, $o:ident, $i:ident, $k:ident, $v:expr, $old:expr) => { match $o {
        13 => $r.push($v)?, 14 => { black_box($r.pop()?); }, 15 => $r.truncate($i)?,
        16 => $r.extend_from_slice(&[$v, $v])?, 17 => $r.insert_from_slice($i, &[$v, $v])?,
        _ => return Err(ProgramError::InvalidInstructionData),
    } };
    (@extra map, $r:ident, $o:ident, $i:ident, $k:ident, $v:expr, $old:expr) => { match $o {
        20 => $r.entry($k)?.or_insert($old)?.set($v)?,
        21 => { black_box($r.entry($k)?.or_insert($v)?); },
        _ => return Err(ProgramError::InvalidInstructionData),
    } };
}

/// Runs opcode 23: layout, operation, ordinal (u16 LE), count (u16 LE), key-prefix flag.
/// Opcodes 24..26 select direct/four-node/eight-node strings; 27 selects an option.
#[inline(never)]
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [opcode, layout, op, lo, hi, nlo, nhi, common] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let index = usize::from(u16::from_le_bytes([*lo, *hi]));
    let count = u32::from(u16::from_le_bytes([*nlo, *nhi]));
    let missing = matches!(*op, 5 | 19 | 21 | 29 | 32 | 35 | 37 | 48);
    let key = Address::new_from_array(matrix_key(
        2 * index as u32 + if missing { 1 } else { 2 },
        *common != 0,
    ));
    mark(10);
    mark(11);
    mark(0);
    if *opcode != 23 {
        return nested(id, state, *opcode, *op);
    }
    if count > 1000 || index > count as usize {
        return Err(ProgramError::InvalidInstructionData);
    }
    let op = *op;
    match *layout {
        0 => numbers(state, id, op, index, key)?,
        1 => words(state, id, op, index, key)?,
        2 => public_keys(state, id, op, index, key)?,
        3 => key_words(state, id, op, index, key)?,
        4 => addresses(state, id, op, index, key)?,
        5 => address_words(state, id, op, index, key)?,
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    mark(4);
    Ok(())
}

macro_rules! extended {
    (@get_mut list, $r:ident, $i:ident, $k:ident) => {
        $r.reborrow().get_mut($i)
    };
    (@get_mut map, $r:ident, $i:ident, $k:ident) => {
        $r.reborrow().get_mut(&$k)
    };
    (@reads list, $r:ident, $i:ident, $k:ident, $old:expr) => {
        if !$r.is_empty() {
            assert_eq!($r.try_get($i)?, Some($old));
            assert_eq!($r.get($i), Some($old));
        }
        assert_eq!($r.get($r.len()), None);
    };
    (@reads map, $r:ident, $i:ident, $k:ident, $old:expr) => {
        assert_eq!($r.try_get(&$k)?, Some($old));
        assert_eq!($r.get(&$k), Some($old));
        assert!($r.try_contains_key(&$k)?);
        assert!($r.contains_key(&$k));
    };
    (@each list, $r:ident, $v:expr) => {
        $r.try_for_each_mut(|child| child.set($v))
    };
    (@each map, $r:ident, $v:expr) => {
        $r.try_for_each_mut(|_, child| child.set($v))
    };
    (@entries list, $r:ident, $o:ident, $i:ident, $k:ident, $v:expr, $old:expr) => {
        return Err(ProgramError::InvalidInstructionData)
    };
    (@entries map, $r:ident, $o:ident, $i:ident, $k:ident, $v:expr, $old:expr) => {
        match $o {
            29 => {
                assert_eq!($r.remove(&$k)?, None);
                let mut output = [0xa5; 64];
                assert_eq!($r.remove_into(&$k, &mut output)?, None);
                assert_eq!(output, [0xa5; 64]);
            }
            30 | 31 => {
                let blubber::Entry::Occupied(entry) = $r.entry($k)? else {
                    return Err(ProgramError::InvalidInstructionData);
                };
                assert_eq!(entry.key(), $k);
                if $o == 30 {
                    $old.check_removed(entry.insert($v)?);
                } else {
                    $old.check_removed(entry.remove()?);
                }
            }
            32 => {
                let blubber::Entry::Vacant(entry) = $r.entry($k)? else {
                    return Err(ProgramError::InvalidInstructionData);
                };
                assert_eq!(entry.key(), $k);
                entry.insert($v)?.set($v)?;
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    };
    (@owned_read list, $r:ident, $out:ident, $old:expr) => {
        assert!($out.iter().all(|value| *value == $old.owned_input()));
    };
    (@owned_read map, $r:ident, $out:ident, $old:expr) => {
        for ((key, _), (copied, value)) in $r.iter().zip(&$out) {
            assert_eq!(key, *copied);
            assert_eq!(*value, $old.owned_input());
        }
    };
    (@owned list, $r:ident, $o:ident, $i:ident, $k:ident, $v:expr, $old:expr) => {
        match $o {
            37 => $r.insert_owned($i, &$v.owned_input())?,
            38 => assert_eq!($r.remove_owned($i)?, $old.owned_input()),
            39 => {
                let empty = $r.is_empty();
                assert_eq!(
                    $r.pop_owned()?,
                    if empty {
                        None
                    } else {
                        Some($old.owned_input())
                    }
                );
            }
            40 => $r.replace_owned($i, &$v.owned_input())?,
            41 => $r.push_owned(&$v.owned_input())?,
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    };
    (@owned map, $r:ident, $o:ident, $i:ident, $k:ident, $v:expr, $old:expr) => {
        match $o {
            37 => {
                assert_eq!($r.insert_owned($k, &$v.owned_input())?, None);
            }
            38 => assert_eq!($r.remove_owned(&$k)?, Some($old.owned_input())),
            47 => $old.check_removed(
                $r.insert_owned($k, &$v.owned_input())?
                    .ok_or(blubber::Error::from(blubber::ErrorKind::InvalidPlan))?,
            ),
            48 => assert_eq!($r.remove_owned(&$k)?, None),
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    };
}

macro_rules! collection_fn {
    ($fn:ident, $root:ty, $kind:ident, $keytype:ty, $value:expr, $old:expr) => {
        #[inline(never)]
        fn $fn(state: &mut AccountView, id: &Address, op: u8, index: usize, key: $keytype) -> ProgramResult {
            #[inline(never)]
            fn reads(state: &mut AccountView, id: &Address, op: u8, index: usize, key: $keytype) -> ProgramResult {
                let _ = (index, &key);
                let store = ReadStore::new(state, id)?;
                let root = <$root>::load(store.bytes())?;
                let values = root.value();
                if op == 42 {
                    assert_eq!(values.try_len()?, values.len());
                    assert_eq!(values.try_is_empty()?, values.is_empty());
                    assert_eq!(values.try_iter().count(), values.iter().count());
                    for value in values.try_iter() { black_box(value?); }
                    values.validate()?;
                    extended!(@reads $kind, values, index, key, $old);
                } else {
                    #[cfg(feature = "alloc")]
                    { let output = values.to_owned()?; assert_eq!(output.len(), values.len()); extended!(@owned_read $kind, values, output, $old); let mirror = root.to_owned()?; assert_eq!(mirror.value, output); }
                    #[cfg(not(feature = "alloc"))]
                    return Err(ProgramError::InvalidInstructionData);
                }
                Ok(())
            }
            #[cfg(feature = "alloc")]
            #[inline(never)]
            fn owned(state: &mut AccountView, id: &Address, op: u8, index: usize, key: $keytype) -> ProgramResult {
                let _ = (index, &key);
                // These operation families include growth and never receive the idle opcode.
                let mut store = super::Store::new_with_headroom(state, id, super::Funding::Prepaid, super::Headroom::Max)?;
                let mut root = <$root>::load_checked_mut(&mut store)?;
                let mut values = root.value_mut();
                extended!(@owned $kind, values, op, index, key, $value, $old);
                root.commit()?;
                Ok(())
            }
            #[inline(never)]
            fn extra(state: &mut AccountView, id: &Address, op: u8, index: usize, key: $keytype) -> ProgramResult {
                if matches!(op, 42 | 43) { return reads(state, id, op, index, key); }
                #[cfg(feature = "alloc")]
                if (37..=41).contains(&op) || matches!(op, 47 | 48) { return owned(state, id, op, index, key); }
                // These operation families include growth and never receive the idle opcode.
                let mut store = super::Store::new_with_headroom(state, id, super::Funding::Prepaid, super::Headroom::Max)?;
                let mut root = <$root>::load_checked_mut(&mut store)?;
                let mut values = root.value_mut();
                match op {
                    27 => extended!(@get_mut $kind, values, index, key)?.unwrap().set($value)?,
                    28 => {
                        assert_eq!(values.try_len()?, values.len());
                        assert_eq!(values.try_is_empty()?, values.is_empty());
                        assert_eq!(values.try_iter()?.count(), values.iter().count());
                        for value in values.try_iter()? { black_box(value?); }
                        values.validate()?;
                        extended!(@reads $kind, values, index, key, $old);
                    }
                    33 => {
                        let mut cursor = values.cursor_mut()?;
                        while let Some(mut child) = cursor.next()? { collection!(@child $kind, child).set($value)?; }
                        assert!(cursor.next()?.is_none());
                    }
                    34 => extended!(@each $kind, values, $value)?,
                    35 => assert!(extended!(@get_mut $kind, values, index, key)?.is_none()),
                    36 => {
                        let mut output = [0xa5; 1];
                        let before = values.try_len()?;
                        assert!(matches!(collection!(@extract $kind, values, index, key, output), Err(blubber::Error::Fault { kind: blubber::ErrorKind::Capacity, .. })));
                        assert_eq!(output, [0xa5]);
                        assert_eq!(values.try_len()?, before);
                    }
                    _ => extended!(@entries $kind, values, op, index, key, $value, $old),
                }
                root.commit()?;
                Ok(())
            }
            if op >= 27 { return extra(state, id, op, index, key); }
            let _ = (index, &key);
            collection!($root, $kind, state, id, op, index, key, $value, $old);
            Ok(())
        }
    };
}
collection_fn!(numbers, Numbers, list, Address, 123u64, 17u64);
collection_fn!(words, Words, list, Address, REPLACEMENT, ORIGINAL);
collection_fn!(public_keys, PublicKeyMap, map, Address, 123u64, 17u64);
collection_fn!(key_words, KeyWords, map, Address, REPLACEMENT, ORIGINAL);
collection_fn!(addresses, AddressMap, map, Address, 123u64, 17u64);
collection_fn!(
    address_words,
    AddressWords,
    map,
    Address,
    REPLACEMENT,
    ORIGINAL
);

#[inline(never)]
fn nested(id: &Address, state: &mut AccountView, opcode: u8, op: u8) -> ProgramResult {
    match opcode {
        24 => direct_string(id, state, op)?,
        25 => depth4(id, state, op)?,
        26 => depth8(id, state, op)?,
        27 => optional(id, state, op)?,
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    mark(4);
    Ok(())
}
macro_rules! nested_fn {
    ($fn:ident, $root:ty, $($at:expr),*) => {
        #[inline(never)]
        fn $fn(id: &Address, state: &mut AccountView, op: u8) -> ProgramResult {
            // Set and append grow beyond exact-size strings; other operations use spare.
            let mut store = if matches!(op, 5 | 22) {
                super::Store::new_with_headroom(state, id, super::Funding::Prepaid, super::Headroom::Max)?
            } else {
                super::Store::new(state, id, super::Funding::Prepaid)?
            };
            mark(1);
            let mut root = <$root>::load_trusted_mut(&mut store)?;
            mark(2);
            let mut value = root.value_mut()$(.at_mut($at)?)*;
            string_edit(&mut value, op)?;
            mark(3);
            root.commit()?;
            Ok(())
        }
    };
}
nested_fn!(depth4, Depth4, 0, 0);
nested_fn!(depth8, Depth8, 0, 0, 0, 0, 0, 0);
#[inline(never)]
fn direct_string(id: &Address, state: &mut AccountView, op: u8) -> ProgramResult {
    // Set and append require more physical string storage.
    let mut store = if matches!(op, 5 | 22) {
        super::Store::new_with_headroom(state, id, super::Funding::Prepaid, super::Headroom::Max)?
    } else {
        super::Store::new(state, id, super::Funding::Prepaid)?
    };
    mark(1);
    let edit = blubber::raw::Edit::load::<blubber::Str>(&mut store, *b"MATRIX01")?;
    mark(2);
    string_edit(&mut blubber::raw::project::<blubber::Str>(edit), op)?;
    mark(3);
    let used = blubber_core::read_u32(store.bytes(), 8)?;
    store.commit(used)?;
    Ok(())
}
#[inline(never)]
fn optional(id: &Address, state: &mut AccountView, op: u8) -> ProgramResult {
    // The longer Some payload needs capacity beyond its exact-size input.
    let mut store = if matches!(op, 26 | 44) {
        super::Store::new_with_headroom(state, id, super::Funding::Prepaid, super::Headroom::Max)?
    } else {
        super::Store::new(state, id, super::Funding::Prepaid)?
    };
    mark(1);
    let mut root = MaybeText::load_checked_mut(&mut store)?;
    mark(2);
    match op {
        4 => (),
        25 => root.value_mut().set(None)?,
        44 => root.value_mut().get_mut()?.unwrap().set(REPLACEMENT)?,
        45 | 46 => {
            root.value_mut().set(None)?;
            assert!(root.value_mut().get_mut()?.is_none());
            if op == 46 {
                assert!(root.value_mut().some_mut().is_err());
            }
        }
        26 => root.value_mut().some_mut()?.set(REPLACEMENT)?,
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    mark(3);
    root.commit()?;
    Ok(())
}

fn string_edit(value: &mut blubber::StrMut<'_>, op: u8) -> Result<()> {
    match op {
        4 => Ok(()),
        5 => value.set(REPLACEMENT),
        6 => value.set("a"),
        22 => value.push_str(REPLACEMENT),
        23 => value.truncate(1),
        24 => value.clear(),
        _ => Err(blubber::ErrorKind::InvalidSchema.into()),
    }
}

#[cfg(feature = "alloc")]
trait OwnedInput {
    type Owned;
    fn owned_input(self) -> Self::Owned;
}
#[cfg(feature = "alloc")]
impl OwnedInput for u64 {
    type Owned = u64;
    fn owned_input(self) -> u64 {
        self
    }
}
#[cfg(feature = "alloc")]
impl OwnedInput for &str {
    type Owned = alloc::string::String;
    fn owned_input(self) -> Self::Owned {
        alloc::string::String::from(self)
    }
}
#[cfg(feature = "alloc")]
extern crate alloc;

/// Sets every fixed codec and exercises optional fixed-value projection.
#[inline(never)]
pub fn scalar_kinds(id: &Address, accounts: &mut [AccountView]) -> ProgramResult {
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let mut store = super::Store::new(state, id, super::Funding::Prepaid)?;
    let mut root = ScalarKinds::load_checked_mut(&mut store)?;
    let expected = ScalarValues {
        small: u8::MAX,
        medium: u16::MAX,
        word: u32::MAX,
        wide: u64::MAX,
        huge: u128::MAX,
        signed_small: i8::MIN,
        signed_medium: i16::MIN,
        signed_word: i32::MIN,
        signed_wide: i64::MIN,
        signed_huge: i128::MIN,
        enabled: true,
        raw: [0xa5; 17],
        key: Address::new_from_array([0x31; 32]),
        address: Address::new_from_array([0x32; 32]),
    };
    let mut value = root.value_mut();
    value.reborrow().set(expected)?;
    assert_eq!(value.get(), expected);
    assert_eq!(value.try_get()?, expected);
    assert_eq!(root.fixed_option_mut().get(), Some(1));
    assert_eq!(root.fixed_option_mut().try_get()?, Some(1));
    root.fixed_option_mut().get_mut()?.unwrap().set(u32::MAX)?;
    root.fixed_option_mut().set(None)?;
    assert!(root.fixed_option_mut().get_mut()?.is_none());
    root.fixed_option_mut().set(Some(u32::MAX))?;
    root.validate()?;
    root.commit()?;
    Ok(())
}

trait RemovedValue {
    type Removed;
    fn check_removed(self, value: Self::Removed);
}
impl RemovedValue for u64 {
    type Removed = u64;
    fn check_removed(self, value: u64) {
        assert_eq!(self, value);
    }
}
impl RemovedValue for &str {
    type Removed = ();
    fn check_removed(self, (): ()) {}
}
