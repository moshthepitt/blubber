# Format and API specification

Blubber stores a tree of exact byte regions in one allocation. Each child of a valid
root is contained in its parent. Relative payload boundaries let a whole subtree move
without changing its internal encoding. Scalars decode by value; strings borrow checked
UTF-8. Account bytes never become native Rust structs or collection headers.

This document specifies the current wire format, typed API contracts and adapter
boundaries. [ASSURANCE.md](ASSURANCE.md) states what has been proved, model-checked or
tested, with the bounds and trusted premises. The generated [public API
listing](verification/public-api.txt) records exact declarations; [API
coverage](verification/public-api.tsv) maps implementation areas to checks.

## Crates and ownership

`blubber-core` is an allocation-free `no_std` kernel. The same annotated executable
bodies compile through Verus erasure and ordinary Rust, including SBF. `blubber`
provides typed views, edits, storage and generated-code support; `blubber-derive` emits
local schema types and fixed codecs. The independent `blubber-model` is unpublished and
test-only.

The `derive` feature is enabled by default. `alloc` adds owned mirrors, Vec storage and
explicit on-demand growth. `account-runtime` supplies account runtime support;
`pinocchio` exposes its adapter. The unpublished `blubber-anchor` crate depends on
`blubber` through `account-runtime` and `__private` support. It does not add a framework
dependency to the published crates. Adapter-specific setup is documented in [its
README](crates/blubber-anchor/README.md).

Every local crate and generated implementation forbids unsafe code. Views use safe
immutable borrows; editors own an exclusive root capability. Holding a child prevents
overlapping parent reads or writes. Wire fixups never depend on projection destructors.
There is no hidden heap fallback.

## Compiled API contract

The demo schema, edit exercise and instruction dispatcher below compile as the
public API acceptance fixture. Host tests compare the schema and exercise with
the demo and lifecycle program copies, and compare all three files with these
examples. The comparison ignores documentation and indentation while preserving
literal contents and executable statements.

The dispatcher creates and initializes on opcode 0, reads under a shared guard
on opcode 1, and authorizes and commits the edit exercise on opcode 2.
Application authorization remains in the program. The complete instruction
includes account opening, validation and completion; separate operation and
runtime costs never replace that whole-instruction measurement.

`programs/blubber-demo/src/state.rs`:

```rust
use blubber::{List, Map, Pubkey, Str};

/// Native limits encoded without Rust padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
pub struct Limits {
    /// Whether edits are enabled.
    pub enabled: bool,
    /// Application item limit.
    pub limit: u16,
}

/// Named record with numeric tags.
#[blubber::schema]
pub struct Record {
    /// UTF-8 record name.
    pub name: Str,
    /// Numeric record tags.
    pub tags: List<u32>,
}

/// The complete format-1 tour account.
#[blubber::schema(account)]
pub struct MyAccount {
    /// Authority allowed to run the exercise.
    pub authority: Pubkey,
    /// Example counter.
    pub counter: u64,
    /// Variable-size records.
    pub records: List<Record>,
    /// Balances ordered by public key.
    pub balances: Map<Pubkey, u64>,
    /// Labels ordered by numeric key.
    pub labels: Map<u16, Str>,
    /// Uncounted UTF-8 note.
    pub note: Str,
    /// Counted binary attachment.
    pub attachment: List<u8>,
    /// Optional UTF-8 memo.
    pub memo: Option<Str>,
    /// Copied native limits.
    pub limits: Limits,
}

/// Borrows static inputs and copies the account's stored `authority`.
pub fn seed(authority: Pubkey) -> MyAccountInit<'static> {
    const RECORDS: &[RecordInit<'static>] = &[
        RecordInit {
            name: "one",
            tags: &[1],
        },
        RecordInit {
            name: "two",
            tags: &[2, 3],
        },
    ];
    MyAccountInit {
        authority,
        counter: 0,
        records: RECORDS,
        balances: &[],
        labels: &[(1, "first")],
        note: "ready",
        attachment: &[],
        memo: None,
        limits: Limits {
            enabled: true,
            limit: 100,
        },
    }
}
```

`programs/blubber-demo/src/exercise.rs`:

```rust
use crate::state::{Limits, MyAccountMut, RecordInit};
use blubber::{ErrorKind, Pubkey, Result};

/// Runs the complete read/edit tour after checking the stored authority.
///
/// Expects an account initialized from [`crate::state::seed`].
pub fn exercise(acct: &mut MyAccountMut<'_>, actor: Pubkey) -> Result<()> {
    if acct.authority() != actor {
        return Err(ErrorKind::Unauthorized.into());
    }

    let note: &str = acct.note();
    let first = acct.records().get(0).unwrap();
    let name: &str = first.name();
    assert_eq!(note, "ready");
    assert_eq!(name, "one");
    for record in acct.records() {
        for tag in record.tags() {
            assert!(tag > 0);
        }
    }
    for (key, amount) in acct.balances() {
        let _: (Pubkey, u64) = (key, amount);
    }
    // Last uses of the borrowed reads precede the first edit.

    acct.counter_mut().set(1)?;
    acct.limits_mut().set(Limits {
        enabled: true,
        limit: 200,
    })?;
    acct.records_mut().at_mut(1)?.name_mut().set("longer")?;
    acct.records_mut()
        .at_mut(1)?
        .tags_mut()
        .extend_from_slice(&[4, 5])?;
    acct.records_mut().extend_from_slice(&[RecordInit {
        name: "third",
        tags: &[5],
    }])?;
    acct.records_mut().insert(
        0,
        RecordInit {
            name: "front",
            tags: &[],
        },
    )?;
    acct.records_mut().replace(
        0,
        RecordInit {
            name: "replaced",
            tags: &[6],
        },
    )?;
    acct.records_mut().remove(0)?;
    let _: Option<()> = acct.records_mut().pop()?;

    let mut record = acct.records_mut().at_mut(1)?;
    record.name_mut().push_str(" still here")?;
    let mut tags = record.tags_mut();
    tags.insert_from_slice(0, &[99])?;
    tags.reborrow().at_mut(0)?.set(98)?;
    assert_eq!(tags.remove(0)?, 98);
    tags.replace(0, 7)?;
    tags.truncate(1)?;
    tags.clear()?;

    let old: Option<u64> = acct.balances_mut().insert(actor, 10)?;
    assert_eq!(old, None);
    assert_eq!(acct.balances().get(&actor), Some(10));
    acct.balances_mut()
        .into_entry(actor)?
        .or_insert(0)?
        .set(12)?;
    acct.balances_mut().at_mut(&actor)?.set(13)?;
    acct.balances_mut().try_for_each_mut(|_key, value| {
        let next = value
            .get()
            .checked_add(1)
            .ok_or(ErrorKind::ArithmeticOverflow)?;
        value.set(next)
    })?;
    assert_eq!(acct.balances_mut().remove(&actor)?, Some(14));
    acct.balances_mut().clear()?;

    assert_eq!(acct.labels_mut().insert(2, "two")?, None);
    assert_eq!(acct.labels_mut().insert(2, "second")?, Some(()));
    acct.labels_mut()
        .into_entry(3)?
        .or_insert("three")?
        .set("THREE")?;
    acct.labels_mut().at_mut(&2)?.push_str(" label")?;
    assert_eq!(acct.labels_mut().remove(&2)?, Some(()));
    acct.labels_mut().clear()?;

    let mut cursor = acct.records_mut().cursor_mut()?;
    while let Some(mut record) = cursor.next()? {
        record.name_mut().set("grown during iteration")?;
        record.tags_mut().push(8)?;
    }
    acct.records_mut()
        .try_for_each_mut(|record| record.name_mut().set("visited"))?;

    acct.note_mut().set("finished")?;
    acct.note_mut().truncate(3)?; // Byte count; must be a UTF-8 boundary.
    acct.note_mut().clear()?;
    acct.note_mut().push_str("done")?;
    acct.attachment_mut().extend_from_slice(&[0xaa])?;
    acct.attachment_mut().insert(0, 0xbb)?;
    acct.attachment_mut().replace(0, 0xcc)?;
    assert_eq!(acct.attachment_mut().remove(0)?, 0xcc);
    assert_eq!(acct.attachment_mut().pop()?, Some(0xaa));
    acct.attachment_mut().truncate(0)?;
    acct.attachment_mut().clear()?;
    acct.memo_mut().set(Some("remember"))?;
    acct.memo_mut().some_mut()?.set("remember this")?;
    acct.memo_mut().set(None)?;
    assert_eq!(acct.memo(), None);

    acct.reserve(64)?; // Request at least used_len + 64 retained physical bytes.
    let _trimmed: bool = acct.shrink_to_fit()?; // Pinocchio trims at commit.
    acct.note_mut().set("grew after physical shrink")?;
    acct.records_mut().truncate(1)?;
    acct.records_mut().clear()?;
    acct.validate()?;
    Ok(())
}
```

`programs/blubber-demo/src/lib.rs`:

```rust
//! Acceptance program for the Blubber account API.

#![cfg_attr(target_os = "solana", no_std)]
#![forbid(unsafe_code)]

/// The account API acceptance exercise.
pub mod exercise;
/// Shared tour schema and borrowed initialization input.
pub mod state;

use blubber::pinocchio::{EditOptions, Funding, Headroom};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
use state::{seed, MyAccount};

#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(process_instruction, 3);
#[cfg(target_os = "solana")]
pinocchio::no_allocator!();
#[cfg(target_os = "solana")]
pinocchio::nostd_panic_handler!();

/// Dispatches creation (0), read-only validation (1), or the edit tour (2).
///
/// Supply state, payer and System Program in that order. Creation requires
/// signatures from state and payer; editing requires the stored authority's
/// signature. The read-only instruction needs no signer or writable account.
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [opcode] = instruction_data else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state, payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if *opcode == 1 {
        return MyAccount::with_read_checked(state, program_id, |acct| {
            let _: &str = acct.note();
            let _: Option<u64> = acct.balances().get(&acct.authority());
            Ok(())
        })
        .map_err(Into::into);
    }
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let actor = (*payer.address()).into();
    match *opcode {
        0 => {
            let _receipt =
                MyAccount::create(state, payer, system_program, program_id, seed(actor))?;
        }
        2 => {
            let (_, _receipt) = MyAccount::with_edit_options(
                state,
                program_id,
                // The tour grows nested lists, maps and strings in one session.
                EditOptions {
                    funding: Funding::TopUp {
                        payer,
                        system_program,
                    },
                    headroom: Headroom::Max,
                    refund: None,
                },
                |acct| exercise::exercise(acct, actor),
            )?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}
```

