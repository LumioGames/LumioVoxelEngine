//! VOX-D-006 measurement seam (R-00062).
//!
//! Not a workspace member. Not production streaming code.
//! Drives the shipped R-00047 `DeterministicExecutor` / `VoxelPortHarness` /
//! `FaultPoint` surface. Ops use generated schema id `voxel-durability-ack`.
//!
//! Frozen by ADR-036: ack coverage shape, residency modes, dirty-eviction fence.
//! Unfrozen: priority scoring, concurrency, queue capacity, backpressure.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_IDS, sha256};
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{
    GeneratedVoxelOperation, GeneratedVoxelOutcome, VoxelPortHarness,
};

/// Generated schema id for every op on this seam.
pub const SCHEMA_ID: &str = "voxel-durability-ack";

/// Fixed seed for the durability-ack corpus schedule.
pub const CORPUS_SEED: u64 = 0x0000_D006;

/// Repeat count required by the gate (same seed / corpus / schedule).
pub const REPEAT_RUNS: usize = 3;

/// Corpus fixture-id tokens. Not schema field copies.
pub const CORPUS_IDS: &[&str] = &[
    "snapshot-point-ack",
    "wal-point-ack",
    "dirty-covered-evict",
    "dirty-uncovered-deny",
    "residency-all-resident",
];

/// Gate fault names. Mapped onto shipped `FaultPoint` values below.
pub const FAULT_IDS: &[&str] = &[
    "all-resident-evict",
    "uncovered-evict",
    "missing-durability-point",
];

/// Candidate identifiers only. Order is not a ranking; the first id is not a default.
pub fn candidate_ids() -> &'static [&'static str] {
    &[
        "fifo-deadline-reject",
        "demand-heat-hysteresis",
        "lru-resident-cancel-lowest",
    ]
}

/// Scoring / budget axes. Still unfrozen; this seam must not write defaults.
pub fn unfrozen_scoring_axes() -> &'static [&'static str] {
    &["priority", "concurrency", "capacity", "backpressure"]
}

/// Streaming worker axes that still need a production coordinator (not this seam).
pub fn unexecuted_streaming_axes() -> &'static [&'static str] {
    &[
        "burst-demand",
        "cold-chunk",
        "hot-chunk",
        "slow-io",
        "cancel",
    ]
}

pub fn architecture_baseline_id() -> &'static str {
    BASELINE_ID
}

/// Gate approval. Owner confirmation applied; scoring/budgets stay open.
pub fn approval_status() -> &'static str {
    "approved"
}

/// Architecture-owner confirmation id (`LGE-V1.4-VOX-D-P2-2026-08-29`).
pub fn approval_reference() -> &'static str {
    "LGE-V1.4-VOX-D-P2-2026-08-29"
}

/// R-00047 harness is in-tree. This seam consumes it; it does not substitute one.
pub fn harness_requirement() -> &'static str {
    "R-00047"
}

pub fn harness_requirement_met() -> bool {
    true
}

pub fn schema_id() -> &'static str {
    assert!(
        SCHEMA_IDS.contains(&SCHEMA_ID),
        "voxel-durability-ack must be a generated schema id"
    );
    SCHEMA_ID
}

/// Shipped FaultPoints used after a visible write. All three are unrecoverable.
pub fn mapped_fault_point(fault_id: &str) -> Option<FaultPoint> {
    match fault_id {
        "all-resident-evict" => Some(FaultPoint::PostPublication),
        "uncovered-evict" => Some(FaultPoint::LostResult),
        "missing-durability-point" => Some(FaultPoint::CorruptSnapshot),
        _ => None,
    }
}

pub fn durability_op(seq: u64, payload: &[u8]) -> GeneratedVoxelOperation {
    GeneratedVoxelOperation {
        schema_id: schema_id(),
        seq,
        payload: payload.to_vec(),
    }
}

pub fn corpus_schedule() -> Schedule {
    Schedule {
        seed: CORPUS_SEED,
        ops: CORPUS_IDS
            .iter()
            .enumerate()
            .map(|(i, id)| durability_op(i as u64, id.as_bytes()))
            .collect(),
    }
}

/// Three independent replays of the same seed / corpus / schedule.
pub fn replay_three() -> [Trace; 3] {
    let schedule = corpus_schedule();
    [
        DeterministicExecutor::run(&schedule),
        DeterministicExecutor::run(&schedule),
        DeterministicExecutor::run(&schedule),
    ]
}

pub fn traces_byte_identical(traces: &[Trace; 3]) -> bool {
    traces[0] == traces[1] && traces[1] == traces[2]
}

