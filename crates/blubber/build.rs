//! Reproducible test schemas; generated code is never linked into the library.

#![forbid(unsafe_code)]

#[path = "tests/support/differential/generate.rs"]
mod generate;

fn main() {
    println!("cargo:rerun-if-changed=tests/support/differential/generate.rs");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    std::fs::write(output.join("differential.rs"), generate::source())
        .expect("write differential schema fixtures");
}
