//! Contracts for bulk edits, lending walks, selectors and entry chains.

use crate::{Edit, Error, Key, Model, PathSegment, Schema, Value};

/// One atomic logical child edit, relative to a lent projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    /// Logical selection below the child.
    pub path: Vec<PathSegment>,
    /// Operation applied after selection.
    pub edit: Edit,
}

/// Production selector route whose absence behavior is part of the contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selector {
    /// Borrow the parent before selecting.
    Borrowed,
    /// Consume the parent for a long-lived selection.
    Consuming,
    /// Require presence, returning an index or missing-value error otherwise.
    At,
    /// Lend a list child to a callback; absence skips the callback.
    With,
}

/// Entry construction and update routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryMode {
    /// Eager borrowed initializer.
    Insert,
    /// Initializer callback on vacancy only.
    InsertWith,
    /// Default initializer on vacancy only.
    Default,
    /// Occupied callback followed by a vacant initializer callback.
    ModifyInsertWith,
    /// Match the entry and replace or insert its value.
    MatchInsert,
    /// Match and remove an occupied value; vacancy is unchanged.
    MatchRemove,
}

/// A public API operation, separate from the storage implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiEdit {
    /// Replace every list element or map value with one logical value.
    Fill(Value),
    /// Replace values in ordinal order, optionally witnessing exact map keys.
    Rewrite {
        /// One input per existing element.
        values: Vec<Value>,
        /// `Some` uses keyed map rewrite; `None` uses list or keyless rewrite.
        keys: Option<Vec<Key>>,
    },
    /// Apply one patch per child through a cursor or callback walk.
    Walk {
        /// True chooses a lending cursor; false chooses callback traversal.
        cursor: bool,
        /// Patches in list ordinal or native map key order.
        patches: Vec<Patch>,
        /// Return a caller error after this ordinal's patch, retaining progress.
        fail_at: Option<usize>,
    },
    /// Select one child using a borrowed, consuming or callback projection.
    Select {
        /// Selector receiver and absence behavior.
        route: Selector,
        /// Child index, key or option payload.
        segment: PathSegment,
        /// Edit performed when the selection exists.
        patch: Patch,
        /// A callback returns an error after its edit.
        fail: bool,
    },
    /// Copy a child's model encoding to caller storage, then remove it.
    RemoveInto {
        /// List index or map key.
        segment: PathSegment,
        /// Destination length, initialized to `0xa5` in both oracles.
        capacity: usize,
    },
    /// Use an occupied or vacant entry, then reuse the returned projection.
    Entry {
        /// Native lookup key.
        key: Key,
        /// Entry combinator or matched entry method.
        mode: EntryMode,
        /// Vacant or replacement input; ignored by occupied combinators.
        insert: Value,
        /// Occupied callback edit.
        modify: Patch,
        /// Edit through the selected editor after the chain completes.
        after: Patch,
        /// Return a caller error after the occupied callback's edit.
        fail_modify: bool,
        /// Consume the map instead of borrowing it for entry selection.
        consuming: bool,
    },
}

/// Observable callback counts, including the call which returned an error.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Callbacks {
    /// List or map traversal calls and scoped child calls.
    pub visit: usize,
    /// Vacant initializer calls.
    pub make: usize,
    /// Occupied modification calls.
    pub modify: usize,
    /// Visited child indices or copied keys, in observed order.
    pub walked: Vec<PathSegment>,
}

/// API return values with native trees and exact copied bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiReturn {
    /// Successful operation without a return payload.
    Unit,
    /// Optional selection presence.
    Selected(bool),
    /// Value selected by an entry chain before reuse of its editor.
    SelectedValue(Value),
    /// Removed subtree length; absence applies only to maps.
    Removed(Option<usize>),
    /// Matched entry's old value, copied for leaves and discarded for containers.
    Previous(Option<Value>),
}

/// Expected result and side effects of an API operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiOutcome {
    /// Return value or contract error. Successful earlier patches remain.
    pub result: Result<ApiReturn, Error>,
    /// Calls required by the logical occupancy and traversal order.
    pub callbacks: Callbacks,
    /// Caller destination, including unchanged spare bytes and error frames.
    pub destination: Option<Vec<u8>>,
    /// Sticky session state after completion.
    pub poisoned: bool,
}

