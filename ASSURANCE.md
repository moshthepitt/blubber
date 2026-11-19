# Assurance

[The specification](SPEC.md) defines format 1 and the current API contracts.
This document separates production proofs, finite model checks and executed
tests. Evidence is scoped to the source/tool/artifact hashes in its receipt.
A retained run is not a claim that changed sources have already passed.
The private gate runner enforces the coverage inventories and produces receipts.
For application loading, writer and recovery choices, see the [trust model](docs/trust-model.md).

## Coverage by API area

The [public implementation inventory](verification/public-api.tsv) maps every
typed implementation and derive template to named checks. The generated
[declaration listing](verification/public-api.txt) checks exact API signatures.
The [core executable inventory](verification/core-proofs.tsv) accounts for
annotated production bodies. The [root theorem inventory](verification/root-theorems.tsv)
binds theorem identity, parameter types, requires/ensures and concrete witness
bodies using normalised-token hashes. Each required theorem must verify;
there are no open obligations. The [core receipt](verification/results/core/all.log)
records the production proof run.

<!-- assurance-counts:start -->
- 201 required root theorem queries, each bound to a concrete premise witness; no open obligations.
- 574 inventoried executable bodies in the production core.
- 361 Kani harnesses across 8 cohorts; per-harness widths, unwind limits and covers are inventoried.
- 13 source control markers and 72 directed mutation controls.
- 92 classified Miri integration modules.
<!-- assurance-counts:end -->

The [Kani receipt](verification/results/kani/all.log) binds the individual logs
for [arithmetic](verification/results/kani/arithmetic.log),
[leaves](verification/results/kani/leaf.log),
[derived encoders](verification/results/kani/derive_encode.log),
[error codes](verification/results/kani/error_codes.log),
[map keys](verification/results/kani/map_key.log),
[fixed structs](verification/results/kani/fixed_struct.log),
[records](verification/results/kani/record.log) and
[typed plans](verification/results/kani/typed_plan.log).

