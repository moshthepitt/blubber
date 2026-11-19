//! Whole-tree serialization and exact-region parsing using standard collections.

use std::collections::BTreeMap;

use crate::{
    schema::{add, mul, wire_size},
    Error, Key, KeySchema, Schema, Value,
};

/// Decoded logical account and physical envelope metadata.
///
/// Spare bytes are ignored by decoding. Encoding initializes them to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// Checked, nonzero discriminator supplied by the caller.
    pub discriminator: [u8; 8],
    /// Logical root, excluding the twelve-byte envelope.
    pub value: Value,
    /// Envelope plus exact root length, excluding spare capacity.
    pub used_len: usize,
    /// Complete physical allocation length, including spare capacity.
    pub storage_len: usize,
}

impl Schema {
    /// Encodes a value canonically with the default eight-node depth budget.
    pub fn encode(&self, value: &Value) -> Result<Vec<u8>, Error> {
        self.encode_with_depth(value, Self::DEFAULT_DEPTH)
    }

    /// Checks all metadata and values, then returns the exact canonical bytes.
    pub fn encode_with_depth(&self, value: &Value, max_depth: usize) -> Result<Vec<u8>, Error> {
        self.check(max_depth)?;
        encode(self, value)
    }

    /// Decodes an exact root region with the default eight-node depth budget.
    pub fn decode(&self, bytes: &[u8]) -> Result<Value, Error> {
        self.decode_with_depth(bytes, Self::DEFAULT_DEPTH)
    }

    /// Fully validates an exact region, including inactive descriptor branches.
    ///
    /// Counts and table extents are checked against the input before allocating
    /// child vectors; arbitrary words never become unchecked allocation sizes.
    pub fn decode_with_depth(&self, bytes: &[u8], max_depth: usize) -> Result<Value, Error> {
        self.check(max_depth)?;
        wire_size(bytes.len())?;
        decode(self, bytes)
    }

    /// Encodes an envelope and zero spare bytes with the default depth budget.
    pub fn encode_account(
        &self,
        discriminator: [u8; 8],
        value: &Value,
        storage_len: usize,
    ) -> Result<Vec<u8>, Error> {
        self.encode_account_with_depth(discriminator, value, storage_len, Self::DEFAULT_DEPTH)
    }

    /// Encodes a complete allocation with checked total length and capacity.
    pub fn encode_account_with_depth(
        &self,
        discriminator: [u8; 8],
        value: &Value,
        storage_len: usize,
        max_depth: usize,
    ) -> Result<Vec<u8>, Error> {
        check_discriminator(discriminator)?;
        wire_size(storage_len)?;
        let root = self.encode_with_depth(value, max_depth)?;
        let used = add(12, root.len())?;
        if used > storage_len {
            return Err(Error::Capacity);
        }
        let mut output = discriminator.to_vec();
        word(&mut output, used)?;
        append(&mut output, &root)?;
        output.resize(storage_len, 0);
        Ok(output)
    }

    /// Decodes an account, excluding spare bytes from root validation.
    pub fn decode_account(&self, expected: [u8; 8], bytes: &[u8]) -> Result<Account, Error> {
        self.decode_account_with_depth(expected, bytes, Self::DEFAULT_DEPTH)
    }

    /// Checks the discriminator, used extent, and full logical tree.
    pub fn decode_account_with_depth(
        &self,
        expected: [u8; 8],
        bytes: &[u8],
        max_depth: usize,
    ) -> Result<Account, Error> {
        check_discriminator(expected)?;
        self.check(max_depth)?;
        wire_size(bytes.len())?;
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != expected {
            return Err(Error::Discriminator);
        }
        let used_len = reader.word()?;
        if used_len < 12 || used_len > bytes.len() {
            return Err(Error::InvalidLength);
        }
        let value = self.decode_with_depth(&bytes[12..used_len], max_depth)?;
        Ok(Account {
            discriminator: expected,
            value,
            used_len,
            storage_len: bytes.len(),
        })
    }
}

