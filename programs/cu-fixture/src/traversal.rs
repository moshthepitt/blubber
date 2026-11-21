//! Isolated traversal diagnostics using the shipping public API.
//! Ordinary callback headline rows come from `mixed_operations`, without the
//! callback isolation and closure attributes used by these diagnostic rows.
#![cfg_attr(
    any(
        feature = "walk-contract",
        feature = "rewrite-kernel",
        feature = "rewrite-api"
    ),
    allow(dead_code, unused_imports, unused_macros)
)]
use crate::{mark, matrix::matrix_key, schemas::*};
use blubber::{pinocchio::ReadStore, List, Schema, Str};
use core::hint::black_box;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

/// Varied borrowed inputs, including empty values and the 8→17 transition.
pub const INPUTS: [&str; 18] = [
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
/// A derived codec whose measurement recursively visits eight child strings.
#[blubber::schema]
pub struct MeasuredRecord {
    /// Fixed prefix.
    pub id: u64,
    /// Borrowed children measured recursively by the generated codec.
    pub children: blubber::List<Str>,
}

// Walks return sums of all values and all copied map-key bytes. Legacy slope
// controls retain the last item to reproduce the original black_box loop.
fn scalar(x: u64) -> u64 {
    black_box(x)
}
fn text(x: &str) -> u64 {
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
#[inline(always)]
fn copied_key(key: Address) -> u64 {
    let bytes = key.to_bytes();
    let a = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let b = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let c = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
    let d = u64::from_le_bytes(bytes[24..].try_into().unwrap());
    let mask = 0x00ff_00ff_00ff_00ff;
    // Four masked words fit each 16-bit lane (at most 2,040); the final
    // checksum is at most 8,160. Wrapping operations preserve that exact sum.
    let x = (a & mask)
        .wrapping_add(b & mask)
        .wrapping_add(c & mask)
        .wrapping_add(d & mask)
        .wrapping_add((a >> 8) & mask)
        .wrapping_add((b >> 8) & mask)
        .wrapping_add((c >> 8) & mask)
        .wrapping_add((d >> 8) & mask);
    x.wrapping_mul(0x0001_0001_0001_0001) >> 48
}

macro_rules! select {
    (list, $v:ident, $i:ident, $k:ident, $m:ident) => {
        $v.$m($i)
    };
    (map, $v:ident, $i:ident, $k:ident, $m:ident) => {
        $v.$m(&$k)
    };
}
#[cfg(feature = "alloc")]
macro_rules! owned_value {
    (scalar, $x:ident) => {
        scalar(*$x)
    };
    (text, $x:ident) => {
        text($x.as_str())
    };
}
#[cfg(feature = "alloc")]
macro_rules! owned_sum {
    (list, $o:ident, $decode:ident) => {
        $o.iter().map(|x| owned_value!($decode, x)).sum::<u64>()
    };
    (map, $o:ident, $decode:ident) => {
        $o.iter()
            .map(|(key, x)| copied_key(*key) + owned_value!($decode, x))
            .sum::<u64>()
    };
}
// Isolate loop register allocation from the multi-method fixture dispatcher.
// Each measured instruction includes this call and consumes every copied key.
#[inline(never)]
fn sum_map<I, V>(iter: I, mut decode: impl FnMut(V) -> u64) -> u64
where
    I: Iterator<Item = (Address, V)>,
{
    let mut out = 0u64;
    for (key, value) in iter {
        out = out.wrapping_add(copied_key(key).wrapping_add(decode(value)));
    }
    out
}
#[inline(never)]
fn try_sum_map<I, V>(iter: I, mut decode: impl FnMut(V) -> u64) -> blubber::Result<u64>
where
    I: Iterator<Item = blubber::Result<(Address, V)>>,
{
    let mut out = 0u64;
    for item in iter {
        let (key, value) = item?;
        out = out.wrapping_add(copied_key(key).wrapping_add(decode(value)));
    }
    Ok(out)
}
macro_rules! walk_sum {
    (list,$out:ident,$iter:expr,$decode:ident) => {
        for x in $iter {
            $out = $out.wrapping_add($decode(x));
        }
    };
    (map,$out:ident,$iter:expr,$decode:ident) => {
        $out = sum_map($iter, $decode);
    };
}
macro_rules! try_walk_sum {
    // The indexed mutable path folds results in the iterator frame.
    (list,$out:ident,$iter:expr,text,mut) => {
        $out = $iter.try_fold(0u64, |sum, item| {
            item.map(|value| sum.wrapping_add(text(value)))
        })?;
    };

    (list,$out:ident,$iter:expr,$decode:ident,$access:ident) => {
        for x in $iter {
            let x = x?;
            $out = $out.wrapping_add($decode(x));
        }
    };
    (map,$out:ident,$iter:expr,$decode:ident,$access:ident) => {
        $out = try_sum_map($iter, $decode)?;
    };
}
macro_rules! each {
    (list, $v:ident, $x:ident, $body:block) => {
        list_each(&mut $v, |$x| $body)
    };
    (map, $v:ident, $x:ident, $body:block) => {
        map_each(&mut $v, |key, $x| {
            black_box(copied_key(key));
            $body
        })
    };
}

// Read callbacks return all copied key bytes as well as every value byte.
macro_rules! each_read {
    (list, $v:ident, $out:ident, $decode:ident) => {
        list_each(&mut $v, |value| {
            $out = $out.wrapping_add($decode(value.try_get()?));
            Ok(())
        })
    };
    (map, $v:ident, $out:ident, scalar) => {
        map_each(&mut $v, {
            // Preserve fixed-slot specialization with complete key/value sums.
            #[inline(always)]
            |key, value| {
                $out = $out.wrapping_add(copied_key(key).wrapping_add(scalar(value.try_get()?)));
                Ok(())
            }
        })
    };
    (map, $v:ident, $out:ident, $decode:ident) => {
        map_each(&mut $v, |key, value| {
            $out = $out.wrapping_add(copied_key(key).wrapping_add($decode(value.try_get()?)));
            Ok(())
        })
    };
}

// Keep callback code generation out of the shared dispatcher's read controls.
// The whole-instruction measurement includes this ordinary call boundary.
#[inline(never)]
fn list_each<T: blubber::Schema>(
    values: &mut blubber::ListMut<'_, T>,
    f: impl for<'s> FnMut(&mut T::Mut<'s>) -> blubber::Result<()>,
) -> blubber::Result<()> {
    values.try_for_each_mut(f)
}
#[inline(never)]
fn map_each<K: blubber::MapKey, V: blubber::Schema>(
    values: &mut blubber::MapMut<'_, K, V>,
    f: impl for<'s> FnMut(K, &mut V::Mut<'s>) -> blubber::Result<()>,
) -> blubber::Result<()> {
    values.try_for_each_mut(f)
}
#[inline(never)]
fn list_walk<T: blubber::Schema>(
    values: blubber::ListMut<'_, T>,
    mut f: impl for<'s> FnMut(&mut T::Mut<'s>) -> blubber::Result<u64>,
) -> blubber::Result<u64> {
    let mut cursor = values.cursor_mut()?;
    let mut out = 0u64;
    while let Some(mut child) = cursor.next()? {
        out = out.wrapping_add(f(&mut child)?);
    }
    Ok(out)
}
#[inline(never)]
fn map_walk<K: blubber::MapKey, V: blubber::Schema>(
    values: blubber::MapMut<'_, K, V>,
    mut f: impl for<'s> FnMut(K, &mut V::Mut<'s>) -> blubber::Result<u64>,
) -> blubber::Result<u64> {
    let mut cursor = values.cursor_mut()?;
    let mut out = 0u64;
    while let Some((key, mut child)) = cursor.next()? {
        out = out.wrapping_add(f(key, &mut child)?);
    }
    Ok(out)
}
macro_rules! isolated_cursor {
    (list, $v:ident, $key:ident, $value:ident, $body:block) => {
        list_walk($v, |$value| {
            let $key = 0u64;
            $body
        })
    };
    (map, $v:ident, $key:ident, $value:ident, $body:block) => {
        map_walk($v, |key, $value| {
            let $key = copied_key(key);
            $body
        })
    };
}
#[inline(never)]
fn mutable_idle<S: blubber::RootSchema>(
    state: &mut AccountView,
    id: &Address,
    discriminator: [u8; 8],
) -> core::result::Result<u64, ProgramError> {
    let mut store = crate::Store::new(state, id, crate::Funding::Prepaid)?;
    mark(1);
    let root = blubber::raw::Edit::load::<S>(&mut store, discriminator)?;
    mark(2);
    mark(3);
    root.commit()?;
    mark(4);
    Ok(0)
}
macro_rules! common_idle {
    ($root:ty, $body:expr, $state:ident, $id:ident, $op:ident) => {
        if *$op == 20 {
            mutable_idle::<$root>($state, $id, <$root>::DISCRIMINATOR)
        } else {
            $body
        }
    };
}
// Keep the growing lease out of the shared traversal dispatcher's frame.
#[inline(never)]
fn open_growing_store<'a>(
    state: &'a mut AccountView,
    id: &Address,
) -> blubber::Result<crate::Store<'a>> {
    crate::Store::new_with_headroom(state, id, crate::Funding::Prepaid, crate::Headroom::Max)
}

