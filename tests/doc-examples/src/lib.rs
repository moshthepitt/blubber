//! Compiled user-document examples for this repository.

#![forbid(unsafe_code)]
#![cfg_attr(doctest, doc = include_str!(concat!(env!("OUT_DIR"), "/readme.md")))]

#[cfg(doctest)]
#[doc = include_str!(concat!(env!("OUT_DIR"), "/guide.md"))]
mod guide {}

#[cfg(doctest)]
#[doc = include_str!(concat!(env!("OUT_DIR"), "/recipes.md"))]
mod recipes {}

#[cfg(doctest)]
#[doc = include_str!(concat!(env!("OUT_DIR"), "/trust-model.md"))]
mod trust_model {}

#[cfg(doctest)]
#[doc = include_str!(concat!(env!("OUT_DIR"), "/performance.md"))]
mod performance {}

#[cfg(all(test, feature = "derive"))]
mod tests {
    use blubber::{ErrorKind, List, SliceStore, Str};

    #[blubber::schema(account)]
    struct Notes {
        messages: List<Str>,
        following: Str,
    }

    // docs:start indexed-append
    #[test]
    fn indexed_append_preserves_sibling() -> blubber::Result<()> {
        let init = NotesInit {
            messages: &["old"],
            following: "keep",
        };
        let used = Notes::encoded_len_of(&init)? as usize;
        let mut bytes = vec![0; used + 7];
        let mut store = SliceStore::new(&mut bytes);
        let mut notes = Notes::initialize(&mut store, init)?;
        let new = "new";
        notes.messages_mut().push(new)?;
        let growth = notes.commit()?.used_len as usize - used;
        assert_eq!(new.len(), 3);
        assert_eq!(growth, 7);
        let runtime_spare = 4;
        assert_eq!(growth - runtime_spare, 3);

        let notes = Notes::load(&bytes)?;
        assert_eq!(notes.messages().try_get(1)?, Some("new"));
        assert_eq!(notes.following(), "keep");
        Ok(())
    }
    // docs:end indexed-append

    // docs:start callback-progress
    #[test]
    fn list_callback_absence_and_partial_progress() -> blubber::Result<()> {
        let mut bytes = [0; 96];
        let mut store = SliceStore::new(&mut bytes);
        let mut notes = Notes::initialize(
            &mut store,
            NotesInit {
                messages: &["old"],
                following: "keep",
            },
        )?;
        let absent = notes.messages_mut().with_mut(9, |_| {
            Err::<(), blubber::Error>(ErrorKind::InvalidBoundary.into())
        })?;
        assert_eq!(absent, None);

        let edit = notes.messages_mut().with_mut(0, |message| {
            message.set("written")?;
            Err::<(), blubber::Error>(ErrorKind::InvalidBoundary.into())
        });
        assert_eq!(edit, Err(ErrorKind::InvalidBoundary.into()));
        assert_eq!(notes.messages().try_get(0)?, Some("written"));
        Ok(())
    }
    // docs:end callback-progress
}
