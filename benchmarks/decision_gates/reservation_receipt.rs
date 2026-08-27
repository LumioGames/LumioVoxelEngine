/! VOX-D-004 measurement seam (R-00060).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Does not encode a production lease, prune, or capacity default.

#![forbid(unsafe_code)]

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn gate_id() -> &'static str {
    "VOX-D-004"
}

pub fn card_id() -> &'static str {
    "R-00060"
}

/// Candidate profile names only. First entry is not a selected default.
pub fn candidate_names() -> &'static [&'static str] {
    &[
        "GenerationBoundLeaseFamily",
        "WallClockLeaseFamily",
        "AckPruneCapacityFamily",
    ]
}

/// Intended corpus (not executed: R-00047 VoxelPortHarness unmet).
pub fn intended_corpus() -> &'static [&'static str] {
    &[
        "repeated-txn",
        "long-txn",
        "crash-replay",
        "capacity-pressure",
        "prune-safety-point",
    ]
}

/// Intended negative / fault matrix (not executed).
pub fn intended_fault_matrix() -> &'static [&'static str] {
    &[
        "lease-boundary-race",
        "fingerprint-conflict",
        "capacity-exhaustion",
        "restart-recovery",
    ]
}
