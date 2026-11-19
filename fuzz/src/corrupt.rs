//! SDK-backed policy-program histories. Model decoding/encoding shares no production codec.
use blubber::{Error, ErrorKind, ReadStorage, Resize, SliceStore};
use blubber_model::{Error as ModelError, IntWidth, Key, KeySchema, Schema, Value};
use blubber_policy::{
    corrupt::{self, State, COMMANDS, RECORD, TRACE},
    Numbers,
};
use mollusk_svm::{result::ProgramResult, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::{cell::RefCell, collections::BTreeMap, ops::AddAssign};

fn schema() -> Schema {
    Schema::Struct(vec![
        Schema::Bool,
        Schema::Map(KeySchema::Bool, Box::new(Schema::Bool)),
        Schema::List(Box::new(Schema::Str)),
        Schema::Map(KeySchema::Unsigned(IntWidth::W1), Box::new(Schema::Str)),
        Schema::Str,
    ])
}

fn initial() -> Vec<u8> {
    let value = Value::Struct(vec![
        Value::Bool(true),
        Value::Map(BTreeMap::from([
            (Key::Bool(false), Value::Bool(false)),
            (Key::Bool(true), Value::Bool(true)),
        ])),
        Value::List(vec![Value::Str("old".into()), Value::Str("λ".into())]),
        Value::Map(BTreeMap::from([
            (Key::Unsigned(1), Value::Str("one".into())),
            (Key::Unsigned(3), Value::Str("three".into())),
        ])),
        Value::Str("sentinel".into()),
    ]);
    account(&schema().encode(&value).unwrap())
}

fn account(root: &[u8]) -> Vec<u8> {
    let mut bytes = State::DISCRIMINATOR.to_vec();
    bytes.extend_from_slice(&((12 + root.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes.resize(bytes.len() + 64, 0xa5);
    bytes
}

fn decode(bytes: &[u8]) -> Result<Value, ModelError> {
    schema()
        .decode_account(State::DISCRIMINATOR, bytes)
        .map(|account| account.value)
}

fn used(bytes: &[u8]) -> Option<usize> {
    let word = bytes.get(8..12)?.try_into().ok()?;
    let used = u32::from_le_bytes(word) as usize;
    (used >= 12 && used <= bytes.len()).then_some(used)
}

fn kind<T>(result: &blubber::Result<T>) -> Option<ErrorKind> {
    match result {
        Err(Error::Fault { kind, .. }) => Some(*kind),
        _ => None,
    }
}

fn field(value: &mut Value, index: usize) -> &mut Value {
    let Value::Struct(fields) = value else {
        unreachable!("state root")
    };
    &mut fields[index]
}

fn items(value: &mut Value) -> &mut Vec<Value> {
    let Value::List(values) = field(value, 2) else {
        unreachable!("items")
    };
    values
}

fn map(value: &mut Value, index: usize) -> &mut BTreeMap<Key, Value> {
    let Value::Map(values) = field(value, index) else {
        unreachable!("map field")
    };
    values
}

// Untouched spare stays intact. Callback setters may leave a contiguous zero
// high-water tail; a shrink clears its old tail. No command can write without
// a well-formed envelope, and none may publish a malformed one.
fn frame(before: &[u8], after: &[u8], callback: bool) {
    let Some(old) = used(before) else {
        assert_eq!(after, before, "no write without an envelope");
        return;
    };
    let new = used(after).expect("published used length");
    let (spare_before, spare_after) = (&before[old.max(new)..], &after[old.max(new)..]);
    if callback {
        // Several setters may grow above both endpoint lengths, then shrink.
        // Their intermediate high-water tail becomes a contiguous zero prefix.
        if let Some(last) = spare_before
            .iter()
            .zip(spare_after)
            .rposition(|(a, b)| a != b)
        {
            assert!(
                spare_after[..=last].iter().all(|byte| *byte == 0),
                "callback cleared high-water tail"
            );
        }
    } else {
        assert_eq!(spare_after, spare_before, "physical spare");
    }
    if new < old {
        assert!(
            after[new..old].iter().all(|byte| *byte == 0),
            "cleared tail"
        );
    }
}

// Bool commands overwrite whole value bytes in place. On any root, every
// changed byte lies inside the published root and holds the requested value.
fn bool_frame(before: &[u8], after: &[u8], command: &[u8; 8]) {
    let op = command[0] % 16;
    let changed: Vec<_> = (0..before.len())
        .filter(|at| before[*at] != after[*at])
        .collect();
    let end = used(before).unwrap_or(12);
    // The flag is the only fixed root field; map values follow the directory.
    let region = if op == 0 { 12..13.min(end) } else { 13..end };
    assert!(
        changed.iter().all(|at| region.contains(at)),
        "bool write frame"
    );
    assert!(
        op != 7 || changed.len() <= 4,
        "cursor visits at most four slots"
    );
    assert!(
        changed.iter().all(|at| after[*at] == command[1] & 1),
        "bool write value"
    );
}

/// Expected result of one command on a valid root after any checked recovery.
struct Outcome {
    ok: bool,
    /// Error category when the model determines it.
    kind: Option<ErrorKind>,
    /// Complete bytes afterwards, or the planned extent of an interrupted codec.
    bytes: Result<Vec<u8>, usize>,
    poison: bool,
}

impl Outcome {
    fn ok(bytes: Vec<u8>) -> Self {
        Self {
            ok: true,
            kind: None,
            bytes: Ok(bytes),
            poison: false,
        }
    }

    // Caller-input, identity and capacity errors precede every write.
    fn rejected(before: &[u8], kind: ErrorKind) -> Self {
        Self {
            ok: false,
            kind: Some(kind),
            bytes: Ok(before.to_vec()),
            poison: false,
        }
    }
}

// Complete logical replacements, independently re-encoded after each operation.
// Callback partial progress is modeled element by element against exact capacity.
fn model(before: &[u8], mut value: Value, command: &[u8; 8]) -> Outcome {
    let index = u32::from_le_bytes(command[4..8].try_into().unwrap()) as usize;
    let text = Value::Str(["", "x", "new", "λ"][command[3] as usize % 4].into());
    let flag = Value::Bool(command[1] & 1 != 0);
    match command[0] % 16 {
        0 => *field(&mut value, 0) = flag,
        1 => match items(&mut value).get_mut(index) {
            Some(slot) => *slot = text,
            None => return Outcome::rejected(before, ErrorKind::IndexOutOfBounds),
        },
        // Byte 2 selects push/insert/extend/insert-run and remove/clear/truncate/pop.
        2 => {
            let list = items(&mut value);
            let run = vec![text.clone(); command[1] as usize % 3];
            let at = match command[2] % 4 {
                0 | 2 => list.len(),
                _ if index > list.len() => {
                    return Outcome::rejected(before, ErrorKind::IndexOutOfBounds)
                }
                _ => index,
            };
            let values = if command[2] % 4 < 2 { vec![text] } else { run };
            list.splice(at..at, values);
        }
        3 => {
            let list = items(&mut value);
            match command[2] % 4 {
                0 => {
                    if index >= list.len() {
                        return Outcome::rejected(before, ErrorKind::IndexOutOfBounds);
                    }
                    list.remove(index);
                }
                1 => list.clear(),
                2 => list.truncate(index),
                _ => {
                    list.pop();
                }
            }
        }
        // Byte 2 selects insert/or-insert/entry-insert/value-set and
        // remove/clear/entry-remove.
        4 => {
            let key = Key::Unsigned(command[1].into());
            let entries = map(&mut value, 3);
            match command[2] % 4 {
                1 => {
                    entries.entry(key).or_insert(text);
                }
                3 => {
                    if let Some(slot) = entries.get_mut(&key) {
                        *slot = text;
                    }
                }
                _ => {
                    entries.insert(key, text);
                }
            }
        }
        5 => {
            let entries = map(&mut value, 3);
            match command[2] % 3 {
                1 => entries.clear(),
                _ => {
                    entries.remove(&Key::Unsigned(command[1].into()));
                }
            }
        }
        6 | 7 => map(&mut value, 1)
            .values_mut()
            .for_each(|slot| *slot = flag.clone()),
        8 => {
            if items(&mut value).len() != command[1] as usize % 5 {
                return Outcome::rejected(before, ErrorKind::InvalidBoundary);
            }
            items(&mut value).fill(text);
        }
        // Byte 2 selects the callback walk or a cursor that sets four values.
        9 => {
            let limit = if command[2].is_multiple_of(2) {
                usize::MAX
            } else {
                4
            };
            return callback(before, value, text, limit);
        }
        11 => return Outcome::rejected(before, ErrorKind::Discriminator),
        12 => return interrupt(before, value),
        // Verification and every load-only command change nothing.
        _ => (),
    }
    model_bytes(before, &value).map_or_else(
        || Outcome::rejected(before, ErrorKind::Capacity),
        Outcome::ok,
    )
}

/// Coverage slot of a map route: commands 4, 5 and 9 select by byte 2.
fn map_route(op: u8, command: &[u8; 8]) -> Option<usize> {
    match op {
        4 => Some(command[2] as usize % 4),
        5 => Some(4 + command[2] as usize % 3),
        9 => Some(7 + command[2] as usize % 2),
        _ => None,
    }
}

// Each setter completes or fails before writing. The first that does not fit
// stops the walk and keeps every earlier replacement.
fn callback(before: &[u8], mut value: Value, text: Value, limit: usize) -> Outcome {
    let keys: Vec<_> = map(&mut value, 3).keys().take(limit).cloned().collect();
    let mut bytes = before.to_vec();
    for key in keys {
        *map(&mut value, 3).get_mut(&key).unwrap() = text.clone();
        let Some(next) = model_bytes(&bytes, &value) else {
            return Outcome {
                bytes: Ok(bytes),
                ..Outcome::rejected(before, ErrorKind::Capacity)
            };
        };
        bytes = next;
    }
    Outcome::ok(bytes)
}

// The custom encoder fails after its first slot. Planning accepts one to four
// items; capacity is checked before poison. Moves stay inside the planned
// extent and publication stays old.
fn interrupt(before: &[u8], mut value: Value) -> Outcome {
    let count = items(&mut value).len();
    if count == 0 || count > 4 {
        return Outcome::rejected(before, ErrorKind::InvalidBoundary);
    }
    items(&mut value).fill(Value::Str("new".into()));
    let Some(planned) = model_bytes(before, &value) else {
        return Outcome::rejected(before, ErrorKind::Capacity);
    };
    Outcome {
        ok: false,
        kind: Some(ErrorKind::InvalidBoundary),
        bytes: Err(used(&planned).expect("planned extent")),
        poison: true,
    }
}

fn model_bytes(before: &[u8], value: &Value) -> Option<Vec<u8>> {
    let root = schema().encode(value).unwrap();
    if root.len() + 12 > before.len() {
        return None;
    }
    let mut bytes = before.to_vec();
    let end = root.len() + 12;
    bytes[8..12].copy_from_slice(&(end as u32).to_le_bytes());
    bytes[12..end].copy_from_slice(&root);
    if let Some(old) = used(before).filter(|old| *old > end) {
        bytes[end..old].fill(0);
    }
    Some(bytes)
}

// Read results derived from the decoded model of a valid root.
fn model_read(mut value: Value, command: &[u8; 8]) -> u64 {
    let index = u32::from_le_bytes(command[4..8].try_into().unwrap()) as usize;
    let text = |value: Option<&Value>| match value {
        Some(Value::Str(text)) => corrupt::digest(text.as_bytes()),
        None => corrupt::MISSING,
        Some(_) => unreachable!("string value"),
    };
    match command[0] % 6 {
        0 => u64::from(*field(&mut value, 0) == Value::Bool(true)),
        1 => text(items(&mut value).get(index)),
        2 => text(map(&mut value, 3).get(&Key::Unsigned(command[1].into()))),
        3 => {
            let pairs: Vec<u8> = map(&mut value, 1)
                .iter()
                .flat_map(|pair| match pair {
                    (Key::Bool(key), Value::Bool(value)) => [u8::from(*key), u8::from(*value)],
                    _ => unreachable!("bool pair"),
                })
                .collect();
            corrupt::digest(&pairs)
        }
        4 => text(Some(field(&mut value, 4))),
        _ => 0,
    }
}

/// Contract events from replayed histories, for non-vacuity checks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Coverage {
    /// Valid-root lazy successes by edit command; 13 counts checked loads.
    pub edits: [u32; 16],
    /// Valid-root lazy successes by read command.
    pub reads: [u32; 6],
    /// Valid-root lazy successes by list route: push, insert, extend,
    /// insert-run, remove, clear, truncate, pop.
    pub routes: [u32; 8],
    /// Malformed lazy list sources that poisoned without publishing.
    pub lazy_poison: u32,
    /// Valid-root lazy successes by map route: insert, or-insert, entry-insert,
    /// value-set, remove, clear, entry-remove, callback walk, cursor walk.
    pub map_routes: [u32; 9],
    /// Malformed lazy map sources that poisoned.
    pub map_poison: u32,
    /// Invalid roots rejected by explicit checked ingress.
    pub checked: u32,
    /// Invalid roots rejected by `verify-on-load` lazy ingress.
    pub eager: u32,
    /// Lazy access rejected by sticky poison.
    pub poisoned: u32,
    /// Poisoned valid roots recovered by checked ingress.
    pub recovered: u32,
    /// Interrupted encoders that poisoned a valid root.
    pub interrupted: u32,
    /// Reopens of a poisoned session.
    pub reopened: u32,
}

impl AddAssign for Coverage {
    fn add_assign(&mut self, other: Self) {
        for (sum, count) in self.edits.iter_mut().zip(other.edits) {
            *sum += count;
        }
        for (sum, count) in self.reads.iter_mut().zip(other.reads) {
            *sum += count;
        }
        for (sum, count) in self.routes.iter_mut().zip(other.routes) {
            *sum += count;
        }
        self.lazy_poison += other.lazy_poison;
        for (sum, count) in self.map_routes.iter_mut().zip(other.map_routes) {
            *sum += count;
        }
        self.map_poison += other.map_poison;
        self.checked += other.checked;
        self.eager += other.eager;
        self.poisoned += other.poisoned;
        self.recovered += other.recovered;
        self.interrupted += other.interrupted;
        self.reopened += other.reopened;
    }
}

impl std::fmt::Display for Coverage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "valid-ok edits={:?} reads={:?} list-routes={:?} map-routes={:?}; rejected checked={} eager={}; poison lazy={} malformed-list={} malformed-map={} recovered={} interrupted={} reopened={}",
            self.edits, self.reads, self.routes, self.map_routes, self.checked, self.eager, self.poisoned, self.lazy_poison, self.map_poison, self.recovered, self.interrupted, self.reopened
        )
    }
}