## Wire formats

### Conventions and scalar encodings

Offsets and counts are unsigned 32-bit integers. Every word is little-endian except a
map key's order-preserving encoding. Every region is byte-aligned, with no padding.
Arithmetic uses checked `usize` and checked conversion to `u32`; production host and SBF
targets are 64-bit. The portable layout never uses native `usize`, Rust padding,
pointers, or host alignment.

`S(T)` is a fixed encoded width when `T::KIND == Kind::Fixed`; otherwise `T` consumes an
exact parent-delimited region. `P` is the start of a region's payload, `s` its payload
length, and all ends are relative to `P`. A parent supplies an exact end even for the
last child. Empty strings are zero bytes. Equal adjacent boundaries are valid.

| Type | Bytes and validity |
|---|---|
| `u8/u16/u32/u64/u128` | 1/2/4/8/16 bytes, unsigned LE |
| `i8/i16/i32/i64/i128` | 1/2/4/8/16 bytes, two's-complement LE |
| `bool` | One byte, exactly `00` or `01` |
| `Pubkey` | Exactly 32 raw bytes; SDK-independent newtype |
| `[u8; N]` | Exactly N raw bytes; N > 0 for fixed/container use |
| Derived `Fixed` struct | Concatenated fixed field encodings in declaration order; bool checks recurse |
| `Str` | Raw UTF-8 bytes, no length word or terminator; entire exact region is the string |
| `List<u8>` | Counted bytes, including the four-byte count word |
| `Option<T>` | Always variable-kind: `00` alone for None; `01` followed by exact T encoding for Some |

`Option<T>` reserves no inactive payload. None with trailing bytes and tags above 1 are
invalid. `Option<Str>::Some("")` is `01`, distinct from None. No floats, native
references, empty fixed structs, or zero-width fixed container elements are supported in
format 1. Arbitrary positive N is supported for arrays; large by-value arrays/fixed
structs remain subject to the SBF frame limit.

### Structs

Partition fields by their descriptor kind. Pack every fixed field first, in its original
declaration order. If there are `m` variable fields, append `max(m-1, 0)` ends, then
concatenate those variable fields in their original declaration order. No count is
stored for schema-known fields.

```text
fixed_prefix = concat(encoding of fixed fields)
P = region_start + len(fixed_prefix) + 4*max(m-1, 0)
start(j) = 0                         if j == 0
           end[j-1]                 otherwise
end(j)   = stored_end[j]            if j+1 < m
           region_end - P           otherwise
```

Stored ends must be nondecreasing and at most `region_end-P`. For `m=0`, the region must
equal the fixed prefix exactly. For `m=1`, there is no directory. Each fixed field and
each variable child must validate in its own exact region. Moving fields into the fixed
prefix is a wire rule, independent of declaration adjacency.

`Record` has no fixed prefix, one four-byte end for `name`, then name bytes and `tags`.
`Record { name: "A", tags: [7, 9] }` is:

```text
01 00 00 00 | 41 | 02 00 00 00 07 00 00 00 09 00 00 00
name end=1  | A  | tags count=2, elements 7 and 9 LE
```

### Lists

| Kind | Exact bytes | Exact region length |
|---|---|---|
| Fixed T | `[n:u32][T encoding] * n` | `4 + n*S(T)` |
| Variable T, n=0 | `00 00 00 00` | 4 |
| Variable T, n>0 | `[n:u32][end:u32] * (n-1)[payload]` | `4 + 4*(n-1) + s` |

For variable T, use the struct start/end equations with `m=n`. The first start and last
end are implicit. Counts 0, 1, and 2 therefore have 0, 0, and 1 stored ends. A count-
zero list with any payload is invalid. There is no local capacity or payload-length
header. Increasing a directory by four bytes moves its payload even on append; this is a
deliberate cost of compact indexing.

`List<Str>` containing `["ab", "", "xyz"]` is:

```text
03 00 00 00 | 02 00 00 00 02 00 00 00 | 61 62 78 79 7a
count=3     | ends 2,2                 | a  b  x  y  z
```

### Maps

Keys have a fixed positive width W and unique canonical encodings. The kernel compares
exactly W encoded bytes lexicographically using one comparator. The typed `MapKey`
relation must reflect the Rust key's total order.

| Key | Map encoding |
|---|---|
| Unsigned integer | Big-endian, fixed width |
| Signed integer | Reinterpret two's-complement bits as unsigned, XOR its most significant sign bit, then big-endian |
| `bool` | `00` or `01` |
| `Pubkey`, `[u8; N]` | Raw bytes, lexicographic order |

For example, i16 values -32768, -1, 0, 32767 encode as `00 00`, `7f ff`, `80 00`, `ff
ff`. Ordinary scalar u16 256 encodes `00 01`; map key u16 256 encodes `01 00`. Fixed
structs can be values; no MapKey derive is provided. The supplied MapKey
implementations form a finite built-in set with order-preservation harnesses.
Handwritten keys need an explicit codec contract and remain outside functional assurance.

| V kind | Exact bytes | P and size |
|---|---|---|
| Fixed | `[n:u32][key:W][value:S(V)] * n` | No separate payload; `4+n*(W+S(V))` |
| Variable | `[n:u32][key:W][end:u32] * n[payload]` | `P=start+4+n*(W+4)`; size `4+n*(W+4)+s` |

Variable maps retain the final end to keep all key/end records at uniform stride.
`start(0)=0`; `start(i)=end[i-1]`; `end(i)=end[i]`. Require all ends nondecreasing,
every end <= s, and final end == s. Empty maps have four bytes and no payload. Values
are packed in key order. Both layouts require strictly increasing keys; duplicates are
invalid. There are no reserved sentinel keys.

For `Map<u16, Str>` with `1 -> "x", 256 -> ""`:

```text
02 00 00 00 | 00 01 01 00 00 00 | 01 00 01 00 00 00 | 78
count=2     | key 1, end 1       | key 256, end 1     | x
```

Maps return old content through V::Removed: native fixed codecs return copied values,
while schema views and variable types return `Some(())` when an old entry existed. A
removed borrowed view cannot survive compaction. `get` and iteration return decoded
keys. `insert` replaces an existing value; it does not create a duplicate.

### Account envelope and capacity

| Account-relative offset | Bytes | Meaning |
|---:|---:|---|
| 0 | 8 | Discriminator |
| 8 | 4 | `used_len`, including these 12 envelope bytes |
| 12 | `used_len-12` | Exact root encoding |
| `used_len` | `storage_len-used_len` | Spare physical capacity |

Require `12 + root_min <= used_len <= storage_len <= u32::MAX`. The Pinocchio backend
further limits storage to 10,485,760 bytes. Spare bytes are excluded from validation and
root equality; their contents are unspecified on load. Initialization and growth
initialize all newly exposed storage to zero, and deletion clears the newly vacated used
suffix. No spare byte can become logical data without an explicit write.

