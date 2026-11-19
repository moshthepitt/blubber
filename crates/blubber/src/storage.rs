//! Safe storage borrows and explicit host session completion.

use crate::{Error, ErrorKind, Result};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

// The account adapter may implement these traits inside this crate, but a
// downstream codec cannot substitute storage with weaker borrow guarantees.
pub(crate) mod private {
    pub trait Sealed {}

    /// Disjoint access to bytes, poison and immutable backend limits.
    pub struct EditAccess<'a> {
        pub bytes: &'a mut [u8],
        pub poisoned: &'a mut bool,
        pub limit: usize,
        pub hard_limit: usize,
        pub limit_error: ErrorKind,
        #[cfg(feature = "pinocchio")]
        pub(crate) preflight: Preflight<'a>,
    }

    use crate::ErrorKind;

    /// The hook's borrow excludes another session loan until this one ends.
    #[cfg(feature = "pinocchio")]
    #[derive(Clone, Copy, Default)]
    pub(crate) struct Preflight<'a> {
        pub rent: Option<&'a crate::pinocchio::RentCheck<'a>>,
        #[cfg(test)]
        pub probe: Option<&'a dyn Fn(usize) -> crate::Result<()>>,
    }

    #[cfg(feature = "pinocchio")]
    impl Preflight<'_> {
        #[inline(always)]
        pub fn check(self, required: usize) -> crate::Result<()> {
            if let Some(rent) = self.rent {
                rent.check(required)?;
            }
            #[cfg(test)]
            if let Some(probe) = self.probe {
                probe(required)?;
            }
            Ok(())
        }
    }

    /// Exact overwrites need bytes and poison, with no growth policy dispatch.
    pub struct FixedAccess<'a> {
        pub bytes: &'a mut [u8],
        pub poisoned: &'a mut bool,
    }

    /// Sealed borrowing boundary for immediate typed operations.
    pub trait Write {
        /// Initialization ignores the uninitialized envelope's stored used word.
        fn check_initialize_resize(&self, needed: usize) -> crate::Result<()>;

        /// Only a live SDK lease authenticates the account's owner and exclusion.
        /// This is history provenance, never evidence of a full content scan.
        fn authenticated(&self) -> bool {
            false
        }
        fn edit_access(&mut self) -> EditAccess<'_>;
        /// Lends bytes and poison after the caller checks session state.
        /// Like `edit_access`, this raw operation remains available while
        /// poisoned; exact encoders use it between poison publication and clear.
        fn exact_access(&mut self) -> FixedAccess<'_> {
            let access = self.edit_access();
            FixedAccess {
                bytes: access.bytes,
                poisoned: access.poisoned,
            }
        }
        fn fixed_access(&mut self) -> crate::Result<FixedAccess<'_>>;
    }
}

/// An open storage borrow, sealed to the backends supplied by Blubber.
///
/// Byte references borrow the store itself. They must end before resizing or
/// committing; no reference points through a saved address into an allocation.
pub trait ReadStorage: private::Sealed {
    /// Whether commit has closed this store, including a failed commit.
    fn is_closed(&self) -> bool;

    /// Borrows the initialized, currently exposed storage, including spare bytes.
    ///
    /// # Panics
    /// Panics if the store is closed. This low-level access remains available
    /// while poisoned so an in-progress kernel operation can finish. Typed
    /// operations must check poison before interpreting these bytes.
    fn bytes(&self) -> &[u8];
}

/// Exclusive resizable storage, sealed to Blubber's safe backends.
///
/// Resize preserves the prefix below both lengths and zeroes any extension.
/// Returned resize failures preserve exposed bytes and length, except terminal
/// account re-lease failure, which poisons/closes access after physical growth.
/// Implementations are object-safe so projections can hold `&mut dyn Resize`.
/// Typed roots own mutation and completion. Deliberate backend mutation uses
/// the protocol exported through [`crate::raw`].
pub trait Resize: ReadStorage {
    /// Returns the exposed initialized length, even after closing.
    fn storage_len(&self) -> usize;

