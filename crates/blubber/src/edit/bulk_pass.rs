//! Fallible bulk encoders complete before outer used-length publication.
use super::*;

impl AccountEdit<'_> {
    #[cfg_attr(target_os = "solana", inline(never))]
    pub(super) fn bulk_replace_with<I>(
        &mut self,
        measure: impl FnOnce(&[u8], Layout, u32, &mut bool) -> Result<(u32, I)>,
        encode: impl FnOnce(I, &mut [u8], &mut bool) -> Result<()>,
    ) -> Result<()> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        self.ingress = None;
        let mut stage = BulkStage {
            input: None,
            encode: Some(encode),
        };
        let detached = stage_bulk(
            self.store.edit_access(),
            (self.discriminator, self.minimum),
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            measure,
            &mut stage,
        )?;
        if let Some(required) = detached {
            let required = required as usize;
            if let Err(error) = self.store.resize(required) {
                let len = stage.input.as_ref().expect("one detached bulk input").1;
                let access = self.store.edit_access();
                let resolved = list_pass::source(
                    blubber_core::raw::resolve_account_edit(
                        access.bytes,
                        self.discriminator,
                        self.minimum,
                        &Layout::Flat {
                            checked: self.layout,
                        },
                        &self.path,
                    ),
                    access.poisoned,
                )?;
                resolved.check_ancestor_error(len, access.poisoned)?;
                return Err(error);
            }
            self.store.set_poisoned(true);
            let (input, len) = stage.input.take().expect("one detached bulk input");
            let encode = stage.encode.take().expect("one detached bulk encoder");
            let result = complete_bulk(
                self.store.edit_access(),
                (self.discriminator, self.minimum, len, required),
                &Layout::Flat {
                    checked: self.layout,
                },
                &self.path,
                input,
                encode,
            );
            self.store.set_poisoned(result.is_err());
            result?;
        }
        Ok(())
    }

    pub(crate) fn try_bulk_insert<T: Schema>(
        &mut self,
        index: Option<usize>,
        values: &[T::Init<'_>],
    ) -> Option<Result<()>> {
        if !self.checked_list_enabled() {
            return None;
        }
        Some(self.bulk_replace_with(
            |bytes, layout, base, poisoned| {
                let plan = crate::list_bulk::HeaderInsert::prepare::<T>(
                    bytes, layout, base, index, values, poisoned,
                )?;
                Ok((plan.encoded_len(), plan))
            },
            #[inline(always)]
            |plan, bytes, poisoned| plan.encode::<T>(values, bytes, poisoned),
        ))
    }
}

struct BulkStage<I, E> {
    input: Option<(I, u32)>,
    encode: Option<E>,
}