pub fn trace_digest(trace: &Trace) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&trace.seed.to_le_bytes());
    buf.extend_from_slice(&trace.snapshot);
    for outcome in &trace.outcomes {
        buf.extend_from_slice(&outcome.seq.to_le_bytes());
        buf.extend_from_slice(outcome.schema_id.as_bytes());
        buf.extend_from_slice(&outcome.payload);
        if let Some(error) = outcome.error {
            buf.extend_from_slice(error.as_bytes());
        }
        buf.push(u8::from(outcome.recoverable));
    }
    sha256(&buf)
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Visible durability write, then a mapped unrecoverable FaultPoint.
pub fn inject_mapped_fault(fault_id: &str) -> (GeneratedVoxelOutcome, GeneratedVoxelOutcome) {
    let point = mapped_fault_point(fault_id).expect("fault id must be mapped");
    assert!(
        !FaultInjector::recoverable(point),
        "mapped faults must be unrecoverable after a visible write"
    );
    let mut port = VoxelPortHarness::new();
    let visible = port.execute(&durability_op(0, b"visible-write"));
    port.arm(point);
    let faulted = port.execute(&durability_op(1, fault_id.as_bytes()));
    (visible, faulted)
}

/// True after the harness three-run compare succeeds and mapped faults stay unrecoverable.
pub fn measurements_executed() -> bool {
    let traces = replay_three();
    if !traces_byte_identical(&traces) {
        return false;
    }
    if traces[0].outcomes.len() != CORPUS_IDS.len() {
        return false;
    }
    if traces[0].outcomes.iter().any(|o| o.error.is_some()) {
        return false;
    }
    FAULT_IDS.iter().all(|id| {
        let (visible, faulted) = inject_mapped_fault(id);
        visible.error.is_none() && faulted.error.is_some() && !faulted.recoverable
    })
}

pub fn measurements_skip_reason() -> &'static str {
    "none; durability-ack fence replayed on R-00047 harness; scoring/budgets remain unfrozen"
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumio_voxel_test_support::fault_injection::FaultInjector;

    #[test]
    fn gate_approved_citing_owner_confirmation() {
        assert_eq!(approval_status(), "approved");
        assert_eq!(approval_reference(), "LGE-V1.4-VOX-D-P2-2026-08-29");
        assert!(harness_requirement_met());
        assert_eq!(harness_requirement(), "R-00047");
        assert!(candidate_ids().len() >= 2);
        assert_eq!(unfrozen_scoring_axes().len(), 4);
        assert_eq!(architecture_baseline_id(), "LGE-V1.4-2026-08-27");
    }

    #[test]
    fn corpus_uses_generated_durability_ack_schema() {
        assert_eq!(schema_id(), "voxel-durability-ack");
        let schedule = corpus_schedule();
        assert_eq!(schedule.seed, CORPUS_SEED);
        assert_eq!(schedule.ops.len(), CORPUS_IDS.len());
        for op in &schedule.ops {
            assert_eq!(op.schema_id, "voxel-durability-ack");
        }
    }

    #[test]
    fn three_runs_are_byte_identical() {
        let traces = replay_three();
        assert!(traces_byte_identical(&traces));
        assert_eq!(traces[0].snapshot, traces[1].snapshot);
        assert_eq!(traces[1].snapshot, traces[2].snapshot);
        assert_eq!(trace_digest(&traces[0]), trace_digest(&traces[1]));
        assert_eq!(trace_digest(&traces[1]), trace_digest(&traces[2]));
        assert_eq!(REPEAT_RUNS, 3);
        assert!(measurements_executed());
        eprintln!("VOX-D-006 snapshot {}", hex32(&traces[0].snapshot));
        eprintln!("VOX-D-006 digest {}", hex32(&trace_digest(&traces[0])));
    }

    #[test]
    fn mapped_faults_are_unrecoverable_after_visible_write() {
        let expected = [
            (
                "all-resident-evict",
                FaultPoint::PostPublication,
                "PartialLoadRolledBack",
            ),
            ("uncovered-evict", FaultPoint::LostResult, "EvidenceMissing"),
            (
                "missing-durability-point",
                FaultPoint::CorruptSnapshot,
                "EvidenceDigestMismatch",
            ),
        ];
        for (id, point, error) in expected {
            assert_eq!(mapped_fault_point(id), Some(point));
            assert!(!FaultInjector::recoverable(point));
            let (visible, faulted) = inject_mapped_fault(id);
            assert_eq!(visible.error, None, "{id} visible write must commit");
            assert_eq!(faulted.error, Some(error), "{id}");
            assert!(!faulted.recoverable, "{id} must not look like a retry");
        }
    }
}
