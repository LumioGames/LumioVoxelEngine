/! VOX-D-001 measurement seam (R-00057).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Does not encode a production numeric default (no chunk extent, page size,
//! or world-bound constants).

#![forbid(unsafe_code)]

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn gate_id() -> &'static str {
    "VOX-D-001"
}

pub fn card_id() -> &'static str {
    "R-00057"
}

/// Candidate family names only. First entry is not a selected default.
pub fn candidate_names() -> &'static [&'static str] {
    &[
        "IsolatedCubicExtentFamily",
        "CoupledPageAxisExtentFamily",
    ]
}

/// Intended corpus (not executed: R-00047 VoxelPortHarness unmet).
pub fn intended_corpus() -> &'static [&'static str] {
    &[
        "sparse",
        "dense",
        "boundary-coords",
        "negative-coords",
        "extreme-coords",
        "cold-read",
        "hot-read",
        "bulk-edit",
    ]
}

/// Intended negative / fault matrix (not executed).
pub fn intended_fault_matrix() -> &'static [&'static str] {
    &[
        "illegal-dimension",
        "extreme-coordinate",
        "memory-pressure",
        "cross-profile-misread",
    ]
}