impl Coverage {
    /// Fails when a claimed contract never ran. Command 11 loads a foreign
    /// schema and command 12 is an encoder that always fails, so neither can
    /// succeed on a valid root.
    pub fn assert_nonvacuous(&self) {
        for op in (0..16).filter(|op| !matches!(op, 11 | 12)) {
            assert!(self.edits[op] > 0, "valid-root edit {op} never succeeded");
        }
        for (op, &count) in self.reads.iter().enumerate() {
            assert!(count > 0, "valid-root read {op} never succeeded");
        }
        for (route, &count) in self.routes.iter().enumerate() {
            assert!(count > 0, "valid-root list route {route} never succeeded");
        }
        assert_eq!(
            self.lazy_poison > 0,
            !cfg!(feature = "verify-on-load"),
            "malformed lazy list poison follows lazy ingress"
        );
        for (route, &count) in self.map_routes.iter().enumerate() {
            assert!(count > 0, "valid-root map route {route} never succeeded");
        }
        assert_eq!(
            self.map_poison > 0,
            !cfg!(feature = "verify-on-load"),
            "malformed lazy map poison follows lazy ingress"
        );
        assert!(self.checked > 0, "no checked rejection");
        assert_eq!(
            self.eager > 0,
            cfg!(feature = "verify-on-load"),
            "eager rejection follows verify-on-load"
        );
        assert!(self.poisoned > 0, "no sticky poison rejection");
        assert!(self.recovered > 0, "no checked poison recovery");
        assert!(self.interrupted > 0, "no interrupted encoder");
        assert!(self.reopened > 0, "no reopen after poison");
    }
}