    /// Returns the known storage capacity available to this backend.
    ///
    /// A fixed slice retains its original room; a Vec is also bounded by the
    /// wire limit and Rust's maximum allocation size. A Pinocchio lease reports
    /// discovered SDK-local session capacity in Max/UpTo mode. OnDemand reports
    /// currently leased capacity; requests beyond it may attempt SDK growth.
    /// Neither predicts transaction resources, CPI ancestor limits or faults.
    fn growth_limit(&self) -> usize;

    /// Checks open/poison state and length limits without changing storage.
    ///
    /// Returns `Closed`, then `Poisoned`, then backend limits, in that precedence.
    /// Host limits return `Capacity`; account leases can return `GrowthLimit`.
    /// Success does not promise that a later allocation will succeed.
    fn check_resize(&self, new_len: usize) -> Result<()>;

    /// Whether an interrupted destructive edit may have left incomplete bytes.
    fn is_poisoned(&self) -> bool;

    /// Internal dispatch; only the library can construct the writer authority.
    #[doc(hidden)]
    fn __backend(&mut self, authority: Writer) -> &mut dyn Backend;
}

/// Authority for entering the backend protocol from the typed implementation.
/// Its private field prevents downstream construction.
#[doc(hidden)]
pub struct Writer {
    _private: (),
}

pub(crate) const WRITER: Writer = Writer { _private: () };

/// Raw mutation protocol, excluded from typed schema preservation.
///
/// Import through [`crate::raw`]. Callers own encoding, poison, publication and
/// checked re-admission after any raw write. Implementations remain sealed.
pub trait Backend: Resize + private::Write {
    /// Borrows the exposed storage for one kernel operation.
    ///
    /// # Panics
    /// Panics if closed. Poison does not prevent this low-level borrow: the
    /// kernel needs it between marking an edit poisoned and finishing it.
    fn bytes_mut(&mut self) -> &mut [u8];

    /// Repeats preflight and changes the exposed length.
    ///
    /// Allocation failure returns `Allocation` without changing bytes or length.
    /// A failed physical shrink is also retained for the commit receipt. A
    /// successful shrink clears a previous shrink error.
    fn resize(&mut self, new_len: usize) -> Result<()>;

    /// Sets the edit flag; only clear it after all wire invariants are restored.
    /// Changing this flag must preserve storage bytes and exposed length.
    ///
    /// # Panics
    /// Panics if the store is closed.
    fn set_poisoned(&mut self, poisoned: bool);

    /// Completes and closes a session, preserving the host's exposed capacity.
    ///
    /// `used_len` must fit the exposed storage. The typed layer checks its
    /// envelope and schema; the backend does not parse them. Every first commit
    /// attempt closes the store, including `Poisoned` or `UsedLength` failure.
    /// Subsequent attempts return `Closed`. Host receipts have no rent changes.
    fn commit(&mut self, used_len: u32) -> Result<CommitInfo>;

    /// Ensures room for `additional` bytes beyond the supplied logical extent.
    ///
    /// The typed root supplies its checked `used_len`. A length beyond current
    /// storage returns `UsedLength`; an unrepresentable sum returns
    /// `ArithmeticOverflow`. Existing spare storage is retained. Growth changes
    /// only initialized storage; account OnDemand may add amortized slack.
    fn reserve(&mut self, used_len: u32, additional: usize) -> Result<()> {
        self.check_resize(self.storage_len())?;
        let used = check_used_len(self.storage_len(), used_len)?;
        let required = used
            .checked_add(additional)
            .ok_or(ErrorKind::ArithmeticOverflow)?;
        blubber_core::checked_u32(required)?;
        self.check_resize(required)?;
        if required > self.storage_len() {
            self.resize(required)?;
        }
        Ok(())
    }

    /// Attempts physical compaction after the logical edit has completed.
    ///
    /// Host stores trim to `used_len`. Returns `Ok(false)` on resize failure,
    /// preserving valid bytes and capacity and recording the error for commit.
    /// Invalid state or a logical extent beyond storage returns an error before
    /// attempting a shrink. Account adapters may instead schedule the trim.
    fn shrink_to_fit(&mut self, used_len: u32) -> Result<bool> {
        self.check_resize(self.storage_len())?;
        let used = check_used_len(self.storage_len(), used_len)?;
        Ok(self.resize(used).is_ok())
    }
}