The [layout-bound identity](#layout-bound-identity) is computed at compile time from
canonical wire descriptor, stable namespace, format version and manual semantic epoch.
`#[blubber::schema(account, discriminator = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
0x08])]` retains its explicit override, with a documented versioning obligation. Exactly
eight bytes are required; all-zero remains reserved for uninitialized storage and is
invalid as a schema identity. A rename preserves identity only when its namespace is
preserved. Discriminators authenticate neither authority nor data integrity.

`reserve(additional)` ensures storage length >= used_len+additional. Host growth is
exact, without doubling. Shrinking logical values leaves capacity available. On host
stores, `shrink_to_fit` attempts to trim storage to used_len after compaction; failure
retains valid bytes and capacity and returns `Ok(false)`. Fixed-slice storage can reduce
its exposed length and regrow within its original slice. Its original room stays
exclusively borrowed throughout.

Pinocchio Max/UpTo sessions pre-grow to discovered SDK-local capacity before borrowing.
OnDemand (alloc only) borrows the opening length and re-leases when growth is needed. In
every mode storage_len equals the current guard slice length. `reserve` ensures
used_len+additional fits, growing OnDemand when necessary, and requests retained reserve
at commit. `shrink_to_fit` clears that retention request and schedules a trim, returning
`Ok(true)`. Logical shrink keeps physical storage until completion. Commit reports
physical shrink failure in `CommitInfo::shrink_error`.

## Schemas and generated types

The schema vocabulary is `List<T>`, `Map<K,V>`, `Str`, `Option<T>`, fixed scalars,
native `Fixed` structs and derived schema structs. Layout metadata, rather than a
textual type-name list, selects fixed or indexed storage. Aliases, module-qualified
types and cross-crate generated schemas resolve normally.

`#[blubber::schema]` consumes a declaration and emits a marker, borrowed `Init`,
immutable `Ref` and mutable `Mut` types. The marker is not a native value struct. With
`alloc`, it also emits an owned mirror. Generated fields preserve declaration names,
visibility and docs; accessor overrides affect method names only. `Fixed` preserves a
native `Copy` struct and derives a packed codec whose width excludes Rust padding.

Declarations have 1 through 65,535 named fields. Generic schemas are supported as nested
fields, including nested generic lists, options and map values. Generic schemas have no
standalone integer table and do not implement `RootSchema`; concrete roots materialise
their tables. Tuple and recursive schemas, conditional fields, floats, zero-width fixed
fields/elements and user-defined map-key derives are outside the supported surface.
Whole-item configuration is supported. The nested generic encoding test in
[schema_encode.rs](crates/blubber/tests/schema_encode.rs) checks exact bytes, empty
children, depth and spare frames.

`Schema` supplies kind, optional fixed width, minimum size, wire descriptor, metadata,
`Read<'a>`, `Mut<'a>`, `Init<'a>` and `Removed`, plus measurement, encoding, reading and
schema-bound projection. `alloc` supplies owned measurement, encoding and decoding.
`RootSchema` supplies a concrete certified layout accessor. `AccountSchema` adds
identity, minimum envelope size, copied constraint prefix, `read_prefix` and checked
account-space calculation.

Built-in fixed scalars and native Fixed structs use native copied Read, Init and Removed
values. `Str` reads and initializes with `&str`, and removes as `()`. Attribute schemas
retain generated views and remove as `()`, including fixed-kind schemas. Use a native
Fixed struct for a copied removed record. `List<T>::Init` is a borrowed slice of child
initializers; `Map<K,V>::Init` is a borrowed sorted slice of key/initializer pairs;
Option uses the child's optional initializer. Lists, maps and options are always
variable-kind.

Measurement checks all counts, lengths, descriptor depth and map input order before
writing. Empty collections and absent options still check metadata depth.
Duplicate/descending input keys fail with no wire offset. Encoders receive an exact
destination after successful measurement, must write every byte, and must match the
schema. Built-in/generated encoders are deterministic and nonallocating on measured
inputs. The fallible slot adapter checks the returned byte count. Short/long counts,
errors or unwinding after destructive work leave poison set.

Handwritten Schema, Fixed and MapKey implementations remain outside functional
assurance. Their audit/replay/order markers default to false. Derives and containers
combine every relevant child and key marker. A true custom marker promises pure
deterministic measurement, exact schema-valid encoding, replay stability, and canonical
key ordering where applicable. Violating this promise can produce wrong data or errors;
it creates no permission for unchecked memory access. See [encoder
assurance](ASSURANCE.md#encoders-and-initialization).

### Metadata and depth

Metadata is a bottom-up integer table with eight-word node/field records: tags,
supported widths, field ordinals, packed offsets, child roots, table spans, key domains
and node depth. Children precede parents. The table contains no native pointers, enum
layouts or padding. A separate resolved wire descriptor carries identity semantics
without optimisation flags.

Generic const metadata uses header slices and a type-level child list. A concrete const
flattener emits the table without fixed-size scratch or a schema-size cap. The flattener
is tested, not proved; the same verified const checker used by runtime certification
establishes witness soundness. `CheckedLayout` privately binds the exact immutable
table, root and admitted depth. Arbitrary raw layouts require certification. Collection
views inherit the parent's witness and node.

Certification checks positive widths, kind agreement, key domains, record/table
containment, dense declaration ordinals, exact packed offsets/sums, children-before-
parent acyclicity and exact maximum depth. The root counts as one node. The default
budget is 8; `max-depth-16` raises it to 16 without changing bytes. Invalid metadata
returns `InvalidSchema` with no offset; an active traversal beyond a smaller requested
budget returns `DepthExceeded`. Inactive children remain subject to metadata
certification.

A certified flag identifies fixed types whose leaves admit every bit pattern. The
checker rejects that flag for variable types or restricted leaves. The zero flag retains
ordinary domain validation. Concrete schema digests and SBF relocation checks cover
correspondence between emitted tables and Rust types.

### Generated-code constraints

Expansion preserves raw identifiers, docs, visibility and source spans. It resolves a
renamed library crate and uses absolute core/library paths. `#[blubber(accessor =
"name")]` changes methods while retaining input fields. Repeated accessor attributes,
unsupported forms and collisions with lifecycle, projection or generated `try_` names
produce compiler diagnostics.

Mutable field getters borrow `&mut self`; read getters on mutable roots borrow `&self`.
Immutable nested getters return data with the original region lifetime. A retained
projection through a temporary nested mutable record requires a parent binding;
immediate chained edits remain valid. Generated helpers need no extension trait in
scope. Shared-reference IntoIterator and Debug implementations retain their standard
bounds and borrowing behaviour.

Account prefixes copy fixed fields only, preserving declaration names and visibility
even with renamed accessors. Nested fixed schemas use their fixed Init representation.
The native prefix cap is 1,024 bytes including padding; `constraint_fields = false`
selects `()`. `space(reserve)` adds the envelope, minimum root and reserve with checked
wire-size arithmetic; it fabricates no initializer and grants no runtime allocation
permission.

## Reads, selection and loads

A `View` contains an exact immutable byte slice, account-relative base and checked
layout. `View::new` certifies metadata and representable base/length; it does not
certify content. `load_trusted` checks a nonzero expected identity, representable
storage, complete envelope, matching identity, minimum root and used range. `load`
additionally verifies the entire active encoding. Codec and validator offsets are
rebased exactly once; the sentinel remains unchanged.

Ref and Mut read families have infallible convenience getters and matching `try_` forms.
A detected byte/state error panics through an infallible read; fallible reads return it.
Absence remains Option inside Result. Lists use constant-time selected geometry; map
lookup checks bounded key probes and requires globally canonical order for meaningful
absence. `contains_key` does not decode a value. Full validation certifies all
boundaries, leaf domains, UTF-8, map order and uniqueness.

Ordinary fallible iteration checks selected children as it advances. Its first error is
yielded once, then the iterator stays exhausted. Infallible iteration panics at that
error and remains exhausted if the panic is caught. Iterators are fused and cloneable
without Clone/Copy bounds on schema markers or reads. A nonempty fallible iterator has
lower size hint one and upper hint the remaining count. Clones preserve remaining state.
Ref iteration by value preserves the original byte lifetime; shared Mut iteration uses
the handle's shared borrow. A live borrowed item prevents conflicting edits.

Batched string iteration eagerly checks its bounded batch before yielding its elements;
it can therefore reject later bytes earlier than ordinary iteration. Aggregated
byte/character counts retain checked arithmetic and UTF-8 semantics. Fixed-map values-
only iteration omits key decoding, yields one Result per value slot and continues after
value errors; setup geometry errors appear once. It makes no claim about key validity.
Consuming fixed folds preserve ordinary iteration results and error rebasing.

### Loads and writer history

| Operation | Contract |
| --- | --- |
| Raw `T::load`, `View::load`, host `T::load_mut` | Complete active-root validation before return |
| Authenticated runtime Store/ReadStore and generated `with_read`/`with_edit` | Expected owner, non-executable state, write permission for edits, identity/envelope and genuine exclusion. Contents load lazily |
| Native `Blubber<State>` | Lazy policy plus fixed-prefix domain checks before publishing constraint fields |
| `load_checked_mut`, checked Stores/scoped helpers, `Blubber<State, Checked>` | Same runtime authentication plus full root validation |
| Explicit raw trusted loads | Envelope checks with a caller-owned valid-schema and writer-history premise |
| Root `verify()` | Full root scan; leaves poison unchanged |
| Selected `validate()` | Full selected-subtree scan |

`verify-on-load` is opt-in and adds full scans to lazy loads, including explicit trusted loads, and to
initialization. It is independent of debug assertions. Explicit checked loads
always scan. With the feature off, lazy load cost is constant in account content.

Authenticated history requires initialization or checked migration for this exact
schema, followed only by approved typed writers. Under that premise and matching
encoders, successful supported edits preserve the whole root. Owner and identity checks
do not prove unvisited bytes valid or authorize application actions. An owner supplied
by unvalidated caller data supplies no provenance. A successful lazy edit may leave
unrelated corrupted bytes intact. Unsorted maps can miss existing keys; corrupted
directories can alias sibling content. Every admitted access remains physically
contained and terminating within its byte/depth geometry, without an instruction-wide CU
promise.

Backend writer tokens, schema-bound Projection and owner-bound ProjectField capabilities
seal the typed writer set. Width equality alone cannot authorize a foreign schema or
bool/u8 reinterpretation. `__private` helpers retain the same authority requirement.
Deliberate low-level bytes, plans, destinations and backend mutation live in explicit
`raw` modules outside typed preservation. External SDK handles, custom codecs and
program upgrades remain application writer-history responsibilities. Raw writes require
a checked load. A history token never converts into a complete-validation
certificate.

### Layout-bound identity

An account identity binds stable namespace, format version, canonical wire schema and a
manual semantic epoch through compile-time SHA-256. The wire fingerprint has explicit
codec/node/key tags, little-endian widths/counts, length-delimited children, signedness,
declaration-to-wire mapping and packed offsets. Aliases with the same wire contract
share descriptor bytes. Internal optimisation flags and native layouts are excluded.

An explicit eight-byte discriminator override is supported with an application
versioning obligation; all-zero is reserved for uninitialized storage. A rename
preserves identity only with a preserved namespace. Equal identities supply neither
application authorization nor semantic field correspondence. Migration checks the old
schema, converts values, fully verifies the new root, and publishes the new identity
last. The canonical grammar follows. Independent vectors are in
[identity-vectors.json](verification/identity-vectors.json).

The preimage is exactly:

```text
"blubber\0schema\0" || format:u32le || namespace_len:u32le || namespace_utf8 ||
semantic_epoch:u32le || descriptor_len:u32le || descriptor
```

Format is 1. Lengths count bytes, including UTF-8 and embedded nulls. The identity
is the first eight bytes of SHA-256, in digest order. Descriptor serialization
and hashing run in constant evaluation. The SDK load compares eight bytes and
performs no hashing. An all-zero identity is reserved for uninitialized storage;
the generator rejects it, and account gateways reject a zero expected identity
with `InvalidSchema`.

### Canonical descriptor version 1

The descriptor starts with `1:u32le`, followed by one node. Every `u32` below is
little endian. Each child is `node_len:u32 || node`; its length excludes the
descriptor version. Concatenation follows the listed order. Tags and rules are
explicit bytes, independent of Rust enum discriminants.

| Node | Encoding after the tag byte |
| --- | --- |
| 1, unsigned LE | width:u32, one of 1/2/4/8/16 |
| 2, signed two's-complement LE | width:u32, one of 1/2/4/8/16 |
| 3, bool | no bytes; domain is 0 or 1 |
| 4, raw | positive width:u32 |
| 5, UTF-8 | no bytes; exact parent-delimited UTF-8 |
| 6, Option | rule:byte = 1, child |
| 7, List | rule:byte = 1, child |
| 8, Map | rule:byte = 1, key_len:u32, key, child value |
| 9, Struct | fixed_bytes:u32, fixed_count:u32, variable_count:u32, field_count:u32, fields |
| 10, Custom | codec namespace_len:u32, namespace_utf8, codec_epoch:u32, child wire |

Option rule 1 means tag 0 alone or tag 1 followed by the child. List rule 1
means a u32 count, then fixed-stride children or a u32 cumulative-end directory
with an implicit terminal end. Map rule 1 means a u32 count and strictly
increasing canonical keys, then fixed-stride entries or a key/end table with a
stored terminal cumulative value end. These are the format-1 rules;
the descriptor version governs their interpretation.

A map key is `domain:byte || width:u32 || custom:byte`. Domains are 1 (unsigned
big endian), 2 (signed big endian with the sign bit flipped), 3 (bool, width 1,
domain 0/1), and 4 (raw lexicographic bytes). Integer widths are 1/2/4/8/16.
If `custom` is 1, append the length-prefixed codec namespace and u32 epoch; 0
selects the built-in contract. A raw key's width must be positive.

Struct fields occur in declaration order. Each is
`declaration:u32 || placement:byte || wire_ordinal:u32 || packed_offset:u32 || child`.
Placement 0 selects the fixed prefix; 1 selects the variable payload sequence.
Ordinals count independently within each placement, starting at zero. Fixed
offsets are packed sums of preceding fixed widths; variable offsets are zero.
Structs have 1 through 65,535 fields. A struct is fixed when every child is
fixed. Option, List, Map and UTF-8 are variable. All byte lengths and packed
widths must fit u32.

No Rust type names, table indices, addresses, witness flags or optimisation
fields occur in this encoding. Pubkey, Address and `[u8; 32]` share the raw
contract. `List<u8>` uses the counted-list contract. Fixed derive and a
schema marker with the same packed fields share the struct contract.


## Typed edits and atomicity

One terminal operation resolves and consumes one plan. Paths contain declaration
ordinals, item indices and Some selectors in bounded inline storage. A selected map
value retains an ordinal under the exclusive root borrow. Retained paths resolve current
storage after edits; they retain no allocation address across growth. Parent length and
keys cannot change through a child capability.

Before destructive work, reject closed/poisoned state, authenticate the envelope,
resolve touched geometry, measure input, check full-width indices, counts, ancestor
arithmetic, capacity and funding, and decode/copy requested removed values. Fresh
Prepared evidence remains affine to the exact borrowed bytes. Growth releases the byte
borrow; detached readiness rechecks geometry after reacquisition. Reallocation may move
storage but must preserve the old prefix.

A lazy mutable path sets sticky poison before returning every detected source-corruption
error, including read-only admission before any write. Source failures poison even
before writes. Caller-input, capacity and funding errors stay unpoisoned and occur
before the first write. A checked load keeps its pre-write error frame. Local
end/ancestor checks performed during a lazy patch may detect corrupted bytes after
movement and leave partial data. The error propagates; on-chain transaction rollback
restores account state.

Poison is set before the first destructive move or encoder call. Successful publication
writes local count/directory and ancestor ends, used_len last, clears the newly vacated
used suffix, and clears poison only after restoring the representation. Encoder errors,
mismatched lengths and host unwind retain poison. Subsequent typed access rejects it.
Root `verify()` cannot clear poison; only successful explicit checked recovery can.
Dropping/reopening loses a volatile poison flag and is not recovery.

Caller/preflight errors preserve supplied buffers. A backend may have grown and zero-
extended physical storage before a detached readiness rejection; the old used encoding
remains byte-identical. Checked core phase errors have full supplied-buffer frames.
Physical trim follows valid logical publication; trim failure retains capacity and is
reported separately. Callback sequences retain earlier completed edits when a later
callback fails; they are not multi-operation transactions.

### Collections and entry

List insertion accepts `index <= len`; replace/remove/selection require `index < len`.
The full usize index is checked before wire conversion. Pop on empty returns None;
truncate beyond len is a checked no-op, including usize::MAX. Bulk slice insertion
preserves input order and is one operation; empty runs still check source geometry and
index. Fixed replacements write only the selected element. Removal validates/copies
`Removed` before movement; truncate/clear may discard leaf domains while retaining
required structure and key checks. No operation implicitly trims physical storage.

Scalar set and whole-string replacement may repair an invalid old leaf when its geometry
is usable. String append/truncate validate retained UTF-8; shorter truncation must fall
on a character boundary, otherwise `Utf8Boundary` has no offset. An equal/larger
truncation is a no-op. Option set replaces tag and active payload together; selecting
None reports `MissingValue`.

Map insert upserts and returns Option<Removed>; remove returns absence without decoding
unrelated values. Found upsert/remove validates the old value before movement. Use a
selected value setter to repair an invalid old leaf. Keys are copied and cannot be
mutated through an editor. Absent insertions retain bounded search, query-domain and
predecessor/query/successor checks.

Map selectors distinguish reusable parent borrowing from consuming child ownership:

```rust
pub fn get_mut(&mut self, key: &K) -> Result<Option<V::Mut<'_>>>;
pub fn entry(&mut self, key: K) -> Result<Entry<'_, K, V>>;
pub fn into_get_mut(self, key: &K) -> Result<Option<V::Mut<'a>>>;
pub fn into_entry(self, key: K) -> Result<Entry<'a, K, V>>;
pub fn at_mut(self, key: &K) -> Result<V::Mut<'a>>;
```

List `get_mut`/`at_mut`, Option `get_mut`/`some_mut`, and collection cursors consume
their handle. `reborrow()` creates a short loan. In-place operations borrow `&mut self`.
Each child blocks another parent use until its last use.

Entry keeps a copied key, ordinal and checked immediate geometry. Occupied `or_insert`
ignores its unused input. `or_insert_with` invokes its FnOnce factory exactly once on
vacancy; `or_default` has the same vacancy rule with `V::Init: Default`. `and_modify`
invokes a fallible higher-ranked FnOnce callback exactly once on occupancy and returns
the Entry for chaining. After a growing callback, it resolves the retained selection
again. Returned callback errors preserve completed edits. Occupied `into_mut`,
insert/remove, and vacant insert consume the entry; no competing map edit runs while it
lives.

### Walks, fill and rewrite

Cursors lend one child tied to `&mut cursor`, capture the outer count and re-resolve
after resizing child work. They exhaust after their first selection error. Fixed walks
stream disjoint contained slots. Checked fixed-map walks validate source keys; lazy
fixed-map walks may admit count/stride geometry and frame unchanged keys. Returning a
key still checks that visited key's domain. Scoped traversal uses a higher-ranked
callback, stops at the first error and retains completed edits. `ListMut::with_mut`
returns `Result<Option<R>>` from a higher-ranked FnOnce callback, so an absent index
invokes no callback.

The fill surface is `ListMut::fill` and `MapMut::fill_values`, both borrowing `&mut
self`. `rewrite_from`, `rewrite_values` and `rewrite_values_from` retain their distinct
exact-count/input contracts. Fill replaces each value with one initializer; rewrite APIs
can vary values or derive them from old reads. Map value operations preserve every key.
Lazy fixed-map fill can omit a source-key scan when independent stride geometry contains
every loan; checked routes retain validation. Empty input, count mismatch,
measurement/capacity errors and codec failures retain their distinct frames and poison
rules. Every required measurement/encode call and exact slot-length check remains.

Variable fill advances payload and directory cursors together. A later encoder failure
therefore preserves the same partial-progress ordering as individual slot work.
Singleton fills use the exact single-slot route. Whole container rewrites publish once
and retain exact slot, key, count, ancestor, used-length and spare-byte frames;
arbitrary callback logic is outside proofs.

### Owned values and encoded extraction

With alloc, owned strings are String, lists are Vec, maps are sorted Vec<(K,V::Owned)>
and options are Option. Sorted Vec permits fallible reservation; no hidden sorting or
infallible tree allocation is used. Owned mirrors retain no account borrow. Explicit
owned insertion/extraction and `to_owned` use checked measurement/decoding and fallible
reservation before movement. Owned encoding borrows the mirror directly without an
intermediate Init array.

`remove_into` is available without alloc. It validates the exact old subtree and
destination room before copying, then removes it. Insufficient room leaves both buffers
unchanged. Success changes only the encoded destination prefix; map absence changes
neither buffer. An empty encoded value returns zero while still removing the entry. The
exclusive root borrow excludes internal source aliases. Wide Removed results use
separate staging frames when their native return slot would exceed the SBF frame limit;
this changes no API result.

Certified flat layouts use a recursive validation walk bounded by the explicit MAX_DEPTH
node budget. Simple containers and leaves finish directly; each remaining recursive call
consumes one depth unit. Arbitrary layouts retain the iterative MAX_DEPTH frame array,
with descriptor, region, next child and traversal state in each frame. Fixed-width
numeric arrays validate length without reading unconstrained element bytes; bool and
UTF-8 payloads are checked. Counts with zero-byte variable elements remain bounded by
the directory byte extent, with the one-element elision handled separately. Descriptors
are finite, acyclic, and validated at schema construction. The depth-8/16,
narrow/medium/wide SBF stack matrix remains mandatory for both allocation policies.

## Physical splice and publication

The equations below describe valid-root movement and strict raw planning. Lazy mutable
routes can check source words as they consume them under sticky poison, as specified
above; they do not gain a complete partition certificate.

Suppose a selected exact region is `[a,b)`, old used end U, replacement length L, and
`d=L-(b-a)`.

```text
new_used = U + d
for each ancestor from leaf parent to root:
    for each stored end whose child ordinal >= the selected child:
        planned_end = old_end + d
    check every planned end's representability/extent and the changed run's joins
# Last implicit ends are derived from the parent's changed extent.
if d > 0: grow storage before taking the destination slice
memmove(source=[b,U), destination=b+d)  # One root suffix move.
encode replacement into [a,a+L)
patch planned ancestor runs; write new_used; clear [new_used,U) if d < 0
# Only now attempt optional physical truncation.
```

All ancestor directories precede the selected child's payload, so their word locations
stay fixed for this edit. Every ancestor receives the same total d, which includes any
changed table bytes inside the replaced region. Descendants of moved untouched subtrees
require no repair. Replacing equal-length content does no suffix move and writes no
unchanged boundary/count words.

`records[1].name = "longer"` resolves root field 2, list index 1, and Record field 0.
Growing the name by d moves `[old_name_end,U)` once. It increments Record's stored name
end, the records list's stored ends from item 1 onward except its implicit terminal end,
and the root's stored ends from records onward. Fixed authority/counter/limits bytes are
unchanged. Internal offsets inside later records, balances, labels, and List<u8> remain
byte-identical after relocation.

Str set is one replacement, never clear followed by push. Str append inserts at its
current end, preserving the prefix through coordinate-based movement; it does not retain
a source slice across the move. Option set replaces its complete `[tag][payload]`
region. Editing Some's child adds a Some path step; selecting None returns
`MissingValue` before writing.

### Fixed list and fixed map edits

Derive emits each fixed field's packed offset and width. Pure fixed-struct chains sum
constant offsets. A root scalar set checks state, discriminator, `12 <= used <=
capacity`, sufficient fixed-prefix extent and destination bounds, then performs the
bounded canonical write. A fixed-only root also requires its exact root size. Zero-delta
fixed-prefix writes need no unrelated variable directories, ancestor patches or general
plan. Dynamic collection indices and Option tags still use the local resolver; fixed
field offsets are constant relative to the selected record. Preserve setter names,
receivers, chaining and errors. Prove equality with generic edits on well-formed input,
exact full-buffer footprint and no-write-on-error on arbitrary bytes. New bool/fixed
values remain canonical; overwritten old leaf domains need no parse.

A fixed list has element width q and element i starts at `b+4+i*q`. Inserting at i
shifts `[element_start,U)` right by q, writes q bytes, updates count, ancestor runs, and
used_len. Removing shifts `[element_end,U)` left by q after copying its fixed return
value, then updates metadata and clears the vacated root tail. Map entries use
`q=W+S(V)` and the same whole-entry movement. Map value replacement has zero size delta
and leaves keys untouched.

Fixed list `replace` validates the complete replacement and writes only that element.
Truncate removes the suffix of elements as one contiguous deletion. Clear writes an
empty count and removes the entire old data area in one operation. Repeated remove loops
are not the implementation of truncate/clear.

### Indexed insertion, including a nested insert

Use b for the selected container start, P for its old payload start, E for its old
container end, and U for the root used end. Insertion ordinal i has old payload offset
p, with p=s for append. New value length is L. The directory grows by t: four bytes for
a nonempty variable list, zero for an empty variable list, and W+4 for a variable map.
Total root delta is `d=t+L`.

Partition the old bytes:

```text
[header][directory D][payload prefix A][payload suffix B][outer suffix O]
                     P              P+p              E              U
```

The required order is:

```text
preflight D, p, L, t, d, ancestor patches and capacity
resize physical storage if U+d exceeds it
1. memmove([P+p,U), P+p+d)   # B and O move together once, right to left semantically.
2. memmove([P,P+p), P+t)     # A moves right only by the directory growth.
3. shift the necessary D suffix right by one directory slot
4. write the new value at P+t+p, and write/adjust the local directory
5. count++, patch ancestors by d, used_len=U+d
```

Move 1 protects all old bytes needed by B/O before move 2 can overlap their old
locations. Move 2 protects A before the enlarged directory overwrites its old start. The
final directory move has sources below old P; payload moves have not overwritten those
sources. Each memmove uses old-source overlap semantics. No ancestor repeats the root
suffix move. Appending still moves A by t, including the entire existing payload when
the directory grows.

For a variable list, let `e[j]` denote the old cumulative end, including the synthesised
terminal `e[n-1]=s`. The new stored ends, for `0 <= j < n`, are:

```text
j < i:  e[j]
j == i: p+L
j > i:  e[j-1]+L
```

For append i=n, all j satisfy j<i; the final new stored end is the old implicit terminal
s. For n=0 there is no directory word to write. For interior insertion, shift old stored
words `[i,n-1)` to `[i+1,n)` before overwriting end i, then add L to the moved words.
Earlier words remain unchanged. The count 1 to 2 transition synthesises its first stored
end correctly for both prepend and append.

For a variable map, shift key/end records `[i,n)` right one record. New record is
`(encoded_key,p+L)`. Add L to ends in moved records. Earlier records retain their ends.
The full terminal end remains stored, including when the inserted value is empty.
Directory keys and ends may be synthesised during movement after protecting their old
sources. The encoder receives a disjoint value-only destination; it cannot overwrite the
directory. Count, ancestor ends and used length remain unpublished until finish, and the
root stays poisoned throughout the intermediate state.

A nested insertion is this same operation with ancestor patch runs added during
preflight. It never invokes a flat resizer once per ancestor.

### Indexed removal and shrink ordering

Let the removed payload be `[P+p,P+p+L)`. Directory shrink amount t is four bytes for a
variable list with n>=2, zero for n=1, and W+4 for a variable map. Total removed bytes
are `d=t+L`.

```text
preflight all ranges, local ends, ancestor reductions, and any return copy
1. compact and adjust the directory first, while its old trailing words still exist
2. memmove([P,P+p), P-t)            # Payload prefix A moves left by t.
3. memmove([P+p+L,U), P+p-t)        # B and O move left by t+L once.
4. count--, patch ancestors by -d, used_len=U-d
5. zero [U-d,U); release byte slice; optionally resize storage to U-d
```

Compacting D must precede moving A left, since A's destination can overwrite the old
trailing directory. Moving A first among payload regions cannot overwrite any byte of
the later source `[P+p+L,U)`. Moving the later suffix first could overwrite A's source,
so that reverse ordering is forbidden.

Variable-list removal of a nonfinal item deletes stored end i and shifts later stored
ends left; their values decrease by L. Removing the final item drops the old final
stored end, which becomes the new implicit terminal; no end word is synthesised.
Transitions 2 to 1 and 1 to 0 remove four and zero table bytes respectively. For a
variable map remove the complete key/end record and decrease every following end by L.
Empty values with L=0 still change count/table size.

Variable-list truncate to k<n applies the same two-region shrink with
`t=4*(max(n-1,0)-max(k-1,0))` and removed payload suffix length `s-end(k-1)` (zero
retained payload at k=0). Drop trailing directory words before moving retained payload
left by t, then move O left by total removed size. The retained terminal end becomes
implicit. Clear uses k=0. Variable-map clear replaces the entire region by a zero count.
These operations remain one plan each.

Physical shrink never precedes compaction. A failure leaves the new logical encoding
valid inside larger storage. A later grow must check against the original invocation
window, and the backend must zero re-exposed storage even when it was truncated earlier
in the same invocation.

## Strict raw gateways

The explicit raw interface admits arbitrary certified descriptors and checked plans. Its
complete partition scans and error order remain distinct from lazy typed geometry
checks. No raw destination loan grants typed validity.

`checked_add`/`checked_mul` cover the full usize domain; wire conversion rejects values
above u32::MAX. `checked_delta` covers every i64 delta and accepts only u32 results.
These arithmetic faults have NO_OFFSET. `checked_span` checks wire-end overflow before
parent containment and identifies the start; an empty span at the parent end is valid.
Public Span fields establish no invariant by themselves.

Word gateways check representable exclusive end before slice bounds. Overflow is
ArithmeticOverflow; short input is Truncated, both at the word start. A failed word
write preserves the complete slice; success changes exactly four bytes. Lazy typed
header count arithmetic identifies the count word, while strict detached partition
extent arithmetic retains NO_OFFSET.

`move_bytes` checks source ordering first (InvalidBoundary at `src.start`), then source
containment in the slice (Truncated at `src.start`), destination-end representability in
`u32` (ArithmeticOverflow at `dst`), and destination containment (Truncated at `dst`).
An empty move still checks both positions; positions at the slice end are valid. On
success, `[dst, dst + src.end - src.start)` receives the original source bytes,
including when the ranges overlap. Slice length and every byte outside that destination
remain unchanged; the source is not cleared. Every failure preserves the entire supplied
slice. The slice length itself need not fit `u32`.

After those verified checks, production movement uses proved in-tree copy with dynamic
partial chunks of at most 1,024 bytes. Capture the complete chunk before any destination
write, and traverse chunks in the direction that preserves unread source bytes.
Original-source, full-buffer frame and no-unwind proofs cover both overlap directions.
Bounds arithmetic and error precedence remain in the verified root.

`fixed_region` accepts a checked fixed-kind descriptor iff the supplied length equals
its width, returning `[0,width)`. It checks metadata first, then returns Truncated at
zero for a short slice or InvalidBoundary at the fixed width for trailing bytes.
Variable-kind descriptors return InvalidSchema with NO_OFFSET.

`validate_fixed` checks metadata and exact width as `fixed_region` does, then checks
leaf domains recursively through fixed-kind structs in packed wire order.
Unsigned/signed integers and raw bytes admit every bit pattern at their declared width.
Bool admits exactly 0 and 1; the first invalid bool returns InvalidBool at its position
relative to the supplied slice, including nested field offsets. Variable-kind
descriptors return InvalidSchema with NO_OFFSET. The checked descriptor's node budget
bounds recursion by MAX_DEPTH; no heap is used.

`partition_option` checks metadata first and rejects a non-Option descriptor with
InvalidSchema and NO_OFFSET. A slice length above `u32::MAX` returns ArithmeticOverflow
with NO_OFFSET; an empty slice returns Truncated at zero. Tags above 1 return InvalidTag
at zero. None requires exactly `[0]`; trailing bytes return InvalidBoundary at one. Some
returns `[1,bytes.len())`, consuming the entire parent-delimited suffix. For a fixed
child, check `1 + width` in `u32` before payload bounds: overflow returns
ArithmeticOverflow with NO_OFFSET, a short payload returns Truncated at one, and
trailing bytes return InvalidBoundary at `1 + width`. Child bit domains and variable-
child structure are checked separately; an empty variable Some payload is a valid
partition, including `Some("")`.

`validate_utf8` checks the complete parent-delimited string region without a descriptor
or envelope. It rejects slice lengths above `u32::MAX` with ArithmeticOverflow and
NO_OFFSET before scanning. Otherwise it accepts exactly canonical UTF-8: one to four
bytes per Unicode scalar, shortest encodings only, no surrogates, and no values above
U+10FFFF. Empty strings, NUL, BOM and noncharacters are valid. An invalid or incomplete
sequence returns InvalidUtf8 at its leading byte, after the maximal valid prefix,
matching `core::str::from_utf8`'s `valid_up_to()`. No str reference is formed. The
scalar byte checker uses constant space and a strictly decreasing remaining-byte
measure;

`map_key` checks the entire map partition before selecting an ordinal, returning
IndexOutOfBounds with NO_OFFSET for an invalid index. Its nonempty span covers exactly W
encoded bytes in an interleaved entry or key/end record, excluding the value/end word.
Key canonicality, order and child validity are separate. Internal scans reuse one
checked partition instead of rescanning directories for every key.

`validate_map_keys` checks the partition, then scans keys in physical order. Integer and
raw key domains admit every fixed-width pattern. An invalid bool key returns InvalidBool
at its key start before comparing it with its predecessor. Equal adjacent keys return
DuplicateKey; descending keys return UnsortedKeys, both at the later key's start. It
stops at the first key fault and does not validate value bytes. Acceptance is equivalent
to a valid partition with canonical, strictly increasing keys.

`search_map` checks the partition first, then the query's exact W-byte width
(InvalidBoundary with NO_OFFSET), then its bool domain if applicable (InvalidBool with
NO_OFFSET). It does not scan stored key domains or order. Each binary-search probe reads
only a contained key span; at most `1 + floor(log2(count))` probes occur for a nonempty
map and none for an empty map, even with malformed key order. A found ordinal always
identifies equal encoded bytes. Strictly ordered inputs additionally guarantee exact
presence and the unique absent insertion split. The strict raw search checks its
complete partition before binary search. These read-only gateways preserve all supplied
bytes.

`validate(bytes, layout, max_depth)` accepts an exact subtree without an envelope iff
the complete finite schema relation holds. Check descriptor metadata at MAX_DEPTH first,
then require a requested budget in `1..=MAX_DEPTH` (otherwise DepthExceeded with
NO_OFFSET), then check the slice's u32 wire length. The root consumes one node budget
unit, matching `check_layout`; an active child beyond the requested budget returns
DepthExceeded with NO_OFFSET. None and empty collections consume no child unit. A
descriptor invalid at MAX_DEPTH remains InvalidSchema, even if an offending branch is
inactive. Each node checks its entire partition before visiting children. Struct
traversal follows packed wire order (fixed fields, then variable fields); maps check all
key domains/order before values. Relative child faults are rebased once from their
absolute frame start; sentinel faults remain unchanged. Certified flat layouts use recursion bounded by the node budget. Arbitrary layouts
use a fixed MAX_DEPTH array of offset-only frames retaining checked partitions and
next-child ordinals. Neither path allocates, forms string references or writes bytes;
child selection reuses admitted partition geometry. Structural and
semantic traversal share the same verified body; structural mode omits bool, UTF-8 and
key-domain/order checks, while the public gateway always uses semantic mode.

`partition_struct` checks metadata at MAX_DEPTH and requires a Struct. Its spans
identify the exact fixed prefix, directory, and variable payload relative to the
supplied slice. It rejects slice/header extents above `u32::MAX` with ArithmeticOverflow
and NO_OFFSET before checking truncation. A short fixed prefix returns Truncated at
zero; a short directory returns Truncated at its first incomplete word. Trailing bytes
with no variable fields return InvalidBoundary at `fixed_bytes`. Stored ends are checked
in directory order; the first descending or out-of-payload end returns InvalidBoundary
at its word. Equal ends and empty fields are valid. `struct_variable_field` checks that
entire partition, then selects a variable field by payload ordinal, returning its exact
contained span. An invalid ordinal returns IndexOutOfBounds with NO_OFFSET. These
gateways do not recursively validate child bytes.

`partition_list` checks metadata at MAX_DEPTH and requires a List. It rejects a slice
length above `u32::MAX` with ArithmeticOverflow and NO_OFFSET before reading the count;
a short count returns Truncated at zero. `count` retains the full u32 domain.
`element_width` is Some(width) for fixed-kind elements and None for variable-kind
elements. The directory begins at four; it is empty for fixed elements and has
`4*max(count-1,0)` bytes for variable elements. The payload begins at the directory end
and ends at the supplied slice length. Computed extents above `u32::MAX` return
ArithmeticOverflow with NO_OFFSET before payload/directory truncation. Fixed elements
require exactly `4+count*width` bytes: a short payload returns Truncated at four;
trailing bytes return InvalidBoundary at the expected end. Variable count-zero lists
require exactly four bytes; any payload returns InvalidBoundary at four. A short
directory returns Truncated at its first incomplete word. The first descending or out-
of-payload end returns InvalidBoundary at its word. Equal ends and empty variable
elements are valid; first and last boundaries remain implicit. `list_element` checks the
entire partition before the index, returns IndexOutOfBounds with NO_OFFSET for an
invalid index, and otherwise returns the exact element span contained in that payload.
Child-byte validation remains separate.

`partition_map` checks metadata at MAX_DEPTH and requires a Map. It rejects slice
lengths above `u32::MAX` with ArithmeticOverflow and NO_OFFSET before reading the count;
a short count returns Truncated at zero. Counts and key/value widths retain the full u32
domain. `value_width` is Some(width) for fixed-kind values and None for variable-kind
values. Empty maps require exactly four bytes even when the unused entry stride exceeds
u32; trailing bytes return InvalidBoundary at four. For nonempty maps, checked key-plus-
value/end stride, count-times-stride and four-byte header extents precede truncation
checks; overflow returns ArithmeticOverflow with NO_OFFSET. Fixed maps require exactly
`4+count*(key_width+value_width)` bytes; short entries return Truncated at four,
trailing bytes return InvalidBoundary at the expected end. Their directory is empty at
four, and their payload contains the interleaved key/value entries. Variable maps have
`count*(key_width+4)` directory bytes after the count, followed by the exact value
payload. A short directory returns Truncated at the first incomplete key/end record's
start. Ends are checked in wire order; the first descending or out-of-payload end
returns InvalidBoundary at its word. The stored final end must equal the payload length;
an undersized terminal end identifies its final word. Equal ends and empty variable
values are valid. `map_value` checks the entire partition before the ordinal, returns
IndexOutOfBounds with NO_OFFSET for an invalid ordinal, and otherwise returns its exact
value span, excluding keys and directory bytes. Key domains/order and child-byte
validity remain separate.

`Plan` is non-Clone and non-Copy, and stays private to an exclusive edit. It contains no
account or replacement-value references, backend pointers, or heap allocation. Map plans
borrow the external encoded query key for readiness and key synthesis; other plans use
an empty static key. A plan records a descriptor/path, operation, expected
used/count/table facts, required storage, up to three moves, one local directory
transform, and at most MAX_DEPTH ancestor patch runs. Patch runs describe an offset,
stride, count, and signed delta, not a stack array of every changed boundary. Planning
scans all values to be rewritten and proves every result representable. Applying streams
the same runs without further checked failure. Counts and used_len are individual final
writes.

`InlinePath::new` and `Default` select the root. `push(Step)` appends at most
MAX_DEPTH-1 selectors because the root consumes one depth unit. An overflowing push
returns DepthExceeded with NO_OFFSET, retains the existing prefix, and records a sticky
failure so an ignored error cannot silently select that prefix. Field selectors use
declaration ordinals; Index selects list elements or map values, excluding keys. A
missing field/index returns IndexOutOfBounds, Some on None returns MissingValue, and a
selector incompatible with its parent kind returns InvalidSchema, all with NO_OFFSET.

`resolve` reads the envelope's used word; `plan_replace` additionally requires its
explicit used_len to match that word. A short envelope returns Truncated at eight;
used_len below 12, beyond storage, or mismatched with the envelope returns UsedLength at
eight. Storage lengths outside u32 return ArithmeticOverflow with NO_OFFSET. The core
cannot authenticate a discriminator against a schema because Layout contains no
discriminator; the typed layer performs that check. A plan captures the existing eight
discriminator bytes for readiness.

The public detached `resolve`/planner gateways certify metadata and retain their strict
complete-partition/key-order checks and fault precedence. Internal typed resolution uses
CheckedLayout and produces a private `Resolved` trail of node IDs, exact parent/child
spans, header geometry, selected adjacent ends and ancestor patch coordinates. Its local
containment predicate does not establish full partition validity. Planner and Apply
consume that geometry without setter `view()`, list/map preflight or append length/index
retraversals. Each retained editor stores a logical path and resolves current bytes once
per new operation. Neither borrowed byte slices nor allocation addresses survive growth
or cursor advances. Child faults are rebased once. Fixed targets require replacement_len
equal to their declared width, otherwise InvalidBoundary with NO_OFFSET. New content is
validated before apply; a length-only plan cannot establish its validity.

`Plan::required_storage_len` is max(old_used, new_used), covering the old suffix source
and its new destination. Existing spare need not be retained. Current storage and this
required extent must fit storage_limit; otherwise planning returns Capacity with
NO_OFFSET. Logical extent overflow returns ArithmeticOverflow with NO_OFFSET. Compressed
ancestor runs include all stored ends at or after the selected variable child, including
indexed maps' terminal ends and excluding implicit struct/list terminal ends. Planning
checks every run word's signed addition before returning. All planning and readiness
gateways borrow bytes immutably and preserve the entire buffer on every return.

`plan_list_edit` selects the list itself. Fixed-element insert accepts `0..=count`;
remove/replace require `index < count`. Pop removes the last element or succeeds without
change on an empty list. Truncate keeps `min(requested,count)` elements; clear removes
all elements in one splice. Insert/replace require input_len equal to the fixed element
width; every other operation requires zero. After envelope/capacity/path checks, reject
a non-list with InvalidSchema, then invalid indices with IndexOutOfBounds, then wrong
input lengths with InvalidBoundary. These faults have NO_OFFSET. Growth count or
logical-extent overflow returns ArithmeticOverflow before required-capacity rejection.
Descriptor rules continue to require positive fixed widths.

For fixed-list edits the plan retains the complete list interval and original count,
plus an internal element splice interval. One conditional suffix move covers both
retained elements after that splice and every outer suffix byte. `Plan::removed_span()`
identifies the original complete element for remove/nonempty pop, in account
coordinates; empty pop and other operations return None. The caller copies and validates
any returned fixed value before Apply. The plan retains no value bytes. Detached
planning keeps strict structure/order checks; typed preparation checks touched local
structure, changed words and move ranges, leaving old leaf domains lazy and new input
validation to the encoder.

Variable-element insert and replace accept any input_len, including zero; remove, pop,
truncate and clear require zero input bytes. The complete input encoding must be
validated by the caller. After the same envelope/capacity/path checks, check the ordinal
before input length, count and extent arithmetic. The directory grows by four bytes when
the old count is nonzero and by zero otherwise. The plan captures the old count, payload
start and insertion payload offset; readiness requires those coordinates to match while
accepting compatible changes to other boundaries and payload contents.
required_storage_len includes the enlarged directory and new payload. removed_span is
None. Apply issues only the new element bytes as its destination, then synthesises local
ends and publishes the count at finish. An empty insertion into a count-zero list still
changes the count to one. Indexed replace keeps the count and directory size, adjusts
the selected and later stored ends, and moves the following payload and outer suffix
once by the payload delta. It shares replacement extent planning, ancestor checks and
final publication. Indexed remove/pop capture both selected payload boundaries;
truncate/clear capture the retained prefix and deleted terminal suffix. These operations
subtract both removed payload bytes and the directory reduction from the root extent.
Readiness compares the operation, old/new counts, selected ordinal interval, payload
start, both payload boundaries, table reduction and input length. Compatible unrelated
boundaries remain editable after a fresh partition scan. removed_span returns the exact
old element for remove/nonempty pop, including Some(empty span) for a zero-byte element;
other operations return None. Empty pop and truncate above count preserve every byte.
The caller copies/decodes removed values before apply.

`plan_map_edit` selects a map with fixed-width values. Insert is an upsert: it inserts
an absent key at its sorted position, or replaces only the existing value. Replace on
absence returns MissingKey with NO_OFFSET. Remove on absence succeeds without change.
Clear requires an empty query; other operations require exactly the encoded key width
and a canonical bool query. Insert/replace require input_len equal to the value width;
remove/clear require zero. Envelope, capacity, path and fixed-map-kind checks precede
query checks; stored key domains/order are checked before the query, absence before
input length, and size overflow before required capacity. Stored-key errors use account
coordinates; query/input/absence faults use NO_OFFSET. Empty no-ops do not compute an
unused stride.

Map plans retain an immutable borrow of the external encoded query; subtree and list
plans use an empty static query. They retain no account or replacement-value reference.
Detached readiness repeats query search and checks operation, presence, ordinal, count
and splice geometry; compatible key changes require the fresh search to select that
geometry. Typed Prepared reuses the immediate lookup over unchanged borrowed bytes.
Insert/replace removed_span returns the old value when present; remove returns the
matching old value; clear and absence return None. Copy/decode returned bytes before
Apply. Apply copies the query key for vacant insertion during movement and exposes
exactly the value bytes as destination. Existing keys never enter that destination. The
caller validates new value content before apply. Local typed edits preserve canonical,
unique, sorted keys under initial Rep; they do not certify corrupt key order. Count
publication precedes used_len, and shrink clears the vacated root tail. Variable-value
map operations retain the same value-only loan and exact end publication.

`Plan::check_ready(bytes)` is the read-only readiness gateway. It requires enough
physical storage, replans against current bytes with the captured
descriptor/path/length/capacity policy, then compares discriminator, old/new used
lengths, exact ancestor/target intervals, child ordinals, run geometry, and any captured
collection operation/count/splice. Any incompatibility returns InvalidPlan with
NO_OFFSET. Relocation, spare changes, and same-geometry content changes are compatible;
a plan does not snapshot all payload bytes or directory words. Fresh scans recheck
partition/order and every adjusted word, including compatible sibling-end changes.
Indexed insertions additionally compare the captured input length, insertion ordinal,
count, payload start, table growth and insertion offset.

`begin_apply` consumes the plan, obtains a freshly checked compatible plan for current
storage, and returns an exclusive `Apply` without changing bytes. Every rejection is
InvalidPlan with NO_OFFSET and preserves the entire supplied buffer. Lower
preconditioned helpers remain private; Verus `requires` annotations alone never
constitute a safe public boundary.

Internal `prepare_*` accepts `&mut [u8]` and returns private `Prepared<'a>` owning that
exact borrow and its plan. It permits read-only removed-span inspection, then consumes
itself into phase-0 Apply; it exposes neither mutable bytes nor plan replacement. Save
ghost `B = bytes@` at entry: preparation proves `local_checked(B,p)` and conditional
refinement premises, and planning/inspection prove `bytes@ == B`. The Apply constructor
proves current capacity and all range/patch facts from those premises. Borrowing
excludes mutation; Verus proves byte equality and the phase transition. Capacity-
sufficient typed calls keep this borrow throughout, with no executable readiness replan.
Growth/re-leasing releases it, grows, reacquires bytes, then uses detached readiness to
replan/check geometry. No byte-validity witness crosses resize. Public Plan::check_ready
and begin_apply retain stale-plan rejection, compatible-geometry behaviour and complete
no-write error frames.

`Apply` requires exactly one `move_regions`, exactly one `destination`, then consuming
`finish`. Duplicate or out-of-order use panics before its next write. For subtree/fixed-
list replacements, movement copies the original `[target.end,old_used)` suffix once when
the delta is nonzero and the suffix is nonempty; otherwise it performs no copy. Indexed
insertion follows the ordered moves in the physical-splice section above. Indexed removal/truncate/clear compact
and adjust retained directory words first, then move the retained payload prefix left by
the table reduction, then move the later payload and outer suffix left by the total
reduction. Indexed replacement uses the same splice with no table reduction. Every
operation moves any outer suffix byte at most once. The destination borrow covers
exactly the measured replacement region and prevents another apply method from running
while it remains live. Its encoder must fill and validate the complete encoding; issuing
a mutable slice cannot certify that content.

`finish` streams the checked ancestor runs, writes a changed fixed-entry list/map count,
publishes used_len, then zeros exactly `[new_used,old_used)` on shrink. Indexed
insertions publish their local directory and count before used_len; their total ancestor
delta includes table growth. Indexed shrink splices prepare the directory during
movement and publish count at finish, before used_len and tail clearing. No physical
shrink occurs before compaction or within core. Equal-length subtree/element
replacements write no metadata; fixed-list no-ops also leave every byte unchanged.
Discriminator bytes, preceding nonboundary bytes, moved subtree bytes, and spare beyond
max(old_used,new_used) retain their specified values. No storage resize occurs in core.
Dropping or forgetting an unfinished Apply can leave an incomplete encoding, so the
typed storage owner keeps its poison flag set until finish returns. Every executable helper requires an inventoried contract.

## Errors and offsets

The error type is SDK-independent, Copy, and carries no string or allocation:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Fault { kind: ErrorKind, offset: u32 },
    Program { code: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorKind {
    Truncated = 1, Discriminator = 2, UsedLength = 3,
    ArithmeticOverflow = 4, InvalidBoundary = 5, InvalidBool = 6,
    InvalidUtf8 = 7, InvalidTag = 8, UnsortedKeys = 9,
    DuplicateKey = 10, IndexOutOfBounds = 11, MissingKey = 12,
    MissingValue = 13, Utf8Boundary = 14, Capacity = 15,
    Allocation = 16, DepthExceeded = 17, WrongOwner = 18,
    NotWritable = 19, BorrowConflict = 20, GrowthLimit = 21,
    RentShortfall = 22, Unauthorized = 23, InvalidPayer = 24,
    DuplicateAccount = 25, AlreadyInitialized = 26, Closed = 27,
    Poisoned = 28, InvalidPlan = 29, InvalidSchema = 30,
}
pub type Result<T> = core::result::Result<T, Error>;
impl Error {
    pub const NO_OFFSET: u32 = u32::MAX;
    pub const fn new(kind: ErrorKind, offset: u32) -> Self;
    pub const fn custom_code(self) -> Option<u32>;
    pub const fn rebase(self, base: u32) -> Self;
}
impl From<ErrorKind> for Error { /* Fault { kind, offset: NO_OFFSET } */ }
```

`From<ErrorKind>` lets application code write `ErrorKind::Unauthorized.into()` and
`.ok_or(ErrorKind::ArithmeticOverflow)?` without spelling an offset.

For Fault, `custom_code = 0xB100 + kind as u32`. Codes are stable and reserved by
applications using this adapter. Program errors preserve the SDK's full u64 conversion,
including native and custom codes; `custom_code` returns None for that variant.
`From<Error> for pinocchio::error::ProgramError` maps Fault to Custom(code), and Program
through the SDK's u64 conversion. `From<ProgramError> for Error` records the reverse u64
representation. Known preflight conditions use the specific Fault kind before invoking
the SDK.

Offsets identify the first offending account byte: count/header word, directory word,
key start, tag, bool, or UTF-8 failing byte. For API indices and policy failures without
a byte location use NO_OFFSET. Subtree validators report their own offsets; callers add
the exact region base once with checked arithmetic. Display formats kind and offset only
on request; the hot path never formats a string. Malformed input may have several
faults; traversal order is fixed: envelope, fixed fields in wire order, directory order,
then variable children in wire order. Equal adjacent keys return DuplicateKey;
descending keys return UnsortedKeys.

The optional `blubber-core/program-error` feature owns the two SDK From implementations
and depends only on the pinned solana-program-error crate. `blubber/pinocchio` enables
it. The SDK reexports this same ProgramError type. SDK-to-u64-to-SDK roundtrips cover
every native variant and all custom u32s, including custom zero. Arbitrary u64 values
remain intact inside Error::Program, but converting a noncanonical code through the SDK
follows its documented fallback to Custom(code as u32); there is no arbitrary-u64 SDK
bijection.

`Error::rebase` performs that addition, preserving Program and NO_OFFSET. If the sum
overflows or equals NO_OFFSET, it returns ArithmeticOverflow without an offset. The
sentinel cannot identify a byte in a region whose exclusive end fits `u32`.

## Storage and runtime adapters

SliceStore retains a plain exclusive slice and exposes at most its original room.
VecStore retains an exclusive Vec, uses fallible reservation and zeroes extensions.
Successful resize preserves all old bytes below the shorter length; failed allocation
preserves bytes/length. Metadata remains observable after closing. Mutable backend
dispatch requires unforgeable writer authority; ordinary callers use root lifecycle
methods.

`reserve(additional)` ensures used_len plus additional physical room with checked
arithmetic. `shrink_to_fit` attempts physical trim after logical compaction. Host
failure returns Ok(false); a commit receipt can retain a trim error. The first commit
attempt closes even when poisoned or given an invalid used length. Closed precedes
Poisoned in state errors. Host completion retains capacity and reports zero
funding/refund. Vec trim allocates/copies/swaps only after successful reservation. Slice
trim reduces exposure within the same loan.

### Runtime headroom and settlement

`Headroom::default() == Headroom::UpTo(0)` in every feature set. Store::new, Funding
conversion, generated scoped helpers and native bound-payer edits all use this default.
Existing opening spare can satisfy growth. Additional physical capacity requires
explicit Max, UpTo(n), or alloc-only OnDemand.

Max/positive UpTo discover safe SDK-local capacity before taking the exclusive data
guard, using at most 15 rejection-before-write probes. Storage length is the current
guard length. The invocation's original length remains the growth authority through
trim, reopens and CPI. The separate account cap is 10 MiB; the invocation increment is
10,240 bytes. Local SDK acceptance cannot predict shared transaction growth, caller CPI
limits or eager runtime allocation faults.

OnDemand allocates a private copied-handle slot before releasing exclusion, resizes and
reacquires before destructive work. Failed reacquisition closes and terminally poisons
access, even if physical growth already occurred. Each attempt consumes a slot,
including failures. An eight-byte SBF handle is counted before allocator overhead across
all accounts, reopens and handoffs. A no-allocator program cannot select this mode.

Commit consumes the typed root, closes the store, releases its guard, attempts requested
trim, and settles actual physical rent. CommitInfo carries used_len, storage_len,
topped_up, refunded and optional shrink_error. Trim failure is diagnostic;
funding/settlement failures return errors and must propagate. Drop attempts trim without
CPI or rent settlement. Forgetting a store can retain borrow state/headroom and does not
complete settlement.

For a resize within the 10 MiB account cap that exceeds the invocation's growth
limit, `OnDemand` returns `RentShortfall` if underfunded and `GrowthLimit` if funded,
while a reserved lease bounded by the invocation's growth limit returns `GrowthLimit`
before checking funding.
Funding checks precede growth/movement. Rent uses the complete sysvar, checked
arithmetic and supported legacy thresholds 1.0/2.0; unsupported thresholds fail before
transfers/refunds. Payer/recipient/System Program identity, writability, signature and
alias constraints are checked. Refund authorization is explicit; settlement uses actual
final length, including retained capacity after failed trim. Finish edits before CPI and
reload afterward.

Generated helpers include lazy and checked read/edit closures, explicit EditOptions, and
`with_edit_checked_options(account, program_id, options, edit)`. The checked options
helper validates the complete root before invoking edit and returns `(R, CommitInfo)`
after successful completion. Creation and prefunded allocation helpers measure input
before System CPI, include the full envelope, check signer/PDA seeds and aliases,
initialize and commit. Creation is bounded by the original-length growth window; larger
accounts require a separate runtime allocation instruction.

### Native account wrapper

`blubber_anchor::Blubber<State, Policy>` has sealed Lazy/Checked policies;
`Blubber<State>` defaults to Lazy and `Blubber<State, Checked>` always scans. The
wrapper authenticates owner/identity/envelope, retains a genuine SDK guard, and checks
copied fixed-prefix domains before publishing constraint fields. Variable data remains
scoped to callbacks.

New wrappers permit initialization but no typed/prefix access. Measurement, capacity and
funding failures retain New state. Successful initialization commits and enters Ready.
`initialize_if_new` ignores input on existing Ready state; reuse still checks the
selected load policy, rent and application authority. Missing initialization fails
successful exit. Automatic SDK reuse checks physical space; explicit growth/trim changes
later space requirements.

The initialization route uses both complete encoder and replay markers. Lazy feature-off
initialization of a covered graph omits the post-write full scan and redundant covered
replay. Checked policy, verify-on-load and uncovered/default-false graphs retain checked
publication; variable-child sizing remains. Prefix publication follows that selected
check, not an unconditional scan. See [initialization assurance](ASSURANCE.md#encoders-and-initialization).

Edits transfer a held guard directly for zero-opening-growth policies and release it
before discovery for positive headroom. No application callback runs during an unguarded
handoff. Commit occurs once; success and returned errors attempt reacquisition. Failed
reacquisition is terminal. Host unwind attempts reacquisition but never publishes a
prefix. Preserve the original operation error if cleanup also fails.

Explicit SDK realloc and managed growth use one sizing authority per handler. Exact
realloc validates used length, cap and invocation window, resizes and settles once;
later edits preserve that allocation through UpTo(0). Explicit shrink clears the opening
reservation floor. Close checks Ready/mutable state, owner, distinct writable authorized
recipient, borrow readiness and lamport addition; it invalidates identity, releases
exclusion, transfers once through the safe SDK close operation and becomes terminal.

`with_cpi`/`with_cpi_read` finish edits and release exclusion, perform checked CPI,
reacquire and fully validate owner, identity, used length and root before refreshing the
prefix. Returned callback/preflight errors require that reload too. Runtime callee
errors can abort the invocation and rely on rollback rather than returned cleanup.
Failed reload/reacquisition is terminal. Read-only CPI arguments do not constrain
captured outside handles. Original invocation length is never reset.

The owning wrapper retains private leaked copied-handle slots, not account bytes.
References remain bounded by wrapper borrows/callbacks. The SDK handle keeps the
wrapper non-Send and non-Sync; leaking a copied handle never extends invocation
memory validity. Account layout is omitted from
the adapter's IDL while account flags, arguments, seeds and relations remain. A stock
account decoder cannot decode format 1. The exact SDK git dependency keeps this adapter
unpublished; it is excluded from the three published archives.

## Resource and assurance premises

Every SBF frame must fit 4,096 bytes, including depth-16 paths and wide native values.
Separate non-inlined frames carry large preparation, encoding and removed-value slots.
No unbounded stack directory scratch is used. Work is bounded by actual spans/counts and
certified depth; repeated packed edits can still move quadratic total bytes over a
sequence.

Strict validation admits exactly the independent schema relation. Typed logical
preservation assumes that relation initially and matching new encoders. Physical
containment, progress, error frames and legal destructive-phase no-unwind are separate
obligations on arbitrary admitted byte geometry. Compiler, solver, vstd primitives, SDK
exclusion, runtime transaction semantics and derive/type projection correspondence
remain explicit trust boundaries. Their evidence and limits are in
[ASSURANCE.md](ASSURANCE.md).