impl Model {
    /// Runs an API contract with an exact root capacity and prior poison state.
    ///
    /// Each patch uses the model's full re-encoding. Callback errors retain
    /// completed patches, including the patch in the failing callback. Caller
    /// and capacity errors never introduce poison. No production types are used.
    pub fn apply_api(
        &mut self,
        path: &[PathSegment],
        edit: &ApiEdit,
        root_limit: usize,
        poisoned: bool,
    ) -> ApiOutcome {
        let mut outcome = ApiOutcome {
            result: Ok(ApiReturn::Unit),
            callbacks: Callbacks::default(),
            destination: match edit {
                ApiEdit::RemoveInto { capacity, .. } => Some(vec![0xa5; *capacity]),
                _ => None,
            },
            poisoned,
        };
        outcome.result = if poisoned {
            Err(Error::Poisoned)
        } else {
            self.api(path, edit, root_limit, &mut outcome)
        };
        outcome
    }

    fn patch(&mut self, path: &[PathSegment], patch: &Patch, limit: usize) -> Result<(), Error> {
        let mut selected = path.to_vec();
        selected.extend_from_slice(&patch.path);
        self.apply_with_limit(&selected, patch.edit.clone(), limit)?;
        Ok(())
    }

    fn api(
        &mut self,
        path: &[PathSegment],
        edit: &ApiEdit,
        limit: usize,
        outcome: &mut ApiOutcome,
    ) -> Result<ApiReturn, Error> {
        let mut tree = self.value().clone();
        let (schema, value) = crate::edit::select(self.schema(), &mut tree, path)?;
        let (schema, value) = (schema.clone(), value.clone());
        match edit {
            ApiEdit::Fill(input) => {
                let replacement = match value {
                    Value::List(values) => Value::List(vec![input.clone(); values.len()]),
                    Value::Map(values) => {
                        Value::Map(values.into_keys().map(|key| (key, input.clone())).collect())
                    }
                    _ => return Err(Error::TypeMismatch),
                };
                self.apply_with_limit(path, Edit::Set(replacement), limit)?;
            }
            ApiEdit::Rewrite {
                values: inputs,
                keys,
            } => {
                let replacement = match value {
                    Value::List(values) if keys.is_none() && values.len() == inputs.len() => {
                        Value::List(inputs.clone())
                    }
                    Value::Map(values)
                        if values.len() == inputs.len()
                            && keys
                                .as_ref()
                                .is_none_or(|keys| keys.iter().eq(values.keys())) =>
                    {
                        Value::Map(values.into_keys().zip(inputs.iter().cloned()).collect())
                    }
                    _ => return Err(Error::InvalidBoundary),
                };
                self.apply_with_limit(path, Edit::Set(replacement), limit)?;
            }
            ApiEdit::Walk {
                cursor,
                patches,
                fail_at,
            } => {
                let children = match value {
                    Value::List(values) => (0..values.len()).map(PathSegment::Index).collect(),
                    Value::Map(values) => values.into_keys().map(PathSegment::Key).collect(),
                    _ => return Err(Error::TypeMismatch),
                };
                let children: Vec<_> = children;
                if patches.len() != children.len() {
                    return Err(Error::InvalidBoundary);
                }
                for (index, (segment, patch)) in children.into_iter().zip(patches).enumerate() {
                    let mut child = path.to_vec();
                    outcome.callbacks.walked.push(segment.clone());
                    child.push(segment);
                    if !cursor {
                        outcome.callbacks.visit += 1;
                    }
                    self.patch(&child, patch, limit)?;
                    if *fail_at == Some(index) {
                        return Err(Error::Callback);
                    }
                }
            }
            ApiEdit::Select {
                route,
                segment,
                patch,
                fail,
            } => {
                let mut child = path.to_vec();
                child.push(segment.clone());
                let mut selected_value = value.clone();
                let present = crate::edit::select(
                    &schema,
                    &mut selected_value,
                    std::slice::from_ref(segment),
                );
                match present {
                    Err(Error::Index | Error::Missing) if *route != Selector::At => {
                        return Ok(ApiReturn::Selected(false));
                    }
                    Err(error) => return Err(error),
                    Ok(_) => (),
                }
                if *route == Selector::With {
                    outcome.callbacks.visit += 1;
                }
                self.patch(&child, patch, limit)?;
                if *fail && *route == Selector::With {
                    return Err(Error::Callback);
                }
                return Ok(ApiReturn::Selected(true));
            }
            ApiEdit::RemoveInto { segment, capacity } => {
                let mut selected_value = value.clone();
                let selected = crate::edit::select(
                    &schema,
                    &mut selected_value,
                    std::slice::from_ref(segment),
                );
                let (child, value) = match selected {
                    Err(Error::Missing) if matches!(segment, PathSegment::Key(_)) => {
                        return Ok(ApiReturn::Removed(None));
                    }
                    other => other?,
                };
                let bytes = child.encode(value)?;
                if bytes.len() > *capacity {
                    return Err(Error::Capacity);
                }
                let remove = match segment {
                    PathSegment::Index(index) => Edit::ListRemove(*index),
                    PathSegment::Key(key) => Edit::MapRemove(key.clone()),
                    _ => return Err(Error::InvalidPath),
                };
                self.apply_with_limit(path, remove, limit)?;
                if let Some(destination) = &mut outcome.destination {
                    destination[..bytes.len()].copy_from_slice(&bytes);
                }
                return Ok(ApiReturn::Removed(Some(bytes.len())));
            }
            ApiEdit::Entry {
                key,
                mode,
                insert,
                modify,
                after,
                fail_modify,
                ..
            } => {
                let (Schema::Map(key_schema, child_schema), Value::Map(entries)) =
                    (&schema, &value)
                else {
                    return Err(Error::TypeMismatch);
                };
                key_schema.encode(key)?;
                let occupied = entries.get(key);
                let mut child = path.to_vec();
                child.push(PathSegment::Key(key.clone()));
                if *mode == EntryMode::MatchRemove {
                    let old = self.apply_with_limit(path, Edit::MapRemove(key.clone()), limit)?;
                    return Ok(ApiReturn::Previous(old.filter(|_| copied(child_schema))));
                }
                if *mode == EntryMode::ModifyInsertWith && occupied.is_some() {
                    outcome.callbacks.modify += 1;
                    self.patch(&child, modify, limit)?;
                    if *fail_modify {
                        return Err(Error::Callback);
                    }
                }
                let mut old = None;
                if occupied.is_none() || *mode == EntryMode::MatchInsert {
                    if matches!(mode, EntryMode::InsertWith | EntryMode::ModifyInsertWith) {
                        outcome.callbacks.make += 1;
                    }
                    let value = if *mode == EntryMode::Default {
                        default_value(child_schema)?
                    } else {
                        insert.clone()
                    };
                    old = self.apply_with_limit(
                        path,
                        Edit::MapInsert {
                            key: key.clone(),
                            value,
                        },
                        limit,
                    )?;
                }
                if *mode == EntryMode::MatchInsert {
                    return Ok(ApiReturn::Previous(old.filter(|_| copied(child_schema))));
                }
                let mut selected_tree = self.value().clone();
                let (_, initial) = crate::edit::select(self.schema(), &mut selected_tree, &child)?;
                let initial = initial.clone();
                self.patch(&child, after, limit)?;
                return Ok(ApiReturn::SelectedValue(initial));
            }
        }
        Ok(ApiReturn::Unit)
    }
}

// Removed is native for leaf schemas; containers and derived structs discard it.
fn copied(schema: &Schema) -> bool {
    matches!(
        schema,
        Schema::Unsigned(_) | Schema::Signed(_) | Schema::Bool | Schema::Pubkey | Schema::Array(_)
    )
}

/// Logical initializer defaults for the built-in schemas.
pub fn default_value(schema: &Schema) -> Result<Value, Error> {
    Ok(match schema {
        Schema::Unsigned(_) => Value::Unsigned(0),
        Schema::Signed(_) => Value::Signed(0),
        Schema::Bool => Value::Bool(false),
        Schema::Pubkey => Value::Pubkey([0; 32]),
        Schema::Array(n) => Value::Array(vec![0; *n as usize]),
        Schema::Str => Value::Str(String::new()),
        Schema::Option(_) => Value::Option(None),
        Schema::List(_) => Value::List(vec![]),
        Schema::Map(_, _) => Value::Map(Default::default()),
        Schema::Struct(_) => return Err(Error::TypeMismatch),
    })
}
