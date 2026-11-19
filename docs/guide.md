# Using Blubber

Start with the fields you want to store. If one field is a string, the
generated input borrows the string and the reader borrows its bytes from the
account. Finish reading before you edit or resize the same storage.

## Schemas

Use `#[blubber::schema(account, namespace = "your-program:state", semantic_epoch = 1)]`
for a persistent root. Use `#[blubber::schema]` for nested records and
`#[derive(Clone, Copy, blubber::Fixed)]` for native fixed structs. The fixed
derive encodes fields without Rust padding; it does not make native memory the
wire representation. A `Pubkey` is a fixed 32-byte value. Map keys use canonical
sorted encoding rather than insertion order.

Type and const generics are supported on nested schemas:

```rust
#[blubber::schema]
struct Record<T: blubber::Schema, const N: usize> {
    value: T,
    tag: [u8; N],
}
```

Generic schema markers do not implement `RootSchema` and cannot use `account`.
Instantiate them inside a concrete account root. Freeze the namespace and
semantic epoch before persisting accounts; [identity and migration](trust-model.md#account-identity)
explains changes in bytes and meaning.

## Host storage

The [README example](../README.md#initialize-edit-and-read) initializes a
`SliceStore`, edits typed fields, commits and reopens a checked reader.
`SliceStore` exposes the full caller-provided slice as capacity. `VecStore`
requires `alloc` and grows with fallible allocation. Neither host store supplies
runtime ownership or application authority checks.

Use the generated `StateInit` to initialize once. Use `State::load` for a checked
immutable reader and `State::load_mut` for a checked host editor. Every fallible
operation returns `blubber::Result`; propagate errors with `?`. `commit` consumes
the root and returns `CommitInfo`, including the used and physical lengths.

## Pinocchio accounts

Enable `pinocchio` alongside `derive`. Account views must come from the runtime
entrypoint. The program supplies the expected owner independently of account
contents, checks signer/seed/application authority, and selects a matching schema.
The helper authenticates storage ownership; it does not decide who may edit your
application state.

The following handler helpers compile against the runtime API. The caller has
already checked application authority. The checked edit scans the complete root
before its callback, requests sixteen additional physical bytes for two `u64`
items, commits once, and propagates both callback and settlement errors:

```rust
# #[cfg(feature = "pinocchio")]
# mod runtime_example {
use blubber::{List, Result};
use blubber::pinocchio::{AccountView, Address, EditOptions, Funding, Headroom};

#[blubber::schema(account, namespace = "example:runtime-queue", semantic_epoch = 1)]
struct Queue { values: List<u64> }

fn append_checked(account: &mut AccountView, owner: &Address) -> Result<()> {
    let (_value, _receipt) = Queue::with_edit_checked_options(
        account, owner,
        EditOptions {
            funding: Funding::Prepaid,
            headroom: Headroom::UpTo(16),
            refund: None,
        },
        |queue| {
            queue.values_mut().push(10)?;
            queue.values_mut().push(20)
        },
    )?;
    Ok(())
}

fn sum_lazy(account: &AccountView, owner: &Address) -> Result<u64> {
    Queue::with_read(account, owner, |queue| {
        queue.values().try_iter().try_fold(0u64, |sum, value| {
            value.map(|value| sum + value)
        })
    })
}
# }
```

`with_read` and `with_edit` perform authenticated lazy loads. Choose
`with_read_checked`, `with_edit_checked` or `with_edit_checked_options` when
writer history is unknown. Checked options preserve the same funding, headroom
and refund rules. A direct session uses `ReadStore::new` or `Store::new`, then
`load::<State>` or `load_checked::<State>`. Commit the mutable root with `?`
before any CPI. Shared and exclusive SDK guards prevent overlapping loans;
closure results cannot borrow a temporary session.

`State::create`, `create_pda`, `allocate_account` and `allocate_pda` measure
inputs before allocation CPI and return a commit receipt. The
[creation and edit tour](../programs/blubber-demo/src/exercise.rs) supplies a
larger executable example. Creation is bounded to the runtime's 10 KiB
invocation growth allowance; the account cap is 10 MiB.

## Reading lists, maps and strings

`State::load(bytes)?` checks the envelope and complete active root, then returns
a borrowed reader. Get fixed fields as copied Rust values and strings as `&str`.
Use `try_get` for a fallible point read and `try_iter` for a fallible loop.
An absent index/key is `Ok(None)`; detected invalid bytes return an error with
an account-relative offset. Infallible `get` and `iter` panic on detected errors.

```rust
use blubber::{List, SliceStore};
#[blubber::schema(account)]
struct Numbers { values: List<u64> }
let mut bytes = [0; 64];
let mut store = SliceStore::new(&mut bytes);
Numbers::initialize(&mut store, NumbersInit { values: &[4, 9] })?.commit()?;
let root = Numbers::load(&bytes)?;
assert_eq!(root.values().try_get(0)?, Some(4));
assert_eq!(root.values().try_get(2)?, None);
let sum = root.values().try_iter().try_fold(0, |sum, value| {
    value.map(|value| sum + value)
})?;
assert_eq!(sum, 13);
# Ok::<(), blubber::Error>(())
```

Authenticated runtime readers use lazy entry by default. They check touched
paths under valid current-schema writer history. `validate()` scans a selected
subtree; account-root `verify()` scans the complete active encoding. Neither
scans spare capacity. See the [trust model](trust-model.md).

### Ordinary and batched string traversal

Ordinary list/map `try_iter()` checks each visited child and stops after its
first error. `try_iter_batched()` scans the entire packed string payload for
UTF-8 during setup, even when the caller takes just one item. It still checks
individual item ends and preserves the ordinary accepted prefix and first
error offset. The aggregate scan can help a full traversal; partial reads pay
for bytes they do not use.

```rust
use blubber::{List, Map, Schema, Str, View};
let list_bytes = [2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b'];
let list = View::new(&list_bytes, *List::<Str>::LAYOUT, 12)?.read::<List<Str>>()?;
assert_eq!(list.try_iter().next().transpose()?, Some("a"));
assert_eq!(list.try_iter_batched().next().transpose()?, Some("a"));
// Map: count, two (key, cumulative end) entries, then two payload bytes.
let map_bytes = [2, 0, 0, 0, 1, 1, 0, 0, 0, 3, 2, 0, 0, 0, b'a', b'b'];
let map = View::new(&map_bytes, *Map::<u8, Str>::LAYOUT, 12)?
    .read::<Map<u8, Str>>()?;
assert_eq!(map.try_iter().next().transpose()?, Some((1, "a")));
assert_eq!(map.try_iter_batched().next().transpose()?, Some((1, "a")));
# Ok::<(), blubber::Error>(())
```

`ListRef<Str>::try_concat_str()` returns the complete packed payload as `&str`
without allocation, after schema/header and aggregate UTF-8 checks. It does
not certify item ends. For example, an end splitting `é` can pass aggregate
validation while an item read returns `InvalidUtf8`. Use iteration for item
positions or errors, and `validate()` for the complete list. Empty lists
return `""`; setup still performs header work. Aggregate and batched setup
cost O(payload bytes).

### Continue after a fixed-value error

`MapRef<K, V>::try_values()` decodes fixed values without decoding keys.
It consumes each physical slot before its codec runs. A value error consumes
one slot and the next call visits the next slot; exhaustion is fused. Ordinary
`try_iter()` decodes keys first and is permanently exhausted after an error.

```rust
use blubber::{ErrorKind, Map, Schema, View};
// Three (u8, bool) entries. The second bool is invalid.
let bytes = [3, 0, 0, 0, 1, 1, 2, 2, 3, 0];
let map = View::new(&bytes, *Map::<u8, bool>::LAYOUT, 20)?
    .read::<Map<u8, bool>>()?;
let mut accepted = Vec::new();
let mut errors = Vec::new();
for value in map.try_values() {
    match value {
        Ok(value) => accepted.push(value),
        Err(error) => errors.push(error),
    }
}
assert_eq!(accepted, [true, false]);
assert_eq!(errors.len(), 1);
assert!(matches!(errors[0], blubber::Error::Fault { kind: ErrorKind::InvalidBool, .. }));
let mut ordinary = map.try_iter();
assert_eq!(ordinary.next().transpose()?, Some((1, true)));
assert!(ordinary.next().is_some_and(|item| item.is_err()));
assert!(ordinary.next().is_none());
# Ok::<(), blubber::Error>(())
```

Values-only iteration allocates no memory. The `Vec`s above belong to the
caller and only record the result. Setup checks fixed widths and extent; a
setup error is yielded once. Value errors retain account-relative offsets.
Key domains, ordering and uniqueness remain unchecked. Call `validate()` to
certify them. Clones retain the same immutable bytes and position.

## Editing accounts

Initialize with the generated borrowed input, select matching fields or
collections, propagate each error, then commit the root once. The
[quickstart](../README.md#initialize-edit-and-read) shows the lifecycle; the
[runtime section](#pinocchio-accounts) adds authority, budgets and rent.
Use fallible reads when inspecting lazy state. End borrowed reads before edits
or resize.

### Scoped child edits

Use `list.with_mut(index, |child| { ... })` for a short point update, especially
an equal-size string replacement. It selects the child once and returns
`Result<Option<R>>`: an absent index skips the callback, and a present child
returns the callback's value. Children and borrowed reads stay inside the
callback. Completed edits remain if the callback later returns an error.

Use `list.reborrow().get_mut(index)` when you need to keep the child editor in
the surrounding scope. Both APIs support growing setters. `with_mut` refreshes
its integer span after a size change; `get_mut` retains its ordinary path-based
behaviour. End borrowed reads before another edit or resize in either API.

### Map selectors

Borrowing selectors keep a named map available after the child editor's last
use. Their child borrow excludes overlapping map or parent access:

```rust
# fn example(mut map: blubber::MapMut<'_, u16, blubber::Str>) -> blubber::Result<()> {
map.get_mut(&1)?.ok_or(blubber::ErrorKind::MissingKey)?.push_str("!")?;
map.entry(2)?.or_insert_with(|| "two")?.push_str("!")?;
assert_eq!(map.get_mut(&2)?.ok_or(blubber::ErrorKind::MissingKey)?.get(), "two!");
# Ok(()) }
```

Use consuming selectors when the child must outlive the map handle itself:

```rust
fn select(map: blubber::MapMut<'_, u16, blubber::Str>) -> blubber::Result<blubber::StrMut<'_>> {
    map.into_entry(2)?.or_insert("two")
}
```

A generated temporary works the same way: `root.labels_mut().into_get_mut(&key)?`
can return a retained child. `root.labels_mut().get_mut(&key)?` is useful for an
immediate call; bind the map first when retaining that borrowed child across
statements. List selectors and lending cursors retain their consuming contracts.
`at_mut(self, &key)` uses `into_get_mut` and converts absence to `MissingKey`.

Map entries support `or_insert_with` for a caller-borrowed initializer and
`or_default` for the initializer's default. Each runs only when the key is
absent. `and_modify` runs a fallible callback only when the key is present,
then returns the entry for chaining with either insertion method. It reselects
the value by its retained ordinal after growth. A callback error or unwind
keeps completed edits; propagate errors to the caller. The `Entry` rustdoc
examples show each combinator. On an occupied entry, chaining `and_modify`
with `or_insert_with` resolves the value again. A hot path that keeps the
modified editor can match `Entry::Occupied` and call `into_mut` once.

### Cursors and callbacks

For a pass over many mutable elements, `cursor_mut()` retains the selection
between steps. Each `next()?` lends one child; finish using it before asking
for the next. A map cursor lends `(key, child)` pairs. Selection errors are
returned once and exhaust the cursor; a later child operation error does not
itself advance or exhaust it. Propagate either error in an instruction handler.

```rust
use blubber::{List, SliceStore};
#[blubber::schema(account)]
struct Counters { values: List<u64> }
let mut bytes = [0; 64];
let mut store = SliceStore::new(&mut bytes);
let mut root = Counters::initialize(&mut store, CountersInit { values: &[1, 2] })?;
{
    let mut cursor = root.values_mut().cursor_mut()?;
    while let Some(mut value) = cursor.next()? {
        let old = value.try_get()?;
        value.set(old + 1)?;
    }
}
assert_eq!(root.values().try_get(1)?, Some(3));
root.commit()?;
# Ok::<(), blubber::Error>(())
```

`try_for_each_mut` scopes each child to a callback. It stops on the first
callback error and keeps completed edits. Growing every indexed child may
repeat suffix moves; use [whole-container rewrites](recipes.md#replacing-a-whole-collection)
when the new inputs can be staged independently. The [performance guide](performance.md)
compares these access patterns.

## Headroom and funding

`UpTo(0)` performs no speculative opening resize and can use existing physical
spare. `UpTo(n)` requests at most n additional bytes and can grant less. An edit
that exceeds the granted capacity returns before writing. Include directory and
parent growth, then subtract opening spare; the [indexed string recipe](recipes.md#indexed-string-growth)
shows the arithmetic. `Max` probes the available SDK window; `OnDemand` requires
`alloc` and a working runtime allocator. Local capacity never certifies remaining
transaction growth or an ancestor CPI limit.

`Funding::Prepaid` requires enough balance for final rent. `Funding::TopUp`
validates a writable System-owned signer payer and executable System Program,
then transfers at most the final net deficit. An optional refund recipient must
be independently authorized by your program. A receipt's `shrink_error` is a
diagnostic: settlement uses actual physical length, and funding failures return
an error. Do not discard the commit result.

## Errors and completion

Input, capacity and funding preflight errors preserve bytes and leave poison
unchanged. Source corruption detected on a lazy mutable path sets sticky poison,
even before writes. A later directory patch or encoder failure can leave changed
bytes with the old published length. Completed earlier edits also remain after
a callback error. Propagate errors for transaction rollback. Drop attempts
trimming without rent settlement; it does not undo edits.

Treat SBF stack diagnostics as build failures. Frames must fit 4,096 bytes;
split large handler frames with `#[inline(never)]`. Direct mapping removes frame
gaps on v0. The [stack-check script](../scripts/sbf-stack.sh) rejects diagnostics.
Measure nested schemas and your actual handler under your own CU and heap budget.
