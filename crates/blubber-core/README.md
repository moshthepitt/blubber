# blubber-core

Checked format-1 byte operations, layouts, validation and encoding kernels.
This `no_std` crate forbids unsafe code. Most applications should use the
`blubber` typed facade; kernel raw operations carry narrower local contracts.

```toml
[dependencies]
blubber-core = "0.1.0"
```

Read [SPEC.md](https://github.com/moshthepitt/blubber/blob/v0.1.0/SPEC.md) for wire contracts and
[ASSURANCE.md](https://github.com/moshthepitt/blubber/blob/v0.1.0/ASSURANCE.md) for proof scope and assumptions.
