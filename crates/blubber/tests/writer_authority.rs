//! Downstream authority checks, including a renamed library dependency.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[test]
#[cfg(not(miri))]
fn downstream_writer_authority() {
    use std::{fs, path::Path, process::Command};

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let fixture = root.join(".tmp/writer-authority");
    for member in ["foreign", "consumer"] {
        fs::create_dir_all(fixture.join(member).join("src")).unwrap();
        fs::write(fixture.join(member).join("Cargo.toml"), format!(
            "[package]\nname = {member:?}\nversion = \"0.0.0\"\nedition = \"2021\"\n[dependencies]\nwire = {{ package = \"blubber\", path = {:?} }}\n{}",
            root.join("crates/blubber"),
            if member == "consumer" { "foreign = { path = \"../foreign\" }\ncore_wire = { package = \"blubber-core\", path = \"../../../crates/blubber-core\" }\n" } else { "" },
        )).unwrap();
    }
    fs::write(
        fixture.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"foreign\", \"consumer\"]\n",
    )
    .unwrap();
    fs::write(
        fixture.join("foreign/src/lib.rs"),
        r#"
        #![no_std]
        #![forbid(unsafe_code)]
        #![deny(warnings, missing_docs)]
        //! Schemas owned by a separate crate.
        #[wire::schema(account)] pub struct Flag { pub value: bool }
        #[wire::schema(account)] pub struct Counter { pub value: u8 }
        #[wire::schema] pub struct Generic<T> { pub value: T }
        #[wire::schema(account)] pub struct Nested { pub child: Generic<bool> }
    "#,
    )
    .unwrap();
    let cases = [
        ("typed", r#"pub fn run(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut root = foreign::Flag::load_mut(store)?;
            root.value_mut().set(true)?;
            root.commit()?;
            Ok(())
        }"#, None),
        ("generic", r#"pub fn run(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut root = foreign::Nested::load_mut(store)?;
            root.child_mut().value_mut().set(false)?;
            root.commit()?;
            Ok(())
        }"#, None),
        ("explicit_raw", r#"pub fn run(store: &mut dyn wire::Resize) {
            wire::raw::backend(store).bytes_mut().fill(0);
            let _ = core_wire::raw::write_u32(&mut [0; 4], 0, 1);
        }"#, None),
        ("commit", "pub fn bad(s: &mut dyn wire::Resize) { let _ = s.commit(12); }", Some("no method named `commit`")),
        ("poison", "pub fn bad(s: &mut dyn wire::Resize) { s.set_poisoned(false); }", Some("no method named `set_poisoned`")),
        ("bytes", "pub fn bad(s: &mut dyn wire::Resize) { s.bytes_mut()[0] = 1; }", Some("no method named `bytes_mut`")),
        ("resize", "pub fn bad(s: &mut dyn wire::Resize) { let _ = s.resize(12); }", Some("no method named `resize`")),
        ("reserve", "pub fn bad(s: &mut dyn wire::Resize) { let _ = s.reserve(12, 4); }", Some("no method named `reserve`")),
        ("trim", "pub fn bad(s: &mut dyn wire::Resize) { let _ = s.shrink_to_fit(12); }", Some("no method named `shrink_to_fit`")),
        ("backend_loan", "pub fn bad(s: &mut dyn wire::Resize) { let _ = s.edit_access(); }", Some("no method named `edit_access`")),
        ("backend_token", "pub fn bad(s: &mut dyn wire::Resize) { let _ = s.__backend(wire::__private::Writer { _private: () }); }", Some("is private")),
        ("bool_as_u8", "pub fn bad(s: wire::__private::Projection<'_, bool>) { let _ = wire::ScalarMut::<u8>::from_edit(s); }", Some("mismatched types")),
        ("foreign_schema", "pub fn bad(s: wire::__private::Projection<'_, foreign::Flag>) { let _ = <foreign::Counter as wire::Schema>::project(s); }", Some("mismatched types")),
        ("foreign_field", "pub fn bad(mut s: wire::__private::Projection<'_, foreign::Flag>) { let _: wire::__private::Projection<'_, u8> = s.field::<0>(); }", Some("mismatched types")),
        ("foreign_mapping", "impl wire::__private::ProjectField<0> for foreign::Flag { type Field = u8; const FIXED: Option<(u16,u32)> = Some((0,0)); const VARIABLE: u16 = 0; }", Some("only traits defined in the current crate")),
        ("forge_selection", "pub fn bad(e: wire::raw::Edit<'_>) { let _ = wire::__private::Projection::<bool>::new(e); }", Some("is private")),
        ("raw_edit", "pub fn bad(e: wire::raw::Edit<'_>) { let _ = <bool as wire::Schema>::project(e); }", Some("mismatched types")),
        ("rewrite_admission", "pub fn bad() { let _ = wire::__private::prepare_list::<u8>; }", Some("cannot find value")),
        ("rewrite_destination", "pub fn bad() { let _ = wire::__private::rewrite_collection::<()>; }", Some("cannot find value")),
        ("slot_writer", "pub fn bad() { let _ = wire::__private::encode_rewrite_slot::<u8>; }", Some("cannot find value")),
        ("core_destination", "pub fn bad() { let _ = core_wire::begin_apply; }", Some("cannot find value `begin_apply`")),
        ("core_word", "pub fn bad() { let _ = core_wire::write_u32; }", Some("is private")),
        ("foreign_identity_init", "pub fn bad(s: &mut dyn wire::Resize) { let _ = wire::__private::Projection::<foreign::Flag>::initialize(s, foreign::Counter::DISCRIMINATOR, foreign::FlagInit { value: true }); }", Some("takes 2 arguments")),
        ("foreign_identity_load", "pub fn bad(s: &mut dyn wire::Resize) { let _ = wire::__private::Projection::<foreign::Flag>::load(s, foreign::Counter::DISCRIMINATOR); }", Some("takes 1 argument")),
        ("unbound_identity", "pub fn bad(s: &mut dyn wire::Resize) { let _ = wire::__private::Projection::<u8>::initialize(s, 2); }", Some("trait bounds were not satisfied")),
        ("measured_foreign_schema", "pub fn bad(s: &mut dyn wire::Resize, m: wire::__private::MeasuredInit<'_, foreign::Flag>) { let _ = wire::__private::Projection::<foreign::Counter>::initialize_measured(s, m); }", Some("mismatched types")),
        ("measured_foreign_identity", "pub fn bad(s: &mut dyn wire::Resize, m: wire::__private::MeasuredInit<'_, foreign::Flag>) { let _ = wire::__private::Projection::<foreign::Flag>::initialize_measured(s, foreign::Counter::DISCRIMINATOR, m); }", Some("takes 2 arguments")),
        ("measured_private_input", "pub fn bad(m: wire::__private::MeasuredInit<'_, foreign::Flag>) { let _ = m.prepared; }", Some("is private")),
        ("owned_mapping", r#"
            #[wire::schema] pub struct Local { pub flag: bool }
            impl wire::__private::ProjectField<0> for Local {
                type Field = u8;
                const FIXED: Option<(u16,u32)> = Some((0,0));
                const VARIABLE: u16 = 0;
            }
        "#, Some("conflicting implementations")),
        ("extra_mapping", r#"
            #[wire::schema] pub struct Local { pub flag: bool }
            impl wire::__private::ProjectField<1> for Local {
                type Field = u8;
                const FIXED: Option<(u16,u32)> = Some((0,0));
                const VARIABLE: u16 = 0;
            }
            pub fn bad(mut p: wire::__private::Projection<'_, Local>) {
                let _ = <u8 as wire::Schema>::project(p.field::<1>()).set(2);
            }
        "#, Some("field ordinal is outside schema")),
        ("packed_mapping", r#"
            #[derive(Clone, Copy, wire::Fixed)] pub struct Packed { pub flag: bool }
            impl wire::__private::ProjectField<0> for Packed {
                type Field = u8;
                const FIXED: Option<(u16,u32)> = Some((0,0));
                const VARIABLE: u16 = 0;
            }
            pub fn bad(mut p: wire::__private::Projection<'_, Packed>) {
                let _ = <u8 as wire::Schema>::project(p.field::<0>()).set(2);
            }
        "#, Some("schema does not grant field projections")),
    ];
    for (name, source, expected) in cases {
        fs::write(
            fixture.join("consumer/src/lib.rs"),
            format!("#![no_std]\n#![forbid(unsafe_code)]\n#![deny(warnings)]\n{source}\n"),
        )
        .unwrap();
        let command = if matches!(name, "extra_mapping" | "packed_mapping") {
            "build"
        } else {
            "check"
        };
        let output = Command::new(env!("CARGO"))
            .args([command, "--offline", "--color", "never", "--manifest-path"])
            .arg(fixture.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", root.join("target"))
            .output()
            .unwrap();
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(fixture.join(format!("{name}.log")), &log).unwrap();
        assert_eq!(output.status.success(), expected.is_none(), "{name}: {log}");
        if let Some(expected) = expected {
            assert!(log.contains(expected), "{name}: {log}");
        }
    }
}
