//! Typed encoding into the verified bulk layout's exact element loans.

use crate::schema::checked_add;
use crate::{ErrorKind, Result, Schema, View};

pub(crate) struct HeaderInsert {
    plan: blubber_core::raw::HeaderBulkPlan,
}

impl HeaderInsert {
    pub fn encoded_len(&self) -> u32 {
        self.plan.encoded_len()
    }

    pub fn prepare<T: Schema>(
        bytes: &[u8],
        layout: blubber_core::Layout,
        base: u32,
        index: Option<usize>,
        values: &[T::Init<'_>],
        poisoned: &mut bool,
    ) -> Result<Self> {
        let source = super::edit::list_pass::source(
            blubber_core::raw::HeaderBulkSource::check(bytes, &layout)
                .map_err(|error| error.rebase(base)),
            poisoned,
        )?;
        let count = source.count();
        let index =
            crate::list_bounds::checked_list_index(index.unwrap_or(count as usize), count, true)?;
        let added = blubber_core::checked_u32(values.len())?;
        let mut input = 0;
        for value in values {
            input = crate::schema::checked_add(input, T::measure(value)?)?;
        }
        let plan = super::edit::list_pass::source_at(
            source
                .plan(index, added, input)
                .map_err(|error| error.rebase(base)),
            poisoned,
        )?;
        Ok(Self { plan })
    }

    pub fn encode<T: Schema>(
        &self,
        values: &[T::Init<'_>],
        bytes: &mut [u8],
        poisoned: &mut bool,
    ) -> Result<()> {
        let mut layout = self.plan.begin(bytes, poisoned)?;
        for value in values {
            layout.reserve(T::measure(value)?)?;
        }
        let mut slots = layout.finish()?;
        for (i, value) in values.iter().enumerate() {
            let slot = slots.slot(i as u32);
            let written = T::encode_slot(value, slot)?;
            if written != slot.len() {
                return Err(ErrorKind::InvalidBoundary.into());
            }
        }
        Ok(())
    }
}

// Coordinates only: no storage borrow survives root growth or movement.
pub(crate) struct Insert {
    plan: blubber_core::raw::BulkListPlan,
}

impl Insert {
    pub fn encoded_len(&self) -> u32 {
        self.plan.encoded_len()
    }

    pub fn prepare<T: Schema>(
        view: View<'_>,
        index: Option<usize>,
        values: &[T::Init<'_>],
    ) -> Result<Self> {
        let source = view.relative(blubber_core::raw::BulkListSource::check(
            view.bytes(),
            &view.layout(),
        ))?;
        let count = source.count();
        let index =
            crate::list_bounds::checked_list_index(index.unwrap_or(count as usize), count, true)?;
        let added = blubber_core::checked_u32(values.len())?;
        let mut input_len = 0;
        for value in values {
            let len = T::measure(value)?;
            if T::FIXED_SIZE.is_some_and(|width| width != len) {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            input_len = checked_add(input_len, len)?;
        }
        let plan = view.relative(source.plan(index, added, input_len))?;
        Ok(Self { plan })
    }

    #[inline(always)]
    pub fn encode<T: Schema>(&self, values: &[T::Init<'_>], bytes: &mut [u8]) -> Result<()> {
        // Layout completes before any user encoder runs. The directory doubles
        // as the slot index, so neither phase needs allocation or length scratch.
        let mut slots = if T::FIXED_SIZE.is_some() {
            self.plan.fixed_layout(bytes)?
        } else {
            let mut layout = self.plan.begin(bytes)?;
            for value in values {
                layout.reserve(T::measure(value)?)?;
            }
            layout.finish()?
        };
        for (i, value) in values.iter().enumerate() {
            let slot = slots.slot(i as u32);
            let written = T::encode_slot(value, slot)?;
            if written != slot.len() {
                return Err(ErrorKind::InvalidBoundary.into());
            }
        }
        Ok(())
    }
}
