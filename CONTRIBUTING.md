# Contributing

If you hit a problem, a failing test with a small schema is a good place to
start. Include the feature flags and, for an SBF run, the architecture.
If an assurance claim does not hold, include the claim and the counterexample.

One maintainer directs coding agents and checks their work with proofs, model
checks, fuzzing and runtime tests. This repository is an export of the working
repository. Its history starts at 0.1.0.

Pull requests are welcome. Accepted patches go into the working repository and
appear in the next export with attribution. Keep the code safe Rust. For a
kernel change, run Verus with `--no-cheating`.

The [reproduction commands](ASSURANCE.md#reproduce) describe the tool versions
and checks.
