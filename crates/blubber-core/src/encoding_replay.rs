//! Prepared source ownership and the retained/replayed encoder protocols.
//!
//! Typed projections implement `Input` under the same trusted boundary as
//! `encoding_product::Source`. Covered library operations are pure and total;
//! default-false callbacks keep their calls and lie outside these proofs.
#![allow(clippy::question_mark)]

use crate::Result;
use vstd::prelude::*;

verus! {

/// Immutable projected input for a complete encoder graph.
///
/// The trusted premise includes deterministic measurement/comparison without
/// observable effects, and exact panic-free encoding after successful preflight.
/// Library constructor proofs discharge those operations; typed projection and
/// handwritten true-marker contracts remain explicit trusted inputs.
pub trait Input {
    /// Trusted projection and complete child replay contracts.
    spec fn trusted_input(&self) -> bool;
    /// Exact output of the projected graph.
    spec fn bytes(&self) -> Seq<u8>;
    /// Complete first-fault observation, including its original offset.
    spec fn measurement(&self) -> Result<u32>;

    /// Measures without changing the input or external state.
    fn measure(&self) -> (result: Result<u32>)
        requires self.trusted_input(),
        ensures result == self.measurement(),
            result.is_ok() ==> result.unwrap() == self.bytes().len(),
        no_unwind;

    /// Writes exactly once into an exact successfully measured destination.
    fn encode(&self, destination: &mut [u8])
        requires self.trusted_input(), self.measurement().is_ok(),
            old(destination).len() == self.bytes().len(),
        ensures final(destination)@ == self.bytes(),
        no_unwind;
}

/// Owns the exact preflight input across library and safe SDK handoffs.
///
/// There is no mutable input accessor or way to substitute a different source.
/// Borrowed data remain shared for the token's lifetime. Interior effects in
/// default-false codecs are permitted at runtime and keep the retained replay.
pub struct Prepared<I: Input> {
    input: I,
    length: u32,
}

/// Exact source moved through preparation, with no reconstructed input.
pub closed spec fn source<I: Input>(prepared: Prepared<I>) -> I { prepared.input }

/// Captured successful preflight extent.
pub closed spec fn length<I: Input>(prepared: Prepared<I>) -> u32 { prepared.length }

impl<I: Input> Prepared<I> {
    /// Successful preflight attached to the exact owned source.
    #[verifier::type_invariant]
    pub closed spec fn valid(self) -> bool {
        self.input.trusted_input() && self.input.measurement() == Ok(self.length)
            && self.length == self.input.bytes().len()
    }

    /// Measures and captures this exact input; errors preserve the first fault.
    ///
    /// A failed measurement consumes the input and runs its ordinary destructor.
    /// Caller destruction remains part of the common protocol continuation;
    /// the borrowed measurement contract does not constrain that destructor.
    #[inline(always)]
    pub fn new(input: I) -> (result: Result<Self>)
        requires input.trusted_input(),
        ensures result.is_ok() == input.measurement().is_ok(),
            result.is_err() ==> result.err() == input.measurement().err(),
            result.is_ok() ==> source(result.unwrap()) == input
                && length(result.unwrap()) == input.measurement().unwrap() && result.unwrap().valid(),
    {
        let length = match input.measure() { Ok(n) => n, Err(error) => return Err(error) };
        Ok(Self { input, length })
    }

    /// Inspects the captured extent without running a codec.
    #[inline(always)]
    pub fn measured_len(&self) -> (result: u32)
        ensures result == length(*self),
        no_unwind
    { self.length }

    /// Retains the original second measurement and every opaque callback.
    #[inline(always)]
    pub fn replay(self) -> (result: Result<(I, u32)>)
        ensures result == Ok((source(self), length(self))),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        let length = match self.input.measure() { Ok(n) => n, Err(error) => return Err(error) };
        Ok((self.input, length))
    }

    /// Consumes the captured input without replay under its trusted contract.
    ///
    /// Native initialization selects this route only when both complete graph
    /// markers promise the trusted input contract. Opaque graphs call `replay`.
    #[inline(always)]
    pub fn into_measured(self) -> (result: (I, u32))
        ensures result == (source(self), length(self)),
        no_unwind
    { (self.input, self.length) }
}

