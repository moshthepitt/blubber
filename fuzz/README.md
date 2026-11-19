# Corrupt-byte fuzzing

Run `bash scripts/reproduce.sh fuzz validate` or `bash scripts/reproduce.sh fuzz edit`
from the repository root. Each run has a 300-second time budget and starts from
a scratch copy of the committed corpus under `target/reproduce/` by default.
The working corpus and failure artifacts stay there. Install a host C++
compiler and its development headers and linker library to build
`libfuzzer-sys`. Host-only runs use cargo-fuzz 0.12.0, libfuzzer-sys 0.4.13 and
the pinned nightly with AddressSanitizer, debug assertions and overflow checks.
The nested lockfile is checked in. The retained report of the release smoke run
is `verification/results/fuzz/smoke.log`.

The 32 fixture schemas cover every signed/unsigned scalar width, bool, Pubkey,
a 33-byte array, strings, fixed and indexed lists/maps, derived packed fixed
values, a mixed generated record, options and a nested record tree at depth 8.
Three direct-field roots exercise compact string/list/map edits. Distant-end,
order, duplicate-key and full-width-count edit seeds retain the corrupt oracle.
They are representative schemas, not arbitrary schema enumeration.

| Target | Input bytes | Checks |
|---|---|---|
| `validate` | schema selector; offset probe; remaining corrupt bytes | Exact-region full validation agrees with the independent model; valid states decode identically and re-encode canonically. Check arbitrary envelopes and a valid envelope around the corrupt subtree. Exercise fallible getters, lookups and fused iterators even on invalid states. |
| `edit` | schema selector; mode; capacity u16 LE; initial length u16 LE; initial bytes; commands | Pre-write rejections preserve the allocation. A callback error can leave completed edits, and a detected source fault can poison the edit. Valid states agree with the independent logical edit oracle after every operation; check previous values, canonical bytes, used length, moved sibling encodings, zeroed shrink tails and full revalidation. |

For `edit`, mode modulo 4 selects an arbitrary full account (0), a valid model
fixture on SliceStore (1), a valid model fixture on VecStore (2), or corrupt
subtree bytes wrapped in a correct envelope (3). Valid fixtures use the initial
bytes as entropy; bit 2 selects sparse rather than expanded initial collections.
The initial byte length is capped at 8,192 and the requested capacity is reduced
modulo 8,193. The allocation is large enough for the selected initial account,
including its envelope where present. VecStore may grow during the history,
which has at most 32 commands. Each complete eight-byte command supplies a u16
node selector, a kind byte, a u16 ordinal and three entropy bytes. Incomplete
commands are ignored.
Operations include scalar/fixed replacement, UTF-8 set/append/clear, list
bulk insertion/replacement/removal/pop/truncation/clear, sorted map insertion/replacement/
removal/clear, and option replacement/selection. Every otherwise valid growth is
also tried on a SliceStore at required-1 and required capacity. The first entropy
byte modulo five selects the bulk run length (zero through four); the ordinal
selects any insertion index. Append positions use extend_from_slice, other
positions use insert_from_slice. The oracle compares complete old values when
returned, presence for unit returns, and preserved physical spare bytes. Successful
edits on partially corrupt states also check physical spare and shrink-tail frames;
a retained seed grows/shrinks one field beside an invalid bool, then repairs it.

Both entry points cap total input at 65,536 bytes. Lazy iterator traversal has a
256-item budget; full validation and the independent decoder still check the
entire subtree. Edits on invalid roots use a lazy load and checked access to the
selected path. Untouched corrupted bytes can remain after a successful edit. A
detected source fault can poison the edit and keep the old published length.
When the model can decode the complete root, it checks subsequent edits against
the full reference result. Panics fail the run.

`cargo +1.97.1 run --manifest-path fuzz/Cargo.toml --example seed --locked`
regenerates the small directed corpus. The seed example uses the host C++
toolchain to build `libfuzzer-sys`.
The workspace member `tests/fuzz-replay` runs these checks without libFuzzer
under `cargo +1.97.1 test --workspace --locked`, including every prefix of each
validation seed, five replacement values at each payload byte, the generated
edit histories and all committed host corpus inputs. It is also a
default member, so plain cargo test includes replay. The nested replay test
remains available. Nine small bulk corpus inputs cover empty/one/many runs
at every index of a two-element indexed list. Every discovered
failure must become a minimized corpus input and a named deterministic test
before a successful smoke run is recorded. An evolving working corpus is not
copied wholesale into Git.

