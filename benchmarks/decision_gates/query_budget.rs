//! VOX-D-003 measurement seam (R-00059).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Does not encode a production batch, cost, cancel, or quota default.
//! Numeric Query budgets stay unfrozen; four-state presence is not redefined.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::{CHUNK_PRESENCE, SCHEMA_IDS, STABLE_ERROR_IDS};
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{
    GeneratedVoxelOperation, GeneratedVoxelOutcome, VoxelPortHarness,
};

/// Generated schema id for every corpus and fault op. Must stay in `SCHEMA_IDS`.
pub const QUERY_SCHEMA_ID: &str = "voxel-query";

/// Fixed schedule seed. Not a production budget.
pub const CORPUS_SEED: u64 = 0x0000_D003;

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "approved"
}

pub fn approval_reference() -> &'static str {
    "LGE-V1.4-VOX-D-P0-2026-08-28"
}

/// Adapter-internal budget family. Not a public batch_limit Schema column.
pub fn selected_family() -> &'static str {
    "StrictAdmissionBudgetFamily"
}

pub fn gate_id() -> &'static str {
    "VOX-D-003"
}

pub fn card_id() -> &'static str {
    "R-00059"
}

/// Shipped harness this seam consumes. Present at R-00047 / `b2f0d8a`.
pub fn harness_requirement() -> &'static str {
    "R-00047"
}

pub fn query_schema_registered() -> bool {
    SCHEMA_IDS.contains(&QUERY_SCHEMA_ID)
}

/// Frozen four-state presence from generated contracts. This seam does not extend it.
pub fn frozen_chunk_presence() -> &'static [&'static str] {
    CHUNK_PRESENCE
}

/// Candidate profile names only. First entry is not a selected default.
pub fn candidate_names() -> &'static [&'static str] {
    &[
        "StrictAdmissionBudgetFamily",
        "ContinuationFirstBudgetFamily",
        "ExplicitMissingQuotaFamily",
    ]
}

/// Corpus names: bound query, continuation, target-revision-unavailable, missing-chunk four-state.
pub fn intended_corpus() -> &'static [&'static str] {
    &[
        "bound-query",
        "continuation",
        "target-revision-unavailable",
        "missing-chunk-four-state",
    ]
}

/// Fault scenarios mapped onto shipped `FaultPoint` values (no new ErrorCodes).
pub fn intended_fault_matrix() -> &'static [&'static str] {
    &["unbound-continuation", "budget-exceeded", "stale-revision"]
}

pub struct MappedFault {
    pub scenario: &'static str,
    pub point: FaultPoint,
}

/// Unbound continuation → PrePublication; budget exceeded → PostPublication;
/// stale revision → StaleCompletion. Error ids come from `FaultInjector`.
pub fn mapped_faults() -> [MappedFault; 3] {
    [
        MappedFault {
            scenario: "unbound-continuation",
            point: FaultPoint::PrePublication,
        },
        MappedFault {
            scenario: "budget-exceeded",
            point: FaultPoint::PostPublication,
        },
        MappedFault {
            scenario: "stale-revision",
            point: FaultPoint::StaleCompletion,
        },
    ]
}

fn query_op(seq: u64, payload: &[u8]) -> GeneratedVoxelOperation {
    debug_assert!(
        SCHEMA_IDS.contains(&QUERY_SCHEMA_ID),
        "operation schema_id must be a generated schema id"
    );
    GeneratedVoxelOperation {
        schema_id: QUERY_SCHEMA_ID,
        seq,
        payload: payload.to_vec(),
    }
}

/// Bound query + continuation + target-revision-unavailable + `CHUNK_PRESENCE` ops.
pub fn corpus_schedule() -> Schedule {
    let mut ops = Vec::with_capacity(3 + CHUNK_PRESENCE.len());
    ops.push(query_op(0, b"bound-query"));
    ops.push(query_op(1, b"continuation"));
    ops.push(query_op(2, b"target-revision-unavailable"));
    for (i, presence) in CHUNK_PRESENCE.iter().enumerate() {
        ops.push(query_op(3 + i as u64, presence.as_bytes()));
    }
    Schedule {
        seed: CORPUS_SEED,
        ops,
    }
}

pub fn replay_corpus_three_times() -> [Trace; 3] {
    let schedule = corpus_schedule();
    [
        DeterministicExecutor::run(&schedule),
        DeterministicExecutor::run(&schedule),
        DeterministicExecutor::run(&schedule),
    ]
}

pub struct CorpusMeasurement {
    pub seed: u64,
    pub snapshots: [[u8; 32]; 3],
    pub identical: bool,
    pub outcome_count: usize,
    pub errors: usize,
}

pub fn measure_corpus() -> CorpusMeasurement {
    let traces = replay_corpus_three_times();
    let snapshots = [traces[0].snapshot, traces[1].snapshot, traces[2].snapshot];
    let errors = traces[0]
        .outcomes
        .iter()
        .filter(|o| o.error.is_some())
        .count();
    CorpusMeasurement {
        seed: traces[0].seed,
        snapshots,
        identical: snapshots[0] == snapshots[1] && snapshots[1] == snapshots[2],
        outcome_count: traces[0].outcomes.len(),
        errors,
    }
}

pub fn corpus_snapshots_agree() -> bool {
    measure_corpus().identical
}

pub struct FaultMeasurement {
    pub scenario: &'static str,
    pub error: Option<&'static str>,
    pub recoverable: bool,
    pub error_is_stable: bool,
}

pub fn measure_faults() -> [FaultMeasurement; 3] {
    let cases = mapped_faults();
    let mut out = [
        empty_fault(cases[0].scenario),
        empty_fault(cases[1].scenario),
        empty_fault(cases[2].scenario),
    ];
    for (i, case) in cases.iter().enumerate() {
        let mut port = VoxelPortHarness::new();
        port.arm(case.point);
        let outcome: GeneratedVoxelOutcome =
            port.execute(&query_op(100 + i as u64, case.scenario.as_bytes()));
        let expected = FaultInjector::error_id(case.point);
        let error_is_stable = outcome
            .error
            .is_some_and(|id| STABLE_ERROR_IDS.contains(&id) && id == expected);
        out[i] = FaultMeasurement {
            scenario: case.scenario,
            error: outcome.error,
            recoverable: outcome.recoverable,
            error_is_stable,
        };
    }
    out
}

fn empty_fault(scenario: &'static str) -> FaultMeasurement {
    FaultMeasurement {
        scenario,
        error: None,
        recoverable: false,
        error_is_stable: false,
    }
}

pub fn mapped_fault_error_ids_are_stable() -> bool {
    mapped_faults().iter().all(|f| {
        let id = FaultInjector::error_id(f.point);
        STABLE_ERROR_IDS.contains(&id)
    })
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