fn check_discriminator(discriminator: [u8; 8]) -> Result<(), Error> {
    if discriminator == [0; 8] {
        Err(Error::Discriminator)
    } else {
        Ok(())
    }
}

fn append(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Error> {
    add(output.len(), bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn word(output: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    let value = u32::try_from(value).map_err(|_| Error::Overflow)?;
    append(output, &value.to_le_bytes())
}

// Building separate child buffers deliberately avoids the production planner's
// in-place directory synthesis and overlap movement.
fn elided(parts: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
    let mut output = Vec::new();
    let mut end = 0;
    for (index, part) in parts.iter().enumerate() {
        end = add(end, part.len())?;
        if index + 1 < parts.len() {
            word(&mut output, end)?;
        }
    }
    for part in parts {
        append(&mut output, part)?;
    }
    Ok(output)
}

fn encode(schema: &Schema, value: &Value) -> Result<Vec<u8>, Error> {
    match (schema, value) {
        (Schema::Unsigned(width), Value::Unsigned(value)) => {
            width.check_unsigned(*value)?;
            Ok(value.to_le_bytes()[..width.bytes()].to_vec())
        }
        (Schema::Signed(width), Value::Signed(value)) => {
            width.check_signed(*value)?;
            Ok(value.to_le_bytes()[..width.bytes()].to_vec())
        }
        (Schema::Bool, Value::Bool(value)) => Ok(vec![u8::from(*value)]),
        (Schema::Pubkey, Value::Pubkey(value)) => Ok(value.to_vec()),
        (Schema::Array(width), Value::Array(value)) => {
            if value.len() != *width as usize {
                return Err(Error::InvalidLength);
            }
            Ok(value.clone())
        }
        (Schema::Str, Value::Str(value)) => {
            wire_size(value.len())?;
            Ok(value.as_bytes().to_vec())
        }
        (Schema::Option(_), Value::Option(None)) => Ok(vec![0]),
        (Schema::Option(child), Value::Option(Some(value))) => {
            let mut output = vec![1];
            append(&mut output, &encode(child, value)?)?;
            Ok(output)
        }
        (Schema::Struct(fields), Value::Struct(values)) => {
            if fields.len() != values.len() {
                return Err(Error::TypeMismatch);
            }
            let mut output = Vec::new();
            let mut variables = Vec::new();
            for (field, value) in fields.iter().zip(values) {
                let bytes = encode(field, value)?;
                if field.fixed().is_some() {
                    append(&mut output, &bytes)?;
                } else {
                    variables.push(bytes);
                }
            }
            append(&mut output, &elided(&variables)?)?;
            Ok(output)
        }
        (Schema::List(child), Value::List(values)) => {
            let mut output = Vec::new();
            word(&mut output, values.len())?;
            let parts = values
                .iter()
                .map(|value| encode(child, value))
                .collect::<Result<Vec<_>, _>>()?;
            if child.fixed().is_some() {
                for part in parts {
                    append(&mut output, &part)?;
                }
            } else {
                append(&mut output, &elided(&parts)?)?;
            }
            Ok(output)
        }
        (Schema::Map(key_schema, child), Value::Map(entries)) => {
            let mut output = Vec::new();
            word(&mut output, entries.len())?;
            let mut payload = Vec::new();
            for (key, value) in entries {
                append(&mut output, &key_schema.encode(key)?)?;
                let bytes = encode(child, value)?;
                if child.fixed().is_some() {
                    append(&mut output, &bytes)?;
                } else {
                    append(&mut payload, &bytes)?;
                    word(&mut output, payload.len())?;
                }
            }
            append(&mut output, &payload)?;
            Ok(output)
        }
        _ => Err(Error::TypeMismatch),
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let bytes = self.remaining.get(..length).ok_or(Error::InvalidLength)?;
        self.remaining = &self.remaining[length..];
        Ok(bytes)
    }

    fn word(&mut self) -> Result<usize, Error> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| Error::InvalidLength)?) as usize)
    }
}

