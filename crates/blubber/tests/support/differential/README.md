The differential suite compares logical model edits with typed production calls
on 32 schemas, each with a fixed slice and a relocating vector store. The API
commands also compare callback counts, visited indices or keys, entry values
before reuse, caller buffers, partial progress and sticky poison.

`BLUBBER_DIFFERENTIAL_CASES` sets cases per seed and fixture. It defaults to 4.
`BLUBBER_DIFFERENTIAL_SEEDS` accepts comma-separated decimal u64 seeds. Its
default is the four seeds in `differential.rs`. Both knobs preserve deterministic
command generation and shrinking. Failure tapes retain old resolved edits and
new API commands; API commands expand deterministically against each replayed
model state.

For a release candidate, run 10,000 cases per fixture with one seed (320,000
cases across the 32 fixtures, each exercised on both stores):

```sh
BLUBBER_DIFFERENTIAL_CASES=10000 BLUBBER_DIFFERENTIAL_SEEDS=0 \
CARGO_BUILD_JOBS=8 cargo +1.97.1 test -p blubber --features alloc \
    --test differential --locked
```

The extra directed cases run with either setting. They isolate each API route
so removals cannot erase later witnesses, check one-byte-short and exact growth
capacity, and pass pre-existing poison to every route. Separate malformed-count
controls check lazy and checked source admission. The full-root encoding and
native values are independent expectations. Production span resolution and raw
plan equivalence are supplemental assertions.
