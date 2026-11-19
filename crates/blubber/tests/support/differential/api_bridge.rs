//! Direct calls to each API route; expectations live in blubber-model.

use super::{Bridge, Input, NativeKey, Observed};
use blubber::{Entry, ErrorKind, ListMut, MapMut, OptionMut};
use blubber_model::{
    ApiEdit, ApiReturn, Callbacks, EntryMode, Patch, PathSegment as Segment, Selector,
};

fn patch<T: Bridge>(mut child: T::Mut<'_>, patch: &Patch) -> blubber::Result<()> {
    T::apply(T::loan(&mut child), &patch.path, &patch.edit)?;
    Ok(())
}

fn callback_error() -> blubber::Error {
    ErrorKind::InvalidPlan.into()
}

fn selected<T: Bridge>(child: Option<T::Mut<'_>>, change: &Patch) -> blubber::Result<ApiReturn> {
    let present = child.is_some();
    if let Some(child) = child {
        patch::<T>(child, change)?;
    }
    Ok(ApiReturn::Selected(present))
}

pub(super) fn option<T: Input>(
    mut handle: OptionMut<'_, T>,
    path: &[Segment],
    edit: &ApiEdit,
    calls: &mut Callbacks,
    destination: &mut Option<Vec<u8>>,
) -> blubber::Result<ApiReturn> {
    if let Some((Segment::Some, rest)) = path.split_first() {
        return T::api(handle.some_mut()?, rest, edit, calls, destination);
    }
    assert!(path.is_empty());
    let ApiEdit::Select {
        route,
        segment: Segment::Some,
        patch: change,
        ..
    } = edit
    else {
        panic!("option API operation")
    };
    match route {
        Selector::Borrowed => selected::<T>(handle.reborrow().get_mut()?, change),
        Selector::Consuming => selected::<T>(handle.get_mut()?, change),
        Selector::At => selected::<T>(Some(handle.some_mut()?), change),
        Selector::With => panic!("options have no scoped callback selector"),
    }
}

pub(super) fn list<T: Bridge>(
    mut handle: ListMut<'_, T>,
    path: &[Segment],
    edit: &ApiEdit,
    calls: &mut Callbacks,
    destination: &mut Option<Vec<u8>>,
) -> blubber::Result<ApiReturn> {
    if let Some((Segment::Index(index), rest)) = path.split_first() {
        return T::api(handle.at_mut(*index)?, rest, edit, calls, destination);
    }
    assert!(path.is_empty());
    match edit {
        ApiEdit::Fill(value) => {
            let arena = bumpalo::Bump::new();
            handle.fill(T::borrowed(value, &arena))?;
        }
        ApiEdit::Rewrite { values, keys: None } => {
            let arena = bumpalo::Bump::new();
            let inputs: Vec<_> = values.iter().map(|v| T::borrowed(v, &arena)).collect();
            handle.rewrite_from(&inputs)?;
        }
        ApiEdit::Walk {
            cursor,
            patches,
            fail_at,
        } => {
            if *cursor {
                let mut cursor = handle.reborrow().cursor_mut()?;
                let mut index = 0;
                while let Some(child) = cursor.next()? {
                    calls.walked.push(Segment::Index(index));
                    patch::<T>(child, &patches[index])?;
                    if *fail_at == Some(index) {
                        return Err(callback_error());
                    }
                    index += 1;
                }
                assert_eq!(index, patches.len());
                assert!(cursor.next()?.is_none());
            } else {
                let mut index = 0;
                handle.try_for_each_mut(|child| {
                    calls.visit += 1;
                    calls.walked.push(Segment::Index(index));
                    patch::<T>(T::loan(child), &patches[index])?;
                    let failed = *fail_at == Some(index);
                    index += 1;
                    if failed {
                        Err(callback_error())
                    } else {
                        Ok(())
                    }
                })?;
                assert_eq!(index, patches.len());
            }
        }
        ApiEdit::Select {
            route,
            segment: Segment::Index(index),
            patch: change,
            fail,
        } => {
            if *route == Selector::With {
                let result = handle.with_mut(*index, |child| {
                    calls.visit += 1;
                    patch::<T>(T::loan(child), change)?;
                    if *fail {
                        Err(callback_error())
                    } else {
                        Ok(())
                    }
                })?;
                return Ok(ApiReturn::Selected(result.is_some()));
            }
            return match route {
                Selector::Borrowed => selected::<T>(handle.reborrow().get_mut(*index)?, change),
                Selector::Consuming => selected::<T>(handle.get_mut(*index)?, change),
                Selector::At => selected::<T>(Some(handle.at_mut(*index)?), change),
                Selector::With => unreachable!(),
            };
        }
        ApiEdit::RemoveInto {
            segment: Segment::Index(index),
            capacity,
        } => {
            *destination = Some(vec![0xa5; *capacity]);
            let copied = handle.remove_into(*index, destination.as_mut().unwrap())?;
            return Ok(ApiReturn::Removed(Some(copied as usize)));
        }
        _ => panic!("list API operation"),
    }
    Ok(ApiReturn::Unit)
}

