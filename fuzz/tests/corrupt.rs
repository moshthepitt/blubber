//! Replay exactly the SDK-backed fuzz targets without libFuzzer.
#![forbid(unsafe_code)]
#![cfg(feature = "sdk-fuzz")]
fn corpus_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus")
}
include!("support/corrupt.rs");
