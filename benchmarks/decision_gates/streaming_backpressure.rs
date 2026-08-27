//! VOX-D-006 measurement seam (R-00062).
//!
//! Not production streaming code. Priority, concurrency, capacity, and
//! backpressure stay unfrozen until the architecture owner approves the gate.

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
        "fifo-deadline-reject",
        "demand-heat-hysteresis",
        "lru-resident-cancel-lowest",
    ]
}

/// Planned measurement axes. No numeric policy thresholds.
pub fn planned_measurement_axes() -> &'static [&'static str] {
    &[
        "burst-demand",
        "cold-chunk",
        "hot-chunk",
        "slow-io",
        "cancel",
        "dirty-unload-fence",
    ]
}

/// Planned fault axes. No numeric policy thresholds.
pub fn planned_fault_axes() -> &'static [&'static str] {
    &[
        "queue-full",
        "expired-ticket",
        "wrong-world",
        "restore-mutex",
        "missing-durability-ack",
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
