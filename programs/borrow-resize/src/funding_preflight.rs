//! Rent affordability must precede every growing typed write.

use blubber::{List, Map, Str};

#[blubber::schema(account, namespace = "qa:m7c:funding")]
/// Small root spanning direct, nested, bulk and callback growth.
pub struct FundingState {
    /// Fixed list with an existing element.
    pub items: List<u64>,
    /// Indexed list with an existing string.
    pub labels: List<Str>,
    /// Indexed map with an existing value.
    pub values: Map<u16, Str>,
    /// Absent variable option.
    pub optional: Option<Str>,
    /// Fixed map eligible for fixed-stride insertion.
    pub fixed: Map<u16, u64>,
    /// Tail string eligible for a direct loan.
    pub tail: Str,
}

#[blubber::schema(account, namespace = "qa:m7c:append")]
/// Reviewer's append-only root with no outer suffix.
pub struct TailList {
    /// Final fixed-stride list eligible for a direct append loan.
    pub items: List<u64>,
}
#[blubber::schema(account, namespace = "qa:m7c:map-append")]
/// Final fixed map eligible for a direct append loan.
pub struct TailMap {
    /// Sorted fixed entries.
    pub items: Map<u16, u64>,
}

/// Applies a route through the appropriate authenticated or checked root load.
#[inline(never)]
pub fn apply(store: &mut dyn blubber::Resize, operation: u8) -> blubber::Result<()> {
    match operation {
        20 => TailList::load_mut(store)?.items_mut().push(42),
        22 => FundingState::load_mut(store)?.items_mut().at_mut(0)?.set(7),
        21 => TailMap::load_mut(store)?
            .items_mut()
            .insert(1, 42)
            .map(|_| ()),
        _ => edit(&mut FundingState::load_mut(store)?, operation),
    }
}

/// Completes the appropriate root after checking the operation's frame.
#[inline(never)]
pub fn complete(
    store: &mut dyn blubber::Resize,
    operation: u8,
) -> blubber::Result<blubber::CommitInfo> {
    match operation {
        20 => TailList::load_mut(store)?.commit(),
        21 => TailMap::load_mut(store)?.commit(),
        _ => FundingState::load_mut(store)?.commit(),
    }
}

/// Verifies the complete resulting schema after an affordable edit.
pub fn verify(bytes: &[u8], operation: u8) -> blubber::Result<()> {
    match operation {
        20 => TailList::load(bytes)?.verify(),
        21 => TailMap::load(bytes)?.verify(),
        _ => FundingState::load(bytes)?.verify(),
    }
}

/// Number of distinct growth routes exercised by the host matrix.
pub const OPERATIONS: u8 = 22;

/// Performs one growing operation through the public editor API.
#[inline(never)]
pub fn edit(root: &mut FundingStateMut<'_>, operation: u8) -> blubber::Result<()> {
    match operation {
        0 => operation_0(root),
        1 => operation_1(root),
        2 => operation_2(root),
        3 => operation_3(root),
        4 => operation_4(root),
        5 => operation_5(root),
        6 => operation_6(root),
        7 => operation_7(root),
        8 => operation_8(root),
        9 => operation_9(root),
        10 => operation_10(root),
        11 => operation_11(root),
        12 => operation_12(root),
        13 => operation_13(root),
        14 => operation_14(root),
        15 => operation_15(root),
        16 => operation_16(root),
        17 => operation_17(root),
        18 => root.fixed_mut().insert(2, 42).map(|_| ()),
        19 => root.fixed_mut().insert(0, 42).map(|_| ()),
        _ => Err(blubber::ErrorKind::IndexOutOfBounds.into()),
    }
}

#[inline(never)]
fn operation_0(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.items_mut().push(42)
}

#[inline(never)]
fn operation_1(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.items_mut().insert(0, 42)
}

#[inline(never)]
fn operation_2(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.labels_mut().push("longer")
}

#[inline(never)]
fn operation_3(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.labels_mut().insert(0, "longer")
}

#[inline(never)]
fn operation_4(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.labels_mut().at_mut(0)?.set("longer")
}

#[inline(never)]
fn operation_5(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.values_mut().insert(2, "longer").map(|_| ())
}

#[inline(never)]
fn operation_6(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.values_mut().insert(0, "longer").map(|_| ())
}