/// Executes either protocol on the same source and exact destination loan.
///
/// # Panics
/// The trusted input, successful measurement and destination bounds must hold.
#[inline(always)]
pub fn write<I: Input>(input: &I, destination: &mut [u8], start: usize, measured: u32, replay: bool)
    -> (result: Result<()>)
    requires input.trusted_input(), input.measurement() == Ok(measured),
        measured == input.bytes().len(), start <= old(destination).len(),
        measured <= old(destination).len() - start,
    ensures result == Ok(()),
        final(destination)@ == crate::encoding_product::splice(old(destination)@, start as int, input.bytes()),
    no_unwind
{
    let width = if replay {
        match input.measure() { Ok(n) => n, Err(error) => return Err(error) }
    } else { measured };
    let (_, rest) = destination.split_at_mut(start);
    let (slot, _) = rest.split_at_mut(width as usize);
    input.encode(slot);
    assert(final(destination)@ =~= crate::encoding_product::splice(old(destination)@, start as int, input.bytes()));
    Ok(())
}

/// Immutable source identity transports bytes and errors across a handoff.
///
/// SDK operations are outside Verus. This proves the library-owned source
/// frame: an SDK borrow/resize/funding operation receives no mutable source.
pub proof fn encoder_prepared_input_stable<I: Input>(prepared: Prepared<I>, handed: I, measured: u32)
    requires prepared.valid(), handed == source(prepared), measured == length(prepared),
    ensures handed == source(prepared), handed.bytes() == source(prepared).bytes(),
        handed.measurement() == source(prepared).measurement(), measured == length(prepared),
        handed.trusted_input(), handed.measurement() == Ok(measured), handed.bytes().len() == measured,
{ }

/// Pure repeated graph measurement preserves the complete first-fault result.
pub proof fn encoder_graph_measure_pure<I: Input>(input: &I, first: Result<u32>, second: Result<u32>)
    requires input.trusted_input(), first == input.measurement(), second == input.measurement(),
    ensures first == second, first.is_err() ==> first.err() == second.err(),
{ }

/// Discharges repeated measurements for every library composition operation.
///
/// The actual drivers prove their result equations and `no_unwind`; this check
/// calls them rather than accepting lengths supplied by a caller. Quantified
/// child types provide induction over arbitrary graphs and cardinalities. The
/// child contracts retain the trusted typed-projection boundary. The result
/// equations include every error, offset and adjacent key comparison, even on
/// inputs whose preflight fails. Immutable borrows and the verified drivers
/// introduce no external effects. Primitive and UTF-8 implementations discharge
/// the child contract in their defining modules.
#[cfg(verus_only)]
pub(crate) fn constructors_measure_pure<S: crate::encoding_product::Source,
    F: crate::encoding_product::Fields, E: crate::encoding_list::Elements,
    M: crate::encoding_map::Entries>(child: &S, fields: &F, elements: &E, entries: &M,
    total: u32, indexed: bool, stride: Option<u32>, key: u32, fixed: Option<u32>)
    requires child.trusted_input(), fields.trusted_input(), elements.trusted_input(), entries.trusted_input(),
        stride.is_some() ==> !indexed && crate::encoding_list::uniform(elements.items(), stride),
    ensures true,
    no_unwind
{
    let a = child.measure(); let b = child.measure(); assert(a == b);
    let a = crate::encoding_option::measure(Some(child)); let b = crate::encoding_option::measure(Some(child)); assert(a == b);
    let a = crate::encoding_product::measure_fields(fields, total);
    let b = crate::encoding_product::measure_fields(fields, total); assert(a == b);
    let a = crate::encoding_list::measure(elements, indexed, stride);
    let b = crate::encoding_list::measure(elements, indexed, stride); assert(a == b);
    let a = crate::encoding_map::measure(entries, key, indexed, fixed);
    let b = crate::encoding_map::measure(entries, key, indexed, fixed); assert(a == b);
}

