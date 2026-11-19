//! Compile schema consumers under renamed dependencies and strict safe-code lints.

#![forbid(unsafe_code)]

#[path = "support/consumer.rs"]
mod consumer;
use consumer::{repo_root, Consumer};

fn check(consumer: &Consumer, name: &str, source: &str, expected: Option<&str>, depth16: bool) {
    consumer.check_source(
        name,
        "consumer/src/lib.rs",
        source,
        expected,
        if depth16 { &["depth16"] } else { &[] },
    );
}

#[test]
fn schema_input_consumers_and_diagnostics() {
    let root = repo_root(env!("CARGO_MANIFEST_DIR"));
    let consumer = Consumer::new(
        root,
        "schema-input",
        "[workspace]\nresolver = \"2\"\nmembers = [\"consumer\", \"foreign\"]\n".into(),
        Some("schema-input-consumer"),
    );
    consumer.write("consumer/Cargo.toml", &format!(
        "[package]\nname = \"schema-input-consumer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [features]\ndepth16 = [\"wire/max-depth-16\"]\n[dependencies]\n\
         wire = {{ package = \"blubber\", path = {:?} }}\n\
         foreign = {{ package = \"schema-input-foreign\", path = \"../foreign\" }}\n", root.join("crates/blubber")));
    consumer.write(
        "foreign/Cargo.toml",
        &format!(
            "[package]\nname = \"schema-input-foreign\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [dependencies]\nwire = {{ package = \"blubber\", path = {:?} }}\n",
            root.join("crates/blubber")
        ),
    );
    let header = "#![no_std]\n#![forbid(unsafe_code)]\n#![deny(warnings, missing_docs)]\n//! Consumer fixture.\n";
    consumer.write(
        "foreign/src/lib.rs",
        &format!(
            "{header}\
        #[wire::schema] pub struct Foreign {{ pub text: wire::Str, pub code: u16 }}\n\
        /// Foreign native value.\n#[derive(Clone, Copy, wire::Fixed)]\n\
        pub struct Native {{ /// Native field.\npub flag: bool }}\n"
        ),
    );
    let cases = [
        (
            "named",
            "pub struct Named { pub text: wire::Str, pub code: u16 }",
            None,
        ),
        (
            "fixed_only",
            "pub struct Fixed { pub first: u8, pub second: u64 }",
            None,
        ),
        (
            "tuple",
            "pub struct Tuple(u16);",
            Some("schema requires a nonempty struct with named fields"),
        ),
        (
            "unit",
            "pub struct Unit;",
            Some("schema requires a nonempty struct with named fields"),
        ),
        (
            "empty",
            "pub struct Empty {}",
            Some("schema requires 1 through 65535 named fields"),
        ),
        (
            "enum",
            "pub enum Choice { A }",
            Some("schema requires a struct with named fields"),
        ),
        (
            "union",
            "pub union Choice { pub a: u8, pub b: u16 }",
            Some("schema requires a struct with named fields"),
        ),
        (
            "function",
            "pub fn function() {}",
            Some("schema requires a struct with named fields"),
        ),
        (
            "generic",
            "pub struct Generic<T> { pub value: T }",
            None,
        ),
        (
            "lifetime",
            "pub struct Borrowed<'a> { pub value: &'a str }",
            Some("schema markers require static types"),
        ),
        (
            "const_generic",
            "pub struct Generic<const N: usize> { pub value: [u8; N] }",
            None,
        ),
        (
            "where",
            "pub struct Bounded where u8: Copy { pub value: u8 }",
            None,
        ),
        (
            "generic_root",
            "pub struct Generic<T> { pub value: T } /// Rejected root.\npub fn bad() { let _ = wire::View::load::<Generic<u8>>(&[], *b\"generic1\"); }",
            Some("error[E0277]"),
        ),
        (
            "forged_witness",
            "pub struct Safe { pub value: u8 } /// Rejected witness.\npub fn bad() -> wire::__private::CheckedLayout<'static> { wire::__private::CheckedLayout { words: &[], root: 0, budget: 8 } }",
            Some("error[E0451]"),
        ),
        (
            "unsupported",
            "pub struct Bad { pub value: f64 }",
            Some("f64: Schema"),
        ),
        (
            "fixed_kind_missing_width",
            r#"pub struct Generic<T> { pub value: T }
struct Missing;
impl wire::Schema for Missing {
    const KIND: wire::Kind = wire::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static wire::__private::Layout = &wire::__private::Layout::Str;
    const __LEN: usize = 0;
    const __HEADER: &'static [u32] = &[];
    type __Children = wire::__private::Nil;
    type Read<'a> = ();
    type Mut<'a> = ();
    type Init<'a> = ();
    type Removed = ();
    fn read<'a>(_: wire::View<'a>) -> wire::Result<()> { Ok(()) }
    fn read_removed(_: wire::View<'_>) -> wire::Result<()> { Ok(()) }
    fn project(_: wire::__private::Projection<'_, Self>) {}
    fn measure(_: &()) -> wire::Result<u32> { Ok(0) }
    fn encode(_: &(), _: &mut [u8]) {}
    wire::__private::with_alloc! {
        type Owned = ();
        fn measure_owned(_: &()) -> wire::Result<u32> { Ok(0) }
        fn encode_owned(_: &(), _: &mut [u8]) {}
        fn decode_owned(_: wire::View<'_>) -> wire::Result<()> { Ok(()) }
    }
}
const _: u32 = <Generic<Missing> as wire::Schema>::MIN_SIZE;
"#,
            Some("fixed field has no width"),
        ),
        (
            "native_width",
            "pub struct Bad { pub value: usize }",
            Some("usize: Schema"),
        ),
        (
            "reference",
            "pub struct Bad { pub value: &'static str }",
            Some("&'static str: Schema"),
        ),
        (
            "zero_width",
            "pub struct Bad { pub value: [u8; 0] }",
            Some("fixed arrays must have positive width"),
        ),
        (
            "oversized_width",
            "pub struct Bad { pub value: [u8; 4294967296] }",
            Some("fixed array width exceeds u32"),
        ),
        (
            "sum_overflow",
            "pub struct Bad { pub value: [u8; 4294967295], pub extra: u8 }",
            Some("schema wire width exceeds u32"),
        ),
        (
            "directory_overflow",
            "pub struct Bad { pub value: [u8; 4294967295], pub a: wire::Str, pub b: wire::Str }",
            Some("schema wire width exceeds u32"),
        ),
        (
            "account_overflow",
            "pub struct Bad { pub value: [u8; 4294967295] }",
            Some("account minimum exceeds u32"),
        ),
        (
            "conditional_field",
            "pub struct Bad { #[cfg(any())] pub value: u8, pub other: u8 }",
            Some("conditional schema fields are unsupported"),
        ),
        (
            "bad_accessor",
            "pub struct Bad { #[blubber(accessor = \"not a name\")] pub value: u8 }",
            Some("accessor must be a Rust identifier"),
        ),
        (
            "unknown_helper",
            "pub struct Bad { #[blubber(codec = \"anything\")] pub value: u8 }",
            Some("expected accessor"),
        ),
        (
            "recursive",
            "pub struct Recursive { pub child: Option<Recursive> }",
            Some("cycle detected"),
        ),
    ];
    for (name, declaration, expected) in cases {
        let options = if name == "account_overflow" {
            "(account)"
        } else {
            ""
        };
        check(
            &consumer,
            name,
            &format!("{header}#[wire::schema{options}]\n{declaration}\n"),
            expected,
            false,
        );
    }
    for (name, options, expected) in [
        (
            "namespace_epoch",
            "account, namespace = \"ledger:λ\", semantic_epoch = 4294967295",
            None,
        ),
        (
            "namespace_without_account",
            "namespace = \"ledger\"",
            Some("namespace and semantic_epoch require account"),
        ),
        (
            "epoch_without_account",
            "semantic_epoch = 1",
            Some("namespace and semantic_epoch require account"),
        ),
        (
            "duplicate_namespace",
            "account, namespace = \"a\", namespace = \"b\"",
            Some("without duplicates"),
        ),
        (
            "duplicate_epoch",
            "account, semantic_epoch = 1, semantic_epoch = 2",
            Some("without duplicates"),
        ),
        (
            "namespace_type",
            "account, namespace = 1",
            Some("namespace requires a string literal"),
        ),
        (
            "epoch_type",
            "account, semantic_epoch = \"1\"",
            Some("semantic_epoch requires a u32 literal"),
        ),
        (
            "epoch_overflow",
            "account, semantic_epoch = 4294967296",
            Some("number too large"),
        ),
        (
            "epoch_negative",
            "account, semantic_epoch = -1",
            Some("semantic_epoch requires a u32 literal"),
        ),
        (
            "explicit",
            "account, discriminator = [1, 2, 3, 4, 5, 6, 7, 8]",
            None,
        ),
        (
            "zero_discriminator",
            "account, discriminator = [0; 8]",
            Some("discriminator requires exactly eight byte literals"),
        ),
        (
            "reserved",
            "account, discriminator = [0,0,0,0,0,0,0,0]",
            Some("all-zero discriminator is reserved"),
        ),
        (
            "short",
            "account, discriminator = [1,2,3]",
            Some("discriminator requires exactly eight byte literals"),
        ),
        (
            "byte_overflow",
            "account, discriminator = [1,2,3,4,5,6,7,256]",
            Some("number too large"),
        ),
        (
            "expression",
            "account, discriminator = [1,2,3,4,5,6,7,1 + 1]",
            Some("expected a byte literal"),
        ),
        (
            "string_byte",
            "account, discriminator = [1,2,3,4,5,6,7,\"a\"]",
            Some("expected a byte literal"),
        ),
        (
            "without_account",
            "discriminator = [1,2,3,4,5,6,7,8]",
            Some("discriminator requires account"),
        ),
        (
            "duplicate_option",
            "account, account",
            Some("without duplicates"),
        ),
        (
            "duplicate_discriminator",
            "account, discriminator = [1,2,3,4,5,6,7,8], discriminator = [1,2,3,4,5,6,7,8]",
            Some("without duplicates"),
        ),
        (
            "unknown_option",
            "unknown",
            Some("expected account, discriminator"),
        ),
    ] {
        check(
            &consumer,
            name,
            &format!(
                "{header}#[wire::schema({options})]\npub struct Example {{ pub value: u8 }}\n"
            ),
            expected,
            false,
        );
    }
    check(
        &consumer,
        "cross_crate_renamed_raw_shadowed",
        &format!(
            "{header}\
        type Text = wire::Str;\ntype Word = u16;\n\
        /// Shadow core.\npub mod core {{}}\n/// Shadow blubber.\npub mod blubber {{}}\n\
        /// Shadow the prelude.\npub struct Option;\n\
        #[wire::schema(account)] pub struct Consumer {{\n\
            pub fixed: foreign::Native, pub nested: foreign::Foreign,\n\
            pub entries: wire::Map<Word, core::option::Option<Text>>,\n\
        }}\n"
        )
        .replace("core::option::Option<Text>", "::core::option::Option<Text>"),
        None,
        false,
    );
    check(
        &consumer,
        "visibility_raw_docs",
        &format!(
            "{header}\
        /// Module boundary.\npub mod application {{\n\
            #[wire::schema] pub struct Outer {{\n\
                /// Field documentation.\n#[blubber(accessor = \"other_name\")]\n\
                pub(crate) r#type: wire::Str,\n\
            }}\n\
            /// Creates borrowed input from a local reference.\n\
            pub fn input(value: &str) -> OuterInit<'_> {{ OuterInit {{ r#type: value }} }}\n\
        }}\n"
        ),
        None,
        false,
    );
    check(
        &consumer,
        "conditional_item",
        &format!(
            "{header}\
        #[wire::schema] #[cfg(any())] pub struct Absent {{ pub field: Nonexistent }}\n\
        #[wire::schema] #[cfg_attr(all(), cfg(any()))]\n\
        pub struct AlsoAbsent {{ pub field: Nonexistent }}\n"
        ),
        None,
        false,
    );

    check(&consumer, "shadowed_primitives_and_macros", &format!("{header}\
        #[allow(unused_macros)] macro_rules! matches {{ ($($tokens:tt)*) => {{ compile_error!(\"captured matches\") }} }}\n\
        #[allow(unused_macros)] macro_rules! assert {{ ($($tokens:tt)*) => {{ compile_error!(\"captured assert\") }} }}\n\
        #[allow(unused_macros)] macro_rules! panic {{ ($($tokens:tt)*) => {{ compile_error!(\"captured panic\") }} }}\n\
        #[allow(non_camel_case_types)] type u32 = wire::Str;\n\
        #[allow(non_camel_case_types)] type u8 = wire::Str;\n\
        #[allow(non_camel_case_types)] type usize = wire::Str;\n\
        #[wire::schema(account)] pub struct Shadowed {{ pub a: u32, pub b: u8, pub c: usize }}\n\
        /// Construct a borrowed input with aliased fields.\n\
        pub fn input(text: &str) -> ShadowedInit<'_> {{ ShadowedInit {{ a: text, b: text, c: text }} }}\n"), None, false);

    let mut nested = header.to_owned();
    for level in 1..=7 {
        let child = if level == 1 {
            "wire::Str".to_owned()
        } else {
            format!("Level{}", level - 1)
        };
        nested.push_str(&format!(
            "#[wire::schema] pub struct Level{level} {{ pub value: {child} }}\n"
        ));
    }
    check(&consumer, "depth8", &nested, None, false);
    nested.push_str("#[wire::schema] pub struct Level8 { pub value: Level7 }\n");
    check(
        &consumer,
        "depth9",
        &nested,
        Some("schema depth exceeds MAX_DEPTH; consider max-depth-16"),
        false,
    );
    check(&consumer, "depth9_feature", &nested, None, true);
    for level in 9..=15 {
        nested.push_str(&format!(
            "#[wire::schema] pub struct Level{level} {{ pub value: Level{} }}\n",
            level - 1
        ));
    }
    check(&consumer, "depth16", &nested, None, true);
    nested.push_str("#[wire::schema] pub struct Level16 { pub value: Level15 }\n");
    check(
        &consumer,
        "depth17",
        &nested,
        Some("schema depth exceeds MAX_DEPTH; consider max-depth-16"),
        true,
    );
    check(
        &consumer,
        "container_depth9",
        &format!(
            "{header}\
        #[wire::schema] pub struct Deep {{ pub value:\n\
        wire::List<wire::Map<u8, ::core::option::Option<wire::List<\n\
        wire::List<wire::List<wire::List<wire::Str>>>>>>> }}\n"
        ),
        Some("schema depth exceeds MAX_DEPTH; consider max-depth-16"),
        false,
    );
}
