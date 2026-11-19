//! Typed staging for checked list and map edits.
use super::*;

impl AccountEdit<'_> {
    // Owned codecs can return wide values. Keep staging outside their callers'
    // SBF frames while sharing the same lazy/checked policy dispatch.
    #[cfg(feature = "alloc")]
    #[cfg_attr(target_os = "solana", inline(never))]
    pub(crate) fn owned_list_encoded<R>(
        &mut self,
        operation: ListRequest,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        self.dispatch_list_encoded(operation, measure, read_removed, encode)
    }

    #[inline(always)]
    pub(crate) fn dispatch_list_encoded<R>(
        &mut self,
        operation: ListRequest,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        if self.checked_list_enabled() {
            let child = self.collection_child()?;
            self.execute_checked_list(
                operation,
                measure,
                #[inline(always)]
                |span, bytes| read_removed(removed_view(span, bytes, child)?),
                encode,
            )
        } else {
            self.list_encoded(operation, measure, read_removed, encode)
        }
    }

    #[inline(always)]
    pub(super) fn checked_list_enabled(&self) -> bool {
        if self.lazy && self.selected.header(0) == 7 {
            let child = self.selected.child(0, true);
            let tag = child.header(0);
            !(matches!(tag, 1..=4) || (tag == 9 && child.header(4) == 0))
        } else {
            false
        }
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        not(all(target_os = "solana", feature = "max-depth-16")),
        inline(always)
    )]
    pub(super) fn execute_checked_list<R>(
        &mut self,
        operation: ListRequest,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        self.ingress = None;
        let mut stage = ListStage {
            removed: Err(ErrorKind::InvalidPlan.into()),
            encode: Some(encode),
        };
        let detached = stage_checked_list(
            self.store.edit_access(),
            (self.discriminator, self.minimum),
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            operation,
            measure,
            read_removed,
            &mut stage,
        )?;
        if let Some(plan) = detached.as_ref() {
            let encode = stage.encode.take().expect("one detached list encoder");
            self.apply_detached_list(plan, encode)?;
        }
        stage.removed
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        not(all(target_os = "solana", feature = "max-depth-16")),
        inline(always)
    )]
    fn apply_detached_list(
        &mut self,
        plan: &blubber_core::raw::CheckedListPlan,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<()> {
        let required = plan.required_len() as usize;
        if let Err(error) = self.store.resize(required) {
            let access = self.store.edit_access();
            plan.check_detached_error(access.bytes, access.poisoned)?;
            return Err(error);
        }
        self.store.set_poisoned(true);
        let mut poisoned = true;
        let result = complete_list_pass(
            plan.begin(&mut self.store.bytes_mut()[..required], &mut poisoned),
            &mut poisoned,
            encode,
        );
        self.store.set_poisoned(result.is_err());
        result
    }
}