/// A17 observation of one destination, including untouched byte regions.
///
/// Measurement and comparison have empty observable effect traces under the
/// input contract; both executable protocols call one encoder on an exact loan
/// and carry `no_unwind`. Initial measurement and admission are a common prefix.
/// Its errors retain the same result and effects without an encoder-validity
/// premise. Only an admitted replay needs the trusted source contract.
pub proof fn encoder_graph_replay_equivalent<I: Input, O>(input: &I, before: Seq<u8>,
    retained: Seq<u8>, elided: Seq<u8>, start: int, first: Result<u32>, second: Result<u32>,
    prewrite: Result<()>,
    continuation: spec_fn(Seq<u8>, Result<u32>) -> O)
    requires checked(first, prewrite).is_ok() ==> input.trusted_input()
            && first == input.measurement() && second == input.measurement(),
        checked(first, prewrite).is_err() ==> second == first,
        checked(first, prewrite).is_ok() ==> 0 <= start <= before.len()
            && input.bytes().len() <= before.len() - start,
        checked(first, prewrite).is_ok() ==> retained == crate::encoding_product::splice(before, start, input.bytes()),
        checked(second, prewrite).is_ok() ==> elided == crate::encoding_product::splice(before, start, input.bytes()),
        checked(first, prewrite).is_err() ==> retained == before,
        checked(second, prewrite).is_err() ==> elided == before,
    ensures first == second, retained == elided, retained.len() == before.len(),
        continuation(retained, checked(first, prewrite)) == continuation(elided, checked(second, prewrite)),
        checked(first, prewrite).is_ok() ==> retained.subrange(start, start + input.bytes().len()) == input.bytes(),
        checked(first, prewrite).is_err() ==> retained == before,
        first.is_err() ==> retained == before && first.err() == second.err(),
        forall|i: int| 0 <= i < before.len() && !(start <= i < start + input.bytes().len())
            ==> #[trigger] retained[i] == before[i],
{
    if checked(first, prewrite).is_ok() { encoder_graph_measure_pure(input, first, second); }
    if checked(first, prewrite).is_ok() { assert(retained.subrange(start, start + input.bytes().len()) =~= input.bytes()); }
}

/// Measurement precedes count/extent/capacity/funding rejection before writes.
pub open spec fn checked(measurement: Result<u32>, prewrite: Result<()>) -> Result<u32> {
    match measurement { Err(error) => Err(error), Ok(n) => match prewrite {
        Err(error) => Err(error), Ok(()) => Ok(n),
    } }
}

/// One complete observation shape after a shared publication continuation.
///
/// Equal inputs to the same continuation preserve every field, including any
/// effects of the one retained encoder and surrounding caller. No property of
/// an SDK callback is assumed: the relation couples the same SDK outcomes after
/// the identical calls. Opaque operations never select replay removal.
/// The theorem also accepts any other result type, including opaque caller
/// error and unwind payloads that need no representation in this record.
pub ghost struct Observation {
    /// Selected error and offset, or success.
    pub result: Result<u32>,
    /// Complete destination including its surrounding regions.
    pub bytes: Seq<u8>,
    /// Whether the caller unwound.
    pub unwound: bool,
    /// Store poison after the continuation.
    pub poisoned: bool,
    /// Whether a typed prefix was published.
    pub published: bool,
    /// Codec/comparator and caller effects, in their original order.
    pub effects: Seq<int>,
    /// One encoding per destination after successful preflight.
    pub encodes: nat,
}

/// Primitive source projection used to exercise the same prepared protocol.
pub struct Projected<S> {
    /// Borrowed library child, without an allocated input mirror.
    pub value: S,
}

impl<S: crate::encoding_product::Source> Input for Projected<S> {
    open spec fn trusted_input(&self) -> bool { self.value.trusted_input() }
    open spec fn bytes(&self) -> Seq<u8> { self.value.bytes() }
    open spec fn measurement(&self) -> Result<u32> { self.value.measurement() }
    fn measure(&self) -> (result: Result<u32>)
        ensures result == self.measurement(), result.is_ok() ==> result.unwrap() == self.bytes().len(),
    { self.value.measure() }
    fn encode(&self, destination: &mut [u8])
        ensures final(destination)@ == self.bytes(),
    { self.value.encode(destination); }
}

/// Non-vacuous repeated measurement and prepared ownership on a real child.
#[cfg(verus_only)]
pub(crate) fn prepared_input_witness()
    ensures true,
{
    let input = Projected { value: crate::encoding_leaf::raw(&[3, 5]) };
    let prepared = match Prepared::new(input) { Ok(p) => p, Err(_) => { assert(false); return; } };
    let ghost original = prepared;
    let pair = match prepared.replay() { Ok(pair) => pair, Err(_) => { assert(false); return; } };
    proof { encoder_prepared_input_stable(original, pair.0, pair.1); }
    assert(pair.1 == 2);
}

