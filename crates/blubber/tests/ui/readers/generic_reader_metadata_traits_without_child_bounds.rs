#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        /// A schema marker without Copy, Clone or Debug.
        #[wire::schema]
        pub struct Leaf {
            /// Borrowed payload.
            pub text: wire::Str,
        }
        /// Generic projection whose child has no standard traits.
        #[wire::schema]
        pub struct Generic<T: wire::Schema> {
            /// Selected child.
            pub value: T,
        }
        /// Trait signatures do not constrain the child schema.
        pub fn check_traits() {
            fn copy<T: ::core::marker::Copy>() {}
            fn clone<T: ::core::clone::Clone>() {}
            fn debug<T: ::core::fmt::Debug>() {}
            copy::<GenericRef<'_, Leaf>>();
            clone::<GenericRef<'_, Leaf>>();
            debug::<GenericRef<'_, Leaf>>();
            debug::<GenericMut<'_, Leaf>>();
        }

use foreign as _;