| Public operations | Proof/test connection and limits |
|---|---|
| Core arithmetic, spans, words, moves, descriptor certification and partition selectors | Every executable body is inventoried in `core-proofs.tsv`; exact arithmetic, iff partitions, failure frames and no-unwind contracts. Core integration suites check boundaries and overlap. `Display`/error marker and optional SDK error conversions are outside Verus; host/Kani check stable errors and conversions. |
| Full validation, root `verify()`, UTF-8, key order and binary search | Verus proves acceptance iff schema validity, depth limits, exact search on ordered input and bounded access on invalid input. `validation`, `leaves`, `utf8`, `search`, `verification_policy` and the runtime CU rows exercise the production paths. Root verification excludes spare bytes and preserves poison; selected `validate()` retains subtree scope. Local selectors prove conditional selection. |
| Generic replacement, fixed/indexed list/map plans, Prepared/Apply and bulk slots | Kernel refinement/frame/error/phase contracts and the pinned theorem inventory include nested and fast admissions, exact walks and streamed rewrites. Bulk validity assumes each encoded new element satisfies its schema, never whole replacement validity. `fixed_list`, `indexed_insert`, `indexed_remove`, `fixed_map`, `indexed_map`, `prepared` and typed exhaustive suites compare actual results. |
| `Fixed`, `MapKey`, `Schema`, `RootSchema`; scalar/string/bytes/list/map/option codecs | `codecs`, `fixed_derive`, `schema_encode`, `identity` and the Kani leaf/map-key/fixed/derive cohorts cover built-ins and representative derives. Native padding is excluded. Arbitrary handwritten codecs remain caller obligations; Rust trait implementation alone establishes no semantic proof. |
| `View::new/load/load_trusted`, fields, read/read_fixed/read_str/validate; collection constructors, get/try_get/contains, len/is_empty, iterators and validation | `readers`, `corrupt_reads`, `schema_read` and `flat_schema` check envelopes, exact child regions, absolute faults, spare bytes, fused iteration and distant corruption. Miri covers the generic host paths; runtime matrix cases check copied values, fallible/shared reads and iterator exhaustion. |
| Generated root `load/load_mut/load_checked_mut/load_trusted`, initialization, field projections, reborrow, lengths, validate/verify, reserve, shrink_to_fit and commit; untyped `raw::Edit` | `schema_read`, `projections`, `prepared`, `fast_edits`, `storage`, `relocation` and `verification_policy` check lifecycle, failure bytes, poison, policy and forced relocation. Raw/host loads are checked; authenticated SDK mutable loads use the lazy default. Only explicit successful checked mutable recovery clears poison. Untyped raw edits are outside typed preservation; no universal Rust-wrapper refinement theorem is claimed. |
| `ScalarMut` get/try_get/set; `StrMut` set/push_str/truncate/clear; `OptionMut` get/try_get/set/get_mut/some_mut; reborrow | `projections`, `fast_edits`, negative histories and runtime scalar/option/depth-eight fixtures cover unchanged/grow/shrink/regrow and UTF-8 boundaries. Root theorems compose valid input bytes; malformed untouched bytes may survive a lazy edit. |
| `ListMut` reads, push/insert/replace/remove/pop/truncate/clear, bulk insert/extend, remove_into, get_mut/at_mut | `list_edits`, `remove_into`, `exhaustive_record`, `exhaustive_typed_plan`, differential histories and runtime matrix assertions check result bytes, removed values, empty transitions and extraction failures. Bulk exact-slot encoder errors and short/long writes poison the store. |
| `MapMut` reads, insert/remove/clear/remove_into, borrowed get_mut/entry, consuming into_get_mut/into_entry/at_mut; occupied/vacant key/insert/remove/into_mut and or_insert/or_insert_with/or_default/and_modify | `map_edits`, `entry`, entry-combinator call-count/borrow/unwind controls, `remove_into`, exhaustive typed plans and runtime cases check ordering, duplicate replacement, absent removal, copied keys and return values. Fast fixed-map front/middle selection is among the pinned root contracts. |
| List/map cursors and try_for_each_mut; shared-Mut IntoIterator | `cursor`, `projections`, UI borrow failures and traversal/runtime cases check lending lifetimes, exact fixed slots, indexed coordinates after growth/relocation, repeated exhaustion, full key/value checksums and partial progress after callback errors. A callback error keeps prior successful edits. The runtime retains packed growing fallback with quadratic worst-case movement; no gap theorem ships. |
| List fill/rewrite_from and map fill_values/rewrite_values/rewrite_values_from | `rewrite_kernel`, `list_edits`, `map_edits`, tractable Kani controls, both Miri models, targeted mutants and SBF oracles check exact count/keys, empty transitions, capacity/overflow, nested frames and sticky error/unwind poison. Production Verus proves slot, prefix, frame, geometry and root publication and keyless preflight and typed encode-slot composition, conditional on individual codec validity. Independent byte histories and counter assertions cover the wrappers; no universal handwritten-codec or variable typed-graph Kani claim. |
| Owned Schema/derived mirrors, to_owned, owned list/map insert/replace/remove/pop/push | `owned`, private allocation controls and both Miri models check roundtrips, sorted Vec maps, allocation errors and returned content. Allocating SBF helpers use n=1/2/10 with the default 32 KiB heap. Larger owned workloads have no runtime claim. |
| `ReadStorage`, `Resize`, `SliceStore`, `VecStore`, `CommitInfo` and trait defaults | Host storage/relocation suites and private Miri controls cover object safety, initialized storage, zeroed regrowth, poison, first-commit closure, trim receipts, Drop/forget/swap and allocation failure. Safe Vec allocation is a trust boundary. |
| Pinocchio ReadStore/Store typed `load/load_checked`, Funding, Headroom, refund binding, create/allocate signer/PDA; generated lazy/checked scoped helpers | `borrow_resize`, `rent`, `initialize`, `scoped`, `verification_policy` and `policy` exercise genuine guards, owner, identity and envelope checks, lazy touched checks, full checked scans, net rent/refund, leases, trim failures, rollback and aliases with both feature states. SDK/runtime behaviour remains outside the Verus/Kani adapter claim. |
| `schema`/`Fixed` macros, generated constructors/accessors, constants, Ref/Mut/Init/Owned families and metadata support | `ui`, `derive_hygiene`, `review_hygiene`, encoding/reader/projection suites, host/MSRV consumers, real rust-analyzer and SBF depth/width matrix check representative expansions and diagnostics. Generated guards compile under forbid(unsafe_code). No proof quantifies over every macro input. |
| `AccountSchema`, generated Prefix, `space` and fixed-copy metadata | `anchor_schema` and `ui` check identity/override, detached copies, fixed aliases/markers, omitted variable fields, preserved visibility, opt-out, native padding and checked sizing without SDK dependencies. The separate compile lane checks two live 1,024-byte prefixes and refresh on SBF v0/v3 in three memory profiles with no heap access. Metadata and copies have native/compiler/runtime evidence, without new Verus/Kani proofs. |
| Native `Blubber<T>` / `Blubber<T, Checked>`, initialization and CPI reload | Lazy loads check copied-prefix domains. Checked loads scan the full root. Complete encoder/replay markers select initialization without a redundant lazy feature-off scan. Checked, feature-on and default-false graphs keep full validation. Native lifecycle/CPI, encoder, mutation and both Miri suites check exclusion, exact bytes, calls, funding, strict returned-path reload and terminal failures. |
| Native `blubber_anchor::Blubber<T>`, SDK re-export and raw recipe | Compile tests check exact SDK types and the extracted raw recipe for the published 2.0.0-rc.2 SDK. SBF v0 and v3 tests check runtime behaviour and heap use. Runtime tests cover checked loads, account guard exclusion, scoped reads, fixed-capacity and managed edits, close and CPI. Initialization tests cover signers, PDAs, prefunded allocation, state transitions, borrowed initialization, opening space, rent, reuse and rollback. The compiled examples cover opaque IDL. The native adapter depends on a pinned SDK revision for safe mutable loads. |


