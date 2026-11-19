//! Independent byte oracle shared by runtime tests and the CU bencher.

use blubber_cu_fixture::{matrix::*, schemas::*};
use blubber_model::{IntWidth, Key, KeySchema, Schema, Value};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);

/// One reproducible collection or nested-string instruction and its independent oracle.
pub struct Case {
    /// Stable scenario name.
    pub name: String,
    /// Fixed/indexed list or map layout code.
    pub layout: u8,
    /// Initial collection length.
    pub n: usize,
    /// Operation code from the fixture dispatcher.
    pub op: u8,
    /// Selected ordinal.
    pub index: usize,
    /// Whether keys share their first 28 bytes.
    pub prefix: bool,
    /// Collection, nested-string or option dispatch code.
    pub opcode: u8,
    /// Descriptor depth for nested strings.
    pub depth: usize,
}

impl Case {
    /// Builds the existing fixture instruction.
    pub fn instruction(&self, op: u8) -> Instruction {
        let [lo, hi] = (self.index as u16).to_le_bytes();
        let [nlo, nhi] = (self.n as u16).to_le_bytes();
        Instruction {
            program_id: OWNER,
            accounts: vec![AccountMeta::new(STATE, false)],
            data: vec![
                self.opcode,
                self.layout,
                op,
                lo,
                hi,
                nlo,
                nhi,
                u8::from(self.prefix),
            ],
        }
    }

    /// Builds the independent logical schema, initial value and discriminator.
    pub fn model(&self) -> (Schema, Value, [u8; 8]) {
        if self.opcode != 23 {
            let mut schema = Schema::Str;
            let mut value = Value::Str(ORIGINAL.into());
            let discriminator = match self.opcode {
                24 => *b"MATRIX01",
                25 | 26 => {
                    for _ in 0..self.depth - 2 {
                        schema = Schema::List(Box::new(schema));
                        value = Value::List(vec![value.clone(), value]);
                    }
                    schema = Schema::Struct(vec![schema, Schema::Str]);
                    value = Value::Struct(vec![value, Value::Str("tail".into())]);
                    if self.opcode == 25 {
                        Depth4::DISCRIMINATOR
                    } else {
                        Depth8::DISCRIMINATOR
                    }
                }
                27 => {
                    schema = Schema::Struct(vec![Schema::Option(Box::new(schema))]);
                    value = Value::Struct(vec![Value::Option(Some(Box::new(value)))]);
                    MaybeText::DISCRIMINATOR
                }
                _ => unreachable!(),
            };
            return (schema, value, discriminator);
        }
        let leaf = if self.layout.is_multiple_of(2) {
            Schema::Unsigned(IntWidth::W8)
        } else {
            Schema::Str
        };
        let value = self.old();
        let (schema, value, disc) = if self.layout < 2 {
            (
                Schema::List(Box::new(leaf)),
                Value::List(vec![value; self.n]),
                if self.layout == 0 {
                    Numbers::DISCRIMINATOR
                } else {
                    Words::DISCRIMINATOR
                },
            )
        } else {
            let entries = (0..self.n)
                .map(|i| (self.key(i, false), value.clone()))
                .collect();
            (
                Schema::Map(KeySchema::Pubkey, Box::new(leaf)),
                Value::Map(entries),
                match self.layout {
                    2 => PublicKeyMap::DISCRIMINATOR,
                    3 => KeyWords::DISCRIMINATOR,
                    4 => AddressMap::DISCRIMINATOR,
                    _ => AddressWords::DISCRIMINATOR,
                },
            )
        };
        (
            Schema::Struct(vec![schema]),
            Value::Struct(vec![value]),
            disc,
        )
    }

    fn old(&self) -> Value {
        if self.layout.is_multiple_of(2) {
            Value::Unsigned(17)
        } else {
            Value::Str(ORIGINAL.into())
        }
    }

    fn new_value(&self) -> Value {
        if self.layout.is_multiple_of(2) {
            Value::Unsigned(123)
        } else {
            Value::Str(REPLACEMENT.into())
        }
    }

    fn key(&self, index: usize, missing: bool) -> Key {
        // Construct independently of the SBF key helper, then compare encodings.
        let mut bytes = [42; 32];
        let at = if self.prefix { 28 } else { 0 };
        bytes[at..at + 4]
            .copy_from_slice(&(2 * index as u32 + if missing { 1 } else { 2 }).to_be_bytes());
        Key::Pubkey(bytes)
    }

    /// Applies the selected edit to the independent logical value.
    pub fn expected(&self, mut value: Value) -> Value {
        if self.opcode != 23 {
            let mut leaf = &mut value;
            if self.opcode == 27 {
                let Value::Struct(fields) = leaf else {
                    unreachable!()
                };
                if matches!(self.op, 25 | 45 | 46) {
                    fields[0] = Value::Option(None);
                    return value;
                }
                let Value::Option(Some(child)) = &mut fields[0] else {
                    unreachable!()
                };
                **child = Value::Str(REPLACEMENT.into());
                return value;
            }
            if self.opcode != 24 {
                let Value::Struct(fields) = leaf else {
                    unreachable!()
                };
                leaf = &mut fields[0];
                for _ in 0..self.depth - 2 {
                    let Value::List(items) = leaf else {
                        unreachable!()
                    };
                    leaf = &mut items[0];
                }
            }
            let Value::Str(text) = leaf else {
                unreachable!()
            };
            match self.op {
                4 => (),
                5 => *text = REPLACEMENT.into(),
                6 => *text = "a".into(),
                22 => text.push_str(REPLACEMENT),
                23 => text.truncate(1),
                24 => text.clear(),
                _ => unreachable!(),
            }
            return value;
        }
        let Value::Struct(fields) = &mut value else {
            unreachable!()
        };
        let replacement = self.new_value();
        match &mut fields[0] {
            Value::List(items) => match self.op {
                5 | 37 => items.insert(self.index, replacement),
                6 | 10 | 27 | 40 => items[self.index] = replacement,
                7 | 8 | 38 => {
                    items.remove(self.index);
                }
                9 => items.clear(),
                11 => items[0] = replacement,
                33 | 34 => items.fill(replacement.clone()),
                13 | 41 => items.push(replacement),
                14 | 39 => {
                    items.pop();
                }
                15 => items.truncate(self.index),
                16 => items.extend([replacement.clone(), replacement]),
                17 => {
                    items.splice(self.index..self.index, [replacement.clone(), replacement]);
                }
                _ => (),
            },
            Value::Map(items) => match self.op {
                5 | 21 | 32 | 37 => {
                    assert!(items
                        .insert(self.key(self.index, true), replacement)
                        .is_none());
                }
                6 | 10 | 20 | 27 | 30 | 47 => {
                    assert!(items
                        .insert(self.key(self.index, false), replacement)
                        .is_some());
                }
                7 | 8 | 31 | 38 => {
                    assert!(items.remove(&self.key(self.index, false)).is_some());
                }
                9 => items.clear(),
                11 => *items.first_entry().unwrap().get_mut() = replacement,
                33 | 34 => items
                    .values_mut()
                    .for_each(|value| *value = replacement.clone()),
                _ => (),
            },
            _ => unreachable!(),
        }
        value
    }
}