macro_rules! fixture {
    ($name:ident, $root:ty, $kind:ident, $decode:ident, $same:expr, $grow:expr, $variable:expr, $old:expr) => {
        #[inline(never)]
        fn $name(
            state: &mut AccountView,
            id: &Address,
            op: u8,
            n: usize,
            key: Address,
        ) -> core::result::Result<u64, ProgramError> {
            let index = n.saturating_sub(1);
            let _ = (index, key);
            let mut out = 0;
            if op < 20 || op == 40 || op == 41 {
                let store = ReadStore::new(state, id)?;
                mark(1);
                let root = if matches!(op, 17 | 40 | 41) {
                    <$root>::load_trusted(store.bytes())?
                } else {
                    <$root>::load(store.bytes())?
                };
                mark(2);
                if op != 0 && op != 40 && op != 10 {
                    let values = root.value();
                    match op {
                        1 | 17 => {
                            black_box(&values);
                        }
                        11 => {
                            black_box(values.iter());
                        }
                        12 => {
                            black_box(values.try_iter());
                        }
                        2 => out = values.len() as u64,
                        13 => out = values.try_len()? as u64,
                        14 => out = u64::from(values.try_is_empty()?),
                        3 => out = u64::from(values.is_empty()),
                        4 | 41 => {
                            out = select!($kind, values, index, key, get)
                                .map($decode)
                                .unwrap_or(0)
                        }
                        5 => {
                            out = select!($kind, values, index, key, try_get)?
                                .map($decode)
                                .unwrap_or(0)
                        }
                        6 => {
                            walk_sum!($kind, out, values.iter(), $decode);
                        }
                        7 => {
                            try_walk_sum!($kind, out, values.try_iter(), $decode, read);
                        }
                        8 => values.validate()?,
                        9 => {
                            #[cfg(feature = "alloc")]
                            {
                                let owned = values.to_owned()?;
                                out = owned_sum!($kind, owned, $decode);
                                black_box(owned);
                            }
                            #[cfg(not(feature = "alloc"))]
                            return Err(ProgramError::InvalidInstructionData);
                        }
                        _ => return Err(ProgramError::InvalidInstructionData),
                    }
                }
                if op == 10 {
                    #[cfg(feature = "alloc")]
                    {
                        let owned = root.to_owned()?;
                        let values = owned.value;
                        out = owned_sum!($kind, values, $decode);
                        black_box(values);
                    }
                    #[cfg(not(feature = "alloc"))]
                    return Err(ProgramError::InvalidInstructionData);
                }
                mark(3);
            } else {
                // Growing indexed controls retain an explicit Max lease;
                // fixed and equal-width callbacks use the default allocation.
                let mut store = if $variable && matches!(op, 35 | 38) {
                    open_growing_store(state, id)?
                } else {
                    crate::Store::new(state, id, crate::Funding::Prepaid)?
                };
                mark(1);
                let mut root = <$root>::load_checked_mut(&mut store)?;
                mark(2);
                if op != 20 {
                    let mut values = root.value_mut();
                    match op {
                        21 => {
                            black_box(&values);
                        }
                        39 => {
                            black_box(values.cursor_mut()?);
                        }
                        43 => {
                            black_box(values.try_iter()?);
                        }
                        49 => {
                            black_box(values.iter());
                        }
                        22 => out = values.len() as u64,
                        44 => out = values.try_len()? as u64,
                        45 => out = u64::from(values.try_is_empty()?),
                        23 => out = u64::from(values.is_empty()),
                        24 => {
                            out = select!($kind, values, index, key, get)
                                .map($decode)
                                .unwrap_or(0)
                        }
                        25 => {
                            out = select!($kind, values, index, key, try_get)?
                                .map($decode)
                                .unwrap_or(0)
                        }
                        26 => {
                            walk_sum!($kind, out, values.iter(), $decode);
                        }
                        27 => {
                            try_walk_sum!($kind, out, values.try_iter()?, $decode, mut);
                        }
                        28 => values.validate()?,
                        29 => {
                            out = isolated_cursor!($kind, values, key_sum, value, {
                                black_box(key_sum);
                                black_box(value);
                                Ok(0)
                            })?;
                        }
                        30 => {
                            out = isolated_cursor!($kind, values, key_sum, value, {
                                Ok(key_sum + $decode(value.try_get()?))
                            })?;
                        }
                        31 => {
                            out = isolated_cursor!($kind, values, key_sum, value, {
                                black_box(key_sum);
                                value.set($same)?;
                                Ok(0)
                            })?;
                        }
                        32 => each!($kind, values, value, {
                            black_box(&value);
                            Ok(())
                        })?,
                        33 => each_read!($kind, values, out, $decode)?,
                        34 => each!($kind, values, value, {
                            value.set($same)?;
                            Ok(())
                        })?,
                        35 => each!($kind, values, value, {
                            value.set($grow)?;
                            Ok(())
                        })?,
                        38 => each!($kind, values, value, {
                            let old = value.try_get()?;
                            let replacement = ($old)(old);
                            value.set(replacement)?;
                            Ok(())
                        })?,
                        _ => return Err(ProgramError::InvalidInstructionData),
                    }
                }
                mark(3);
                root.commit()?;
            }
            mark(4);
            Ok(out)
        }
    };
}
fixture!(
    numbers,
    Numbers,
    list,
    scalar,
    123u64,
    123u64,
    false,
    |old: u64| old + 1
);
fixture!(
    words,
    Words,
    list,
    text,
    "sameSIZE",
    "replacement grows",
    true,
    |old: &str| if old == "original" {
        "replacement grows"
    } else {
        "a"
    }
);
fixture!(
    keys,
    PublicKeyMap,
    map,
    scalar,
    123u64,
    123u64,
    false,
    |old: u64| old + 1
);
fixture!(
    key_words,
    KeyWords,
    map,
    text,
    "sameSIZE",
    "replacement grows",
    true,
    |old: &str| if old == "original" {
        "replacement grows"
    } else {
        "a"
    }
);

