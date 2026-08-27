/! VOX-D-003 measurement seam (R-00059).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Does not encode a production batch, cost, cancel, or quota default.

#![forbid(unsafe_code)]

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn gate_id() -> &'static str {
    "VOX-D-003"
}

pub fn card_id() -> &'static str {
    "R-00059"
}

/// Candidate profile names only. First entry is not a selected default.
pub fn candidate_names() -> &'static [&'static str] {
    &[
        "StrictAdmissionBudgetFamily",
        "ContinuationFirstBudgetFamily",
        "ExplicitMissingQuotaFamily",
    ]
}

/// Intended corpus (not executed: R-00047 VoxelPortHarness unmet).
pub fn intended_corpus() -> &'static [&'static str] {
    &[
        "point-query",
        "region-query",
        "four-state-missing",
        "concurrent-write",
        "cancellation",
    ]
}

/// Intended negative / fault matrix (not executed).
pub fn intended_fault_matrix() -> &'static [&'static str] {
    &[
        "budget-exhaustion",
        "cancel-race",
        "oversized-request",
        "partial-missing-chunk",
    ]
}
