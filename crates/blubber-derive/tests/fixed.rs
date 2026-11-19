//! Compile real derive consumers, including unsupported fields and const checks.

#![forbid(unsafe_code)]

#[path = "support/consumer.rs"]
mod consumer;
use consumer::{repo_root, Consumer};

fn check(consumer: &Consumer, name: &str, source: &str, expected: Option<&str>, depth16: bool) {
    consumer.check_source(
        name,
        "src/lib.rs",
        source,
        expected,
        if depth16 { &["depth16"] } else { &[] },
    );
}

#[test]
fn fixed_consumers_and_field_diagnostics() {
    let root = repo_root(env!("CARGO_MANIFEST_DIR"));
    let consumer = Consumer::new(root, "fixed-derive", format!(
        "[package]\nname = \"blubber-fixed-compile-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [workspace]\n[features]\ndepth16 = [\"blubber/max-depth-16\"]\n\
         [dependencies]\nblubber = {{ path = {:?} }}\n", root.join("crates/blubber")), None);
    let header = "#![no_std]\n#![forbid(unsafe_code)]\n";
    let derive = "#[derive(Clone, Copy, blubber::Fixed)]\n";
    let cases = [
        (
            "named",
            "pub struct Native { pub enabled: bool, pub value: u16 }",
            None,
        ),
        (
            "tuple",
            "pub struct Tuple(u16);",
            Some("Fixed requires a nonempty struct with named fields"),
        ),
        (
            "unit",
            "pub struct Unit;",
            Some("Fixed requires a nonempty struct with named fields"),
        ),
        (
            "empty",
            "pub struct Empty {}",
            Some("Fixed requires a nonempty struct with named fields"),
        ),
        (
            "enum",
            "pub enum Choice { A }",
            Some("Fixed requires a struct with named fields"),
        ),
        (
            "union",
            "pub union Choice { pub a: u8, pub b: u16 }",
            Some("Fixed requires a struct with named fields"),
        ),
        (
            "generic",
            "pub struct Generic<T> { pub value: T }",
            Some("Fixed requires a nongeneric struct"),
        ),
        (
            "lifetime",
            "pub struct Borrowed<'a> { pub value: &'a u8 }",
            Some("Fixed requires a nongeneric struct"),
        ),
        (
            "const_generic",
            "pub struct Generic<const N: usize> { pub value: [u8; N] }",
            Some("Fixed requires a nongeneric struct"),
        ),
        (
            "where_clause",
            "pub struct Bounded where u8: Copy { pub value: u8 }",
            Some("Fixed requires a nongeneric struct"),
        ),
        (
            "variable",
            "pub struct Variable { pub value: blubber::Str }",
            Some("blubber::Str: blubber::Fixed"),
        ),
        (
            "option",
            "pub struct Optional { pub value: Option<u8> }",
            Some("Option<u8>: blubber::Fixed"),
        ),
        (
            "float",
            "pub struct Float { pub value: f64 }",
            Some("f64: blubber::Fixed"),
        ),
        (
            "native_width",
            "pub struct NativeWidth { pub value: usize }",
            Some("usize: blubber::Fixed"),
        ),
        (
            "empty_array",
            "pub struct EmptyArray { pub value: [u8; 0] }",
            Some("fixed arrays must have positive width"),
        ),
        (
            "oversized_array",
            "pub struct Oversized { pub value: [u8; 4294967296] }",
            Some("fixed array width exceeds u32"),
        ),
        (
            "sum_overflow",
            "pub struct Overflow { pub value: [u8; 4294967295], pub extra: u8 }",
            Some("Fixed wire width exceeds u32"),
        ),
    ];
    for (name, declaration, expected) in cases {
        check(
            &consumer,
            name,
            &format!("{header}{derive}{declaration}\n"),
            expected,
            false,
        );
    }
    check(
        &consumer,
        "missing_copy",
        &format!("{header}#[derive(blubber::Fixed)]\npub struct Native {{ pub value: u8 }}\n"),
        Some("Native: Copy"),
        false,
    );
    check(
        &consumer,
        "native_layout",
        &format!(
            "{header}#[repr(C, align(32))]\n{derive}\
        pub struct Aligned {{ pub flag: bool, pub value: u64 }}\n\
        #[repr(C, packed)]\n{derive}pub struct Packed {{ pub value: u32, pub flag: bool }}\n\
        const _: () = {{ assert!(<Aligned as blubber::Fixed>::SIZE == 9);\n\
        assert!(core::mem::size_of::<Aligned>() == 32);\n\
        assert!(<Packed as blubber::Fixed>::SIZE == 5); }};\n"
        ),
        None,
        false,
    );

    let mut nested = header.to_owned();
    for level in 1..=7 {
        let child = if level == 1 {
            "u8".to_owned()
        } else {
            format!("Level{}", level - 1)
        };
        nested.push_str(&format!(
            "{derive}pub struct Level{level} {{ pub value: {child} }}\n"
        ));
    }
    check(&consumer, "depth8", &nested, None, false);
    nested.push_str(&format!(
        "{derive}pub struct Level8 {{ pub value: Level7 }}\n"
    ));
    check(
        &consumer,
        "depth9",
        &nested,
        Some("Fixed schema depth exceeds MAX_DEPTH; consider max-depth-16"),
        false,
    );
    check(&consumer, "depth9_with_feature", &nested, None, true);
    for level in 9..=15 {
        nested.push_str(&format!(
            "{derive}pub struct Level{level} {{ pub value: Level{} }}\n",
            level - 1,
        ));
    }
    check(&consumer, "depth16", &nested, None, true);
    nested.push_str(&format!(
        "{derive}pub struct Level16 {{ pub value: Level15 }}\n"
    ));
    check(
        &consumer,
        "depth17",
        &nested,
        Some("Fixed schema depth exceeds MAX_DEPTH; consider max-depth-16"),
        true,
    );
}
