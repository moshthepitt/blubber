//! Replay the fuzz library and committed corpus in the ordinary host gate.
#![forbid(unsafe_code)]

use blubber_fuzz_replay as blubber_fuzz;

fn corpus_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus")
}

include!("../../../fuzz/tests/support/replay.rs");