thread_local! {
    static VMS: RefCell<Vec<Mollusk>> = RefCell::new((0..if std::env::var_os("BLUBBER_POLICY_ELF_V3").is_some() { 6 } else { 3 }).map(|profile| {
        let artifact = std::env::var(if profile < 3 { "BLUBBER_POLICY_ELF" } else { "BLUBBER_POLICY_ELF_V3" }).expect("dedicated fresh SBF artifact");
        let mut vm = Mollusk::default();
        // Corrupt work is bounded by bytes, not by a deployable CU promise.
        vm.compute_budget.compute_unit_limit = 100_000_000;
        vm.feature_set.virtual_address_space_adjustments = profile % 3 != 0;
        vm.feature_set.account_data_direct_mapping = profile % 3 == 2;
        vm.program_cache = mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&Pubkey::new_from_array([7; 32]), &artifact);
        vm
    }).collect());
}

/// Runs at most 32 commands on at most 65,536 input bytes, without catching panics.
/// Feature-off jobs use genuine authenticated SDK admission. Raw host replay is
/// a differential control; valid roots additionally use the independent model.
/// Header: profile, flags, initial-length:u16le, initial account, eight-byte commands.
/// Flags: 1=model-built start, 2=checked ingress, 4=wrap initial bytes in an envelope.
pub fn check(data: &[u8], edit: bool) -> Coverage {
    let mut coverage = Coverage::default();
    let data = &data[..data.len().min(65_536)];
    let [profile, flags, lo, hi, rest @ ..] = data else {
        return coverage;
    };
    let count = usize::from(u16::from_le_bytes([*lo, *hi])).min(rest.len());
    let (bytes, commands) = rest.split_at(count);
    let mut bytes = if flags & 1 != 0 {
        initial()
    } else if flags & 4 != 0 {
        account(bytes)
    } else {
        bytes.to_vec()
    };
    let original = bytes.clone();
    let commands = &commands[..commands.len().min(COMMANDS * 8) / 8 * 8];
    let mut expected = vec![0; COMMANDS * RECORD];
    let (records, _) = expected.as_chunks_mut::<RECORD>();
    let eager = cfg!(feature = "verify-on-load");
    let mut len = bytes.len();
    if edit {
        let mut store = SliceStore::new(&mut bytes);
        for (command, out) in commands.as_chunks::<8>().0.iter().zip(records) {
            let op = command[0] % 16;
            if op == 14 || op == 15 {
                // Mirror SDK Drop: trim to a well-formed used length. Poison is
                // volatile, so the new session opens clean; that is not recovery.
                coverage.reopened += u32::from(store.is_poisoned());
                len = used(&bytes[..len]).unwrap_or(len);
                store = SliceStore::new(&mut bytes[..len]);
            }
            let poisoned = store.is_poisoned();
            let before = store.bytes().to_vec();
            let checked = op == 13 || (flags & 2 != 0 && !matches!(op, 11 | 12));
            let result = if op == 11 {
                Numbers::load_trusted_mut(&mut store).map(|_| ())
            } else if op == 12 {
                corrupt::interrupt(blubber::raw::backend(&mut store))
            } else if checked {
                State::load_checked_mut(&mut store)
                    .and_then(|mut root| corrupt::edit(&mut root, command))
            } else {
                State::load_trusted_mut(&mut store)
                    .and_then(|mut root| corrupt::edit(&mut root, command))
            };
            let after = store.bytes();
            assert_eq!(after.len(), before.len(), "exposed length");
            assert_eq!(after.get(..8), before.get(..8), "identity bytes");
            if matches!(op, 10 | 11 | 13 | 14 | 15) {
                assert_eq!(after, before, "load-only command");
            }
            if op == 12 {
                // Raw publication happens only after a complete encoding.
                assert!(result.is_err(), "interrupted encoder succeeded");
                assert_eq!(
                    after.get(..12),
                    before.get(..12),
                    "interrupt kept publication"
                );
                assert!(
                    after == before || store.is_poisoned(),
                    "destructive interrupt poisons"
                );
            }
            if matches!(op, 0 | 6 | 7) {
                bool_frame(&before, after, command);
            }
            let valid = decode(&before);
            if poisoned && !checked {
                assert_eq!(kind(&result), Some(ErrorKind::Poisoned), "sticky poison");
                assert_eq!(after, before, "poisoned access wrote");
                assert!(store.is_poisoned(), "poison cleared without recovery");
                coverage.poisoned += 1;
            } else if let Ok(value) = valid {
                let outcome = model(&before, value, command);
                assert_eq!(result.is_ok(), outcome.ok, "valid-root result: {result:?}");
                if let Some(kind_expected) = outcome.kind {
                    assert_eq!(kind(&result), Some(kind_expected), "valid-root error");
                }
                match &outcome.bytes {
                    Ok(bytes) => assert_eq!(after, bytes, "valid-root model bytes"),
                    Err(planned) => {
                        let reach = used(&before).unwrap().max(*planned);
                        assert_eq!(after[reach..], before[reach..], "interrupt spare");
                        coverage.interrupted += 1;
                    }
                }
                assert_eq!(store.is_poisoned(), outcome.poison, "valid-root poison");
                // Count lazy successes; command 13 is checked by definition.
                coverage.edits[op as usize] += u32::from(result.is_ok() && (!checked || op == 13));
                if matches!(op, 2 | 3) {
                    let route = (op as usize - 2) * 4 + command[2] as usize % 4;
                    coverage.routes[route] += u32::from(result.is_ok() && !checked);
                }
                if let Some(route) = map_route(op, command) {
                    coverage.map_routes[route] += u32::from(result.is_ok() && !checked);
                }
                coverage.recovered += u32::from(poisoned);
            } else if poisoned {
                assert!(result.is_err(), "checked recovery accepted an invalid root");
                assert_eq!(after, before, "rejected recovery wrote");
                assert!(store.is_poisoned(), "rejected recovery cleared poison");
            } else if checked || (eager && op != 11 && op != 12) {
                assert!(result.is_err(), "ingress accepted an invalid root");
                assert_eq!(after, before, "rejected ingress wrote");
                assert!(!store.is_poisoned(), "rejected ingress poisoned");
                coverage.checked += u32::from(checked);
                coverage.eager += u32::from(!checked);
            } else if !store.is_poisoned() {
                // Cursor visits and callbacks keep completed edits before an error.
                if result.is_err() && !matches!(op, 7 | 9) {
                    assert_eq!(after, before, "pre-write failure frame");
                }
                frame(&before, after, op == 9);
            } else {
                if !matches!(op, 7 | 9 | 12) {
                    // A lazy edit that detects malformed source poisons before
                    // returning and never publishes a new used length.
                    assert!(result.is_err(), "lazy poison without an error");
                    assert_eq!(after.get(..12), before.get(..12), "poisoned edit published");
                    coverage.lazy_poison += u32::from(matches!(op, 2 | 3));
                }
                // A walk keeps completed sets before the visit that poisons.
                if map_route(op, command).is_some() {
                    assert!(result.is_err(), "lazy map poison without an error");
                    coverage.map_poison += 1;
                }
            }
            out[..8].copy_from_slice(&corrupt::code(result).to_le_bytes());
            out[16] = u8::from(store.is_poisoned());
        }
    } else {
        let valid = decode(&bytes).ok();
        for (command, out) in commands.as_chunks::<8>().0.iter().zip(records) {
            let checked = flags & 2 != 0;
            let result = if checked {
                State::load(&bytes)
            } else {
                State::load_trusted(&bytes)
            }
            .and_then(|root| corrupt::read(root, command));
            if let Some(value) = &valid {
                let read = model_read(value.clone(), command);
                assert_eq!(result, Ok(read), "valid-root read");
                coverage.reads[command[0] as usize % 6] += u32::from(!checked);
            } else if checked || eager {
                assert!(result.is_err(), "read ingress accepted an invalid root");
                coverage.checked += u32::from(checked);
                coverage.eager += u32::from(!checked);
            }
            let value = result.as_ref().map_or(0, |value| *value);
            out[..8].copy_from_slice(&corrupt::code(result.map(|_| ())).to_le_bytes());
            out[8..16].copy_from_slice(&value.to_le_bytes());
        }
    }
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let trace = Pubkey::new_from_array([9; 32]);
    let accounts = [
        (
            key,
            Account {
                lamports: 1_000_000_000,
                data: original.clone(),
                owner,
                ..Account::default()
            },
        ),
        (
            trace,
            Account {
                lamports: 1_000_000_000,
                data: vec![0; TRACE + original.len()],
                owner,
                ..Account::default()
            },
        ),
    ];
    let mut instruction = vec![0x77, u8::from(edit) | (flags & 2)];
    instruction.extend_from_slice(commands);
    let result = VMS.with(|vms| {
        let vms = vms.borrow_mut();
        vms[*profile as usize % vms.len()].process_instruction(
            &Instruction {
                program_id: owner,
                // The program itself is listed so command 15 can invoke it.
                accounts: vec![
                    AccountMeta::new(key, false),
                    AccountMeta::new(trace, false),
                    AccountMeta::new_readonly(owner, false),
                ],
                data: instruction,
            },
            &accounts,
        )
    });
    assert_eq!(
        result.program_result,
        ProgramResult::Success,
        "VM fault/timeout is never an expected rejection"
    );
    let actual = &result.resulting_accounts[1].1.data;
    assert_eq!(
        &actual[..COMMANDS * RECORD],
        expected,
        "SDK/host policy, error, read value and poison trace"
    );
    let final_len = u32::from_le_bytes(actual[COMMANDS * RECORD..TRACE].try_into().unwrap());
    assert_eq!(final_len as usize, len, "SDK/host reopen trim");
    assert_eq!(
        actual[TRACE..TRACE + len],
        bytes[..len],
        "SDK/host complete physical frame"
    );
    assert!(actual[TRACE + len..].iter().all(|byte| *byte == 0));
    let output = &result.resulting_accounts[0].1;
    let drop_len = if edit {
        used(&bytes[..len]).unwrap_or(len)
    } else {
        len
    };
    assert_eq!(
        output.data,
        bytes[..drop_len],
        "ordinary best-effort Drop trim"
    );
    assert_eq!(output.lamports, accounts[0].1.lamports);
    assert_eq!(output.owner, owner);
    coverage
}