/// Non-vacuous observation equality, including zero-width output.
#[cfg(verus_only)]
pub(crate) fn graph_measure_witness()
    ensures true,
{
    let input = Projected { value: crate::encoding_leaf::raw(&[3, 5]) };
    let first = input.measure(); let second = input.measure();
    proof { encoder_graph_measure_pure(&input, first, second); }
    assert(first == Ok(2));
    proof { reveal_strlit(""); }
    let empty = Projected { value: crate::encoding_text::text("") };
    let a = empty.measure(); let b = empty.measure();
    proof { encoder_graph_measure_pure(&empty, a, b); }
    assert(a == Ok(0));
    let child = crate::encoding_leaf::raw(&[3]);
    let fields = crate::encoding_product::Field { source: crate::encoding_leaf::raw(&[5]) };
    let elements = crate::encoding_list_fixed_inputs::FixedInputs { values: &[[3], [5]] };
    let entries = crate::encoding_map_fixed_inputs::FixedInputs { values: &[([3], [5]), ([3], [7])] };
    constructors_measure_pure(&child, &fields, &elements, &entries, 0, false, Some(1), 1, Some(1));
    proof { reveal_with_fuel(crate::encoding_map::fold, 4); }
    let failed = crate::encoding_map::measure(&entries, 1, false, Some(1));
    assert(failed == Err(crate::Error::Fault { kind: crate::ErrorKind::DuplicateKey, offset: u32::MAX }));
}

/// Both actual protocols write once into equal loans with surrounding frames.
#[cfg(verus_only)]
pub(crate) fn graph_replay_witness()
    ensures true,
{
    let input = Projected { value: crate::encoding_leaf::raw(&[3, 5]) };
    let mut retained = [19u8; 4]; let mut elided = [19u8; 4];
    let ghost before = retained@;
    assert(elided@ == before);
    let a = write(&input, &mut retained, 1, 2, true); assert(a == Ok(()));
    let b = write(&input, &mut elided, 1, 2, false); assert(b == Ok(()));
    let first = input.measure(); let second = input.measure();
    proof {
        let continuation = |bytes: Seq<u8>, result: Result<u32>| Observation {
            bytes, result, unwound: false, poisoned: false, published: result.is_ok(),
            effects: seq![1int], encodes: 1,
        };
        encoder_graph_replay_equivalent(&input, before, retained@, elided@, 1, first, second, Ok(()), continuation);
        encoder_graph_replay_equivalent(&input, before, before, before, 1, first, second,
            Err(crate::Error::Fault { kind: crate::ErrorKind::Capacity, offset: u32::MAX }), continuation);
    }
    assert(retained@ == seq![19u8,3,5,19]);
    let rejected = crate::encoding_replay_inputs::RejectedMap { values: &[([3], [5]), ([3], [7])] };
    let first = rejected.measure(); let second = rejected.measure();
    proof {
        encoder_graph_measure_pure(&rejected, first, second);
        let continuation = |bytes: Seq<u8>, result: Result<u32>| Observation {
            bytes, result, unwound: false, poisoned: false, published: false,
            effects: Seq::empty(), encodes: 0,
        };
        encoder_graph_replay_equivalent(&rejected, Seq::empty(), Seq::empty(), Seq::empty(),
            0, first, second, Ok(()), continuation);
    }
    assert(first == Err(crate::Error::Fault { kind: crate::ErrorKind::DuplicateKey, offset: u32::MAX }));
    let opaque = crate::encoding_replay_inputs::RejectedMap { values: &[] };
    proof {
        assert(!opaque.trusted_input());
        let continuation = |bytes: Seq<u8>, result: Result<u32>| Observation {
            bytes, result, unwound: true, poisoned: true, published: false,
            effects: seq![7int, 11int], encodes: 0,
        };
        let common = Err(crate::Error::Fault { kind: crate::ErrorKind::InvalidBoundary, offset: 17 });
        encoder_graph_replay_equivalent(&opaque, Seq::empty(), Seq::empty(), Seq::empty(),
            0, common, common, Ok(()), continuation);
        encoder_graph_replay_equivalent(&opaque, Seq::empty(), Seq::empty(), Seq::empty(),
            0, Ok(2), Ok(2), Err(crate::Error::Fault { kind: crate::ErrorKind::Capacity, offset: 19 }), continuation);
    }
}

}