## Production proofs and bounds

Verus verifies the same production executable bodies compiled by Rust, with
`--no-cheating`. No assumed specification, assumed proposition, admitted proof
or external-body escape is allowed. Complete report checks reject absent,
empty, cached-only, partial or failed query output and stale source bindings.
Every named executable contract and legal destructive-phase no-unwind query
is required. Concrete premise witnesses have no arguments or assumptions,
call their production theorem and have independently pinned bodies.

The schema relation is independently specified. It covers exact scalar widths,
bool/UTF-8 domains, option tags, complete directories, strict map keys and
recursive child validity. Validation proves acceptance iff that relation.
Nested splice reconstruction derives ancestor/sibling frames from actual
movement and patched boundaries. Fixed/indexed list/map root compositions
include admissions, exact selected elements, ordered keys, bulk slots and
publication. New slots require individually schema-valid encodings; proofs do
not assume the whole replacement already valid.

Physical containment/progress on arbitrary admitted geometry is separate from
logical preservation from a valid root. Lazy patch passes can report detected
source errors after movement under sticky poison. A checked load retains its
pre-write error frame. All caller-input, capacity and funding rejection precedes
destructive writes. Negative controls weaken premises, alter destinations,
remove frames/publication checks and change concrete witnesses; each must fail
its intended obligation. Parse, installation and resource failures do not count
as negative-control success.

Movement captures original source bytes in partial chunks of at most 1,024
bytes before writing. Direction preserves unread overlapping sources. The
proofs cover both directions, exact destination, outside-byte frames and legal
no-unwind phases. The copy implementation adds no trusted copy contract.

### Model checking

[Kani harnesses](verification/kani/harnesses.tsv) independently list each name,
full symbolic width, unwind bound and required non-vacuity covers. The
[record families](verification/kani/record-families.tsv) account for composed
cohorts. Bounds are per harness, not inferred from a representative example.
Kani/CBMC runs all default bounds, overflow, pointer, safety and unwinding
checks. Each harness has one CBMC process and a 600-second limit. Record
concurrency is at most four and respects the runner's allocated CPUs; other
cohorts run one worker. Timeout, exhausted memory, unknown, missing covers or
missing harnesses fail.

Full-width arithmetic, key codecs/order, fixed derives, load policy,
geometry, exact slot frames and publication have tractable production checks.
Variable list/map ends use partitioned cases or small production byte wrappers
with arbitrary words and admitted numeric bounds. Their byte/poison/frame
oracles remain independent of the implementation.

Successful arbitrary variable typed splice graphs, general-validator
compositions and virtual callback wrappers are excluded where they do not
finish within these bounds. This is a model-checking limit, not an exception
to production proof or test obligations. Generic proofs, native exhaustive
histories, both Miri models and property/fuzz tests cover those paths separately.
No representative Kani fixture certifies every derived graph or custom codec.

### Native, exhaustive and runtime testing

The independent `blubber-model` uses owned values and complete re-encoding;
it shares no production move or directory-update algorithm. Golden bytes,
deterministic generated schemas, property histories and exhaustive finite
geometries compare success/errors, complete used bytes, spare bytes and poison.
Nested fixtures include depth limits, both branches, empty/one/two-element
transitions, UTF-8 and absent options. Source inventories and retained reports
state the finite cases; they do not supply symbolic arbitrary-content claims.