#[cfg(feature = "walk-contract")]
#[path = "../../../crates/blubber/tests/fixtures/walk_contract.rs"]
mod source_contract;

/// Runs `[layout, operation, n:u16 LE, common_prefix]` against one account.
/// ReadStore paths and mutable Store paths use full validated ingress except
/// explicitly labeled trusted operations 40/41. Every row publishes eight bytes.
#[cfg(all(
    not(feature = "walk-contract"),
    not(feature = "rewrite-kernel"),
    not(feature = "rewrite-api")
))]
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [layout, op, lo, hi, common] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = usize::from(u16::from_le_bytes([*lo, *hi]));
    if n > 1000 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let key = Address::new_from_array(matrix_key(2 * n as u32, *common != 0));
    mark(10);
    mark(11);
    mark(0);
    let out = if *op >= 50 {
        let store = ReadStore::new(state, id)?;
        let root = Numbers::load(store.bytes())?;
        black_box(root);
        mark(1);
        mark(2);
        let mut total = 0u64;
        if n > 0 {
            match op {
                50 => {
                    for _ in 0..n {
                        total += u64::from(Str::measure(black_box(&"original"))?);
                    }
                }
                51 => {
                    for _ in 0..n {
                        total +=
                            u64::from(List::<u8>::measure(black_box(&b"original".as_slice()))?);
                    }
                }
                52 => {
                    for _ in 0..n {
                        total +=
                            u64::from(MeasuredRecord::measure(black_box(&MeasuredRecordInit {
                                id: 3,
                                children: &INPUTS[..8],
                            }))?);
                    }
                }
                _ => return Err(ProgramError::InvalidInstructionData),
            }
        }
        mark(3);
        mark(4);
        total
    } else {
        match layout {
            0 => common_idle!(Numbers, numbers(state, id, *op, n, key), state, id, op)?,
            1 => common_idle!(Words, words(state, id, *op, n, key), state, id, op)?,
            2 => common_idle!(PublicKeyMap, keys(state, id, *op, n, key), state, id, op)?,
            3 => common_idle!(KeyWords, key_words(state, id, *op, n, key), state, id, op)?,
            _ => return Err(ProgramError::InvalidInstructionData),
        }
    };
    pinocchio::cpi::set_return_data(&out.to_le_bytes());
    Ok(())
}

