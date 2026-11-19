//! Shared runtime and compiler fixtures for macro name resolution.

/// Consumer aliases must affect input types only.
#[allow(non_camel_case_types, dead_code, missing_docs)]
pub mod primitives {
    type u8 = ::core::primitive::u16;
    type u16 = ::core::primitive::u32;
    type u32 = ::core::primitive::u64;
    type usize = ::core::primitive::u8;
    type bool = ::core::primitive::u16;
    type str = ::core::primitive::u32;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, wire::Fixed)]
    pub struct Native {
        pub byte: u8,
        pub word: u16,
        pub flag: ::core::primitive::bool,
    }

    #[wire::schema(account)]
    pub struct Account {
        pub value: Native,
        pub count: u32,
        pub text: wire::Str,
    }
}

/// Const expressions may use any spelling chosen for generated helpers.
#[allow(dead_code, missing_docs)]
pub mod constants {
    pub const __BLUBBER_OFFSETS: usize = 2;
    pub const __BLUBBER_OFFSETS_1: usize = 3;
    pub const __BLUBBER_FIXED_COUNT: usize = 1;
    pub const __BLUBBER_FIXED_COUNT_1: usize = 2;
    pub const __BLUBBER_VARIABLE_COUNT: usize = 3;
    pub const __BLUBBER_DIRECTORY: usize = 4;
    pub const __BLUBBER_FIXED: usize = 5;
    pub const __BLUBBER_VARIABLE: usize = 6;
    pub const __BLUBBER_MINIMUM: usize = 7;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, wire::Fixed)]
    pub struct Native {
        pub first: [u8; __BLUBBER_OFFSETS],
        pub second: [u8; r#__BLUBBER_OFFSETS_1],
    }

    #[wire::schema]
    pub struct Record {
        pub a: [u8; __BLUBBER_FIXED_COUNT],
        pub b: [u8; r#__BLUBBER_FIXED_COUNT_1],
        pub c: [u8; __BLUBBER_VARIABLE_COUNT],
        pub d: [u8; __BLUBBER_DIRECTORY],
        pub e: [u8; __BLUBBER_FIXED],
        pub f: [u8; __BLUBBER_VARIABLE],
        pub g: [u8; const { __BLUBBER_MINIMUM }],
        pub text: wire::Str,
    }
}

/// Projection temporaries must not capture constants in consumer field types.
#[allow(non_upper_case_globals, dead_code, missing_docs)]
pub mod projection_locals {
    pub const LEN_A: usize = 1;
    pub const LEN_B: usize = 2;
    pub const LEN_C: usize = 3;
    pub const LEN_D: usize = 4;
    pub const selected: usize = 5;
    pub const __blubber_selected: usize = 6;

    #[wire::schema(account)]
    pub struct Account {
        pub a: [u8; LEN_A],
        pub b: [u8; LEN_B],
        pub c: [u8; LEN_C],
        pub d: [u8; LEN_D],
        pub e: [u8; selected],
        pub f: [u8; __blubber_selected],
    }
}
