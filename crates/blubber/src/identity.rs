//! Versioned wire descriptors and compile-time account identities.
//!
//! This metadata describes the codec contract, independently of implementation
//! tables and native Rust layout. See `SPEC.md` for the byte grammar.

use blubber_core::{KeyDomain, KeyLayout};

/// Stable identity of a handwritten codec, independent of its Rust type name.
#[derive(Clone, Copy, Debug)]
pub struct CodecIdentity {
    /// Application-controlled, nonempty codec namespace.
    pub namespace: &'static str,
    /// Incremented when the codec's wire or semantic contract changes.
    pub epoch: u32,
}

/// Canonical map-key contract, including its order-preserving encoding.
#[derive(Clone, Copy, Debug)]
pub struct WireKey {
    /// Fixed width and canonical key domain.
    pub layout: KeyLayout,
    /// Explicit identity for handwritten encoders; absent for built-ins.
    pub codec: Option<CodecIdentity>,
}

/// Canonical format-1 codec tree, without optimization or native-layout data.
///
/// Struct children occur in declaration order. Their kinds determine packed
/// fixed offsets and variable payload ordinals. Field and Rust type names do
/// not enter this contract. Handwritten schemas must declare `Custom` metadata
/// before using automatic account identities; their implementation remains an
/// assurance boundary.
#[derive(Clone, Copy, Debug)]
pub enum WireDescriptor {
    /// No stable codec contract supplied; automatic identity generation rejects it.
    Unspecified,
    /// Little-endian unsigned integer with a supported byte width.
    Unsigned(u32),
    /// Little-endian two's-complement integer with a supported byte width.
    Signed(u32),
    /// A single byte restricted to zero or one.
    Bool,
    /// Positive-width raw bytes, with no native padding.
    Raw(u32),
    /// UTF-8 bytes delimited by the parent region.
    Str,
    /// Tag zero alone, or tag one followed by the child encoding.
    Option(&'static WireDescriptor),
    /// A u32 count with fixed-stride or elided-terminal-end elements.
    List(&'static WireDescriptor),
    /// Sorted keys with fixed-stride values or stored cumulative value ends.
    Map {
        /// Key codec; `None` rejects automatic identity generation.
        key: Option<WireKey>,
        /// Value codec.
        value: &'static WireDescriptor,
    },
    /// Declaration-ordered children, packed into a fixed prefix and variable tail.
    Struct(&'static [&'static WireDescriptor]),
    /// Explicit handwritten codec identity and its underlying wire contract.
    Custom {
        /// Stable application-controlled codec name and epoch.
        codec: CodecIdentity,
        /// Structural/domain contract implemented by the codec.
        wire: &'static WireDescriptor,
    },
}

const fn add(a: usize, b: usize) -> usize {
    match a.checked_add(b) {
        Some(n) if n <= u32::MAX as usize => n,
        _ => panic!("canonical descriptor exceeds u32"),
    }
}

impl WireDescriptor {
    /// Fixed encoded width, or `None` for a variable encoding.
    pub const fn fixed_width(&self) -> Option<u32> {
        match self {
            Self::Unsigned(n) | Self::Signed(n) => {
                assert!(
                    matches!(*n, 1 | 2 | 4 | 8 | 16),
                    "invalid integer codec width"
                );
                Some(*n)
            }
            Self::Bool => Some(1),
            Self::Raw(n) => {
                assert!(*n > 0, "raw codec width must be positive");
                Some(*n)
            }
            Self::Struct(fields) => {
                let mut width = 0;
                let mut i = 0;
                while i < fields.len() {
                    match fields[i].fixed_width() {
                        Some(n) => width = add(width, n as usize),
                        None => return None,
                    }
                    i += 1;
                }
                Some(width as u32)
            }
            Self::Custom { wire, .. } => wire.fixed_width(),
            Self::Unspecified => {
                panic!("automatic identity requires an explicit stable codec descriptor")
            }
            _ => None,
        }
    }

