//! Name collision fixtures shared by runtime and compiler tests.

/// Referenced and unreferenced constants cannot become generated patterns.
#[allow(non_upper_case_globals, dead_code, missing_docs)]
pub mod bindings {
    use wire::{Fixed, Schema};
    pub const count: usize = 1;
    pub const width: usize = 2;
    pub const fields: usize = 3;
    pub const offset: usize = 4;
    pub const end: usize = 5;
    pub const minimum: usize = 6;
    pub const size: usize = 7;
    pub const child: usize = 8;
    pub const remaining: usize = 9;
    pub const word: usize = 10;
    pub const words: usize = 11;
    pub const init: usize = 12;
    pub const value: usize = 13;
    pub const destination: usize = 14;
    pub const fixed: usize = 15;
    pub const rest: usize = 16;
    pub const directory: usize = 17;
    pub const payload: usize = 18;
    pub const view: usize = 19;
    pub const edit: usize = 20;
    pub const seeds_with_bump: usize = 21;
    pub const state: usize = 22;
    pub const payer: usize = 23;
    pub const system_program: usize = 24;
    pub const program_id: usize = 25;
    pub const space: usize = 26;
    pub const store: usize = 27;
    pub const bytes: usize = 28;
    pub const account: usize = 29;
    pub const read: usize = 30;
    pub const funding: usize = 31;
    pub const receipt: usize = 32;
    pub const n: usize = 33;
    pub const widths: usize = 34;
    pub const offsets: usize = 35;
    pub const index: usize = 36;
    pub const kind: usize = 37;
    pub const error: usize = 38;
    pub const encoded: usize = 39;
    pub const additional: usize = 40;
    pub const __blubber_index: usize = 41;
    pub const __blubber_index_1: usize = 42;
    pub const __blubber_offset: usize = 43;
    pub const __blubber_fixed: usize = 44;
    pub const __blubber_variable: usize = 45;
    pub const __blubber_is_variable: usize = 46;
    pub const __blubber_selected: usize = 47;
    pub const __blubber_selected_1: usize = 48;
    // These names are deliberately absent from all macro inputs.
    pub const __blubber_local_init: usize = 49;
    pub const __blubber_local_init_1: usize = 50;
    pub const __blubber_local_count: usize = 51;
    pub const __blubber_local_value: usize = 52;
    pub const __blubber_local___blubber_index: usize = 53;
    pub const __blubber_local___blubber_selected: usize = 54;

    #[derive(Clone, Copy, wire::Fixed)]
    pub struct Native {
        pub r#type: [u8; width],
        pub flag: bool,
    }
    #[wire::schema(account)]
    pub struct Account {
        pub fixed: Native,
        pub value: [u8; count],
        pub bytes: [u8; r#size],
        pub collision: [u8; __blubber_local_init],
        pub text: wire::Str,
    }
    #[wire::schema]
    pub struct Generic<const r#n: usize = count> {
        pub text: wire::Str,
        pub array: [u8; r#n],
    }

    pub fn exercise() {
        let input = AccountInit {
            fixed: Native {
                r#type: [7; width],
                flag: true,
            },
            value: [8; count],
            bytes: [9; size],
            collision: [0; __blubber_local_init],
            text: "text",
        };
        let mut room = [0u8; 256];
        let mut backing = wire::SliceStore::new(&mut room);
        let mut root = Account::initialize(&mut backing, input).unwrap();
        root.value_mut().set([4; count]).unwrap();
        root.text_mut().set("different").unwrap();
        root.validate().unwrap();
        root.commit().unwrap();
        assert!(Account::load(&room).unwrap().value() == [4; count]);
        assert!(
            Generic::<2>::measure(&GenericInit {
                text: "",
                array: [3; 2]
            }) == Ok(2)
        );
        let native = Native {
            r#type: [5; width],
            flag: false,
        };
        assert!(Native::decode(native.encode().as_ref()).unwrap().r#type == [5; width]);
    }
}

/// Field types keep user HRTBs and `Self` when emitted in companion impls.
#[allow(dead_code, missing_docs)]
pub mod self_and_hrtb {
    pub trait Fields {
        type Payload;
    }
    #[derive(Clone, Copy, wire::Fixed)]
    pub struct Native {
        pub value: <Self as Fields>::Payload,
    }
    impl Fields for Native {
        type Payload = u16;
    }
    #[wire::schema]
    pub struct SelfArray {
        pub array: [u8; Self::COUNT],
    }
    impl SelfArray {
        pub const COUNT: usize = 3;
    }
    pub trait Choose<'a> {
        type Selected;
    }
    pub struct Resolver;
    impl<'a> Choose<'a> for Resolver {
        type Selected = u16;
    }
    #[wire::schema]
    pub struct Owner<T: for<'__blubber> Choose<'__blubber, Selected = u16>> {
        pub value: <Self as Fields>::Payload,
        pub other: <T as Choose<'static>>::Selected,
    }
    impl<T: for<'a> Choose<'a, Selected = u16> + 'static> Fields for Owner<T> {
        type Payload = u8;
    }
    pub fn exercise() {
        let _: OwnerInit<'_, Resolver> = OwnerInit { value: 4, other: 5 };
        let _ = SelfArrayInit { array: [1; 3] };
    }
}

