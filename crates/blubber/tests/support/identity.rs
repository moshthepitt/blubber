//! identity vectors, shared by host, MSRV and fresh SBF fixtures.
#![forbid(unsafe_code)]
#![allow(dead_code)]
use blubber::{AccountSchema, List, Map, Pubkey, Str};

/// Source-independent vector exported as ordinary constant data.
pub type Vector = (&'static str, [u8; 8], &'static str, u32, &'static [u8]);

/// Default namespace and epoch remain source compatible.
#[blubber::schema(account)]
pub struct DefaultState {
    pub value: u64,
}

macro_rules! state {
    ($module:ident, $($field:ident : $ty:ty),+ $(,)?) => {
        mod $module {
            #[allow(unused_imports)]
            use super::*;
            #[blubber::schema(account, namespace = "test:state")]
            pub struct State { $(pub $field: $ty),+ }
        }
    };
}
state!(unsigned, value: u64);
state!(signed, value: i64);
state!(boolean, value: bool);
state!(byte, value: u8);
state!(raw, value: [u8;8]);
state!(first, text: Str, value: u64, flag: bool);
state!(second, value: u64, text: Str, flag: bool);
state!(unsigned_key, values: Map<u16, Str>);
state!(signed_key, values: Map<i16, Str>);
state!(bool_key, values: Map<bool, u8>);
state!(raw_key, values: Map<[u8;2], Str>);
state!(nested, values: List<Option<Str>>);

/// Same descriptor and namespace after a Rust rename.
#[blubber::schema(account, namespace = "test:state")]
pub struct Renamed {
    pub other_name: u64,
}
/// Same wire bytes with a new semantic interpretation.
#[blubber::schema(account, namespace = "test:state", semantic_epoch = 1)]
pub struct Epoch {
    pub value: u64,
}
/// UTF-8 namespaces use byte counts, including embedded nulls.
#[blubber::schema(account, namespace = "ledger:λ\0v1", semantic_epoch = 4294967295)]
pub struct Unicode {
    pub value: u64,
}
/// Explicit overrides preserve legacy identities independently of the layout hash.
#[blubber::schema(account, discriminator = [1,2,3,4,5,6,7,255])]
pub struct Override {
    pub value: i64,
}
/// Native padding never enters a fixed codec descriptor.
#[derive(Clone, Copy, blubber::Fixed)]
pub struct Padded {
    /// Checked boolean byte.
    pub flag: bool,
    /// Packed integer without native alignment.
    pub value: u64,
}
/// Fixed derives and schema markers describe the same packed wire contract.
#[blubber::schema]
pub struct Packed {
    pub flag: bool,
    pub value: u64,
}
/// Generic instantiations preserve the resolved child's descriptor.
#[blubber::schema]
pub struct Generic<T> {
    pub value: T,
}
/// Renamed scalar aliases have identical descriptors.
pub type Count = u64;
state!(alias, value: Count);
state!(padded, value: Padded);
state!(packed, value: Packed);
state!(generic, value: Generic<u64>);
state!(bytes, value: List<u8>);
state!(list, value: List<u8>);
state!(pubkey, value: Pubkey);
state!(array, value: [u8;32]);

/// Number of nonempty, pinned identity vectors.
pub const COUNT: usize = 25;
fn of<T: AccountSchema>(name: &'static str) -> Vector {
    (
        name,
        T::DISCRIMINATOR,
        T::NAMESPACE,
        T::SEMANTIC_EPOCH,
        T::WIRE_DESCRIPTOR_BYTES,
    )
}
/// Selects an exact generated descriptor and identity, without runtime hashing.
pub fn vector(index: usize) -> Option<Vector> {
    Some(match index {
        0 => of::<DefaultState>("default"),
        1 => of::<unsigned::State>("unsigned"),
        2 => of::<signed::State>("signed"),
        3 => of::<boolean::State>("bool"),
        4 => of::<byte::State>("byte"),
        5 => of::<raw::State>("raw"),
        6 => of::<first::State>("first"),
        7 => of::<second::State>("second"),
        8 => of::<unsigned_key::State>("unsigned_key"),
        9 => of::<signed_key::State>("signed_key"),
        10 => of::<bool_key::State>("bool_key"),
        11 => of::<raw_key::State>("raw_key"),
        12 => of::<nested::State>("nested"),
        13 => of::<Renamed>("renamed"),
        14 => of::<Epoch>("epoch"),
        15 => of::<Unicode>("unicode"),
        16 => of::<Override>("override"),
        17 => of::<alias::State>("alias"),
        18 => of::<padded::State>("padded"),
        19 => of::<packed::State>("packed"),
        20 => of::<generic::State>("generic"),
        21 => of::<bytes::State>("bytes"),
        22 => of::<list::State>("list"),
        23 => of::<pubkey::State>("pubkey"),
        24 => of::<array::State>("array"),
        _ => return None,
    })
}
