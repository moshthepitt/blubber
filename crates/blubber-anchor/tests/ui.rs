//! Scoped adapter callback and copied constraint field checks.
#![forbid(unsafe_code)]

#[path = "../../blubber-derive/tests/support/consumer.rs"]
mod consumer;
use consumer::{repo_root, Consumer};

#[test]
fn native_anchor_callback_lifetimes_and_constraint_fields() {
    let root = repo_root(env!("CARGO_MANIFEST_DIR"));
    let consumer = Consumer::new(root, "adapter-ui", format!(
        "[package]\nname = \"anchor-load-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n\
         [dependencies]\nwire = {{ package = \"blubber\", path = {:?}, features = [\"derive\", \"account-runtime\"] }}\n\
         adapter = {{ package = \"blubber-anchor\", path = {:?} }}\n", root.join("crates/blubber"), root.join("crates/blubber-anchor")), None);
    let count = consumer.check_ui(
        &root.join("crates/blubber-anchor/tests/ui"),
        include_str!("ui/cases.tsv"),
        "adapter",
    );
    eprintln!("Adapter UI contracts checked: {count}");
}