/// One directed damage with its pinned model error, first checked fault and
/// feature-off lazy probe results, so a family cannot silently drift.
struct Family {
    name: &'static str,
    bytes: Vec<u8>,
    model: ModelError,
    checked: Error,
    /// Lazy read probe: a digest of the observed value or the touched fault.
    read: blubber::Result<u64>,
    /// Lazy edit probe: its result and complete bytes afterwards. Poison stays clear.
    edit: blubber::Result<()>,
    after: Vec<u8>,
}

fn fault<T>(kind: ErrorKind, offset: u32) -> blubber::Result<T> {
    Err(Error::new(kind, offset))
}

fn families() -> Vec<Family> {
    use {ErrorKind as K, ModelError as M};
    let initial = initial();
    let max = u32::MAX.to_le_bytes().to_vec();
    let pairs = |pairs: [u8; 4]| Ok(corrupt::digest(&pairs));
    let old = Ok(corrupt::digest(b"old"));
    // Offsets: flag 12, root directory 13..25, fixed map 25, items 33,
    // entries 46, tail 68, used 76. Lazy reads do not certify map order, and
    // edits keep untouched damage. Touched damage fails before any write,
    // except that a cursor keeps its first value before the bad second key.
    let damage = [
        (
            "duplicate-key",
            31,
            vec![0],
            M::KeyOrder,
            K::DuplicateKey,
            31,
            pairs([0, 0, 0, 1]),
            Ok(()),
            vec![(32, 0)],
        ),
        (
            "descending-key",
            29,
            vec![1],
            M::KeyOrder,
            K::UnsortedKeys,
            31,
            pairs([1, 0, 0, 1]),
            Ok(()),
            vec![(32, 0)],
        ),
        (
            "distant-duplicate",
            55,
            vec![1],
            M::KeyOrder,
            K::DuplicateKey,
            55,
            pairs([0, 0, 1, 1]),
            Ok(()),
            vec![(32, 0)],
        ),
        (
            "distant-unsorted",
            50,
            vec![5],
            M::KeyOrder,
            K::UnsortedKeys,
            55,
            pairs([0, 0, 1, 1]),
            Ok(()),
            vec![(32, 0)],
        ),
        (
            "selected-list-end",
            37,
            max.clone(),
            M::InvalidBoundary,
            K::InvalidBoundary,
            37,
            fault(K::InvalidBoundary, 37),
            fault(K::InvalidBoundary, 37),
            vec![],
        ),
        (
            "selected-map-end",
            51,
            max.clone(),
            M::InvalidBoundary,
            K::InvalidBoundary,
            51,
            old,
            Ok(()),
            vec![(41, b'n'), (42, b'e'), (43, b'w')],
        ),
        (
            "bool-key",
            31,
            vec![2],
            M::InvalidBool,
            K::InvalidBool,
            31,
            fault(K::InvalidBool, 31),
            fault(K::InvalidBool, 31),
            vec![(30, 1)],
        ),
        (
            "bool-value",
            32,
            vec![2],
            M::InvalidBool,
            K::InvalidBool,
            32,
            fault(K::InvalidBool, 32),
            Ok(()),
            vec![(32, 0)],
        ),
        (
            "prefix-domain",
            12,
            vec![2],
            M::InvalidBool,
            K::InvalidBool,
            12,
            old,
            Ok(()),
            vec![(41, b'n'), (42, b'e'), (43, b'w')],
        ),
        (
            "identity",
            0,
            vec![0],
            M::Discriminator,
            K::Discriminator,
            0,
            fault(K::Discriminator, 0),
            fault(K::Discriminator, 0),
            vec![],
        ),
        (
            "alias-directory",
            17,
            8u32.to_le_bytes().into(),
            M::InvalidLength,
            K::Truncated,
            33,
            fault(K::Truncated, 33),
            fault(K::Truncated, 33),
            vec![],
        ),
        (
            "nonmonotone-directory",
            17,
            vec![0; 4],
            M::InvalidBoundary,
            K::InvalidBoundary,
            17,
            fault(K::InvalidBoundary, 17),
            fault(K::InvalidBoundary, Error::NO_OFFSET),
            vec![],
        ),
        (
            "full-width-end",
            13,
            max.clone(),
            M::InvalidBoundary,
            K::InvalidBoundary,
            13,
            fault(K::InvalidBoundary, 13),
            fault(K::InvalidBoundary, Error::NO_OFFSET),
            vec![],
        ),
        (
            "full-width-count",
            25,
            max,
            M::Overflow,
            K::ArithmeticOverflow,
            Error::NO_OFFSET,
            // Lazy header faults name the count word; checked ingress does not.
            fault(K::ArithmeticOverflow, 25),
            fault(K::ArithmeticOverflow, 25),
            vec![],
        ),
        (
            "terminal-mismatch",
            56,
            1u32.to_le_bytes().into(),
            M::InvalidBoundary,
            K::InvalidBoundary,
            56,
            old,
            Ok(()),
            vec![],
        ),
    ];
    damage
        .into_iter()
        .map(
            |(name, at, replacement, model, kind, offset, read, edit, patch)| {
                let mut bytes = initial.clone();
                bytes[at..at + replacement.len()].copy_from_slice(&replacement);
                if name == "descending-key" {
                    bytes[31] = 0;
                }
                let mut after = bytes.clone();
                for (at, byte) in patch {
                    after[at] = byte;
                }
                if name == "terminal-mismatch" {
                    // The rewrite grows items by one byte and moves the damaged
                    // entries directory intact: the model encoding, damage shifted.
                    let mut value = decode(&initial).unwrap();
                    items(&mut value).fill(Value::Str("new".into()));
                    after = model_bytes(&initial, &value).unwrap();
                    after[at + 1..at + 5].copy_from_slice(&replacement);
                }
                Family {
                    name,
                    bytes,
                    model,
                    checked: Error::new(kind, offset),
                    read,
                    edit,
                    after,
                }
            },
        )
        .collect()
}

