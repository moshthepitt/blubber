# Working on Blubber

## Use Blubber in a program

If you are writing or editing a Solana program that uses Blubber, read [`docs/agents.md#find-the-task`](docs/agents.md#find-the-task).

## Check a library change

Keep `#![forbid(unsafe_code)]` in crate roots, because Blubber ships with zero `unsafe` in its source.

Run these checks before sending a patch:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `bash scripts/reproduce.sh test`, so locked workspace tests, no-default-feature doc tests, and all-feature doc-example tests pass.

The byte kernel is the low-level byte code in `blubber-core`. Verus checks its byte operations under their stated premises; see [`ASSURANCE.md#production-proofs-and-bounds`](ASSURANCE.md#production-proofs-and-bounds). If you edit `blubber-core`, run `bash scripts/reproduce.sh verus` (`cargo verus verify -p blubber-core --locked --fwd-verus-args-to roots -- --no-cheating --rlimit 60`). Keep `--no-cheating`, or an unproven assumption can pass. See [`ASSURANCE.md#reproduce`](ASSURANCE.md#reproduce).

## Report the results

Include the commands you ran and their results in the pull request, so a reviewer can see what passed. See [`CONTRIBUTING.md#check-a-patch`](CONTRIBUTING.md#check-a-patch).