/// Runs only the low-level rewrite fixtures in their separate SBF artifact.
#[cfg(all(feature = "rewrite-kernel", not(feature = "walk-contract")))]
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    crate::rewrite::run(id, accounts, ix)
}

/// Runs the exact-count public APIs in a separate SBF artifact.
#[cfg(all(
    feature = "rewrite-api",
    not(feature = "walk-contract"),
    not(feature = "rewrite-kernel")
))]
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    crate::rewrite_api::run(id, accounts, ix)
}

/// Runs the identical public source fixture in an isolated SBF artifact.
#[cfg(feature = "walk-contract")]
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [0, op @ 53..=61, _, _, 0] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let mut store = crate::Store::new(state, id, crate::Funding::Prepaid)?;
    mark(1);
    let out = if *op <= 54 {
        source_contract::exercise(&mut store, Numbers::DISCRIMINATOR, *op == 53, mark)?
    } else {
        source_contract::slots(&mut store, Numbers::DISCRIMINATOR, *op, mark)?
    };
    pinocchio::cpi::set_return_data(&out.to_le_bytes());
    Ok(())
}

#[cfg(test)]
#[test]
fn copied_key_checksum_matches_every_byte_lane() {
    for byte in 0..=255u8 {
        let all = Address::new_from_array([byte; 32]);
        assert_eq!(copied_key(all), u64::from(byte) * 32);
        for lane in 0..32 {
            let mut bytes = [42; 32];
            bytes[lane] = byte;
            assert_eq!(
                copied_key(Address::new_from_array(bytes)),
                bytes.into_iter().map(u64::from).sum::<u64>()
            );
        }
    }
}