Both Miri alias models run with strict provenance. The
[Miri inventory](verification/miri-modules.tsv) classifies every Cargo-discovered
integration target; exclusions name their native or proof coverage. Forced Vec
relocation, live child borrows, callback errors, encoded extraction, allocation
failure and safe host guard transitions exercise real paths. Compile-fail UI
cases check escaping children, parent reuse while a child lives and cursor
exclusion. Miri tests host stores and safe guard facades, not invented SDK
account memory.

Runtime fixtures use real SDK accounts and SBF v0/v3 in every configured memory
mapping. They check exact independent bytes after scalar, collection, option,
owned and depth-limit edits; lease/resize, signer/PDA initialization, rent,
prefunding, aliases, Drop/forget, CPI and rollback. Host injected failures cover
returned-error/reacquisition paths unavailable in the pinned runtime. Some
callee failures abort an invocation instead of returning to cleanup; runtime
assertions require rollback. Allocating owned SBF examples use bounded small
counts and the default 32 KiB heap. Their results do not establish affordable
arbitrary-sized owned decoding.

The devnet replay compared 1,284 base cases with their local runtime results.
[Devnet receipt](verification/results/devnet/summary.log) ·
[Structured results](verification/results/devnet/summary.json)

The local validator matched Mollusk on 1,284 base cases and 24 near-10 MiB
cases, including the resulting account bytes and fixture CU in every case.
[Local validator receipt](verification/results/localnet/summary.log) ·
[Structured results](verification/results/localnet/summary.json)

Mutation controls retain the complete viable inventory, baseline and expected
outcomes. [Directed controls](verification/mutant-controls.tsv) and
[source controls](verification/control-markers.tsv) are independent quality
gates. Renaming a harness must preserve its inventory row, widths, unwind and
covers. Control markers stay beside their guarded transition.

The main mutation cohort runs the `blubber` library test executable, including
the folded prepared-initialization controls for matching bytes, minimum size
and handwritten measurement replay. The gate checks each control cohort in
cargo-mutants' own library baseline. The search progress mutation uses its
dedicated point-read test.

## Encoders and initialization

The generic encoder proofs start at projected inputs satisfying their contracts.
They cover leaf/key bytes, products, options, struct descriptor coherence,
fixed/indexed lists/maps, complete graph validity, exact destination frames,
measurement purity, replay observations, prepared input transport and native
publication. Applications need no verifier job, enrollment or proof receipt.

Derive/type projection glue remains trusted and tested. Equal-width field swaps
can remain schema-valid and retain the same wire identity; the exact-byte
projection-swap negative test demonstrates why validity alone cannot certify
field selection. Metadata flattening and proc macro expansion are not proved.
The verified table checker establishes witness soundness independently.
Generic nested schema encoding is supported and tested in
[schema_encode.rs](crates/blubber/tests/schema_encode.rs), including nested
lists, empty children, complete bytes and neighbouring frames.

In [the native initialization path](crates/blubber-anchor/src/lib.rs),
`initialized_view` calls production `encoding_publication::init_policy` with
complete encoder/replay markers, the Checked policy and verify-on-load state.
Only a covered Lazy feature-off graph omits the redundant post-write scan.
Checked, feature-on and default-false graphs retain full validation.
Covered native replay and fixed-child replay may reuse measured lengths;
variable-child sizing required for exact slots remains. The private prepared
initializer owns the exact input and preflight length across the safe handoff.

Schema/Fixed/MapKey audit, replay and order markers default conservatively.
Derives combine every relevant child; map keys have an independent encoder
audit. A handwritten true marker is an implementer promise, not proof of custom
code. Opaque/default-false graphs retain checked publication and required
measurement calls. Tests check first-error/call order, exact bytes and spare,
short/long encoding, changed widths, partial panic, New/Ready, prefix denial,
reacquisition and successful fallback. Mutation runs compile and exercise these
helpers. CPI reload always performs full validation independently of markers.

### Native runtime fixtures

Native account tests retain genuine SDK guards through constraints, callbacks
and exit. Shared aliases may coexist; exclusive combinations fail. Drop
releases exclusion, while a forgotten guard retains it until runtime teardown.
Preflight allocation failures occur before guard release. Prefix invalidation
precedes callbacks; returned poison/refresh failures and unwind cleanup leave
typed access terminal. SDK invocation memory, copied-handle slots and allocator
behaviour remain trust assumptions.

