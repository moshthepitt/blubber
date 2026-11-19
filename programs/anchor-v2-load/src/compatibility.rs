//! Release comparisons with fixed-slot lists and sorted serialized maps.
use super::*;
use anchor_lang::accounts::Slab;

#[blubber::schema(account)]
/// Fixed-value collection cohort; each case populates one collection.
pub struct Series {
    /// Signer required by every mutable comparison.
    pub authority: Address,
    /// Scalar shared by all comparison layouts.
    pub counter: u64,
    /// Packed fixed-width list.
    pub values: List<u64>,
    /// Canonically sorted fixed-width map.
    pub entries: Map<u32, u32>,
}
impl Owner for Series {
    const OWNER: Address = ID;
}

#[account(borsh)]
/// Stock serialized map baseline with canonically sorted pairs.
pub struct Serialized {
    /// Same authority constraint as the native schema.
    pub authority: Address,
    /// Same initial scalar and scalar update.
    pub counter: u64,
    /// Sorted map represented by serialized key/value pairs.
    pub entries: Vec<(u32, u32)>,
}

#[derive(Accounts)]
/// Fully validated native read.
pub struct SeriesRead {
    /// Runtime-owned account.
    pub state: Blubber<Series>,
}
#[derive(Accounts)]
/// Managed edits with matched authority and explicit prepaid capacity.
pub struct SeriesWrite {
    #[account(mut, funding::payer = payer, funding::system_program = system_program)]
    /// Runtime-owned account.
    pub state: Blubber<Series>,
    #[account(mut, address = state.authority)]
    /// Authorizes and funds growth.
    pub payer: Signer,
    /// Used only for the final net deficit.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Stock map read with normal deserialization.
pub struct SerializedRead {
    /// Stock Borsh account.
    pub state: BorshAccount<Serialized>,
}
#[derive(Accounts)]
/// Stock mutable comparison with the same authority check.
pub struct SerializedWrite {
    #[account(mut)]
    /// Stock Borsh account.
    pub state: BorshAccount<Serialized>,
    #[account(mut, address = state.authority)]
    /// Same signer and writable flags as SeriesWrite.
    pub payer: Signer,
    /// Included to match the native instruction accounts.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
#[instruction(_space: u32)]
/// Signer creation with exactly the requested allocation.
pub struct CreateSeries {
    #[account(mut)]
    /// Pays identical runtime rent.
    pub payer: Signer,
    #[account(init, payer = payer, space = _space as usize)]
    /// Encoded from borrowed input after allocation.
    pub state: Blubber<Series>,
    /// Allocates the signer target.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
#[instruction(_space: u32)]
/// Stock signer creation with the same allocation and rent policy.
pub struct CreateSerialized {
    #[account(mut)]
    /// Pays identical runtime rent.
    pub payer: Signer,
    #[account(init, payer = payer, space = _space as usize)]
    /// Stock initialized account.
    pub state: BorshAccount<Serialized>,
    /// Allocates the signer target.
    pub system_program: Program<System>,
}

/// Checks all native items; output survives optimization through the assertion.
pub fn read(state: &Blubber<Series>, kind: u8, n: u32) -> Result<()> {
    let checksum = state.read(|s| {
        let mut sum = 0u64;
        if kind == 0 {
            for item in s.values().iter() {
                sum += item;
            }
        } else {
            for item in s.entries().iter() {
                let (key, value) = item;
                sum += u64::from(key) + u64::from(value);
            }
        }
        Ok(sum)
    })?;
    let expected = u64::from(n) * u64::from(n.saturating_sub(1)) / 2;
    require!(
        checksum == expected * if kind == 0 { 1 } else { 2 },
        Error::InvalidAccountData
    );
    Ok(())
}

/// Separately measures scalar, collection, empty commit, resize and funding.
pub fn edit(
    state: &mut Blubber<Series>,
    kind: u8,
    n: u32,
    operation: u8,
    funding: Funding<'_>,
) -> Result<()> {
    let edit = |s: &mut SeriesMut<'_>| {
        match operation {
            0 => s.counter_mut().set(11)?,
            1 => {
                if kind == 0 {
                    s.values_mut().push(u64::from(n))?;
                } else {
                    s.entries_mut().insert(n, n)?;
                }
            }
            2 => (),
            3 | 4 => s.reserve(128)?,
            _ => return Err(blubber::ErrorKind::InvalidSchema.into()),
        }
        Ok(())
    };
    if matches!(operation, 3 | 4) {
        // These reserve rows request 128 retained bytes beyond used storage;
        // their opening floor already includes 64, so request 64 more bytes.
        state.edit_with(
            EditOptions {
                funding,
                headroom: Headroom::UpTo(64),
                refund: None,
            },
            edit,
        )?;
    } else {
        state.edit(edit)?;
    }
    Ok(())
}

/// Populates a native initializer without serializing a second account mirror.
pub fn initialize(state: &mut Blubber<Series>, authority: Address, kind: u8, n: u32) -> Result<()> {
    let values: Vec<u64> = if kind == 0 {
        (0..n).map(u64::from).collect()
    } else {
        Vec::new()
    };
    let entries: Vec<(u32, u32)> = if kind == 1 {
        (0..n).map(|i| (i, i)).collect()
    } else {
        Vec::new()
    };
    state.initialize(SeriesInit {
        authority,
        counter: 7,
        values: &values,
        entries: &entries,
    })?;
    Ok(())
}

#[derive(Accounts)]
/// Genuine zero-copy Slab list baseline.
pub struct SlabRead {
    /// Same header as the fixed Account baseline, with byte-aligned u64 slots.
    pub state: Slab<Baseline, [u8; 8]>,
}
#[derive(Accounts)]
/// Genuine slab append with the same authority and account flags.
pub struct SlabWrite {
    #[account(mut)]
    /// Slab with one prepaid spare slot.
    pub state: Slab<Baseline, [u8; 8]>,
    #[account(mut, address = state.authority)]
    /// Authorizes the operation.
    pub payer: Signer,
    /// Included to match native account metadata.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
#[instruction(_space: u32)]
/// Slab signer creation at the same physical size as the native comparison.
pub struct CreateSlab {
    #[account(mut)]
    /// Pays full runtime rent.
    pub payer: Signer,
    #[account(init, payer = payer, space = _space as usize)]
    /// Newly allocated slab.
    pub state: Slab<Baseline, [u8; 8]>,
    /// Allocates and funds the account.
    pub system_program: Program<System>,
}
