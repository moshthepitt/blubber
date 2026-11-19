//! Recreates the small checked-in seed set without running libFuzzer.
#![forbid(unsafe_code)]

fn main() -> std::io::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus");
    for (target, seeds) in [
        ("validate", blubber_fuzz::validation_seeds()),
        ("edit", blubber_fuzz::edit_seeds()),
    ] {
        std::fs::create_dir_all(root.join(target))?;
        for (name, bytes) in seeds {
            std::fs::write(root.join(target).join(name), bytes)?;
        }
    }
    // Replay requires every named SDK family here with identical bytes.
    #[cfg(feature = "sdk-fuzz")]
    for (target, edit) in [("policy_read", false), ("policy_edit", true)] {
        std::fs::create_dir_all(root.join(target))?;
        for (name, bytes) in blubber_fuzz::corrupt::corpus(edit) {
            std::fs::write(root.join(target).join(name), bytes)?;
        }
    }
    Ok(())
}