/// Receipt from explicitly completing a storage session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitInfo {
    /// Logical account length, including its envelope.
    pub used_len: u32,
    /// Actual exposed storage length after completion policy.
    pub storage_len: u32,
    /// Lamports transferred to cover rent; zero for host stores.
    pub topped_up: u64,
    /// Lamports refunded after settlement; zero for host stores.
    pub refunded: u64,
    /// Most recent failed shrink, cleared by a later successful shrink.
    pub shrink_error: Option<Error>,
}

#[derive(Default)]
struct State {
    closed: bool,
    poisoned: bool,
    shrink_error: Option<Error>,
}

impl State {
    fn assert_open(&self) {
        assert!(!self.closed, "Blubber storage is closed");
    }

    fn check_open(&self) -> Result<()> {
        if self.closed {
            Err(ErrorKind::Closed.into())
        } else if self.poisoned {
            Err(ErrorKind::Poisoned.into())
        } else {
            Ok(())
        }
    }

    fn commit(&mut self, storage_len: usize, used_len: u32) -> Result<CommitInfo> {
        if self.closed {
            return Err(ErrorKind::Closed.into());
        }
        self.closed = true;
        if self.poisoned {
            return Err(ErrorKind::Poisoned.into());
        }
        check_used_len(storage_len, used_len)?;
        let storage_len = u32::try_from(storage_len).map_err(|_| ErrorKind::Capacity)?;
        Ok(CommitInfo {
            used_len,
            storage_len,
            topped_up: 0,
            refunded: 0,
            shrink_error: self.shrink_error,
        })
    }
}

fn check_used_len(storage_len: usize, used_len: u32) -> Result<usize> {
    let used = used_len as usize;
    if used > storage_len {
        Err(ErrorKind::UsedLength.into())
    } else {
        Ok(used)
    }
}

/// Fixed-room host storage holding an exclusive borrow of the original slice.
///
/// Shrinking hides a suffix without releasing its borrow. Regrowth zeroes the
/// newly exposed suffix, including bytes hidden by a previous shrink. Dropping
/// or forgetting this store performs no deferred wire edits.
///
/// A read borrow prevents resizing until its last use:
///
/// ```compile_fail
/// # #![forbid(unsafe_code)]
/// use blubber::{ReadStorage, SliceStore};
/// use blubber::raw::Resize;
/// let mut room = [0; 24];
/// let mut store = SliceStore::new(&mut room);
/// let old = store.bytes();
/// store.resize(12).unwrap();
/// assert_eq!(old.len(), 24);
/// ```
pub struct SliceStore<'a> {
    room: &'a mut [u8],
    exposed: usize,
    state: State,
}

impl<'a> SliceStore<'a> {
    /// Borrows the whole slice and initially exposes all of it.
    pub fn new(room: &'a mut [u8]) -> Self {
        Self {
            exposed: room.len(),
            room,
            state: State::default(),
        }
    }
}

impl private::Sealed for SliceStore<'_> {}

impl ReadStorage for SliceStore<'_> {
    fn is_closed(&self) -> bool {
        self.state.closed
    }

    fn bytes(&self) -> &[u8] {
        self.state.assert_open();
        &self.room[..self.exposed]
    }
}

