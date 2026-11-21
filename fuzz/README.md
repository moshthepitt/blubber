# Fuzzing

Four fuzz targets feed corrupted bytes and edit sequences into Blubber. An independent owned implementation computes the expected values and bytes as a ground truth model. Each differential test feeds the same inputs to the model and to production storage, then compares the results.

## Choose a target

Two targets run on the host against slice and vector storage. Two run through SBF across two binary architectures, v0 and v3. SBF is the on-chain build target, and the SDK runs use real borrow guards and runtime memory profiles including direct mapping.

- [`validate`](fuzz_targets/validate.rs) - picks one of 32<!-- num:fuzz.host.schemas --> representative schemas, probes a byte offset and compares checked validity against the model for raw bytes and for bytes wrapped in a valid header. A checked load scans the complete used encoding against the schema before returning a view or running an edit callback.
- [`edit`](fuzz_targets/edit.rs) - runs up to 32<!-- num:fuzz.history.commands --> edit commands of 8<!-- num:fuzz.history.command_bytes --> bytes each using up to 8,192<!-- num:fuzz.host.initial_bytes --> input bytes for the initial state. On valid roots it compares full logical values, used bytes and the untouched regions before the write, in sibling fields, in spare capacity and in any shrunk tail. On invalid roots it runs fallible selected reads and edits and fails on any panic.
- [`policy_read`](fuzz_targets/policy_read.rs) - runs authenticated reads on v0 and v3 SBF fixtures across memory profiles and compares the return status and read digest with host replay.
- [`policy_edit`](fuzz_targets/policy_edit.rs) - runs up to 32<!-- num:fuzz.history.commands --> authenticated edit commands of 8<!-- num:fuzz.history.command_bytes --> bytes each with real guards on v0 and v3 SBF fixtures. It compares return status, read digest, session poison and full physical bytes before `Drop` against host replay, and checks decodable roots against the model. A detected fault that poisons the session blocks ordinary typed reuse, and a closed session has ended its borrow.

## Run a target

Run every target from the repository root with [`scripts/reproduce.sh`](../scripts/reproduce.sh). Every fuzz build needs a host C++ compiler, C++ headers and a linker library, and compiles with AddressSanitizer, debug assertions and overflow checks enabled. Both host and SDK lanes require `cargo-fuzz` 0.12.0<!-- num:fuzz.cargo_fuzz.version --> and `libfuzzer-sys` 0.4.13<!-- num:fuzz.libfuzzer_sys.version -->.

Host targets pin Rust `nightly-2025-12-08`<!-- num-code:"{{num:fuzz.host.rust_nightly}}" -->:

```bash
bash scripts/reproduce.sh fuzz validate
bash scripts/reproduce.sh fuzz edit
```

SDK targets pin Rust 1.97.1<!-- num:fuzz.sdk.rust_version --> with `RUSTC_BOOTSTRAP=1`. The wrapper checks `cargo-build-sbf` and `platform-tools` versions, then builds corrupt-policy SBF fixtures with `verify-on-load` off for v0 and v3 before starting the fuzzer:

```bash
bash scripts/reproduce.sh fuzz policy_read
bash scripts/reproduce.sh fuzz policy_edit
```

The wrapper copies `fuzz/corpus/<target>` into a fresh working directory `corpus-<target>.*` under `$CARGO_TARGET_DIR/reproduce`, which defaults to `target/reproduce`. It writes crash artifacts there, so the committed corpus stays clean. Each wrapper job has a 300<!-- num:fuzz.wrapper.time_seconds -->-second time budget with a 65,536<!-- num:fuzz.wrapper.input_bytes -->-byte input cap and a 30<!-- num:fuzz.wrapper.timeout_seconds -->-second per-input timeout. Host jobs use seed 34<!-- num:fuzz.wrapper.host.seed --> and a 2,048<!-- num:fuzz.wrapper.host.rss_mb --> MB RSS limit; SDK jobs use seed 217<!-- num:fuzz.wrapper.sdk.seed --> and a 4,096<!-- num:fuzz.wrapper.sdk.rss_mb --> MB RSS limit.

