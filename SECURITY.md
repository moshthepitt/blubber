# Reporting a problem privately

Open the repository's Security tab on GitHub and start a private report. Include the library version and a small case that reproduces
the problem. Report panics or incorrect results on arbitrary bytes under
checked loading, incorrect bytes after a successful edit, and failures of an
[assurance claim](ASSURANCE.md).

Lazy loads can leave untouched invalid bytes unchecked. Your program is
responsible for checking application authority. See the
[trust model](docs/trust-model.md) for these boundaries.

The latest 0.1.x release is supported. There has been no external audit.
One maintainer handles reports on a best effort basis.
