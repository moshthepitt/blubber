//! Concrete replay tapes: model encodings and resolved edits, without generators.
#![forbid(unsafe_code)]

use blubber_model::{Edit, PathSegment as Segment, Schema};
use serde_json::{json, Value as Json};

fn number(value: &Json) -> usize {
    value.as_u64().unwrap().try_into().unwrap()
}
fn bytes(value: &Json) -> Vec<u8> {
    serde_json::from_value(value.clone()).unwrap()
}

fn select<'a>(schema: &'a Schema, path: &Json) -> (&'a Schema, Vec<Segment>) {
    let mut schema = schema;
    let mut segments = vec![];
    for step in path.as_array().unwrap() {
        let kind = step[0].as_str().unwrap();
        let (next, segment) = match (schema, kind) {
            (Schema::Struct(fields), "field") => {
                let index = number(&step[1]);
                (&fields[index], Segment::Field(index))
            }
            (Schema::List(child), "index") => (&**child, Segment::Index(number(&step[1]))),
            (Schema::Option(child), "some") => (&**child, Segment::Some),
            (Schema::Map(key, child), "key") => (
                &**child,
                Segment::Key(key.decode(&bytes(&step[1])).unwrap()),
            ),
            _ => panic!("regression path does not match schema"),
        };
        schema = next;
        segments.push(segment);
    }
    (schema, segments)
}

pub(crate) fn encode(schema: &Schema, path: &[Segment], edit: &Edit) -> Json {
    let mut schema = schema;
    let path: Vec<_> = path
        .iter()
        .map(|segment| {
            let (next, encoded) = match (schema, segment) {
                (Schema::Struct(fields), Segment::Field(i)) => (&fields[*i], json!(["field", i])),
                (Schema::List(child), Segment::Index(i)) => (&**child, json!(["index", i])),
                (Schema::Option(child), Segment::Some) => (&**child, json!(["some"])),
                (Schema::Map(key, child), Segment::Key(k)) => {
                    (&**child, json!(["key", key.encode(k).unwrap()]))
                }
                _ => panic!("regression path"),
            };
            schema = next;
            encoded
        })
        .collect();
    let edit = match edit {
        Edit::Set(value) => json!(["set", schema.encode(value).unwrap()]),
        Edit::ListInsert { index, values } => {
            let Schema::List(child) = schema else {
                panic!("list edit")
            };
            json!([
                "insert",
                index,
                values
                    .iter()
                    .map(|v| child.encode(v).unwrap())
                    .collect::<Vec<_>>()
            ])
        }
        Edit::ListRemove(i) => json!(["remove", i]),
        Edit::ListPop => json!(["pop"]),
        Edit::ListTruncate(n) => json!(["truncate", n]),
        Edit::Clear => json!(["clear"]),
        Edit::StringPush(text) => json!(["push", text]),
        Edit::MapInsert { key, value } => {
            let Schema::Map(k, child) = schema else {
                panic!("map edit")
            };
            json!([
                "upsert",
                k.encode(key).unwrap(),
                child.encode(value).unwrap()
            ])
        }
        Edit::MapRemove(key) => {
            let Schema::Map(k, _) = schema else {
                panic!("map edit")
            };
            json!(["erase", k.encode(key).unwrap()])
        }
    };
    json!({ "path": path, "edit": edit })
}

pub(crate) fn decode(schema: &Schema, action: &Json) -> (Vec<Segment>, Edit) {
    let (schema, path) = select(schema, &action["path"]);
    let edit = &action["edit"];
    let edit = match edit[0].as_str().unwrap() {
        "set" => Edit::Set(schema.decode(&bytes(&edit[1])).unwrap()),
        "insert" => {
            let Schema::List(child) = schema else {
                panic!("list edit")
            };
            Edit::ListInsert {
                index: number(&edit[1]),
                values: edit[2]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| child.decode(&bytes(v)).unwrap())
                    .collect(),
            }
        }
        "remove" => Edit::ListRemove(number(&edit[1])),
        "pop" => Edit::ListPop,
        "truncate" => Edit::ListTruncate(number(&edit[1])),
        "clear" => Edit::Clear,
        "push" => Edit::StringPush(edit[1].as_str().unwrap().to_owned()),
        "upsert" => {
            let Schema::Map(k, child) = schema else {
                panic!("map edit")
            };
            Edit::MapInsert {
                key: k.decode(&bytes(&edit[1])).unwrap(),
                value: child.decode(&bytes(&edit[2])).unwrap(),
            }
        }
        "erase" => {
            let Schema::Map(k, _) = schema else {
                panic!("map edit")
            };
            Edit::MapRemove(k.decode(&bytes(&edit[1])).unwrap())
        }
        _ => panic!("unknown regression edit"),
    };
    (path, edit)
}

pub(crate) fn directory() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/differential")
}

pub(crate) fn save(id: usize, schema: &Schema, initial: &[u8], relocating: bool, actions: &[Json]) {
    let tape = json!({ "version": 1, "schema": format!("{schema:?}"), "initial": initial,
                       "relocating": relocating, "actions": actions });
    let path = directory().join(format!("fixture-{id:02}.json"));
    std::fs::write(&path, serde_json::to_vec_pretty(&tape).unwrap()).unwrap();
    eprintln!("Concrete counterexample saved to {}", path.display());
}
