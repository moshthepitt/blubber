//! Replay authenticated SDK histories against each freshly built SBF artifact.
#![forbid(unsafe_code)]
#![cfg(feature = "sdk-fuzz")]
use blubber_fuzz_replay as blubber_fuzz;
fn corpus_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus")
}
include!("../../../fuzz/tests/support/corrupt.rs");
