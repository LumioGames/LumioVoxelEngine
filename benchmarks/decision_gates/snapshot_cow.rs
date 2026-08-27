//! VOX-D-005 measurement seam (R-00061).
//!
//! Not production snapshot code. Pin/COW budgets, Diff grain, and materialize
//! rules stay unfrozen until the architecture owner approves the gate.

/// Gate approval. Blocked while R-00047 is unmet and owner approval is absent.
pub fn approval_status() -> &'static str {
    "blocked"
}

/// Unmet harness prerequisite. Do not invent a substitute.
pub fn unmet_harness_requirement() -> &'static str {
    "R-00047"
}

/// Candidate identifiers only. Order is not a ranking; the first id is not a default.
pub fn candidate_ids() -> &'static [&'static str] {
    &[
        "pin-count-chunk-wire-diff",
        "page-cow-internal-page-diff",
        "eager-full-copy-chunk-diff",
    ]
}

/// Planned measurement axes. No numeric policy thresholds.
pub fn planned_measurement_axes() -> &'static [&'static str] {
    &[
        "long-pin",
        "high-write",
        "sparse-diff",
        "dense-diff",
        "multi-capture",
    ]
}

/// Planned fault axes. No numeric policy thresholds.
pub fn planned_fault_axes() -> &'static [&'static str] {
    &[
        "pin-over-budget",
        "capture-cancel",
        "concurrent-write",
        "corrupt-diff",
    ]
}

/// True only after R-00047 harness plus three-run hash compare.
pub fn measurements_executed() -> bool {
    false
}

pub fn measurements_skip_reason() -> &'static str {
    "R-00047 unmet; VoxelPortHarness not present; measurements 未执行"
}

#[cfg(test)]
mod tests {
    #[test]
    fn gate_remains_blocked() {
        assert_eq!(super::approval_status(), "blocked");
        assert!(!super::measurements_executed());
        assert_eq!(super::unmet_harness_requirement(), "R-00047");
        assert!(super::candidate_ids().len() >= 2);
    }
}
