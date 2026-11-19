//! Arithmetic for caller-selected UpTo leases; SDK exclusion and rent stay
//! with the existing Store. These lemmas require a successfully granted lease.

use vstd::prelude::*;

verus! {
/// Sum of nonnegative growth requests for a known sequence.
pub open spec fn growth(edits: Seq<nat>) -> nat
    decreases edits.len(),
{
    if edits.len() == 0 { 0 } else { edits[0] + growth(edits.skip(1)) }
}

/// Every prefix of the aggregate budget fits the granted opening capacity.
pub proof fn exact_aggregate(edits: Seq<nat>, opening: nat, prefix: int)
    requires 0 <= prefix <= edits.len(),
    ensures growth(edits.take(prefix)) <= growth(edits),
        opening + growth(edits.take(prefix)) <= opening + growth(edits),
    decreases edits.len(),
{
    if prefix > 0 {
        exact_aggregate(edits.skip(1), opening, prefix - 1);
        assert(edits.take(prefix).skip(1) =~= edits.skip(1).take(prefix - 1));
    }
}

/// The first over-budget edit exceeds capacity before its publication can run.
/// Runtime byte preservation is checked against Store's immutable preflight.
pub proof fn under_budget(opening: nat, used: nat, delta: nat, room: nat)
    requires used <= opening + room,
        delta > opening + room - used,
    ensures used + delta > opening + room,
        !(used + delta <= opening + room),
{
}

/// Nonempty two-edit and rejected-next-edit witnesses.
pub proof fn budget_witness()
{
    reveal_with_fuel(growth, 3);
    exact_aggregate(seq![8nat, 8nat], 16, 1);
    exact_aggregate(seq![8nat, 8nat], 16, 2);
    under_budget(16, 24, 8, 15);
    under_budget(32, 16, 24, 0);
    assert(growth(seq![8nat, 8nat]) == 16);
}
}