impl AccountEdit<'_> {
    #[inline(always)]
    pub(crate) fn dispatch_map_encoded<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        if self.checked_map_enabled() {
            let child = self.collection_child()?;
            self.execute_checked_map(
                operation,
                key,
                None,
                measure,
                {
                    #[inline(always)]
                    |span, bytes| read_removed(removed_view(span, bytes, child)?)
                },
                encode,
            )
        } else {
            self.map_encoded(operation, key, measure, read_removed, encode)
        }
    }

    #[inline(always)]
    pub(super) fn dispatch_map_entry_encoded<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        hint: blubber_core::Search,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        if self.checked_map_enabled() {
            let child = self.collection_child()?;
            self.execute_checked_map(
                operation,
                key,
                Some(hint),
                measure,
                {
                    #[inline(always)]
                    |span, bytes| read_removed(removed_view(span, bytes, child)?)
                },
                encode,
            )
        } else {
            self.strict_map_entry_encoded(operation, key, hint, measure, read_removed, encode)
        }
    }

    #[cfg_attr(target_os = "solana", inline(never))]
    fn strict_map_entry_encoded<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        hint: blubber_core::Search,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<View<'_>>) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        self.map_encoded_at(operation, key, Some(hint), measure, read_removed, encode)
    }

    #[inline(always)]
    pub(super) fn checked_map_enabled(&self) -> bool {
        if self.lazy && self.selected.header(0) == 8 {
            let child = self.selected.child(0, true);
            let tag = child.header(0);
            !(matches!(tag, 1..=4) || (tag == 9 && child.header(4) == 0))
        } else {
            false
        }
    }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(
        not(all(target_os = "solana", feature = "max-depth-16")),
        inline(always)
    )]
    pub(super) fn execute_checked_map<R>(
        &mut self,
        operation: blubber_core::raw::MapEdit,
        key: &[u8],
        hint: Option<blubber_core::Search>,
        measure: impl FnOnce() -> Result<u32>,
        read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
        encode: impl FnOnce(&mut [u8]),
    ) -> Result<R> {
        check_state(self.store, false)?;
        if self.path_error.is_some() {
            self.root_view()?;
            self.check_path()?;
        }
        self.ingress = None;
        let mut stage = ListStage {
            removed: Err(ErrorKind::InvalidPlan.into()),
            encode: Some(encode),
        };
        let detached = stage_checked_map(
            self.store.edit_access(),
            (self.discriminator, self.minimum),
            &Layout::Flat {
                checked: self.layout,
            },
            &self.path,
            operation,
            key,
            hint,
            measure,
            read_removed,
            &mut stage,
        )?;
        if let Some(plan) = detached.as_ref() {
            let encode = stage.encode.take().expect("one detached map encoder");
            self.apply_detached_list(plan, encode)?;
        }
        stage.removed
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
#[allow(clippy::too_many_arguments)]
fn stage_checked_map<'key, R, E: FnOnce(&mut [u8])>(
    access: crate::storage::private::EditAccess<'_>,
    (discriminator, minimum): ([u8; 8], u32),
    layout: &Layout,
    path: &InlinePath,
    operation: blubber_core::raw::MapEdit,
    key: &'key [u8],
    hint: Option<blubber_core::Search>,
    measure: impl FnOnce() -> Result<u32>,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<Option<blubber_core::raw::CheckedListPlan<'key>>> {
    let resolved = match blubber_core::raw::resolve_account_edit(
        access.bytes,
        discriminator,
        minimum,
        layout,
        path,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return source(Err(error), access.poisoned),
    };
    let input = measure()?;
    let prepared =
        resolved.prepare_map_checked(operation, key, input, u32::MAX, hint, access.poisoned);
    let limits = (access.hard_limit, access.limit, access.limit_error);
    match (blubber_core::MAX_DEPTH, core::mem::size_of::<R>()) {
        (8, 0..=256) => finish_checked_map_small(
            prepared,
            limits,
            #[cfg(feature = "pinocchio")]
            access.preflight,
            access.poisoned,
            read_removed,
            stage,
        ),
        _ => finish_checked_list(
            prepared,
            limits,
            #[cfg(feature = "pinocchio")]
            access.preflight,
            access.poisoned,
            read_removed,
            stage,
        ),
    }
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    not(all(target_os = "solana", feature = "max-depth-16")),
    inline(always)
)]
fn finish_checked_map_small<'key, R, E: FnOnce(&mut [u8])>(
    prepared: Result<blubber_core::raw::CheckedListPrepared<'_, 'key>>,
    limits: (usize, usize, ErrorKind),
    #[cfg(feature = "pinocchio")] preflight: crate::storage::private::Preflight<'_>,
    poisoned: &mut bool,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<Option<blubber_core::raw::CheckedListPlan<'key>>> {
    // Exported values need a separate publication frame; unit Removed stays local.
    if core::mem::size_of::<R>() > 1 {
        return finish_checked_list(
            prepared,
            limits,
            #[cfg(feature = "pinocchio")]
            preflight,
            poisoned,
            read_removed,
            stage,
        );
    }
    finish_list_body::<false, _, _>(
        prepared,
        limits,
        #[cfg(feature = "pinocchio")]
        preflight,
        poisoned,
        read_removed,
        stage,
    )
}