impl private::Write for SliceStore<'_> {
    #[inline(always)]
    fn check_initialize_resize(&self, needed: usize) -> Result<()> {
        <Self as crate::Resize>::check_resize(self, needed)
    }

    fn exact_access(&mut self) -> private::FixedAccess<'_> {
        self.state.assert_open();
        private::FixedAccess {
            bytes: &mut self.room[..self.exposed],
            poisoned: &mut self.state.poisoned,
        }
    }

    fn fixed_access(&mut self) -> Result<private::FixedAccess<'_>> {
        self.state.check_open()?;
        Ok(private::FixedAccess {
            bytes: &mut self.room[..self.exposed],
            poisoned: &mut self.state.poisoned,
        })
    }
    fn edit_access(&mut self) -> private::EditAccess<'_> {
        self.state.assert_open();
        private::EditAccess {
            limit: self.room.len().min(u32::MAX as usize),
            hard_limit: u32::MAX as usize,
            limit_error: ErrorKind::Capacity,
            #[cfg(feature = "pinocchio")]
            preflight: private::Preflight::default(),
            bytes: &mut self.room[..self.exposed],
            poisoned: &mut self.state.poisoned,
        }
    }
}

impl crate::Resize for SliceStore<'_> {
    fn storage_len(&self) -> usize {
        self.exposed
    }

    fn growth_limit(&self) -> usize {
        self.room.len()
    }

    fn check_resize(&self, new_len: usize) -> Result<()> {
        self.state.check_open()?;
        if new_len > self.room.len() || new_len > u32::MAX as usize {
            return Err(ErrorKind::Capacity.into());
        }
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        self.state.poisoned
    }
    fn __backend(&mut self, _: crate::storage::Writer) -> &mut dyn crate::storage::Backend {
        self
    }
}

impl Backend for SliceStore<'_> {
    fn bytes_mut(&mut self) -> &mut [u8] {
        self.state.assert_open();
        &mut self.room[..self.exposed]
    }

    fn resize(&mut self, new_len: usize) -> Result<()> {
        self.check_resize(new_len)?;
        if new_len > self.exposed {
            self.room[self.exposed..new_len].fill(0);
        } else {
            self.state.shrink_error = None;
        }
        self.exposed = new_len;
        Ok(())
    }

    fn set_poisoned(&mut self, poisoned: bool) {
        self.state.assert_open();
        self.state.poisoned = poisoned;
    }

    fn commit(&mut self, used_len: u32) -> Result<CommitInfo> {
        self.state.commit(self.exposed, used_len)
    }
}

/// Host storage holding an exclusive borrow of a Vec's initialized bytes.
///
/// Available with `alloc`. Growth uses fallible reservation and may relocate;
/// callers must end all byte borrows first. Physical shrink allocates and copies
/// the retained prefix before swapping allocations, so failure preserves the
/// entire old buffer. Logical deletion alone does not request physical shrink.
///
/// A byte borrow also prevents replacing the borrowed Vec:
///
/// ```compile_fail
/// # #![forbid(unsafe_code)]
/// use blubber::{ReadStorage, VecStore};
/// let mut buffer = vec![0; 12];
/// let store = VecStore::new(&mut buffer);
/// let old = store.bytes();
/// buffer = vec![1; 24];
/// assert_eq!(old[0], 0);
/// ```
#[cfg(feature = "alloc")]
pub struct VecStore<'a> {
    buffer: &'a mut Vec<u8>,
    state: State,
    #[cfg(test)]
    test_mode: TestMode,
}

#[cfg(feature = "alloc")]
impl<'a> VecStore<'a> {
    /// Borrows a Vec, exposing its initialized length rather than its capacity.
    pub fn new(buffer: &'a mut Vec<u8>) -> Self {
        Self {
            buffer,
            state: State::default(),
            #[cfg(test)]
            test_mode: TestMode::default(),
        }
    }

    fn resize_buffer(&mut self, new_len: usize) -> Result<()> {
        let old_len = self.buffer.len();
        if new_len == old_len {
            return Ok(());
        }
        #[cfg(test)]
        if self.test_mode.fail_next == Some(new_len > old_len) {
            self.test_mode.fail_next = None;
            return Err(ErrorKind::Allocation.into());
        }
        #[cfg(test)]
        if self.test_mode.relocate && new_len > old_len {
            return self.copy_and_swap(new_len);
        }
        if new_len > old_len {
            self.buffer
                .try_reserve_exact(new_len - old_len)
                .map_err(|_| ErrorKind::Allocation)?;
            self.buffer.resize(new_len, 0);
            Ok(())
        } else {
            self.copy_and_swap(new_len)
        }
    }

