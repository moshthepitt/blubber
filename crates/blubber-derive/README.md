# blubber-derive

Procedural macros for Blubber schema markers and packed fixed codecs.
Prefer `blubber`'s default `derive` feature, which re-exports both macros.

```toml
[dependencies]
blubber-derive = "0.1.0"
```

`#[blubber::schema]` accepts nonempty named records with nested type/const
generics. Concrete account roots also generate identity, sizing and lifecycle
helpers. `#[derive(blubber::Fixed)]` encodes copied fixed structs without Rust
padding. The [guide](https://github.com/moshthepitt/blubber/blob/v0.1.0/docs/guide.md#schemas) and
[quickstart](https://github.com/moshthepitt/blubber/blob/v0.1.0/README.md#initialize-edit-and-read) show complete usage.
