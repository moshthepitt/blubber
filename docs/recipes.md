# Editing recipes

Say you append to a string list and run out of room. Return the error from the
instruction so the runtime rolls back. Until then, completed edits remain in
storage even if the callback returns an error.

## Checked editing with a budget

The [runtime quickstart](guide.md#pinocchio-accounts) uses
`with_edit_checked_options`: a checked load, an explicit additional
sixteen-byte budget, two fixed `u64` pushes and one propagated commit.
Use `with_edit_options` only while valid current-schema writer history holds.
Both helpers default to `UpTo(0)` when given default options.

## Indexed string growth

Appending `"new"` to a nonempty `List<Str>` needs seven encoded bytes: three
payload bytes and one four-byte stored list end. The first item of an empty
list needs only its payload, because its terminal end is implicit. Parent
ends may need updating, but their existing words do not grow. A newly present
optional parent or another newly created container needs its own bytes too.

With four opening spare bytes, the append needs three more physical bytes:
select `Headroom::UpTo(3)` on a runtime account. The host example below gives
the store seven existing spare bytes, which would fit runtime `UpTo(0)`.

```rust
use blubber::{List, SliceStore, Str};

#[blubber::schema(account)]
struct Messages { items: List<Str>, note: Str }

let init = MessagesInit { items: &["old"], note: "tail" };
let used = Messages::encoded_len_of(&init)? as usize;
let mut bytes = vec![0; used + 7];
let mut store = SliceStore::new(&mut bytes);
let mut messages = Messages::initialize(&mut store, init)?;
messages.items_mut().push("new")?;
assert_eq!(messages.note(), "tail"); // The parent's end and suffix moved.
let receipt = messages.commit()?;
assert_eq!(receipt.used_len as usize, used + 7);
# Ok::<(), blubber::Error>(())
```

See the compiled example on `blubber::pinocchio` for both additional reservation
and reuse of opening spare. Keep the parent directory, list count and all new
payload bytes in your budget when constructing a new value.

## A missing index and an error after an edit

`with_mut` returns `Ok(None)` when the index is absent and skips the callback.
It does not turn absence into an error. A present callback returns `Some(value)`.
If the callback changes a value and then fails, that completed change remains:

```rust
use blubber::{Error, ErrorKind, List, SliceStore};

#[blubber::schema(account)]
struct Numbers { values: List<u64> }

let mut bytes = [0; 64];
let mut store = SliceStore::new(&mut bytes);
let mut root = Numbers::initialize(&mut store, NumbersInit { values: &[1] })?;
let absent = root.values_mut().with_mut(9, |_| -> blubber::Result<()> {
    Err(ErrorKind::InvalidBoundary.into())
})?;
assert_eq!(absent, None);
let failed = root.values_mut().with_mut(0, |value| {
    value.set(7)?;
    Err::<(), Error>(ErrorKind::InvalidBoundary.into())
});
assert!(failed.is_err());
assert_eq!(root.values().try_get(0)?, Some(7));
// This host test inspects partial progress. A handler should return the error.
# Ok::<(), Error>(())
```

## Replacing a whole collection

Use `fill` or `fill_values` for the same variable-size replacement everywhere.
Use `rewrite_from` or `rewrite_values` for one independent input per existing
slot. Map values follow current sorted key order. `rewrite_values_from` also
checks supplied keys. Count/key mismatches fail before writes. Codec failures
during encoding poison the session.

```rust
use blubber::{List, Map, SliceStore, Str};
#[blubber::schema(account)]
struct Names { list: List<Str>, map: Map<u16, Str> }
let mut bytes = [0; 256];
let mut store = SliceStore::new(&mut bytes);
let mut root = Names::initialize(&mut store, NamesInit {
    list: &["a", "b"], map: &[(1, "a"), (3, "b")],
})?;
root.list_mut().fill("same")?;
root.map_mut().rewrite_values(&["first", "last"])?;
root.commit()?;
# Ok::<(), blubber::Error>(())
```

If replacements depend on old values, copy them into caller-owned storage,
end all reads, then rewrite. Inputs cannot borrow the destination. Whole-map
rewrite takes the complete map, so arbitrary chunks cannot be submitted.
Staging 1000 strings can exceed the default 32 KiB SBF heap; measure the
[whole recipe](../benchmarks/TRAVERSAL.md#ordinary-caller-costs).
Use `fill` for a constant replacement in a fixed-scalar list too. Use a
callback when the replacement varies by item.
[Measured fixed-list costs](performance.md).

## Map selectors

Borrow from a named map to select several keys sequentially. Use consuming
selectors to retain a child from a temporary generated map:

```rust
fn selected(map: blubber::MapMut<'_, u16, blubber::Str>)
    -> blubber::Result<blubber::StrMut<'_>>
{
    map.into_entry(2)?.or_insert("two")
}
```

`get_mut` returns `None` for a missing key; `at_mut` converts absence to
`MissingKey`. `and_modify` runs only for occupied entries; `or_insert_with`
invokes its factory only for vacant entries. A callback error keeps earlier
changes. Retain `Entry::Occupied` and call `into_mut` once if avoiding a second
selection matters. See the [editing guide](guide.md#map-selectors).
