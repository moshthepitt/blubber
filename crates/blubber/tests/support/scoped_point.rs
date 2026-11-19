//! Scoped point edits share the ordinary child selection and failure frames.

use blubber::{ErrorKind, List, Resize, SliceStore, Str};

#[blubber::schema(account)]
struct Words {
    words: List<Str>,
    numbers: List<u16>,
    tail: Str,
}

fn init() -> WordsInit<'static> {
    WordsInit {
        words: &["one", "two", "end"],
        numbers: &[10, 20],
        tail: "suffix",
    }
}

#[test]
fn scoped_point_span_identity_and_completed_edits() {
    for index in 0..5 {
        let mut bytes = [0; 128];
        let mut store = SliceStore::new(&mut bytes);
        let mut root = Words::initialize(&mut store, init()).unwrap();
        let mut calls = 0;
        let result = root
            .words_mut()
            .with_mut(index, |word| {
                calls += 1;
                assert_eq!(word.get(), ["one", "two", "end"][index]);
                word.set("new")?;
                Ok(word.get().len())
            })
            .unwrap();
        assert_eq!(calls, usize::from(index < 3));
        assert_eq!(result, (index < 3).then_some(3));
        for (i, value) in ["one", "two", "end"].iter().enumerate() {
            assert_eq!(
                root.words().get(i),
                Some(if index == i { "new" } else { value })
            );
        }
        assert_eq!(root.tail(), "suffix");
        let failure = blubber::Error::from(ErrorKind::Capacity);
        assert_eq!(
            root.words_mut().with_mut(1, |word| {
                word.set("yes")?;
                Err::<(), _>(failure)
            }),
            Err(failure)
        );
        assert_eq!(root.words().get(1), Some("yes"));
        assert_eq!(
            root.numbers_mut().with_mut(1, |value| {
                value.set(42)?;
                Ok(value.get())
            }),
            Ok(Some(42))
        );
        root.validate().unwrap();
        root.commit().unwrap();
        assert!(!store.is_poisoned());
    }
}

#[test]
fn scoped_point_equal_size_check_and_growth_refresh() {
    let mut bytes = [0; 256];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = Words::initialize(&mut store, init()).unwrap();
    root.words_mut()
        .with_mut(1, |word| {
            word.set("a longer string")?;
            assert_eq!(word.get(), "a longer string");
            word.set("x")?;
            assert_eq!(word.get(), "x");
            word.reborrow().set("y")?;
            Ok(())
        })
        .unwrap();
    assert_eq!(root.words().iter().collect::<Vec<_>>(), ["one", "y", "end"]);
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
    root.commit().unwrap();
}

#[test]
fn scoped_point_accepts_once_callbacks_and_nested_children() {
    let mut bytes = [0; 512];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = super::Catalog::initialize(&mut store, super::init()).unwrap();
    let returned = String::from("owned result");
    assert_eq!(
        root.records_mut().with_mut(0, |record| {
            record.name_mut().set("A")?;
            record.tags_mut().with_mut(0, |tag| tag.set(7))?;
            record.name_mut().push_str(" longer")?;
            Ok(returned)
        }),
        Ok(Some(String::from("owned result")))
    );
    assert_eq!(root.records().get(0).unwrap().name(), "A longer");
    assert_eq!(root.records().get(0).unwrap().tags().get(0), Some(7));
    assert_eq!(root.records().get(2).unwrap().name(), "last");
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
}

#[test]
fn scoped_point_matches_ordinary_selection_for_every_byte_change() {
    let mut original = [0; 80];
    Words::initialize(&mut SliceStore::new(&mut original), init())
        .unwrap()
        .commit()
        .unwrap();
    for at in 0..60 {
        for byte in [0, 1, 127, 255] {
            for index in [0, 1, 2, 3, usize::MAX] {
                let mut ordinary = original;
                ordinary[at] = byte;
                let mut scoped = ordinary;
                let a = {
                    let mut store = SliceStore::new(&mut ordinary);
                    Words::load_trusted_mut(&mut store).and_then(|mut root| {
                        root.words_mut()
                            .get_mut(index)?
                            .map(|mut word| word.set("new"))
                            .transpose()
                    })
                };
                let b = {
                    let mut store = SliceStore::new(&mut scoped);
                    Words::load_trusted_mut(&mut store).and_then(|mut root| {
                        root.words_mut().with_mut(index, |word| word.set("new"))
                    })
                };
                assert_eq!(a, b, "offset={at} byte={byte} index={index}");
                assert_eq!(ordinary, scoped, "offset={at} byte={byte} index={index}");
            }
        }
    }
}

#[test]
fn scoped_point_absence_and_callback_unwind_keep_state() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = Words::initialize(&mut store, init()).unwrap();
    assert_eq!(
        root.words_mut()
            .with_mut(usize::MAX, |_| panic!("absent child")),
        Ok(None::<()>)
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        root.words_mut().with_mut(0, |word| -> blubber::Result<()> {
            word.set("new")?;
            panic!("callback after completed edit");
        })
    }));
    assert!(result.is_err());
    assert_eq!(root.words().get(0), Some("new"));
    root.validate().unwrap();
    root.commit().unwrap();
    assert!(!store.is_poisoned());
}

#[test]
fn scoped_point_poison_precedes_absence_and_projection_errors() {
    use super::Interrupt;
    blubber::__root_schema!(__PointPoison, List<List<Interrupt>>);
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    let edit =
        blubber::raw::Edit::initialize::<__PointPoison>(&mut store, *b"loanpois", &[&[1], &[2]])
            .unwrap();
    let mut lists = blubber::raw::project::<List<List<Interrupt>>>(edit);
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        lists.with_mut(0, |child| child.push(99))
    }));
    assert!(error.is_err());
    assert_eq!(
        lists.with_mut(usize::MAX, |_| Ok(())),
        Err(ErrorKind::Poisoned.into())
    );
    assert_eq!(
        lists.with_mut(0, |_| Ok(())),
        Err(ErrorKind::Poisoned.into())
    );
    assert!(store.is_poisoned());
}