#[cfg_attr(target_os = "solana", inline(never))]
fn stage_bulk<I, E: FnOnce(I, &mut [u8], &mut bool) -> Result<()>>(
    access: crate::storage::private::EditAccess<'_>,
    (discriminator, minimum): ([u8; 8], u32),
    layout: &Layout,
    path: &InlinePath,
    measure: impl FnOnce(&[u8], Layout, u32, &mut bool) -> Result<(u32, I)>,
    stage: &mut BulkStage<I, E>,
) -> Result<Option<u32>> {
    let resolved = list_pass::source(
        blubber_core::raw::resolve_account_edit(access.bytes, discriminator, minimum, layout, path),
        access.poisoned,
    )?;
    let (span, selected) = resolved.selection();
    let (len, input) = measure(
        &resolved.inspect_resolved()[span.start as usize..span.end as usize],
        selected,
        span.start,
        access.poisoned,
    )?;
    if resolved.subtree_fits(len) && !resolved.has_ancestor_words() {
        let mut write = list_pass::source_at(resolved.prepare_subtree(len), access.poisoned)?;
        let required = write.required_len() as usize;
        if required > access.hard_limit {
            return Err(ErrorKind::Capacity.into());
        }
        if required > access.limit {
            return Err(access.limit_error.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > span.end - span.start {
            access.preflight.check(required)?;
        }
        *access.poisoned = true;
        stage.encode.take().expect("one bulk encoder")(
            input,
            write.destination(),
            access.poisoned,
        )?;
        write.finish();
        *access.poisoned = false;
        return Ok(None);
    }
    if resolved.subtree_fits(len) {
        let write = list_pass::source_at(
            resolved.prepare_subtree_checked(len, access.poisoned),
            access.poisoned,
        )?;
        let required = write.required_len() as usize;
        if required > access.hard_limit {
            write.check_before_error(access.poisoned)?;
            return Err(ErrorKind::Capacity.into());
        }
        if required > access.limit {
            write.check_before_error(access.poisoned)?;
            return Err(access.limit_error.into());
        }
        #[cfg(feature = "pinocchio")]
        if len > span.end - span.start {
            if let Err(error) = access.preflight.check(required) {
                write.check_before_error(access.poisoned)?;
                return Err(error);
            }
        }
        *access.poisoned = true;
        let mut write = write.move_regions(access.poisoned).destination();
        stage.encode.take().expect("one bulk encoder")(input, write.bytes(), access.poisoned)?;
        write.finish(access.poisoned)?;
        Ok(None)
    } else {
        let required = resolved.subtree_required(len)? as usize;
        if required > access.hard_limit {
            resolved.check_ancestor_error(len, access.poisoned)?;
            return Err(ErrorKind::Capacity.into());
        }
        if required > access.limit {
            resolved.check_ancestor_error(len, access.poisoned)?;
            return Err(access.limit_error.into());
        }
        #[cfg(feature = "pinocchio")]
        if let Err(error) = access.preflight.check(required) {
            resolved.check_ancestor_error(len, access.poisoned)?;
            return Err(error);
        }
        stage.input = Some((input, len));
        Ok(Some(required as u32))
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
fn complete_bulk<I>(
    access: crate::storage::private::EditAccess<'_>,
    (discriminator, minimum, len, required): ([u8; 8], u32, u32, usize),
    layout: &Layout,
    path: &InlinePath,
    input: I,
    encode: impl FnOnce(I, &mut [u8], &mut bool) -> Result<()>,
) -> Result<()> {
    let resolved = list_pass::source(
        blubber_core::raw::resolve_account_edit(access.bytes, discriminator, minimum, layout, path),
        access.poisoned,
    )?;
    if !resolved.has_ancestor_words() {
        let mut write = list_pass::source_at(resolved.prepare_subtree(len), access.poisoned)?;
        if write.required_len() as usize != required {
            return Err(ErrorKind::InvalidPlan.into());
        }
        encode(input, write.destination(), access.poisoned)?;
        write.finish();
        return Ok(());
    }
    let write = list_pass::source_at(
        resolved.prepare_subtree_checked(len, access.poisoned),
        access.poisoned,
    )?;
    if write.required_len() as usize != required {
        return Err(ErrorKind::InvalidPlan.into());
    }
    let mut write = write.move_regions(access.poisoned).destination();
    encode(input, write.bytes(), access.poisoned)?;
    write.finish(access.poisoned)
}

#[cfg(all(test, feature = "derive"))]
mod tests {
    use super::*;
    use crate::{ReadStorage, Resize, Schema, SliceStore};
    use core::cell::Cell;

    #[crate::schema(account)]
    struct ReopenFrame {
        values: crate::List<crate::Str>,
        suffix: crate::Str,
    }

    #[crate::schema(account)]
    struct ReopenPlain {
        values: crate::List<crate::Str>,
    }

    #[test]
    fn reopened_bulk_geometry_must_match_preflight_before_encoding() {
        for ancestors in [false, true] {
            for difference in [-1i64, 1] {
                let mut bytes = [0; 128];
                let mut store = SliceStore::new(&mut bytes);
                let (discriminator, minimum, layout) = if ancestors {
                    ReopenFrame::initialize(
                        &mut store,
                        ReopenFrameInit {
                            values: &["a"],
                            suffix: "tail",
                        },
                    )
                    .unwrap();
                    (
                        ReopenFrame::DISCRIMINATOR,
                        ReopenFrame::MIN_SIZE,
                        ReopenFrame::LAYOUT,
                    )
                } else {
                    ReopenPlain::initialize(&mut store, ReopenPlainInit { values: &["a"] })
                        .unwrap();
                    (
                        ReopenPlain::DISCRIMINATOR,
                        ReopenPlain::MIN_SIZE,
                        ReopenPlain::LAYOUT,
                    )
                };
                let before: [u8; 128] = store.bytes().try_into().unwrap();
                let actual = blubber_core::read_u32(store.bytes(), 8).unwrap() + 7;
                let expected = blubber_core::checked_delta(actual, difference).unwrap() as usize;
                let mut path = InlinePath::new();
                path.push(Step::Field(0)).unwrap();
                let called = Cell::new(false);
                let backend = crate::raw::backend(&mut store);
                backend.set_poisoned(true);
                let result = complete_bulk(
                    backend.edit_access(),
                    (discriminator, minimum, 12, expected),
                    layout,
                    &path,
                    (),
                    |(), _, _| {
                        called.set(true);
                        Ok(())
                    },
                );
                assert_eq!(result, Err(ErrorKind::InvalidPlan.into()));
                assert!(!called.get());
                assert_eq!(store.bytes(), before);
                assert!(store.is_poisoned());
            }
        }
    }
}
