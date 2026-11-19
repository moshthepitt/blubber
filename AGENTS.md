# Working on Blubber

Using Blubber in a program? Read [the agent guide](docs/agents.md).
Run `bash scripts/reproduce.sh test` for workspace tests and doc examples.
Run `cargo fmt --all -- --check` before sending a patch.
Run `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`.
Write safe Rust and keep `#![forbid(unsafe_code)]` in crate roots.
Run `bash scripts/reproduce.sh verus` after changing `blubber-core`.
Keep `--no-cheating` enabled and read [the proof scope](ASSURANCE.md).
Report the commands and results in your pull request.