For a longer host-only run, use the corresponding command in
`scripts/reproduce.sh` as a template. Select Rust `nightly-2025-12-08`, pass
your working corpus directory to `cargo fuzz run` and increase
`-max_total_time`. Reuse that directory to resume from its evolved inputs.
Shared `bridge.rs` and `oracle.rs` sources keep the deterministic replay and
fuzz dispatchers identical. The host-only seed example compiles on Rust 1.89
and 1.97.1. The `sdk-fuzz` graph requires Rust 1.97.1.

## Authenticated SDK histories

The release gate builds the `programs/policy` fixture for v0 and v3 in both
feature states and replays the directed corpus in three runtime memory
mappings. It then runs `policy_read` and `policy_edit` serially for 300 seconds
each with ASan, debug assertions and overflow checks.
`bash scripts/reproduce.sh fuzz policy_read` (or `policy_edit`) builds
feature-off fixtures for v0 and v3 and starts one timed run. The wrapper
supplies both fixture paths. The target checks authenticated lazy and checked
loads under genuine SDK guards and also exercises the host oracle for the 32
schemas.

The SDK targets take `profile:u8, flags:u8, initial_len:u16le`, an initial
account and at most 32 eight-byte commands. Profile modulo six selects one of
three memory mappings on v0 (0 to 2) or v3 (3 to 5). Flags select a model-built
valid start (1), checked load (2), or an envelope around arbitrary subtree
bytes (4). Commands select fallible reads, full-width indices, scalar/list/map
edits, fixed-map fill and cursor visits, indexed rewrites/callbacks, root
verification, foreign-schema loads, interrupted encoders and checked loads.
Edit commands 14 and 15 drop the session and reopen it; 15 first invokes the
program on the account. Drop attempts to trim to a well-formed used length.
Reopening creates a new session, so its volatile poison flag starts clear.
That is the documented volatile poison, not recovery.
The downstream compile-fail tests also reject wrong-schema projection.

No fallible-path panic is caught. SDK statuses, read digests, poison and the
full pre-Drop physical buffer must match host replay. On a valid root, the
independent model decides success or failure, the error kind where it is
determined, every resulting byte and poison. Callback partial progress is
modeled setter by setter, and an encoder interruption after a write must
poison, keep the old published length and stay inside its planned extent.
Planning failures preserve bytes and leave the session unpoisoned. Reads on a
valid root must return the model's values. Checked loads and lazy loads with
`verify-on-load` reject invalid roots before writing. The raw encoder control
exercises a separate path. Poisoned lazy access returns `Poisoned`; a checked
load recovers only a valid root. On invalid roots, pre-write failures preserve
bytes, bool commands write only requested values inside the root, and
cursor/callback partial progress keeps its frame. Untouched corruption need not
disappear after a successful lazy edit. Callback frame checks permit the
contiguous zero tail left by intermediate growth above both endpoint lengths.

Each directed damage family pins its model error, checked fault and feature-off
lazy read/edit probe results. The committed corpus must hold every family with
the exact tape `corpus()` generates. Seed replay counts valid-root successes per
command and read, checked/eager rejections, poison rejections, recoveries,
interrupts and reopens after poison, and requires nonzero counts for the
expected paths. Feature-off replay expects zero eager rejections, and
feature-on replay requires eager rejections. Edit commands 11 (foreign schema)
and 12 (failing encoder) must have zero successes.

Inputs are capped at 65,536 bytes. Fallible iterator reads visit at most 32
items. Strict validation and edits terminate over loaded extents. The SBF
corrupt runner allows 100M CU for this exploration; acceptance fixtures keep
their existing meters and ceilings. A VM failure, a 30-second per-input timeout,
a sanitizer failure or a missing/short job fails the release check. The
300-second jobs must finish with nonzero execution counts. Working corpora, raw
traces and failure artifacts stay in a scratch directory. Each seeded job starts
from the committed corpus alone. The retained receipt,
`verification/results/policy/corrupt-qa.log`, is at most 40 lines, and every
line comes from a log of the same run. Each negative control must fail its host
regression with a named assertion. Six controls must also fail seed replay. The
early publication and nested ancestor publication controls have host-only
replay evidence. The corrupted-history host suite has nine tests in each Miri
alias model.

The SDK targets need Rust 1.97.1, with `RUSTC_BOOTSTRAP=1` for sanitizer flags;
the host-only targets use the pinned nightly. The workspace and nested lockfile
agree on the runtime packages they share. From the repository root, run
`CARGO_TARGET_DIR=target/fuzz-seed cargo +1.97.1 run --manifest-path fuzz/Cargo.toml --example seed --locked --features sdk-fuzz`
to regenerate the SDK corpus. The separate target directory keeps the nested
build's `blubber-policy` artifact apart from the workspace build. The full
directed matrix is generated in memory. The repository retains the compact
directed tapes.