/// Declarative table construction introduces neither const names nor bindings.
#[allow(non_camel_case_types, non_upper_case_globals, dead_code, missing_docs)]
pub mod table {
    pub const WORDS: ::core::primitive::usize = 3;
    pub const CERTIFICATE: ::core::primitive::usize = 2;
    pub const checked: ::core::primitive::usize = 1;
    type u8 = ::core::primitive::u16;
    type u32 = ::core::primitive::u64;
    type usize = ::core::primitive::u8;
    type Option = ();
    wire::__root_schema!(Root, [::core::primitive::u8; WORDS]);
    pub fn exercise() {
        let _ = wire::__layout_of!([::core::primitive::u8; WORDS]);
        let _ = wire::__layout_of!([::core::primitive::u8; CERTIFICATE]);
        let _ = <Root as wire::RootSchema>::__layout();
    }
}

/// Opaque user macros keep the spelling and scope of ordinary parameters.
#[allow(dead_code, missing_docs)]
pub mod type_macros {
    macro_rules! identity {
        ($ty:ty) => {
            $ty
        };
    }
    macro_rules! spelling {
        ($name:ident) => {
            ::core::stringify!($name).len()
        };
    }
    #[wire::schema]
    pub struct Opaque<T> {
        pub value: identity!(T),
        pub spelling: [u8; spelling!(T)],
    }
    pub fn exercise() {
        let init = OpaqueInit::<'_, u8> {
            value: 7,
            spelling: [9],
        };
        assert!(<Opaque<u8> as wire::Schema>::measure(&init) == Ok(2));
        let mut bytes = [0; 2];
        <Opaque<u8> as wire::Schema>::encode(&init, &mut bytes);
        assert!(bytes == [7, 9]);
    }
}

/// Opaque macro constants resolve before generated binding masks exist.
#[allow(dead_code, missing_docs, non_upper_case_globals, non_camel_case_types)]
pub mod hidden_macro_constants {
    // Sibling items are absent from the derive inputs used to pick helper names.
    pub struct __BlubberField_Native_0;
    pub struct __BlubberField_Account_0;
    pub struct __BlubberField_Generic_0;
    pub struct __BlubberTypes_Generic;
    pub struct __BlubberIdentity_Generic;
    pub type __BlubberField_Captured_0 = u16;
    pub const __blubber_local_size: usize = 3;
    pub const __blubber_local_count: usize = 2;
    macro_rules! width {
        () => {
            __blubber_local_size
        };
    }
    macro_rules! count {
        () => {
            __blubber_local_count
        };
    }
    macro_rules! identity {
        ($ty:ty) => {
            $ty
        };
    }
    macro_rules! hidden_field {
        () => {
            __BlubberField_Captured_0
        };
    }
    #[wire::schema]
    pub struct Captured {
        pub value: hidden_field!(),
    }
    pub trait Fields {
        type Payload;
    }
    #[derive(Clone, Copy, wire::Fixed)]
    pub struct Native {
        pub value: identity!([u8; width!()]),
    }
    #[wire::schema(account)]
    pub struct Account {
        pub fixed: Native,
        pub values: identity!([u8; count!()]),
        pub text: wire::Str,
    }
    #[wire::schema]
    pub struct Generic<T> {
        pub value: identity!(T),
        pub array: [u8; width!()],
    }
    #[wire::schema]
    pub struct ExplicitOwner {
        pub value: identity!(<ExplicitOwner as Fields>::Payload),
        pub array: [u8; width!()],
    }
    impl Fields for ExplicitOwner {
        type Payload = u16;
    }
    pub fn exercise() {
        assert!(<Captured as wire::Schema>::measure(&CapturedInit { value: 6 }) == Ok(2));
        let native = Native { value: [1, 2, 3] };
        let mut bytes = [0; 128];
        let mut store = wire::SliceStore::new(&mut bytes);
        let mut root = Account::initialize(
            &mut store,
            AccountInit {
                fixed: native,
                values: [4, 5],
                text: "old",
            },
        )
        .unwrap();
        root.text_mut().set("new").unwrap();
        root.commit().unwrap();
        assert!(Account::load(&bytes).unwrap().fixed().value == [1, 2, 3]);
        assert!(
            <Generic<u8> as wire::Schema>::measure(&GenericInit {
                value: 4,
                array: [5; 3]
            }) == Ok(4)
        );
        assert!(
            <ExplicitOwner as wire::Schema>::measure(&ExplicitOwnerInit {
                value: 4,
                array: [5; 3]
            }) == Ok(5)
        );
    }
}