#[inline(never)]
fn operation_7(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.values_mut()
        .get_mut(&1)?
        .expect("one value")
        .set("longer")
}

#[inline(never)]
fn operation_8(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.tail_mut().set("longer")
}

#[inline(never)]
fn operation_9(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.tail_mut().push_str("longer")
}

#[inline(never)]
fn operation_10(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.reserve(8)
}

#[inline(never)]
fn operation_11(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.items_mut().extend_from_slice(&[42, 43])
}

#[inline(never)]
fn operation_12(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.labels_mut().insert_from_slice(0, &["longer", "more"])
}

#[inline(never)]
fn operation_13(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.labels_mut().fill("longer")
}

#[inline(never)]
fn operation_14(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.values_mut().rewrite_values_from(&[(1, "longer")])
}

#[inline(never)]
fn operation_15(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    {
        let labels = root.labels_mut();
        let mut cursor = labels.cursor_mut()?;
        cursor.next()?.expect("one label").set("longer")
    }
}

#[inline(never)]
fn operation_16(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.values_mut()
        .try_for_each_mut(|_, value| value.set("longer"))
}

#[inline(never)]
fn operation_17(root: &mut FundingStateMut<'_>) -> blubber::Result<()> {
    root.optional_mut().set(Some("longer"))
}

/// Catches a funding rejection to inspect its frame before returning success.
#[inline(never)]
pub fn run(
    id: &pinocchio::Address,
    accounts: &mut [pinocchio::AccountView],
    ix: &[u8],
) -> pinocchio::ProgramResult {
    use blubber::{
        pinocchio::{Funding, Headroom, Store},
        ErrorKind, ReadStorage, Resize,
    };
    use pinocchio::error::ProgramError;
    let [_, operation, funding, mode] = *ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [account, payer, system] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let funding = if funding == 0 {
        Funding::Prepaid
    } else {
        Funding::TopUp {
            payer,
            system_program: system,
        }
    };
    let headroom = match mode {
        0 => Headroom::UpTo(64),
        1 => Headroom::Max,
        #[cfg(feature = "alloc")]
        2 => Headroom::OnDemand,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    let mut store = Store::new_with_headroom(account, id, funding, headroom)?;
    // Snapshot the complete root and check every initialized headroom byte.
    let used = u32::from_le_bytes(store.bytes()[8..12].try_into().expect("used word")) as usize;
    let mut before = [0; 256];
    before[..used].copy_from_slice(&store.bytes()[..used]);
    assert!(store.bytes()[used..].iter().all(|byte| *byte == 0));
    let result = apply(&mut store, operation);
    let rejected = matches!(
        result,
        Err(blubber::Error::Fault {
            kind: ErrorKind::RentShortfall,
            ..
        })
    );
    let unchanged = store.bytes()[..used] == before[..used]
        && store.bytes()[used..].iter().all(|byte| *byte == 0);
    let clean = !store.is_poisoned();
    if rejected {
        assert!(unchanged && clean, "funding rejection frame");
    } else {
        result?;
    }
    complete(&mut store, operation)?;
    pinocchio::cpi::set_return_data(&[u8::from(rejected), u8::from(unchanged), u8::from(clean)]);
    Ok(())
}

/// Checks initialization funding independently of an uninitialized used word.
#[inline(never)]
pub fn run_initialize(
    id: &pinocchio::Address,
    accounts: &mut [pinocchio::AccountView],
    ix: &[u8],
) -> pinocchio::ProgramResult {
    use blubber::{
        pinocchio::{Funding, Headroom, Store},
        ErrorKind, ReadStorage, Resize,
    };
    let [account, payer, system] = accounts else {
        return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
    };
    let funding = if ix.get(1) == Some(&0) {
        Funding::Prepaid
    } else {
        Funding::TopUp {
            payer,
            system_program: system,
        }
    };
    let mut store = Store::new_with_headroom(account, id, funding, Headroom::UpTo(0))?;
    let mut before = [0; 64];
    before.copy_from_slice(store.bytes());
    let result = TailList::initialize(&mut store, TailListInit { items: &[42] })
        .and_then(|root| root.commit());
    let rejected = matches!(
        result,
        Err(blubber::Error::Fault {
            kind: ErrorKind::RentShortfall,
            ..
        })
    );
    if rejected {
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    } else {
        result?;
    }
    pinocchio::cpi::set_return_data(&[u8::from(rejected)]);
    Ok(())
}
