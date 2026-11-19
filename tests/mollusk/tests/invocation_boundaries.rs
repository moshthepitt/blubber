//! Shared transaction contexts, caller growth ceilings and bank rent checks.
#![forbid(unsafe_code)]

#[path = "../fixtures/invocation_boundaries.rs"]
mod cases;
#[path = "../fixtures/runtime_report.rs"]
mod runtime_report;

#[test]
fn fresh_top_level_windows_accumulate_in_one_transaction() {
    cases::fresh_top_level_windows_accumulate_in_one_transaction(false);
}

#[test]
fn cpi_return_obeys_the_callers_original_window() {
    cases::cpi_return_obeys_the_callers_original_window(false);
}

#[test]
fn later_instruction_failure_reverts_commits_and_system_transfers() {
    cases::later_instruction_failure_reverts_commits_and_system_transfers(false);
}

#[test]
fn prepaid_requires_funding_before_commit_even_when_bank_rent_is_deferred() {
    cases::prepaid_requires_funding_before_commit_even_when_bank_rent_is_deferred(false);
}

#[test]
fn signed_aggregate_growth_is_twenty_mib_and_shrink_restores_budget() {
    cases::signed_aggregate_growth_is_twenty_mib_and_shrink_restores_budget(false);
}

#[test]
fn duplicate_metas_merge_privileges_and_share_genuine_guard_exclusion() {
    cases::duplicate_metas_merge_privileges_and_share_genuine_guard_exclusion(false);
}

#[test]
fn exhausted_transaction_budget_rejects_max_but_allows_zero() {
    cases::exhausted_transaction_budget_rejects_max_but_allows_zero(false);
}
