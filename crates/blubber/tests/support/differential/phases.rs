//! Full-buffer correspondence of strict, detached and immediate production plans.
#![forbid(unsafe_code)]

use blubber::RootSchema;
use blubber_core::raw::{Apply, InlinePath, ListEdit, MapEdit, Prepared, Step};
use blubber_core::Layout;
use blubber_model::{Edit, Model, PathSegment, Schema as ModelSchema, Value};

use super::oracle;

enum Operation {
    Replace,
    List(ListEdit),
    Map(MapEdit, Vec<u8>),
}

fn prepare<'a, 'key>(
    operation: &'key Operation,
    bytes: &'a mut [u8],
    layout: &Layout,
    path: &InlinePath,
    len: u32,
) -> Prepared<'a, 'key> {
    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let resolved = blubber_core::raw::resolve_edit(bytes, used, layout, path).unwrap();
    match operation {
        Operation::Replace => resolved.prepare_replace(len, u32::MAX).unwrap(),
        Operation::List(op) => resolved.prepare_list(*op, len, u32::MAX).unwrap(),
        Operation::Map(op, key) => resolved.prepare_map(*op, key, len, u32::MAX).unwrap(),
    }
}

fn finish(mut apply: Apply<'_, '_>, input: &[u8]) {
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();
}

pub(super) fn compare<S: RootSchema>(
    before: &[u8],
    old: &Model,
    path: &[PathSegment],
    edit: &Edit,
    after: &[u8],
    new: &Model,
) {
    let mut inline = InlinePath::new();
    let (mut schema, mut value) = (old.schema(), old.value());
    for segment in path {
        let step = match segment {
            PathSegment::Field(i) => Step::Field((*i).try_into().unwrap()),
            PathSegment::Index(i) => Step::Index((*i).try_into().unwrap()),
            PathSegment::Some => Step::Some,
            PathSegment::Key(key) => {
                let Value::Map(entries) = value else {
                    panic!("map path");
                };
                Step::Index(
                    entries
                        .keys()
                        .position(|k| k == key)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                )
            }
        };
        inline.push(step).unwrap();
        (schema, value) = oracle::at(schema, value, std::slice::from_ref(segment));
    }
    let (operation, input) = match (schema, edit) {
        (ModelSchema::List(child), Edit::ListInsert { index, values }) if values.len() == 1 => (
            Operation::List(ListEdit::Insert(*index as u32)),
            child.encode(&values[0]).unwrap(),
        ),
        (ModelSchema::List(_), Edit::ListRemove(index)) => {
            (Operation::List(ListEdit::Remove(*index as u32)), vec![])
        }
        (ModelSchema::List(_), Edit::ListPop) => (Operation::List(ListEdit::Pop), vec![]),
        (ModelSchema::List(_), Edit::ListTruncate(len)) => (
            Operation::List(ListEdit::Truncate((*len).min(u32::MAX as usize) as u32)),
            vec![],
        ),
        (ModelSchema::List(_), Edit::Clear) => (Operation::List(ListEdit::Clear), vec![]),
        (ModelSchema::Map(key_schema, child), Edit::MapInsert { key, value }) => (
            Operation::Map(MapEdit::Insert, key_schema.encode(key).unwrap()),
            child.encode(value).unwrap(),
        ),
        (ModelSchema::Map(key_schema, _), Edit::MapRemove(key)) => (
            Operation::Map(MapEdit::Remove, key_schema.encode(key).unwrap()),
            vec![],
        ),
        (ModelSchema::Map(_, _), Edit::Clear) => (Operation::Map(MapEdit::Clear, vec![]), vec![]),
        _ => {
            let (_, value) = oracle::at(new.schema(), new.value(), path);
            (Operation::Replace, schema.encode(value).unwrap())
        }
    };
    let layout = Layout::Flat {
        checked: S::__layout(),
    };
    for mode in 0..3 {
        let mut bytes = before.to_vec();
        bytes.resize(after.len(), 0);
        let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let len = input.len() as u32;
        if mode == 0 {
            let plan = match &operation {
                Operation::Replace => {
                    blubber_core::raw::plan_replace(&bytes, used, &layout, &inline, len, u32::MAX)
                        .unwrap()
                }
                Operation::List(op) => blubber_core::raw::plan_list_edit(
                    &bytes,
                    used,
                    &layout,
                    &inline,
                    *op,
                    len,
                    u32::MAX,
                )
                .unwrap(),
                Operation::Map(op, key) => blubber_core::raw::plan_map_edit(
                    &bytes,
                    used,
                    &layout,
                    &inline,
                    *op,
                    key,
                    len,
                    u32::MAX,
                )
                .unwrap(),
            };
            finish(
                blubber_core::raw::begin_apply(&mut bytes, plan).unwrap(),
                &input,
            );
        } else if mode == 1 {
            let prepared = prepare(&operation, &mut bytes, &layout, &inline, len);
            finish(prepared.begin_prepared().unwrap(), &input);
        } else {
            let plan = prepare(&operation, &mut bytes, &layout, &inline, len).detach();
            finish(
                blubber_core::raw::begin_apply(&mut bytes, plan).unwrap(),
                &input,
            );
        }
        assert_eq!(
            bytes, after,
            "strict/detached/prepared mode {mode}: {path:?} {edit:?}"
        );
    }
}