fn exact(bytes: &[u8], length: usize) -> Result<(), Error> {
    if bytes.len() == length {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn boolean(bytes: &[u8]) -> Result<bool, Error> {
    exact(bytes, 1)?;
    match bytes[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::InvalidBool),
    }
}

fn unsigned(bytes: &[u8]) -> u128 {
    bytes.iter().enumerate().fold(0, |value, (index, byte)| {
        value | (u128::from(*byte) << (8 * index))
    })
}

fn signed(bytes: &[u8]) -> i128 {
    let bits = unsigned(bytes);
    let shift = 128 - 8 * bytes.len();
    ((bits << shift) as i128) >> shift
}

// The input is only directory plus payload, with no count or fixed prefix.
fn segments(bytes: &[u8], count: usize) -> Result<Vec<&[u8]>, Error> {
    if count == 0 {
        return if bytes.is_empty() {
            Ok(Vec::new())
        } else {
            Err(Error::InvalidBoundary)
        };
    }
    let mut reader = Reader::new(bytes);
    let table = reader.take(mul(4, count - 1)?)?;
    let payload = reader.remaining;
    let mut ends = Reader::new(table);
    let mut start = 0;
    let mut parts = Vec::new();
    for index in 0..count {
        let end = if index + 1 == count {
            payload.len()
        } else {
            ends.word()?
        };
        parts.push(payload.get(start..end).ok_or(Error::InvalidBoundary)?);
        start = end;
    }
    Ok(parts)
}

fn decode(schema: &Schema, bytes: &[u8]) -> Result<Value, Error> {
    match schema {
        Schema::Unsigned(width) => {
            exact(bytes, width.bytes())?;
            Ok(Value::Unsigned(unsigned(bytes)))
        }
        Schema::Signed(width) => {
            exact(bytes, width.bytes())?;
            Ok(Value::Signed(signed(bytes)))
        }
        Schema::Bool => Ok(Value::Bool(boolean(bytes)?)),
        Schema::Pubkey => Ok(Value::Pubkey(
            bytes.try_into().map_err(|_| Error::InvalidLength)?,
        )),
        Schema::Array(width) => {
            exact(bytes, *width as usize)?;
            Ok(Value::Array(bytes.to_vec()))
        }
        Schema::Str => Ok(Value::Str(
            std::str::from_utf8(bytes)
                .map_err(|_| Error::InvalidUtf8)?
                .to_owned(),
        )),
        Schema::Option(child) => match bytes.split_first() {
            Some((0, rest)) => {
                exact(rest, 0)?;
                Ok(Value::Option(None))
            }
            Some((1, rest)) => Ok(Value::Option(Some(Box::new(decode(child, rest)?)))),
            Some(_) => Err(Error::InvalidTag),
            None => Err(Error::InvalidLength),
        },
        Schema::Struct(fields) => {
            let mut reader = Reader::new(bytes);
            let mut values = vec![None; fields.len()];
            let mut variables = Vec::new();
            for (index, field) in fields.iter().enumerate() {
                if let Some(width) = field.fixed() {
                    values[index] = Some(decode(field, reader.take(width)?)?);
                } else {
                    variables.push((index, field));
                }
            }
            let parts = segments(reader.remaining, variables.len())?;
            for ((index, field), part) in variables.into_iter().zip(parts) {
                values[index] = Some(decode(field, part)?);
            }
            Ok(Value::Struct(
                values
                    .into_iter()
                    .collect::<Option<_>>()
                    .ok_or(Error::InvalidSchema)?,
            ))
        }
        Schema::List(child) => {
            let mut reader = Reader::new(bytes);
            let count = reader.word()?;
            let parts = if let Some(width) = child.fixed() {
                exact(reader.remaining, mul(count, width)?)?;
                reader.remaining.chunks_exact(width).collect()
            } else {
                segments(reader.remaining, count)?
            };
            Ok(Value::List(
                parts
                    .into_iter()
                    .map(|part| decode(child, part))
                    .collect::<Result<_, _>>()?,
            ))
        }
        Schema::Map(key, child) => decode_map(key, child, bytes),
    }
}

fn decode_map(key_schema: &KeySchema, child: &Schema, bytes: &[u8]) -> Result<Value, Error> {
    let mut reader = Reader::new(bytes);
    let count = reader.word()?;
    let key_width = key_schema.width()?;
    let mut entries = BTreeMap::new();
    let mut previous = None;
    if let Some(width) = child.fixed() {
        let stride = key_width.checked_add(width).ok_or(Error::Overflow)?;
        exact(reader.remaining, mul(count, stride)?)?;
        for _ in 0..count {
            let key = key_schema.decode(reader.take(key_width)?)?;
            ordered(&mut previous, &key)?;
            entries.insert(key, decode(child, reader.take(width)?)?);
        }
    } else {
        let stride = key_width.checked_add(4).ok_or(Error::Overflow)?;
        let table = reader.take(mul(count, stride)?)?;
        let payload = reader.remaining;
        let mut table = Reader::new(table);
        let mut start = 0;
        for _ in 0..count {
            let key = key_schema.decode(table.take(key_width)?)?;
            ordered(&mut previous, &key)?;
            let end = table.word()?;
            let part = payload.get(start..end).ok_or(Error::InvalidBoundary)?;
            entries.insert(key, decode(child, part)?);
            start = end;
        }
        if start != payload.len() {
            return Err(Error::InvalidBoundary);
        }
    }
    Ok(Value::Map(entries))
}

fn ordered(previous: &mut Option<Key>, key: &Key) -> Result<(), Error> {
    if previous.as_ref().is_some_and(|previous| previous >= key) {
        return Err(Error::KeyOrder);
    }
    *previous = Some(key.clone());
    Ok(())
}

impl KeySchema {
    /// Encodes one native key independently of scalar encoding.
    pub fn encode(&self, key: &Key) -> Result<Vec<u8>, Error> {
        let width = self.width()?;
        match (self, key) {
            (Self::Unsigned(bits), Key::Unsigned(value)) => {
                bits.check_unsigned(*value)?;
                Ok(value.to_be_bytes()[16 - width..].to_vec())
            }
            (Self::Signed(bits), Key::Signed(value)) => {
                bits.check_signed(*value)?;
                let mut bytes = value.to_be_bytes()[16 - width..].to_vec();
                bytes[0] ^= 0x80;
                Ok(bytes)
            }
            (Self::Bool, Key::Bool(value)) => Ok(vec![u8::from(*value)]),
            (Self::Pubkey, Key::Pubkey(value)) => Ok(value.to_vec()),
            (Self::Array(_), Key::Array(value)) => {
                exact(value, width)?;
                Ok(value.clone())
            }
            _ => Err(Error::TypeMismatch),
        }
    }

    /// Decodes one exact, canonical map-key region to its native domain.
    pub fn decode(&self, bytes: &[u8]) -> Result<Key, Error> {
        exact(bytes, self.width()?)?;
        match self {
            Self::Unsigned(_) => {
                Ok(Key::Unsigned(bytes.iter().fold(0u128, |value, byte| {
                    (value << 8) | u128::from(*byte)
                })))
            }
            Self::Signed(_) => {
                let mut bytes = bytes.to_vec();
                bytes[0] ^= 0x80;
                bytes.reverse();
                Ok(Key::Signed(signed(&bytes)))
            }
            Self::Bool => Ok(Key::Bool(boolean(bytes)?)),
            Self::Pubkey => Ok(Key::Pubkey(
                bytes.try_into().map_err(|_| Error::InvalidLength)?,
            )),
            Self::Array(_) => Ok(Key::Array(bytes.to_vec())),
        }
    }
}
