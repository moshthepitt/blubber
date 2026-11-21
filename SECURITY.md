# Security policy

## Report privately

Report security issues through the repository's GitHub Security tab instead of opening a public issue. A public issue exposes the flaw before a fix is ready.

## Describe the failure

Include the `blubber` version and a small case that reproduces the bug. Report any of the following:

- A checked load that panics or returns a wrong result. A checked load scans the whole used encoding against the schema before returning a view or calling your edit callback.
- An edit that succeeds but turns a valid encoding into an invalid one.
- A broken claim in [`ASSURANCE.md#find-a-claim`](ASSURANCE.md#find-a-claim).

Your handler still checks who may act, and a lazy load skips untouched bytes by design; see [`docs/trust-model.md#choose-a-load`](docs/trust-model.md#choose-a-load).

## Release support and review

This policy covers the latest `0.1.x` release. I handle reports myself on a best-effort basis.

Blubber has not had an external audit yet, though I plan to arrange one. For what the in-tree proofs and tests cover and what stays outside them, see [`ASSURANCE.md#trust-boundaries`](ASSURANCE.md#trust-boundaries).
