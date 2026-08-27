/! VOX-D-002 measurement seam (R-00058).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Does not encode a production backend default or pull an unaudited crate.

#![forbid(unsafe_code)]

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn gate_id() -> &'static str {
    "VOX-D-002"
}

pub fn card_id() -> &'static str {
    "R-00058"
}

/// Candidate backend names only. First entry is not a selected default.
pub fn candidate_names() -> &'static [&'static str] {
    &[
        "DenseUncompressedAdapter",
        "PaletteRleAdapter",
        "ExternalLz4PageAdapter",
        "ExternalZstdPageAdapter",
    ]
}

/// Intended corpus (not executed: R-00047 VoxelPortHarness unmet).
pub fn intended_corpus() -> &'static [&'static str] {
    &[
        "air",
        "repeated",
        "high-entropy",
    ]
}

/// Intended negative / fault matrix (not executed).
pub fn intended_fault_matrix() -> &'static [&'static str] {
    &[
        "truncated-page",
        "corrupt-dictionary",
        "decompress-cap",
        "thread-count-divergence",
        "backend-unavailable",
    ]
}