To run a longer host campaign, reuse the scratch corpus directory and raise `-max_total_time` with `cargo +nightly-2025-12-08 fuzz run <target> <scratch-corpus> --fuzz-dir fuzz -- -max_total_time=<seconds>`<!-- num-code:"cargo +{{num:fuzz.host.rust_nightly}} fuzz run <target\u003e <scratch-corpus\u003e --fuzz-dir fuzz -- -max_total_time=<seconds\u003e" -->.

## Replay or regenerate a case

You do not need `libFuzzer` or the nightly compiler to replay the committed host corpus, seed mutations and generated edit histories. The default workspace member [`tests/fuzz-replay`](../tests/fuzz-replay/) runs them deterministically:

```bash
bash scripts/reproduce.sh test
# or directly:
cargo +1.97.1 test --workspace --locked
```
<!-- num-fence:{"3": "cargo +{{num:fuzz.sdk.rust_version}} test --workspace --locked"} -->

[`fuzz/tests/replay.rs`](tests/replay.rs) shares the same oracle and replay source [`fuzz/tests/support/replay.rs`](tests/support/replay.rs).

To regenerate the directed host seeds, run:

```bash
cargo +1.97.1 run --manifest-path fuzz/Cargo.toml --example seed --locked
```
<!-- num-fence:{"1": "cargo +{{num:fuzz.sdk.rust_version}} run --manifest-path fuzz/Cargo.toml --example seed --locked"} -->

To regenerate the SDK seeds without colliding with workspace build artifacts, set a separate target directory:

```bash
CARGO_TARGET_DIR=target/fuzz-seed cargo +1.97.1 run --manifest-path fuzz/Cargo.toml --example seed --locked --features sdk-fuzz
```
<!-- num-fence:{"1": "CARGO_TARGET_DIR=target/fuzz-seed cargo +{{num:fuzz.sdk.rust_version}} run --manifest-path fuzz/Cargo.toml --example seed --locked --features sdk-fuzz"} -->

When a fuzz run finds a failure, reduce it to a minimal input committed under `fuzz/corpus/` and add a named deterministic test before recording a clean smoke run. Leave the rest of the evolving scratch corpus in `target/reproduce`.

## Check what the run covered

With `verify-on-load` off, a lazy load checks the account header up front and checks only the fields you touch. When a target runs a fallible read or edit on an invalid root, corrupt bytes outside the touched path can stay untouched. Earlier edits completed before a callback error also remain in storage unless the instruction rolls back or the caller restores the bytes.

Fallible iterator probes cap visits at 256<!-- num:fuzz.host.iteration_items --> items on the host and 32<!-- num:fuzz.sdk.iteration_items --> items in the SDK targets so a large corrupt count cannot stall a single input. Strict validation still checks the complete selected region.

The SDK targets cover checked rejection, lazy touched faults, interrupted encoders, partial progress inside callbacks, CPI handoff and reopen, and checked recovery. Reopening starts a session with a fresh in-memory poison flag and leaves any underlying corruption subject to the selected load policy. The input layouts are in [`src/lib.rs`](src/lib.rs) for the host targets and [`src/corrupt.rs`](src/corrupt.rs) for the SDK targets.

The host fixtures test 32<!-- num:fuzz.host.schemas --> representative schemas up to depth 8<!-- num:schema.default.depth -->. Host edit and SDK histories are capped at 32<!-- num:fuzz.history.commands --> commands. Because the schemas and histories are bounded, a passing timed run is bounded evidence rather than a proof over every schema or history length. See [`verification/results/fuzz/smoke.log`](../verification/results/fuzz/smoke.log) for the recorded host receipt and [`verification/results/policy/corrupt-qa.log`](../verification/results/policy/corrupt-qa.log) for the recorded v0 and v3 SDK receipt. [`ASSURANCE.md#corrupted-byte-qa-evidence`](../ASSURANCE.md#corrupted-byte-qa-evidence) and [`ASSURANCE.md#reproduce`](../ASSURANCE.md#reproduce) cover the proof boundaries and reproduction contract.