struct ListStage<R, E> {
    removed: Result<R>,
    encode: Option<E>,
}

#[inline(always)]
pub(crate) fn source<T>(result: Result<T>, poisoned: &mut bool) -> Result<T> {
    if matches!(
        result,
        Err(Error::Fault {
            kind: ErrorKind::Truncated
                | ErrorKind::Discriminator
                | ErrorKind::UsedLength
                | ErrorKind::ArithmeticOverflow
                | ErrorKind::InvalidBoundary
                | ErrorKind::InvalidBool
                | ErrorKind::InvalidUtf8
                | ErrorKind::InvalidTag
                | ErrorKind::UnsortedKeys
                | ErrorKind::DuplicateKey,
            ..
        })
    ) {
        // control: list-source-poison
        *poisoned = true;
    }
    result
}

#[cold]
#[inline(never)]
pub(crate) fn poison_source_error(lazy: bool, error: Error, poisoned: &mut bool) {
    if lazy {
        let _ = source::<()>(Err(error), poisoned);
    }
}

// Keep the original Result and borrow policy only on its error branch.
macro_rules! source_try {
    ($lazy:expr, $result:expr, $poisoned:expr $(,)?) => {
        $result.inspect_err(|error| {
            $crate::edit::list_pass::poison_source_error($lazy, *error, $poisoned);
        })?
    };
}
pub(crate) use source_try;

#[inline(always)]
pub(crate) fn source_at<T>(result: Result<T>, poisoned: &mut bool) -> Result<T> {
    if let Err(Error::Fault { offset, .. }) = &result {
        if *offset != Error::NO_OFFSET {
            return source(result, poisoned);
        }
    }
    result
}

