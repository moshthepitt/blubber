//! Logical edits by tree selection, followed by complete reference re-encoding.

use crate::{Error, Key, Schema, Value};

/// One selection in a logical tree; no segment contains a wire offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathSegment {
    /// Struct field in declaration order, regardless of its packed position.
    Field(usize),
    /// Existing list element by ordinal.
    Index(usize),
    /// Existing map value by native key.
    Key(Key),
    /// Present option payload.
    Some,
}

/// A logical operation on the selected node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// Replaces the whole selected value and returns its previous contents.
    Set(Value),
    /// Inserts a run before an ordinal, including the append position.
    ListInsert {
        /// Insertion ordinal in 0..=count.
        index: usize,
        /// Complete logical elements; an empty run still checks the ordinal.
        values: Vec<Value>,
    },
    /// Removes and returns an existing list element.
    ListRemove(usize),
    /// Removes and returns the final element, or returns `None` for empty lists.
    ListPop,
    /// Retains at most the requested list length.
    ListTruncate(usize),
    /// Inserts or replaces a map value and returns the previous value if present.
    MapInsert {
        /// Native key in the map's declared key domain.
        key: Key,
        /// Complete logical value.
        value: Value,
    },
    /// Removes a key and returns its value, or `None` if absent.
    MapRemove(Key),
    /// Clears a list, map, string, or option; returns no old value.
    Clear,
    /// Appends UTF-8 to a string.
    StringPush(String),
}

/// Validated owned tree and its schema, independent of production edit plans.
///
/// Each edit clones the tree, changes standard collections, and checks the
/// entire new encoding before publishing. Errors preserve both value and bytes.
/// Returned old values are always owned, even where production `Removed` is
/// `()`. The oracle models logical effects, not moves, poison, or rent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    schema: Schema,
    value: Value,
    max_depth: usize,
}

impl Model {
    /// Validates an initial tree with the default depth budget.
    pub fn new(schema: Schema, value: Value) -> Result<Self, Error> {
        Self::with_depth(schema, value, Schema::DEFAULT_DEPTH)
    }

    /// Validates an initial tree with an explicit budget in 1..=16.
    pub fn with_depth(schema: Schema, value: Value, max_depth: usize) -> Result<Self, Error> {
        schema.encode_with_depth(&value, max_depth)?;
        Ok(Self {
            schema,
            value,
            max_depth,
        })
    }

    /// Returns the immutable logical root.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Returns the immutable owned schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Rebuilds canonical root bytes, excluding an account envelope.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.schema.encode_with_depth(&self.value, self.max_depth)
    }

    /// Applies one edit atomically; an empty path selects the root.
    ///
    /// `Set`, remove, pop, and map insertion return owned previous content when
    /// present. Other operations return `None`.
    pub fn apply(&mut self, path: &[PathSegment], edit: Edit) -> Result<Option<Value>, Error> {
        self.apply_with_limit(path, edit, u32::MAX as usize)
    }

    /// Applies an edit only if the new exact root encoding fits `root_limit`.
    ///
    /// The limit excludes the twelve-byte envelope. This models a failed
    /// capacity preflight without reproducing a storage backend's algorithms.
    pub fn apply_with_limit(
        &mut self,
        path: &[PathSegment],
        edit: Edit,
        root_limit: usize,
    ) -> Result<Option<Value>, Error> {
        let mut next = self.value.clone();
        let (schema, selected) = select(&self.schema, &mut next, path)?;
        let previous = apply(schema, selected, edit)?;
        let bytes = self.schema.encode_with_depth(&next, self.max_depth)?;
        if bytes.len() > root_limit {
            return Err(Error::Capacity);
        }
        self.value = next;
        Ok(previous)
    }
}

pub(crate) fn select<'a>(
    mut schema: &'a Schema,
    mut value: &'a mut Value,
    path: &[PathSegment],
) -> Result<(&'a Schema, &'a mut Value), Error> {
    for segment in path {
        (schema, value) = match (segment, schema, value) {
            (PathSegment::Field(index), Schema::Struct(fields), Value::Struct(values)) => (
                fields.get(*index).ok_or(Error::Index)?,
                values.get_mut(*index).ok_or(Error::Index)?,
            ),
            (PathSegment::Index(index), Schema::List(child), Value::List(values)) => {
                (child.as_ref(), values.get_mut(*index).ok_or(Error::Index)?)
            }
            (PathSegment::Key(key), Schema::Map(key_schema, child), Value::Map(entries)) => {
                key_schema.encode(key)?;
                (child.as_ref(), entries.get_mut(key).ok_or(Error::Missing)?)
            }
            (PathSegment::Some, Schema::Option(child), Value::Option(value)) => {
                (child.as_ref(), value.as_deref_mut().ok_or(Error::Missing)?)
            }
            _ => return Err(Error::InvalidPath),
        };
    }
    Ok((schema, value))
}

fn apply(schema: &Schema, selected: &mut Value, edit: Edit) -> Result<Option<Value>, Error> {
    if let Edit::Set(value) = edit {
        return Ok(Some(std::mem::replace(selected, value)));
    }
    match (schema, selected, edit) {
        (
            Schema::List(_),
            Value::List(values),
            Edit::ListInsert {
                index,
                values: input,
            },
        ) => {
            if index > values.len() {
                return Err(Error::Index);
            }
            values.splice(index..index, input);
        }
        (Schema::List(_), Value::List(values), Edit::ListRemove(index)) => {
            if index >= values.len() {
                return Err(Error::Index);
            }
            return Ok(Some(values.remove(index)));
        }
        (Schema::List(_), Value::List(values), Edit::ListPop) => return Ok(values.pop()),
        (Schema::List(_), Value::List(values), Edit::ListTruncate(count)) => values.truncate(count),
        (Schema::Map(key_schema, _), Value::Map(entries), Edit::MapInsert { key, value }) => {
            key_schema.encode(&key)?;
            return Ok(entries.insert(key, value));
        }
        (Schema::Map(key_schema, _), Value::Map(entries), Edit::MapRemove(key)) => {
            key_schema.encode(&key)?;
            return Ok(entries.remove(&key));
        }
        (Schema::List(_), Value::List(values), Edit::Clear) => values.clear(),
        (Schema::Map(_, _), Value::Map(entries), Edit::Clear) => entries.clear(),
        (Schema::Str, Value::Str(value), Edit::Clear) => value.clear(),
        (Schema::Option(_), Value::Option(value), Edit::Clear) => *value = None,
        (Schema::Str, Value::Str(value), Edit::StringPush(suffix)) => value.push_str(&suffix),
        _ => return Err(Error::TypeMismatch),
    }
    Ok(None)
}