pub(super) fn map<K: NativeKey, T: Bridge>(
    mut handle: MapMut<'_, K, T>,
    path: &[Segment],
    edit: &ApiEdit,
    calls: &mut Callbacks,
    destination: &mut Option<Vec<u8>>,
) -> blubber::Result<ApiReturn> {
    if let Some((Segment::Key(key), rest)) = path.split_first() {
        return T::api(
            handle.at_mut(&K::owned(key))?,
            rest,
            edit,
            calls,
            destination,
        );
    }
    assert!(path.is_empty());
    match edit {
        ApiEdit::Fill(value) => {
            let arena = bumpalo::Bump::new();
            handle.fill_values(T::borrowed(value, &arena))?;
        }
        ApiEdit::Rewrite { values, keys } => {
            let arena = bumpalo::Bump::new();
            let inputs: Vec<_> = values.iter().map(|v| T::borrowed(v, &arena)).collect();
            if let Some(keys) = keys {
                let entries: Vec<_> = keys
                    .iter()
                    .zip(inputs)
                    .map(|(k, v)| (K::owned(k), v))
                    .collect();
                handle.rewrite_values_from(&entries)?;
            } else {
                handle.rewrite_values(&inputs)?;
            }
        }
        ApiEdit::Walk {
            cursor,
            patches,
            fail_at,
        } => {
            if *cursor {
                let mut cursor = handle.reborrow().cursor_mut()?;
                let mut index = 0;
                while let Some((key, child)) = cursor.next()? {
                    calls.walked.push(Segment::Key(K::value(key)));
                    patch::<T>(child, &patches[index])?;
                    if *fail_at == Some(index) {
                        return Err(callback_error());
                    }
                    index += 1;
                }
                assert_eq!(index, patches.len());
                assert!(cursor.next()?.is_none());
            } else {
                let mut index = 0;
                handle.try_for_each_mut(|key, child| {
                    calls.walked.push(Segment::Key(K::value(key)));
                    calls.visit += 1;
                    patch::<T>(T::loan(child), &patches[index])?;
                    let failed = *fail_at == Some(index);
                    index += 1;
                    if failed {
                        Err(callback_error())
                    } else {
                        Ok(())
                    }
                })?;
                assert_eq!(index, patches.len());
            }
        }
        ApiEdit::Select {
            route,
            segment: Segment::Key(key),
            patch: change,
            ..
        } => {
            let key = K::owned(key);
            return match route {
                Selector::Borrowed => selected::<T>(handle.get_mut(&key)?, change),
                Selector::Consuming => selected::<T>(handle.into_get_mut(&key)?, change),
                Selector::At => selected::<T>(Some(handle.at_mut(&key)?), change),
                Selector::With => panic!("maps have no scoped callback selector"),
            };
        }
        ApiEdit::RemoveInto {
            segment: Segment::Key(key),
            capacity,
        } => {
            *destination = Some(vec![0xa5; *capacity]);
            let copied = handle.remove_into(&K::owned(key), destination.as_mut().unwrap())?;
            return Ok(ApiReturn::Removed(copied.map(|n| n as usize)));
        }
        ApiEdit::Entry { key, consuming, .. } => {
            let key = K::owned(key);
            return if *consuming {
                entry::<K, T>(handle.into_entry(key)?, key, edit, calls)
            } else {
                entry::<K, T>(handle.entry(key)?, key, edit, calls)
            };
        }
        _ => panic!("map API operation"),
    }
    Ok(ApiReturn::Unit)
}

fn entry<K: NativeKey, T: Bridge>(
    entry: Entry<'_, K, T>,
    key: K,
    edit: &ApiEdit,
    calls: &mut Callbacks,
) -> blubber::Result<ApiReturn> {
    let ApiEdit::Entry {
        mode,
        insert,
        modify,
        after,
        fail_modify,
        ..
    } = edit
    else {
        unreachable!()
    };
    let arena = bumpalo::Bump::new();
    if matches!(mode, EntryMode::MatchInsert | EntryMode::MatchRemove) {
        return match entry {
            Entry::Occupied(entry) => {
                assert!(entry.key() == key);
                let old = if *mode == EntryMode::MatchInsert {
                    entry.insert(T::borrowed(insert, &arena))?
                } else {
                    entry.remove()?
                };
                let old = match T::removed(Some(old)) {
                    Observed::Old(old) => old,
                    Observed::Present(_) => None,
                    Observed::Unit => unreachable!(),
                };
                Ok(ApiReturn::Previous(old))
            }
            Entry::Vacant(entry) => {
                assert!(entry.key() == key);
                if *mode == EntryMode::MatchInsert {
                    let _child = entry.insert(T::borrowed(insert, &arena))?;
                }
                Ok(ApiReturn::Previous(None))
            }
        };
    }
    let entry = if *mode == EntryMode::ModifyInsertWith {
        entry.and_modify(|child| {
            calls.modify += 1;
            patch::<T>(T::loan(child), modify)?;
            if *fail_modify {
                Err(callback_error())
            } else {
                Ok(())
            }
        })?
    } else {
        entry
    };
    let child = match mode {
        EntryMode::Insert => entry.or_insert(T::borrowed(insert, &arena))?,
        EntryMode::InsertWith | EntryMode::ModifyInsertWith => entry.or_insert_with(|| {
            calls.make += 1;
            T::borrowed(insert, &arena)
        })?,
        EntryMode::Default => T::default_entry(entry)?,
        EntryMode::MatchInsert | EntryMode::MatchRemove => unreachable!(),
    };
    let initial = T::snapshot(&child)?;
    patch::<T>(child, after)?;
    Ok(ApiReturn::SelectedValue(initial))
}
