//! Generate IDL with the pinned Anchor builder, including resolution metadata.
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let program = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let idl = anchor_lang_idl::build::IdlBuilder::new()
        .program_path(program.to_path_buf())
        .cargo_args(vec!["--lib".into(), "--locked".into()])
        .build()?;
    let destination = std::env::args().nth(1).ok_or("expected IDL destination")?;
    std::fs::write(destination, serde_json::to_string_pretty(&idl)? + "\n")?;
    Ok(())
}
