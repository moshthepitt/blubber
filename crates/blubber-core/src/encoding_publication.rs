//! Library-owned native handoff and prefix publication transitions.
//!
//! Safe SDK acquisition and store operations remain outside Verus. These
//! transitions preserve encoding-error precedence and publish only after every
//! returned acquisition/admission check succeeds. Host unwind cleanup keeps the
//! prefix withdrawn; runtime aborts keep the existing instruction rollback.
#![allow(clippy::question_mark)]

use vstd::prelude::*;

verus! {

/// Initialization checks selected before any projected codec runs.
#[derive(Clone, Copy)]
pub struct InitPolicy {
    /// Full post-write validation, including every opaque graph.
    pub scan: bool,
    /// Repeated native measurement, preserving opaque effects and faults.
    pub replay: bool,
}

/// Selects proved replay reduction and authenticated lazy post-write admission.
///
/// Checked ingress and feature-on initialization always keep their full scan.
/// Both graph contracts are required before either check can be removed.
#[inline(always)]
pub const fn init_policy(markers: crate::encoding_product::Markers, checked: bool, verify: bool)
    -> (policy: InitPolicy)
    ensures policy.scan == (checked || verify || !(markers.encoder && markers.replay)),
        policy.replay == !(markers.encoder && markers.replay),
    no_unwind
{
    let covered = crate::encoding_product::contract(markers);
    InitPolicy { scan: checked || verify || !covered, replay: !covered }
}

/// Covered lazy admission has the same validation and replay observations.
///
/// The constructor trace comes from the generic writer proofs, not a validity
/// premise. Prepared input owns the same projection across SDK handoffs. The
/// common continuation includes prefix publication, errors, unwind and poison;
/// SDK operations and trusted typed projections remain outside this proof.
#[cfg(verus_only)]
pub proof fn encoder_lazy_init_policy_equivalent<I: crate::encoding_replay::Input, O>(
    prepared: crate::encoding_replay::Prepared<I>, graph: crate::encoding_graph::Graph,
    budget: nat, markers: crate::encoding_product::Markers, policy: InitPolicy,
    continuation: spec_fn(bool, crate::Result<u32>) -> O)
    requires prepared.valid(), crate::encoding_graph::trace(graph, budget),
        crate::encoding_replay::source(prepared).bytes() == graph.bytes,
        (markers.encoder && markers.replay),
        policy.scan == !(markers.encoder && markers.replay),
        policy.replay == !(markers.encoder && markers.replay),
    ensures !policy.scan && !policy.replay,
        crate::validation::schema_valid(graph.bytes, graph.layout, budget, true),
        continuation(crate::validation::schema_valid(graph.bytes, graph.layout, budget, true),
            crate::encoding_replay::source(prepared).measurement())
            == continuation(true, Ok(crate::encoding_replay::length(prepared))),
{
    crate::encoding_graph::encoder_graph_valid(graph, budget);
    crate::encoding_replay::encoder_prepared_input_stable(prepared,
        crate::encoding_replay::source(prepared), crate::encoding_replay::length(prepared));
}

/// Checked, feature-on and opaque routes retain their required observations.
///
/// Opaque replay invokes the original callback sequence, represented by an
/// arbitrary observation type. No purity or writer premise is placed on it.
#[cfg(verus_only)]
pub proof fn encoder_checked_fallback_preserved<O>(markers: crate::encoding_product::Markers,
    checked: bool, verify: bool, policy: InitPolicy, retained: O, reduced: O)
    requires policy.scan == (checked || verify || !(markers.encoder && markers.replay)),
        policy.replay == !(markers.encoder && markers.replay),
    ensures checked || verify || !(markers.encoder && markers.replay) ==> policy.scan,
        !(markers.encoder && markers.replay) ==> policy.replay
            && (if policy.replay { retained } else { reduced }) == retained,
{ }

/// Concrete writer, prepared input and executable policy establish lazy use.
#[cfg(verus_only)]
pub(crate) fn lazy_init_policy_witness()
    ensures true,
{
    use crate::encoding_replay::{Input, Prepared, Projected};
    let input = Projected { value: crate::encoding_leaf::raw(&[3u8, 5]) };
    let prepared = match Prepared::new(input) { Ok(p) => p, Err(_) => { assert(false); return; } };
    let ghost original = prepared;
    let markers = crate::encoding_product::Markers { encoder: true, replay: true };
    let policy = init_policy(markers, false, false);
    let mut bytes = [0u8; 2];
    let (source, length) = prepared.into_measured();
    source.encode(&mut bytes);
    let ghost graph = crate::encoding_graph::Graph {
        layout: crate::Layout::Leaf(crate::Leaf::Raw(2)), bytes: bytes@,
        before: seq![0u8, 0], children: Seq::empty(), keys: Seq::empty(),
    };
    proof {
        crate::encoding_leaf::encoder_leaf_valid(&source.value, bytes@, 1);
        reveal(crate::layout_head::head_spec);
        reveal(crate::validation::schema_valid);
        reveal(crate::validation::local_valid);
        assert(crate::encoding_graph::trace(graph, 1));
        encoder_lazy_init_policy_equivalent(original, graph, 1, markers, policy,
            |valid: bool, measured: crate::Result<u32>| (valid, measured));
    }
    assert(length == 2 && !policy.scan && !policy.replay);
}

/// Both partial markers, checked admission and feature-on admission stay strict.
#[cfg(verus_only)]
pub(crate) fn checked_fallback_witness()
    ensures true,
{
    let yes = crate::encoding_product::Markers { encoder: true, replay: true };
    let no = crate::encoding_product::Markers { encoder: false, replay: true };
    let partial = crate::encoding_product::Markers { encoder: true, replay: false };
    let opaque = init_policy(no, false, false);
    let other = init_policy(partial, false, false);
    let checked = init_policy(yes, true, false);
    let feature = init_policy(yes, false, true);
    proof {
        encoder_checked_fallback_preserved(no, false, false, opaque, seq![7int, 9], Seq::empty());
        encoder_checked_fallback_preserved(partial, false, false, other, seq![7int, 9], Seq::empty());
        encoder_checked_fallback_preserved(yes, true, false, checked, true, false);
        encoder_checked_fallback_preserved(yes, false, true, feature, true, false);
    }
    assert(opaque.scan && opaque.replay && other.scan && other.replay);
    assert(checked.scan && !checked.replay && feature.scan && !feature.replay);
}

/// Withdraws typed access before releasing a native lease.
#[inline(always)]
pub fn withdraw<P: Copy>(new: &mut bool, prefix: &mut Option<P>)
    ensures !*final(new), final(prefix).is_none(),
    no_unwind
{
    *new = false;
    *prefix = None;
}

/// Returned handoff result, preserving an earlier encoding error.
pub open spec fn settled<R, E>(encoded: core::result::Result<R, E>, acquired: core::result::Result<(), E>)
    -> core::result::Result<R, E> {
    match encoded { Err(error) => Err(error), Ok(receipt) => match acquired {
        Err(error) => Err(error), Ok(()) => Ok(receipt),
    } }
}

/// Combines encoding and reacquisition without losing the first returned error.
#[inline(always)]
pub fn settle<R, E>(encoded: core::result::Result<R, E>, acquired: core::result::Result<(), E>)
    -> (result: core::result::Result<R, E>)
    ensures result == settled(encoded, acquired),
{
    match encoded { Err(error) => Err(error), Ok(receipt) => match acquired {
        Err(error) => Err(error), Ok(()) => Ok(receipt),
    } }
}

/// Publishes a fully admitted copied prefix, leaving failure terminal.
#[inline(always)]
pub fn publish<P: Copy, E: core::fmt::Debug>(prefix: &mut Option<P>, admitted: core::result::Result<P, E>)
    -> (result: core::result::Result<(), E>)
    requires old(prefix).is_none(),
    ensures result.is_ok() == admitted.is_ok(),
        admitted.is_ok() ==> result == Ok(()),
        admitted.is_ok() ==> *final(prefix) == Some(admitted.unwrap()),
        admitted.is_err() ==> final(prefix).is_none() && result.err() == admitted.err(),
    no_unwind
{
    let value = match admitted { Ok(value) => value, Err(error) => return Err(error) };
    *prefix = Some(value);
    Ok(())
}

/// Native publication preserves replay equality and all failure outcomes.
///
/// `settle` is used after actual SDK reacquisition; `publish` is used after
/// policy admission and copied-prefix validation. Before those transitions,
/// prefix withdrawal covers returned failures and host unwinding. Neither
/// transition can clear store poison or create root-history evidence.
pub proof fn encoder_init_publication<R, P, E: core::fmt::Debug>(encoded: core::result::Result<R, E>,
    acquired: core::result::Result<(), E>, admitted: core::result::Result<P, E>,
    retained: core::result::Result<R, E>, elided: core::result::Result<R, E>,
    prefix: Option<P>)
    requires retained == settled(encoded, acquired), elided == settled(encoded, acquired),
        prefix == (if retained.is_ok() && admitted.is_ok() { Some(admitted.unwrap()) } else { None }),
    ensures retained == elided,
        prefix.is_some() == (encoded.is_ok() && acquired.is_ok() && admitted.is_ok()),
        encoded.is_err() ==> retained.err() == encoded.err() && prefix.is_none(),
        encoded.is_ok() && acquired.is_err() ==> retained.err() == acquired.err() && prefix.is_none(),
        retained.is_ok() && admitted.is_err() ==> prefix.is_none(),
{ }

/// Actual transitions exercise success, both error priorities and admission.
#[cfg(verus_only)]
pub(crate) fn init_publication_witness()
    ensures true,
{
    let mut new = true; let mut prefix = Some(7u32);
    withdraw(&mut new, &mut prefix);
    assert(!new && prefix.is_none());
    let encoded: core::result::Result<u32, u32> = Ok(23);
    let acquired: core::result::Result<(), u32> = Ok(());
    let first = settle(encoded, acquired); let second = settle(encoded, acquired);
    let admitted: core::result::Result<u32, u32> = Ok(7);
    let published = publish(&mut prefix, admitted); assert(published == Ok(()));
    proof { encoder_init_publication(encoded, acquired, admitted, first, second, prefix); }
    let bad: core::result::Result<u32, u32> = Err(31);
    let lost: core::result::Result<(), u32> = Err(37);
    let a = settle(bad, lost); let b = settle(bad, lost);
    proof { encoder_init_publication(bad, lost, admitted, a, b, None); }
    assert(a == Err(31));
    let a = settle(encoded, lost); let b = settle(encoded, lost);
    proof { encoder_init_publication(encoded, lost, admitted, a, b, None); }
    assert(a == Err(37));
    let mut denied: Option<u32> = None;
    let invalid: core::result::Result<u32, u32> = Err(41);
    let rejected = publish(&mut denied, invalid); assert(rejected == Err(41));
    proof { encoder_init_publication(encoded, acquired, invalid, first, second, denied); }
}

}