// The single directed command each damage family keeps in the committed corpus.
fn probe(name: &str, edit: bool) -> [u8; 8] {
    let keys = ["key", "distant", "bool-value", "count"]
        .iter()
        .any(|part| name.contains(part));
    let op = if !edit {
        if keys {
            3
        } else {
            1
        }
    } else if name == "bool-key" {
        7
    } else if keys {
        6
    } else if name.contains("directory") || name.contains("terminal") {
        8
    } else {
        1
    };
    [op, if name == "bool-key" { 1 } else { 2 }, 0, 2, 0, 0, 0, 0]
}

fn tape(profile: u8, checked: u8, bytes: &[u8], commands: &[[u8; 8]]) -> Vec<u8> {
    let mut seed = vec![
        profile,
        checked,
        bytes.len() as u8,
        (bytes.len() >> 8) as u8,
    ];
    seed.extend_from_slice(bytes);
    seed.extend(commands.iter().flatten());
    seed
}

// Complete histories; each family is a tape whose initial bytes carry damage.
fn histories() -> Vec<(String, Vec<u8>, Vec<[u8; 8]>)> {
    let initial = initial();
    let mut histories = vec![];
    let mut every = vec![];
    // Visit before fill so a later invalid key tests partial progress.
    for op in [7, 0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15] {
        every.push([op, 1, 0, 2, 0, 0, 0, 0]);
        every.push([op, 3, 0, 3, 255, 255, 255, 255]);
    }
    histories.push(("valid".to_string(), initial.clone(), every.clone()));
    // List routes selected by byte 2, including full-width misses, empty runs,
    // pop/clear on an empty list and growth after clear.
    let max = [255; 4];
    let routes: Vec<[u8; 8]> = [
        ([2, 1, 1, 2], [1, 0, 0, 0]),
        ([2, 2, 2, 1], [0; 4]),
        ([2, 2, 3, 3], [0; 4]),
        ([2, 0, 3, 0], max),
        ([2, 0, 1, 1], max),
        ([2, 0, 2, 0], [0; 4]),
        ([3, 0, 3, 0], [0; 4]),
        ([3, 0, 2, 0], [2, 0, 0, 0]),
        ([3, 0, 2, 0], max),
        ([3, 0, 0, 0], max),
        ([10, 0, 0, 0], [0; 4]),
        ([3, 0, 1, 0], [0; 4]),
        ([3, 0, 3, 0], [0; 4]),
        ([3, 0, 1, 0], [0; 4]),
        ([3, 0, 2, 0], [0; 4]),
        ([2, 0, 0, 2], [0; 4]),
        ([2, 1, 1, 1], [0; 4]),
        ([2, 2, 3, 2], [1, 0, 0, 0]),
        ([10, 0, 0, 0], [0; 4]),
    ]
    .iter()
    .map(|(head, index)| {
        let mut command = [0; 8];
        command[..4].copy_from_slice(head);
        command[4..].copy_from_slice(index);
        command
    })
    .collect();
    histories.push(("list-routes".to_string(), initial.clone(), routes.clone()));
    // A full-width list count fails lazy header admission: clear poisons
    // without publishing, typed access then fails, checked ingress rejects the
    // root, and each reopen starts clean before the next route.
    let mut header = initial.clone();
    header[33..37].copy_from_slice(&u32::MAX.to_le_bytes());
    histories.push((
        "list-count-routes".to_string(),
        header,
        vec![
            [3, 0, 1, 0, 0, 0, 0, 0],
            [1, 0, 0, 1, 0, 0, 0, 0],
            [13, 0, 0, 0, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [3, 0, 2, 0, 0, 0, 0, 0],
            [15, 0, 0, 0, 0, 0, 0, 0],
            [3, 0, 3, 0, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [2, 2, 2, 1, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [2, 1, 1, 2, 0, 0, 0, 0],
        ],
    ));
    // Map routes selected by byte 2 on keys 1 and 3: vacant and occupied entry
    // inserts, value sets on present and absent keys, both walks, entry removal
    // of present and absent keys, clear, and growth after clear.
    let map_routes: Vec<[u8; 8]> = [
        [4, 2, 1, 1],
        [4, 2, 1, 2],
        [4, 5, 2, 2],
        [4, 1, 2, 3],
        [4, 3, 3, 1],
        [4, 9, 3, 1],
        [4, 7, 0, 2],
        [9, 0, 1, 2],
        [9, 0, 0, 1],
        [5, 2, 2, 0],
        [5, 2, 2, 0],
        [5, 5, 0, 0],
        [10, 0, 0, 0],
        [5, 0, 1, 0],
        [5, 0, 1, 0],
        [9, 0, 1, 2],
        [4, 1, 1, 3],
        [9, 0, 1, 2],
        [5, 1, 2, 0],
        [9, 0, 0, 2],
        [10, 0, 0, 0],
    ]
    .iter()
    .map(|head| {
        let mut command = [0; 8];
        command[..4].copy_from_slice(head);
        command
    })
    .collect();
    histories.push(("map-routes".to_string(), initial.clone(), map_routes));
    // On damaged roots each map route runs in its own session: poison is sticky
    // until the reopen that follows it. Walks run first and clear runs last,
    // because a successful clear discards the damage.
    let family_map_routes: Vec<[u8; 8]> = [
        [9, 0, 0, 1],
        [9, 0, 1, 2],
        [4, 3, 3, 1],
        [4, 1, 2, 3],
        [4, 2, 1, 1],
        [4, 2, 0, 1],
        [5, 3, 2, 0],
        [5, 1, 0, 0],
        [5, 0, 1, 0],
    ]
    .iter()
    .flat_map(|head| {
        let mut command = [0; 8];
        command[..4].copy_from_slice(head);
        [command, [14, 0, 0, 0, 0, 0, 0, 0]]
    })
    .collect();
    // A full-width map count fails lazy header admission on every map route;
    // each reopen starts clean before the next route.
    let mut header = initial.clone();
    header[46..50].copy_from_slice(&u32::MAX.to_le_bytes());
    histories.push((
        "map-count-routes".to_string(),
        header,
        vec![
            [5, 0, 1, 0, 0, 0, 0, 0],
            [13, 0, 0, 0, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [4, 2, 0, 1, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [4, 1, 3, 2, 0, 0, 0, 0],
            [15, 0, 0, 0, 0, 0, 0, 0],
            [5, 1, 2, 0, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [9, 0, 0, 1, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [9, 0, 1, 1, 0, 0, 0, 0],
        ],
    ));
    // No envelope: every command fails without writing, and Drop cannot trim.
    histories.push((
        "short-account".to_string(),
        initial[..7].to_vec(),
        every.clone(),
    ));
    for family in families() {
        histories.push((
            format!("{}-routes", family.name),
            family.bytes.clone(),
            routes.clone(),
        ));
        histories.push((
            format!("{}-map-routes", family.name),
            family.bytes.clone(),
            family_map_routes.clone(),
        ));
        histories.push((family.name.to_string(), family.bytes, every.clone()));
    }
    // First insert an empty value, then grow it before shrinking later values.
    // Exercise exact intermediate tail clearing on valid and distant-invalid roots.
    for corrupt in [false, true] {
        let mut bytes = initial.clone();
        if corrupt {
            bytes[31] = 0;
            bytes[33..37].copy_from_slice(&66u32.to_le_bytes());
            assert_eq!(decode(&bytes).err(), Some(ModelError::KeyOrder));
        }
        let state = if corrupt { "corrupt" } else { "valid" };
        histories.push((
            format!("callback-high-water-{state}"),
            bytes,
            vec![[4, 0, 0, 0, 0, 0, 0, 0], [9, 0, 0, 1, 0, 0, 0, 0]],
        ));
    }
    // Equal-length items let the interrupted encoder rewrite its first slot in
    // place, so the poisoned root stays valid: lazy access fails, checked
    // recovery succeeds, and reopens (one across a CPI) start clean.
    histories.push((
        "poison-recovery".to_string(),
        initial,
        vec![
            [8, 2, 0, 2, 0, 0, 0, 0],
            [12, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0],
            [13, 0, 0, 0, 0, 0, 0, 0],
            [12, 0, 0, 0, 0, 0, 0, 0],
            [14, 0, 0, 0, 0, 0, 0, 0],
            [12, 0, 0, 0, 0, 0, 0, 0],
            [15, 0, 0, 0, 0, 0, 0, 0],
            [0, 1, 0, 0, 0, 0, 0, 0],
            [2, 0, 0, 1, 0, 0, 0, 0],
        ],
    ));
    histories
}

/// Small directed SDK seeds: valid histories, identity/domain/order/directory damage,
/// full-width indices/counts/ends, poison recovery and reopen across CPI. Every
/// tape runs on each runtime mapping with lazy and checked ingress.
pub fn seeds() -> Vec<(String, Vec<u8>)> {
    check_families();
    let mut seeds = vec![];
    for (name, bytes, commands) in histories() {
        for checked in [0, 2] {
            for profile in 0..3 {
                seeds.push((
                    format!("{name}-p{profile}-c{checked}"),
                    tape(profile, checked, &bytes, &commands),
                ));
            }
        }
    }
    seeds
}

/// Committed corpus tapes for one target: complete valid, callback and poison
/// histories, plus one directed command per damage family; profile 0, lazy.
pub fn corpus(edit: bool) -> Vec<(String, Vec<u8>)> {
    let names: Vec<_> = families().iter().map(|family| family.name).collect();
    histories()
        .into_iter()
        .map(|(name, bytes, commands)| {
            let tape = if names.contains(&name.as_str()) {
                tape(0, 0, &bytes, &[probe(&name, edit)])
            } else {
                tape(0, 0, &bytes, &commands)
            };
            (name, tape)
        })
        .collect()
}

// Pins the valid start and every family's model error, checked faults and
// lazy probe results. With `verify-on-load`, lazy probes fail like checked ones.
fn check_families() {
    let initial = initial();
    decode(&initial).expect("valid seed decodes");
    State::load(&initial).expect("valid seed passes checked reads");
    let mut copy = initial.clone();
    State::load_checked_mut(&mut SliceStore::new(&mut copy))
        .expect("valid seed passes checked edits");
    for family in families() {
        let name = family.name;
        let eager = cfg!(feature = "verify-on-load");
        assert_eq!(
            decode(&family.bytes).err(),
            Some(family.model),
            "model error: {name}"
        );
        assert_eq!(
            State::load(&family.bytes).err(),
            Some(family.checked),
            "checked read: {name}"
        );
        let mut copy = family.bytes.clone();
        let mut store = SliceStore::new(&mut copy);
        let checked = State::load_checked_mut(&mut store).map(|_| ());
        assert_eq!(checked, Err(family.checked), "checked edit: {name}");
        let read = State::load_trusted(&family.bytes)
            .and_then(|root| corrupt::read(root, &probe(name, false)));
        let expected = if eager {
            Err(family.checked)
        } else {
            family.read
        };
        assert_eq!(read, expected, "lazy read probe: {name}");
        let edit = State::load_trusted_mut(&mut store)
            .and_then(|mut root| corrupt::edit(&mut root, &probe(name, true)));
        let (expected, after) = if eager {
            (Err(family.checked), &family.bytes)
        } else {
            (family.edit, &family.after)
        };
        assert_eq!(edit, expected, "lazy edit probe: {name}");
        assert_eq!(store.bytes(), after, "lazy edit probe bytes: {name}");
        // A lazy source fault poisons. Eager ingress and the identity check
        // reject before any session exists, so they leave no poison.
        let poison = !eager && edit.is_err() && name != "identity";
        assert_eq!(
            store.is_poisoned(),
            poison,
            "lazy edit probe poison: {name}"
        );
    }
}