Initialization fixtures cover signer/PDA targets, prefunding, payer aliases,
wrong owner/seeds/bump, missing initialization, exact opening space, full-rent
reuse and application authority. Edit/lifecycle fixtures check reservation
floors, explicit reserve/trim, actual-length rent/refunds, single sizing
authority, close, invalid regrowth and same-transaction reinitialization.
Generated and checked SDK CPI fixtures cover signatures, duplicate handles,
readonly/writable privileges, guarded CPI rejection, returned failures,
reacquisition, saved original windows and rollback. Owning sessions run with
safe RefCell guards under both Miri models; those tests manufacture no SDK
account memory and prove no SDK body.

Heap traces bind the exact tested ELF and include native slots, generated SDK
vectors and serialized-map allocations. Their fixture-specific bounds do not
supply an application-wide heap guarantee. The complete default SBF heap is
32 KiB; each load/handoff/on-demand attempt and application allocation counts.
Separate raw/native/demo program sizes have different instruction inventories
and cannot establish adapter overhead by subtraction.

## Trust boundaries

| Boundary | Assumption and evidence limit |
| --- | --- |
| Rust, core, alloc | Correct safe slice/integer/UTF-8/fallible Vec contracts, borrowing and panic semantics. Both Miri models support host integration; allocation failures use safe injected controls. |
| Verus, vstd, SMT | Correct verifier, erasure and solver plus the pinned primitive contracts. The production source is checked without proof escapes. Negative queries and witness pins detect specified gaps. |
| rustc, LLVM, SBF tools | Correct macro expansion, compilation and lowering. Digests bind source/ELFs; machine code is not proved. Fresh/cached stack controls require fatal frame diagnostics at 4,096 bytes. |
| Kani, CBMC | Correct symbolic execution/solver within each inventoried width/unwind bound. Model-checking exclusions above remain explicit. |
| SDK account guards | Genuine runtime memory, shared/exclusive exclusion, copied handle identity, rejection-before-write resize and safe close/lamport APIs. Host tests do not manufacture SDK account memory. |
| Owning native wrapper | Invocation memory validity, private persistent copied-handle slots, no escaping callback references, correct terminal and reacquisition transitions. [Wrapper trust](ASSURANCE.md#trust-boundaries) records the boundary. |
| Runtime, System Program | Correct serialization, CPI/borrow enforcement, rent, growth accounting and transaction rollback in the pinned cohort. These are runtime tests, not Verus theorems. |
| Schemas, encoders, application | Matching metadata/encoding, semantic field projection, correct application authorization, callback behaviour and approved writer history. Arbitrary custom code and upgrades are outside the functional claim. |

All local production/test/generated roots forbid unsafe code. The isolated
competitor workspace is excluded and cannot enter a published crate's dependency
graph. Dependency implementation correctness remains a trust boundary.
Authenticated lazy loads check the owner, identity and envelope, then validate
paths as they are touched. Raw writes and external handoffs require a checked
load. A successful `verify()` preserves poison. After a
destructive error, propagate failure or perform successful explicit checked
recovery; dropping a volatile flag is not recovery.

## Measured verification capacity

The [feature-off](verification/results/policy/verify-capacity-off.log) and
[feature-on](verification/results/policy/verify-capacity-on.log) receipts measure PolicyState:
a fixed bool, `"ok"`, two bool-valued map entries and a list of n bools.
Each cell below is v0/v3 CU on the pinned runtime. Delta subtracts matched
lazy load CU from the whole instruction.

| n | Feature off whole CU | Feature off delta CU | Feature on whole CU | Feature on delta CU |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 2506/2446 | 2056/1998 | 4871/4753 | 2058/2000 |
| 1 | 2583/2522 | 2133/2074 | 5025/4905 | 2135/2076 |
| 10 | 2790/2729 | 2340/2281 | 5439/5319 | 2342/2283 |
| 100 | 4860/4799 | 4410/4351 | 9579/9459 | 4412/4353 |
| 1000 | 25560/25499 | 25110/25051 | 50979/50859 | 25112/25053 |

With the feature on, the fixture scans at load and again at explicit verification. These fixture sizes
establish neither a maximum affordable account size nor a general per-byte
rate. Wider keys, long strings and nested collections need their own budgets.

## Corrupted-byte QA evidence

The retained receipt binds the source, corpus and runtime artifacts for the
checks below. [Corrupted-byte receipt](verification/results/policy/corrupt-qa.log)

<!-- assurance-qa:start -->
- 8 production negative controls rejected; 6 include a named replay with a digest.
- `policy_read` and `policy_edit` each had a 300-second budget, started from the committed corpus and recorded zero crashes or timeouts.
- Replay corpus: `policy_read` 54 tapes; `policy_edit` 55 tapes in each feature/architecture replay.
- Feature off, v0: 324 directed seed tapes.
- Feature off, v3: 324 directed seed tapes.
- Feature on, v0: 324 directed seed tapes.
- Feature on, v3: 324 directed seed tapes.
<!-- assurance-qa:end -->

The host targets `validate` and `edit` each had a 300-second budget, started
from the committed corpus and completed with zero crashes or timeouts.
[Host fuzz receipt](verification/results/fuzz/smoke.log)

Inputs are bounded to 65,536 bytes and 32 operations per root, one worker per
read/edit job. An independent valid-root model checks success, error kinds,
every byte and poison. Checked loads must reject invalid roots. Replay
includes reopen and self-CPI reopen across lazy/checked policy, both feature
states, both SBF architectures and configured memory mappings. Some controls
are host-only or SDK/host policy checks; their receipt states that distinction.
The QA exploration uses a labelled CU bound above the production budget; production acceptance
meters and the 4,096-byte SBF frame limit still apply. Successful lazy
operations may leave unvisited corrupted bytes intact.

## Tool cohort

| Component | Cohort |
| --- | --- |
| Host Rust | 1.97.1 |
| Minimum supported Rust | 1.89.0, edition 2021 |
| Verus | 0.2026.10.18.7d82994, commit `7d829946231d6413a398f6e4160cdfe951261dad`, isolated Rust 1.99.1 |
| vstd | 0.0.0-2026-10-18-0147, default features disabled |
| Kani / CBMC | 0.68.0 / 6.11.0, nightly-2026-08-21 |
| Miri | nightly-2025-12-08, both alias models, strict provenance |
| SBF | cargo-build-sbf 4.0.0, platform-tools v1.53, LLVM 20.1.7, v0/v3 |
| Runtime | Mollusk/bencher 0.16.2, Agave 4.3.0 |
| Account runtime SDK | Pinocchio 0.11.2, account-view 2.0.0 with copy, System CPI 0.6.1 |
| Native adapter SDK | Exact git revision in the unpublished adapter's manifest/lockfile |
| Fuzz | cargo-fuzz 0.12.0, libfuzzer-sys 0.4.13; SDK-backed ASan jobs use host Rust 1.97.1 |
| IDE | Standalone rust-analyzer 0.3.3081, host Rust 1.97.1 |

Manifests use bare semver requirements with caret semantics; lockfiles bind
resolved dependency versions. Tool wrappers enforce the verification/compiler
cohort independently. [Toolchain receipts](verification/results/toolchains/)
record availability, same-source smoke checks and binary/fixture hashes.
Smoke checks establish tool operation; they do not prove production containers.

## Reproduce

Use the tools in the cohort table from `PATH` and run these commands from the
repository root. The scripts check the tool versions used by each command.

| Evidence | Command |
| --- | --- |
| Host and documentation tests | `bash scripts/reproduce.sh test` |
| Production byte kernel and root theorems | `bash scripts/reproduce.sh verus` |
| Bounded harnesses and the map key identity proof | `bash scripts/reproduce.sh kani` |
| Host alias models and strict provenance | `bash scripts/reproduce.sh miri` |
| SBF stack checks and Mollusk runtime tests | `bash scripts/reproduce.sh sbf` |
| Fuzz targets | `bash scripts/reproduce.sh fuzz validate` or `bash scripts/reproduce.sh fuzz edit` |
| Instruction measurements | `bash scripts/reproduce.sh cu` |
| Comparison measurements | `bash scripts/reproduce.sh cu compare` |

CI runs formatting, Clippy, the workspace and doctests on Rust 1.97.1,
the minimum Rust build, package checks for `blubber-core` and `blubber-derive`,
and local link checks. A second job runs Verus on `blubber-core` with
`--no-cheating`.

The release gate also runs Kani, both Miri models, SBF and CU, mutation controls
and fuzzing. The private gate runner enforces the published inventories.
The commands above rerun the public proof, model, runtime and measurement
inputs. Mutation controls run in the release gate. Their inventory records
the changed condition and the expected rejection.

The three library packages exclude the unpublished archive test oracle.
The adapter and the support, program and test crates are unpublished.
