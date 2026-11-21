# Contributing

## Report a problem

If you find a security issue, use [`SECURITY.md#report-privately`](SECURITY.md#report-privately) so a fix can ship before the bug is public.

For an ordinary bug, use the [Bug form](https://github.com/moshthepitt/blubber/issues/new?template=bug.yml). Include the crate version, feature flags, SBF architecture where it applies, a minimal schema and steps, expected and observed results, and the account bytes, so I can reproduce the failure without guessing your setup.

For a gap in a proof or test claim, use the [Proof or assurance gap form](https://github.com/moshthepitt/blubber/issues/new?template=assurance.yml). Quote the exact claim from [`ASSURANCE.md#find-a-claim`](ASSURANCE.md#find-a-claim) and give a counterexample or missing case. Proofs cover the byte kernel under stated premises; SDK behaviour, compiler lowering, custom codecs and application authority sit outside them.

## Check a patch

Run these checks before opening a pull request, and paste the commands and results into the description:

```bash
bash scripts/reproduce.sh test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Keep `#![forbid(unsafe_code)]` in crate roots, because the shipped crates stay in safe Rust.

The byte kernel is the low-level code that moves and checks account slices. Verus is the verifier that checks its contracts. If you change the byte kernel, run `bash scripts/reproduce.sh verus` (`cargo verus verify -p blubber-core --locked --fwd-verus-args-to roots -- --no-cheating --rlimit 60`) with `--no-cheating` kept, so the verifier rejects proof escapes. See [`ASSURANCE.md#reproduce`](ASSURANCE.md#reproduce).

To reproduce a byte or edit mismatch, replay a fuzz input via [`fuzz/README.md#run-a-target`](fuzz/README.md#run-a-target) or run the differential suite using [its README](crates/blubber/tests/support/differential/README.md). A differential test feeds the same edits to an independent owned model and to production storage, then compares the bytes.

## How patches land

I build Blubber by directing coding agents and checking their work with proofs, model checks, fuzzing and runtime tests.

The public repository is exported from my working repository. Accepted pull requests enter the working repository and appear in the next public export with your attribution.
