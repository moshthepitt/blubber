//! Prepares standalone doctests from the user guides.

#![forbid(unsafe_code)]

use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env::var("OUT_DIR")?);
    for (name, source) in [
        ("readme.md", "../../README.md"),
        ("guide.md", "../../docs/guide.md"),
        ("recipes.md", "../../docs/recipes.md"),
        ("trust-model.md", "../../docs/trust-model.md"),
        ("performance.md", "../../docs/performance.md"),
    ] {
        println!("cargo:rerun-if-changed={source}");
        let source = fs::read_to_string(source)?;
        let mut text = String::with_capacity(source.len());
        let mut excerpt = false;
        for line in source.split_inclusive('\n') {
            if excerpt && line == "```rust\n" {
                // The excerpt registry names the test that runs each region.
                text.push_str("```rust,ignore\n");
            } else {
                text.push_str(line);
            }
            excerpt = line.starts_with("<!-- code:") && line.ends_with(" -->\n");
        }
        fs::write(out.join(name), text)?;
    }
    Ok(())
}
