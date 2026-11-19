//! Generic tag and exact child loans for optional projected values.
#![allow(clippy::question_mark)]

use crate::encoding_product::Source;
use crate::{Error, ErrorKind, Result};
#[cfg(verus_only)]
use crate::{Layout, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Exact tagged bytes of an optional projected source.
pub open spec fn bytes<S: Source>(source: Option<&S>) -> Seq<u8> {
    match source { None => seq![0u8], Some(value) => seq![1u8] + value.bytes() }
}

/// A present child supplies its typed projection and codec contract.
pub open spec fn trusted<S: Source>(source: Option<&S>) -> bool {
    match source { None => true, Some(value) => value.trusted_input() }
}

/// Measures an option while retaining the original child call and first error.
pub open spec fn measurement<S: Source>(source: Option<&S>) -> Result<u32> {
    match source {
        None => Ok(1),
        Some(value) => match value.measurement() {
            Err(error) => Err(error),
            Ok(size) => if size < u32::MAX { Ok((size + 1) as u32) }
                else { Err(crate::Error::Fault { kind: crate::ErrorKind::ArithmeticOverflow, offset: u32::MAX }) },
        },
    }
}

/// Measures an option with its complete deterministic first-fault observation.
#[inline(always)]
pub fn measure<S: Source>(source: Option<&S>) -> (result: Result<u32>)
    requires trusted(source),
    ensures result.is_ok() == (bytes(source).len() <= u32::MAX),
        result.is_ok() ==> result.unwrap() == bytes(source).len(),
        result == measurement(source),
    no_unwind
{
    match source {
        None => Ok(1),
        Some(value) => {
            let size = match value.measure() { Ok(size) => size, Err(error) => return Err(error) };
            match size.checked_add(1) {
                Some(size) => Ok(size),
                None => Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
            }
        }
    }
}

/// Writes an option's tag and exact child loan with no allocation.
///
/// # Panics
/// The destination must have the previously measured length; a present child
/// must satisfy its documented encoding contract.
#[inline(always)]
pub fn encode<S: Source>(source: Option<&S>, destination: &mut [u8])
    requires trusted(source), old(destination).len() == bytes(source).len(),
    ensures final(destination)@ == bytes(source),
    no_unwind
{
    match source {
        None => crate::encoding_leaf::encode_array(&[0], destination),
        Some(value) => {
            let (tag, payload) = destination.split_at_mut(1);
            tag[0] = 1;
            value.encode(payload);
            proof { assert(destination@ =~= seq![1u8] + value.bytes()); }
        }
    }
}

/// Exact generic optional encoding establishes the independent schema relation.
///
/// Child validity comes from its library theorem or the explicit true-marker
/// implementer premise. Projection glue determines which child is supplied.
pub proof fn encoder_option_valid<S: Source>(source: Option<&S>, child: Layout,
    after: Seq<u8>, budget: nat)
    requires trusted(source), after == bytes(source), after.len() <= u32::MAX,
        1 < budget <= MAX_DEPTH,
        crate::layout::descriptor_valid(child, (budget - 1) as nat),
        match source { None => true, Some(value) => value.layout() == child
            && crate::validation::schema_valid(value.bytes(), child, (budget - 1) as nat, true) },
    ensures crate::validation::schema_valid(after, Layout::Option { value: &child }, budget, true),
{
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::validation::children_valid, 3);
    let layout = Layout::Option { value: &child };
    assert(crate::layout::descriptor_valid(layout, budget));
    crate::encoding_metadata::descriptor_budget(child, (budget - 1) as nat, (MAX_DEPTH - 1) as nat);
    assert(crate::layout::descriptor_valid(layout, MAX_DEPTH as nat));
    if let Some(value) = source {
        assert(after.subrange(1, after.len() as int) =~= value.bytes());
        assert(crate::validation::local_valid(value.bytes(), child, true));
        match child.head() {
            crate::LayoutHead::Leaf(_) => {},
            crate::LayoutHead::Struct { .. } => {},
            _ => {},
        }
        assert(crate::layout::width_spec(child).is_some() ==>
            value.bytes().len() == crate::layout::width_spec(child).unwrap());
        assert(crate::leaves::option_shape(after, layout));
        assert(crate::validation::child_valid(after, layout, 0, (budget - 1) as nat, true));
        assert(crate::validation::child_count(after, layout) == 1);
        reveal_with_fuel(crate::validation::children_valid, 2);
        assert(crate::validation::children_valid(after, layout, 0, (budget - 1) as nat, true));
    } else {
        assert(after == seq![0u8]);
        assert(crate::validation::child_count(after, layout) == 0);
        reveal_with_fuel(crate::validation::children_valid, 1);
        assert(crate::validation::children_valid(after, layout, 0, (budget - 1) as nat, true));
    }
    assert(crate::validation::local_valid(after, layout, true));
}





/// Present and absent cases use actual primitive sources and the generic writer.
#[cfg(verus_only)]
pub(crate) fn option_witness()
    ensures true,
{
    proof { reveal(crate::layout_head::head_spec); }
    let child = crate::encoding_leaf::boolean(&[1]);
    let mut present = [19u8; 2];
    encode(Some(&child), &mut present);
    proof {
        crate::encoding_leaf::encoder_leaf_valid(&child, child.bytes(), 1);
        encoder_option_valid(Some(&child), child.layout(), present@, 2);
    }
    let mut absent = [19u8; 1];
    encode::<crate::encoding_leaf::Array<'_, 1>>(None, &mut absent);
    proof { encoder_option_valid::<crate::encoding_leaf::Array<'_, 1>>(None, child.layout(), absent@, 2); }
    assert(present@ == seq![1u8, 1u8] && absent@ == seq![0u8]);
}

}