    const fn node_len(&self) -> usize {
        match self {
            Self::Unspecified => {
                panic!("automatic identity requires an explicit stable codec descriptor")
            }
            Self::Unsigned(_) | Self::Signed(_) | Self::Raw(_) => {
                let _ = self.fixed_width();
                5
            }
            Self::Bool | Self::Str => 1,
            Self::Option(child) | Self::List(child) => add(6, child.node_len()),
            Self::Map { key, value } => add(add(10, key_len(*key)), value.node_len()),
            Self::Struct(fields) => {
                assert!(
                    !fields.is_empty() && fields.len() <= u16::MAX as usize,
                    "invalid struct field count"
                );
                let mut n = 17;
                let mut i = 0;
                while i < fields.len() {
                    n = add(n, add(17, fields[i].node_len()));
                    i += 1;
                }
                n
            }
            Self::Custom { codec, wire } => add(add(13, codec_len(*codec)), wire.node_len()),
        }
    }

    /// Length of the versioned canonical byte encoding, checked in the u32 domain.
    pub const fn encoded_len(&self) -> usize {
        add(4, self.node_len())
    }

    /// Serializes this descriptor with canonical encoding version one.
    ///
    /// `N` must equal [`Self::encoded_len`]. Invalid or unspecified codec metadata
    /// panics during constant evaluation when used by a generated account.
    pub const fn encode<const N: usize>(&self) -> [u8; N] {
        assert!(
            N == self.encoded_len(),
            "canonical descriptor length mismatch"
        );
        let mut output = Writer {
            bytes: [0; N],
            position: 0,
        };
        output.word(1);
        output.node(self);
        assert!(output.position == N, "canonical descriptor length mismatch");
        output.bytes
    }
}

const fn codec_len(codec: CodecIdentity) -> usize {
    assert!(
        !codec.namespace.is_empty(),
        "custom codec namespace must be nonempty"
    );
    add(0, codec.namespace.len())
}

const fn key_len(key: Option<WireKey>) -> usize {
    let key = match key {
        Some(key) => key,
        None => panic!("automatic identity requires an explicit stable map-key descriptor"),
    };
    assert!(key.layout.width > 0, "map-key width must be positive");
    match key.layout.domain {
        KeyDomain::Unsigned | KeyDomain::Signed => assert!(
            matches!(key.layout.width, 1 | 2 | 4 | 8 | 16),
            "invalid integer key width"
        ),
        KeyDomain::Bool => assert!(key.layout.width == 1, "bool key width must be one"),
        KeyDomain::Raw => (),
    }
    match key.codec {
        None => 6,
        Some(codec) => add(14, codec_len(codec)),
    }
}

struct Writer<const N: usize> {
    bytes: [u8; N],
    position: usize,
}
impl<const N: usize> Writer<N> {
    const fn byte(&mut self, byte: u8) {
        self.bytes[self.position] = byte;
        self.position += 1;
    }
    const fn word(&mut self, word: u32) {
        let bytes = word.to_le_bytes();
        let mut i = 0;
        while i < 4 {
            self.byte(bytes[i]);
            i += 1;
        }
    }
    const fn codec(&mut self, codec: CodecIdentity) {
        self.word(codec.namespace.len() as u32);
        let mut i = 0;
        while i < codec.namespace.len() {
            self.byte(codec.namespace.as_bytes()[i]);
            i += 1;
        }
        self.word(codec.epoch);
    }
    const fn child(&mut self, child: &WireDescriptor) {
        self.word(child.node_len() as u32);
        self.node(child);
    }
    const fn node(&mut self, node: &WireDescriptor) {
        match node {
            WireDescriptor::Unspecified => panic!("unspecified codec"),
            WireDescriptor::Unsigned(n) => {
                self.byte(1);
                self.word(*n);
            }
            WireDescriptor::Signed(n) => {
                self.byte(2);
                self.word(*n);
            }
            WireDescriptor::Bool => self.byte(3),
            WireDescriptor::Raw(n) => {
                self.byte(4);
                self.word(*n);
            }
            WireDescriptor::Str => self.byte(5),
            WireDescriptor::Option(child) | WireDescriptor::List(child) => {
                self.byte(if matches!(node, WireDescriptor::Option(_)) {
                    6
                } else {
                    7
                });
                self.byte(1); // Format-1 tag/count and terminal-end rule.
                self.child(child);
            }
            WireDescriptor::Map { key, value } => {
                self.byte(8);
                self.byte(1);
                self.word(key_len(*key) as u32);
                let key = match key {
                    Some(key) => key,
                    None => panic!("unspecified key codec"),
                };
                self.byte(match key.layout.domain {
                    KeyDomain::Unsigned => 1,
                    KeyDomain::Signed => 2,
                    KeyDomain::Bool => 3,
                    KeyDomain::Raw => 4,
                });
                self.word(key.layout.width);
                self.byte(if key.codec.is_some() { 1 } else { 0 });
                if let Some(codec) = key.codec {
                    self.codec(codec);
                }
                self.child(value);
            }
            WireDescriptor::Struct(fields) => {
                self.byte(9);
                let mut count = 0;
                let mut width = 0;
                let mut i = 0;
                while i < fields.len() {
                    if let Some(n) = fields[i].fixed_width() {
                        count += 1;
                        width = add(width, n as usize);
                    }
                    i += 1;
                }
                self.word(width as u32);
                self.word(count);
                self.word(fields.len() as u32 - count);
                self.word(fields.len() as u32);
                let mut fixed_ordinal = 0;
                let mut variable_ordinal = 0;
                let mut offset = 0;
                i = 0;
                while i < fields.len() {
                    self.word(i as u32);
                    if let Some(n) = fields[i].fixed_width() {
                        self.byte(0);
                        self.word(fixed_ordinal);
                        self.word(offset as u32);
                        fixed_ordinal += 1;
                        offset = add(offset, n as usize);
                    } else {
                        self.byte(1);
                        self.word(variable_ordinal);
                        self.word(0);
                        variable_ordinal += 1;
                    }
                    self.child(fields[i]);
                    i += 1;
                }
            }
            WireDescriptor::Custom { codec, wire } => {
                self.byte(10);
                self.codec(*codec);
                self.child(wire);
            }
        }
    }
}

/// Computes the D21 layout-bound identity in constant evaluation.
///
/// Generated accounts call this only from associated constants. The preimage is
/// `blubber\0schema\0`, format 1 (u32 LE), length-prefixed namespace, semantic
/// epoch (u32 LE), and length-prefixed canonical descriptor. The first eight
/// SHA-256 bytes identify a contract; they supply no authentication or integrity.
/// Panics for a length above u32 or an all-zero identity (reserved for new data).
#[doc(hidden)]
pub const fn account_identity(namespace: &str, epoch: u32, descriptor: &[u8]) -> [u8; 8] {
    let mut hash = crate::identity_sha256::Sha256::new();
    hash.update(b"blubber\0schema\0");
    hash.update(&1u32.to_le_bytes());
    hash.update(&(add(0, namespace.len()) as u32).to_le_bytes());
    hash.update(namespace.as_bytes());
    hash.update(&epoch.to_le_bytes());
    hash.update(&(add(0, descriptor.len()) as u32).to_le_bytes());
    hash.update(descriptor);
    let digest = hash.finish();
    let bytes = [
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ];
    let mut nonzero = false;
    let mut i = 0;
    while i < 8 {
        nonzero = nonzero || bytes[i] != 0;
        i += 1;
    }
    assert!(
        nonzero,
        "all-zero discriminator is reserved; select a different semantic epoch"
    );
    bytes
}
