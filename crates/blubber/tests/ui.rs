//! Compile public API contracts under strict lints and real borrow checking.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "../../blubber-derive/tests/support/consumer.rs"]
mod consumer;
use consumer::{repo_root, Consumer};

#[test]
fn public_api_compile_contracts() {
    let root = repo_root(env!("CARGO_MANIFEST_DIR"));
    let directory = root.join("crates/blubber/tests/ui");
    let inventory = include_str!("ui/cases.tsv");
    let features = [
        cfg!(feature = "alloc").then_some("alloc"),
        cfg!(feature = "pinocchio").then_some("pinocchio"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let readers = Consumer::new(root, "readers-ui", format!(
        "[package]\nname = \"schema-read-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n\
         [dependencies]\nwire = {{ package = \"blubber\", path = {:?}, features = {:?} }}\n\
         foreign = {{ path = \"foreign\" }}\n", root.join("crates/blubber"), features), None);
    readers.write(
        "foreign/Cargo.toml",
        &format!(
            "[package]\nname = \"foreign\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [dependencies]\nwire = {{ package = \"blubber\", path = {:?} }}\n",
            root.join("crates/blubber")
        ),
    );
    readers.write("foreign/src/lib.rs", "#![no_std]\n#![forbid(unsafe_code)]\n#![deny(warnings, missing_docs)]\n//! Foreign consumer.\n#[wire::schema] pub struct Record { pub text: wire::Str }\n");
    let mut count = readers.check_ui(&directory, inventory, "readers");
    let metadata = Consumer::new(root, "metadata-ui", format!(
        "[package]\nname = \"account-schema-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n\
         [dependencies]\nwire = {{ package = \"blubber\", path = {:?}, default-features = false, features = [\"derive\"] }}\n", root.join("crates/blubber")), None);
    count += metadata.check_ui(&directory, inventory, "metadata");
    let tree = metadata.dependency_tree();
    assert!(
        !tree.lines().skip(1).any(|line| ["pinocchio", "solana-"]
            .iter()
            .any(|sdk| line.starts_with(sdk))),
        "{tree}"
    );
    let scoped = Consumer::new(root, "scoped-ui", format!(
        "[package]\nname = \"scoped-point-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n\
         [dependencies]\nblubber = {{ path = {:?}, features = {:?} }}\n", root.join("crates/blubber"), features), None);
    count += scoped.check_ui(&directory, inventory, "scoped");
    eprintln!("UI contracts checked: {count}");
}