    fn copy_and_swap(&mut self, new_len: usize) -> Result<()> {
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(new_len)
            .map_err(|_| ErrorKind::Allocation)?;
        replacement.extend_from_slice(&self.buffer[..self.buffer.len().min(new_len)]);
        replacement.resize(new_len, 0);
        core::mem::swap(self.buffer, &mut replacement);
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl private::Sealed for VecStore<'_> {}

#[cfg(feature = "alloc")]
impl ReadStorage for VecStore<'_> {
    fn is_closed(&self) -> bool {
        self.state.closed
    }

    fn bytes(&self) -> &[u8] {
        self.state.assert_open();
        self.buffer.as_slice()
    }
}

#[cfg(feature = "alloc")]
impl private::Write for VecStore<'_> {
    #[inline(always)]
    fn check_initialize_resize(&self, needed: usize) -> Result<()> {
        <Self as crate::Resize>::check_resize(self, needed)
    }

    fn exact_access(&mut self) -> private::FixedAccess<'_> {
        self.state.assert_open();
        private::FixedAccess {
            bytes: self.buffer.as_mut_slice(),
            poisoned: &mut self.state.poisoned,
        }
    }

    fn fixed_access(&mut self) -> Result<private::FixedAccess<'_>> {
        self.state.check_open()?;
        Ok(private::FixedAccess {
            bytes: self.buffer.as_mut_slice(),
            poisoned: &mut self.state.poisoned,
        })
    }
    fn edit_access(&mut self) -> private::EditAccess<'_> {
        self.state.assert_open();
        private::EditAccess {
            limit: (u32::MAX as usize).min(isize::MAX as usize),
            hard_limit: (u32::MAX as usize).min(isize::MAX as usize),
            limit_error: ErrorKind::Capacity,
            #[cfg(feature = "pinocchio")]
            preflight: private::Preflight::default(),
            bytes: self.buffer.as_mut_slice(),
            poisoned: &mut self.state.poisoned,
        }
    }
}

#[cfg(feature = "alloc")]
impl crate::Resize for VecStore<'_> {
    fn storage_len(&self) -> usize {
        self.buffer.len()
    }

    fn growth_limit(&self) -> usize {
        (u32::MAX as usize).min(isize::MAX as usize)
    }

    fn check_resize(&self, new_len: usize) -> Result<()> {
        self.state.check_open()?;
        if new_len > self.growth_limit() {
            return Err(ErrorKind::Capacity.into());
        }
        Ok(())
    }

    fn is_poisoned(&self) -> bool {
        self.state.poisoned
    }
    fn __backend(&mut self, _: crate::storage::Writer) -> &mut dyn crate::storage::Backend {
        self
    }
}

#[cfg(feature = "alloc")]
impl Backend for VecStore<'_> {
    fn bytes_mut(&mut self) -> &mut [u8] {
        self.state.assert_open();
        self.buffer.as_mut_slice()
    }

    fn resize(&mut self, new_len: usize) -> Result<()> {
        self.check_resize(new_len)?;
        #[cfg(test)]
        {
            self.test_mode.resize_calls += 1;
        }
        let shrinking = new_len <= self.buffer.len();
        let result = self.resize_buffer(new_len);
        if shrinking {
            self.state.shrink_error = result.err();
        }
        result
    }

    fn set_poisoned(&mut self, poisoned: bool) {
        self.state.assert_open();
        self.state.poisoned = poisoned;
    }

    fn commit(&mut self, used_len: u32) -> Result<CommitInfo> {
        self.state.commit(self.buffer.len(), used_len)
    }
}

// Fault injection and forced relocation are absent from production builds.
#[cfg(all(test, feature = "alloc"))]
#[derive(Default)]
struct TestMode {
    relocate: bool,
    resize_calls: usize,
    // Some(true) fails the next growth; Some(false) the next shrink.
    fail_next: Option<bool>,
}

#[cfg(all(test, feature = "alloc"))]
mod tests;