#[cfg_attr(target_os = "solana", inline(never))]
#[allow(clippy::too_many_arguments)]
fn stage_checked_list<R, E: FnOnce(&mut [u8])>(
    access: crate::storage::private::EditAccess<'_>,
    (discriminator, minimum): ([u8; 8], u32),
    layout: &Layout,
    path: &InlinePath,
    operation: ListRequest,
    measure: impl FnOnce() -> Result<u32>,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<Option<blubber_core::raw::CheckedListPlan<'static>>> {
    let resolved = source(
        blubber_core::raw::resolve_account_edit(access.bytes, discriminator, minimum, layout, path),
        access.poisoned,
    )?;
    let count = source(
        blubber_core::read_u32(resolved.inspect_resolved(), resolved.selection().0.start),
        access.poisoned,
    )?;
    let operation = operation.checked(count)?;
    let input = measure()?;
    let prepared = resolved.prepare_list_checked(operation, input, u32::MAX, access.poisoned);
    let limits = (access.hard_limit, access.limit, access.limit_error);
    // Small values at the default depth share the existing staging frame.
    // Wider values and deeper paths need separate decoding/movement frames.
    match (blubber_core::MAX_DEPTH, core::mem::size_of::<R>()) {
        (8, 0..=256) => finish_list_body::<false, _, _>(
            prepared,
            limits,
            #[cfg(feature = "pinocchio")]
            access.preflight,
            access.poisoned,
            read_removed,
            stage,
        ),
        _ => finish_checked_list(
            prepared,
            limits,
            #[cfg(feature = "pinocchio")]
            access.preflight,
            access.poisoned,
            read_removed,
            stage,
        ),
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
fn finish_checked_list<'key, R, E: FnOnce(&mut [u8])>(
    prepared: Result<blubber_core::raw::CheckedListPrepared<'_, 'key>>,
    limits: (usize, usize, ErrorKind),
    #[cfg(feature = "pinocchio")] preflight: crate::storage::private::Preflight<'_>,
    poisoned: &mut bool,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<Option<blubber_core::raw::CheckedListPlan<'key>>> {
    finish_list_body::<true, _, _>(
        prepared,
        limits,
        #[cfg(feature = "pinocchio")]
        preflight,
        poisoned,
        read_removed,
        stage,
    )
}

#[inline(always)]
fn finish_list_body<'key, const SPLIT: bool, R, E: FnOnce(&mut [u8])>(
    prepared: Result<blubber_core::raw::CheckedListPrepared<'_, 'key>>,
    (hard_limit, limit, limit_error): (usize, usize, ErrorKind),
    #[cfg(feature = "pinocchio")] preflight: crate::storage::private::Preflight<'_>,
    poisoned: &mut bool,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<Option<blubber_core::raw::CheckedListPlan<'key>>> {
    let prepared = prepared?;
    let required = prepared.prepared_len() as usize;
    if required > hard_limit {
        prepared.check_before_error(poisoned)?;
        return Err(ErrorKind::Capacity.into());
    }
    if required > limit {
        prepared.check_before_error(poisoned)?;
        return Err(limit_error.into());
    }
    #[cfg(feature = "pinocchio")]
    if required > blubber_core::read_u32(prepared.inspect(), 8)? as usize {
        if let Err(error) = preflight.check(required) {
            prepared.check_before_error(poisoned)?;
            return Err(error);
        }
    }
    let removed = if SPLIT {
        read_list_removed(&prepared, poisoned, read_removed, stage)
    } else {
        read_list_removed_inline(&prepared, poisoned, read_removed, stage)
    };
    if let Err(error) = removed {
        prepared.check_before_error(poisoned)?;
        return Err(error);
    }
    if required > prepared.inspect().len() {
        return Ok(Some(prepared.detach()));
    }
    *poisoned = true;
    let encode = stage.encode.take().expect("one immediate list encoder");
    if SPLIT {
        execute_prepared_list(prepared, poisoned, encode)?;
    } else {
        complete_list_pass(prepared.begin_immediate(), poisoned, encode)?;
    }
    *poisoned = false;
    Ok(None)
}

// Keep a wide decoded value out of the frame that owns the movement plan.
#[cfg_attr(target_os = "solana", inline(never))]
fn read_list_removed<R, E>(
    prepared: &blubber_core::raw::CheckedListPrepared<'_, '_>,
    poisoned: &mut bool,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<()> {
    read_list_removed_inline(prepared, poisoned, read_removed, stage)
}

#[inline(always)]
fn read_list_removed_inline<R, E>(
    prepared: &blubber_core::raw::CheckedListPrepared<'_, '_>,
    poisoned: &mut bool,
    read_removed: impl FnOnce(Option<Span>, &[u8]) -> Result<R>,
    stage: &mut ListStage<R, E>,
) -> Result<()> {
    stage.removed = read_removed(prepared.removed(), prepared.inspect());
    source(
        stage.removed.as_ref().map(|_| ()).map_err(|error| *error),
        poisoned,
    )
}

#[cfg_attr(target_os = "solana", inline(never))]
fn execute_prepared_list(
    prepared: blubber_core::raw::CheckedListPrepared<'_, '_>,
    poisoned: &mut bool,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    complete_list_pass(prepared.begin_immediate(), poisoned, encode)
}

// Consuming phase transitions stay in one publication frame.
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(
    not(all(target_os = "solana", feature = "max-depth-16")),
    inline(always)
)]
fn complete_list_pass(
    apply: Result<blubber_core::raw::CheckedListReady<'_, '_>>,
    poisoned: &mut bool,
    encode: impl FnOnce(&mut [u8]),
) -> Result<()> {
    apply
        .and_then(|apply| apply.move_regions(poisoned))
        .and_then(|moved| {
            let mut apply = moved.destination();
            encode(apply.bytes());
            apply.finish()
        })
}
