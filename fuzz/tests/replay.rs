//! Replay seeds and corrupting mutations through exactly the fuzz target checks.
#![forbid(unsafe_code)]

fn corpus_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus")
}

include!("support/replay.rs");
