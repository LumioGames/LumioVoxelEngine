//! VOX-D-005 measurement seam (R-00061).
//!
//! Drives shipped R-00047 `DeterministicExecutor` / `VoxelPortHarness` /
//! `FaultPoint`. Ops use generated schema_id `voxel-snapshot-payload`.
//! Not production snapshot code. Pin/COW budgets, Diff grain, and
//! materialize rules stay unfrozen until the architecture owner approves.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS, state_transition_table};
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};

const SNAPSHOT_SCHEMA: &str = "voxel-snapshot-payload";
const REPEAT_RUNS: usize = 3;
const SEED: u64 = 61;

/// Frozen `voxel-snapshot-payload` capture states (schema `$defs.voxelCaptureState`).
const CAPTURE_STATES: &[&str] = &[
    "Requested",
    "Cutting",
    "Pinned",
    "Encoding",
    "Verified",
    "Ready",
    "Released",
    "Cancelled",
    "Failed",
];

/// Happy-path capture (Ready still requires `pinState = Pinned`).
const CAPTURE_HAPPY_PATH: &[&str] = &[
    "Requested",
    "Cutting",
    "Pinned",
    "Encoding",
    "Verified",
    "Ready",
    "Released",
];

const CORPUS_IDS: &[&str] = &[
    "full-cut",
    "partial-aoi",
    "capture-ready-pinned",
    "pin-expired",
];

const FAULT_MAP: &[(&str, FaultPoint)] = &[
    ("pin-expired-ready", FaultPoint::StaleCompletion),
    ("payload-bad-hash", FaultPoint::CorruptSnapshot),
    ("diff-no-advance", FaultPoint::PrePublication),
];

pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn harness_requirement() -> &'static str {
    "R-00047"
}

pub fn harness_requirement_met() -> bool {
    SCHEMA_IDS.contains(&SNAPSHOT_SCHEMA)
}

pub fn numeric_policy_frozen() -> bool {
    false
}

pub fn pin_budget() -> &'static str {
    "pending-architecture-owner"
}

pub fn diff_granularity() -> &'static str {
    "pending-architecture-owner"
}

pub fn materialize_rule() -> &'static str {
    "pending-architecture-owner"
}

/// Candidate identifiers only. Order is not a ranking; the first id is not a default.
pub fn candidate_ids() -> &'static [&'static str] {
    &[
        "pin-count-chunk-wire-diff",
        "page-cow-internal-page-diff",
        "eager-full-copy-chunk-diff",
    ]
}

pub fn corpus_ids() -> &'static [&'static str] {
    CORPUS_IDS
}

pub fn fault_ids() -> &'static [&'static str] {
    &["pin-expired-ready", "payload-bad-hash", "diff-no-advance"]
}

/// Planned production axes. Harness replay does not freeze these numerics.
pub fn planned_measurement_axes() -> &'static [&'static str] {
    &[
        "long-pin",
        "high-write",
        "sparse-diff",
        "dense-diff",
        "multi-capture",
    ]
}

pub fn capture_states() -> &'static [&'static str] {
    CAPTURE_STATES
}

pub fn snapshot_schema_id() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == SNAPSHOT_SCHEMA)
        .expect("generated SCHEMA_IDS must include voxel-snapshot-payload")
}

pub fn map_fault(id: &str) -> Option<FaultPoint> {
    FAULT_MAP
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, point)| *point)
}

pub fn generated_capture_transitions() -> Vec<(&'static str, &'static str, &'static str)> {
    state_transition_table()
        .iter()
        .filter(|t| t.machine == "VoxelSnapshotCapture")
        .map(|t| (t.from, t.event, t.to))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorpusReplay {
    pub id: &'static str,
    pub snapshot: [u8; 32],
    pub identical: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultReplay {
    pub id: &'static str,
    pub error_id: &'static str,
    pub recoverable: bool,
    pub committed: bool,
    pub identical: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayReport {
    pub corpus: Vec<CorpusReplay>,
    pub faults: Vec<FaultReplay>,
    pub three_run_identical: bool,
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn replay_all() -> ReplayReport {
    let mut corpus = Vec::with_capacity(CORPUS_IDS.len());
    let mut all_identical = true;

    for &id in CORPUS_IDS {
        let schedule = corpus_schedule(id);
        let (snapshot, identical) = match id {
            "pin-expired" => repeat_faulted(&schedule, FaultPoint::StaleCompletion),
            _ => repeat_executor(&schedule),
        };
        all_identical &= identical;
        corpus.push(CorpusReplay {
            id,
            snapshot,
            identical,
        });
    }

    let mut faults = Vec::with_capacity(FAULT_MAP.len());
    for &(id, point) in FAULT_MAP {
        let schedule = Schedule {
            seed: SEED,
            ops: vec![op(0, id)],
        };
        let (trace, identical) = repeat_faulted_trace(&schedule, point);
        all_identical &= identical;
        let outcome = trace
            .outcomes
            .first()
            .expect("fault schedule executes one op");
        let error_id = FaultInjector::error_id(point);
        debug_assert!(STABLE_ERROR_IDS.contains(&error_id));
        assert_eq!(outcome.error, Some(error_id));
        assert_eq!(outcome.recoverable, FaultInjector::recoverable(point));
        faults.push(FaultReplay {
            id,
            error_id,
            recoverable: FaultInjector::recoverable(point),
            committed: trace.snapshot != empty_snapshot_hash(),
            identical,
        });
    }

    ReplayReport {
        corpus,
        faults,
        three_run_identical: all_identical,
    }
}

pub fn replay_text() -> String {
    let report = replay_all();
    let mut out = String::new();
    out.push_str("VOX-D-005 harness replay\n");
    out.push_str(&format!(
        "approval_status={} numeric_policy_frozen={}\n",
        approval_status(),
        numeric_policy_frozen()
    ));
    out.push_str(&format!(
        "three_run_identical={}\n",
        report.three_run_identical
    ));
    for row in &report.corpus {
        out.push_str(&format!(
            "corpus {} identical={} snapshot={}\n",
            row.id,
            row.identical,
            hex32(&row.snapshot)
        ));
    }
    for row in &report.faults {
        out.push_str(&format!(
            "fault {} error={} recoverable={} committed={} identical={}\n",
            row.id, row.error_id, row.recoverable, row.committed, row.identical
        ));
    }
    out
}

/// True when three-run harness traces match. Does not freeze pin/COW numerics.
pub fn measurements_executed() -> bool {
    replay_all().three_run_identical
}

pub fn measurements_skip_reason() -> &'static str {
    "pin/COW numeric policy unfrozen; architecture owner approval absent"
}

fn corpus_schedule(id: &str) -> Schedule {
    let ops = match id {
        "full-cut" => CAPTURE_HAPPY_PATH
            .iter()
            .enumerate()
            .map(|(i, state)| op(i as u64, state))
            .collect(),
        "partial-aoi" => vec![op(0, "partial-aoi")],
        "capture-ready-pinned" => CAPTURE_HAPPY_PATH
            .iter()
            .take_while(|state| **state != "Released")
            .enumerate()
            .map(|(i, state)| op(i as u64, state))
            .collect(),
        "pin-expired" => vec![op(0, "pin-expired-ready")],
        other => panic!("unknown corpus id {other}"),
    };
    Schedule { seed: SEED, ops }
}

fn op(seq: u64, payload: &str) -> GeneratedVoxelOperation {
    GeneratedVoxelOperation {
        schema_id: snapshot_schema_id(),
        seq,
        payload: payload.as_bytes().to_vec(),
    }
}

fn repeat_executor(schedule: &Schedule) -> ([u8; 32], bool) {
    let mut traces: Vec<Trace> = Vec::with_capacity(REPEAT_RUNS);
    for _ in 0..REPEAT_RUNS {
        traces.push(DeterministicExecutor::run(schedule));
    }
    let identical = traces.windows(2).all(|w| w[0] == w[1]);
    (traces[0].snapshot, identical)
}

fn repeat_faulted(schedule: &Schedule, point: FaultPoint) -> ([u8; 32], bool) {
    let (trace, identical) = repeat_faulted_trace(schedule, point);
    (trace.snapshot, identical)
}

fn repeat_faulted_trace(schedule: &Schedule, point: FaultPoint) -> (Trace, bool) {
    let mut traces: Vec<Trace> = Vec::with_capacity(REPEAT_RUNS);
    for _ in 0..REPEAT_RUNS {
        let mut port = VoxelPortHarness::new();
        port.arm(point);
        let mut outcomes = Vec::with_capacity(schedule.ops.len());
        for item in &schedule.ops {
            outcomes.push(port.execute(item));
        }
        traces.push(Trace {
            seed: schedule.seed,
            outcomes,
            snapshot: port.snapshot_hash(),
        });
    }
    let identical = traces.windows(2).all(|w| w[0] == w[1]);
    let first = traces.remove(0);
    (first, identical)
}

fn empty_snapshot_hash() -> [u8; 32] {
    VoxelPortHarness::new().snapshot_hash()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumio_voxel_contracts::{MACHINE_IDS, SCHEMA_IDS, STABLE_ERROR_IDS};

    #[test]
    fn gate_remains_blocked() {
        assert_eq!(approval_status(), "blocked");
        assert!(!numeric_policy_frozen());
        assert_eq!(pin_budget(), "pending-architecture-owner");
        assert_eq!(diff_granularity(), "pending-architecture-owner");
        assert_eq!(materialize_rule(), "pending-architecture-owner");
        assert_eq!(harness_requirement(), "R-00047");
        assert!(harness_requirement_met());
        assert!(candidate_ids().len() >= 2);
        assert_eq!(corpus_ids().len(), 4);
        assert_eq!(fault_ids().len(), 3);
    }

    #[test]
    fn snapshot_schema_is_generated() {
        assert_eq!(snapshot_schema_id(), "voxel-snapshot-payload");
        assert!(SCHEMA_IDS.contains(&snapshot_schema_id()));
        assert!(MACHINE_IDS.contains(&"VoxelSnapshotCapture"));
    }

    #[test]
    fn capture_states_match_frozen_contract() {
        for state in [
            "Requested",
            "Cutting",
            "Pinned",
            "Encoding",
            "Verified",
            "Ready",
            "Released",
            "Cancelled",
            "Failed",
        ] {
            assert!(capture_states().contains(&state), "{state}");
        }
        let table = generated_capture_transitions();
        assert!(!table.is_empty());
        for (from, _event, to) in table {
            assert!(capture_states().contains(&from), "{from}");
            assert!(capture_states().contains(&to), "{to}");
        }
    }

    #[test]
    fn faults_map_to_shipped_points() {
        assert_eq!(
            map_fault("pin-expired-ready"),
            Some(FaultPoint::StaleCompletion)
        );
        assert_eq!(
            map_fault("payload-bad-hash"),
            Some(FaultPoint::CorruptSnapshot)
        );
        assert_eq!(
            map_fault("diff-no-advance"),
            Some(FaultPoint::PrePublication)
        );
        assert_eq!(
            FaultInjector::error_id(FaultPoint::StaleCompletion),
            "StaleEpoch"
        );
        assert_eq!(
            FaultInjector::error_id(FaultPoint::CorruptSnapshot),
            "EvidenceDigestMismatch"
        );
        assert_eq!(
            FaultInjector::error_id(FaultPoint::PrePublication),
            "InvalidHandle"
        );
        assert!(FaultInjector::recoverable(FaultPoint::StaleCompletion));
        assert!(!FaultInjector::recoverable(FaultPoint::CorruptSnapshot));
        assert!(FaultInjector::recoverable(FaultPoint::PrePublication));
        for id in [
            "StaleEpoch",
            "EvidenceDigestMismatch",
            "InvalidHandle",
            "SnapshotBaseMismatch",
            "BudgetExceeded",
        ] {
            assert!(STABLE_ERROR_IDS.contains(&id), "{id}");
        }
    }

    #[test]
    fn three_run_harness_replay() {
        let report = replay_all();
        assert!(report.three_run_identical, "{report:?}");
        assert!(measurements_executed());
        assert_eq!(report.corpus.len(), 4);
        assert_eq!(report.faults.len(), 3);
        for row in &report.corpus {
            assert!(row.identical, "{}", row.id);
        }
        let pin = report
            .faults
            .iter()
            .find(|f| f.id == "pin-expired-ready")
            .expect("pin-expired-ready");
        assert_eq!(pin.error_id, "StaleEpoch");
        assert!(pin.recoverable);
        assert!(!pin.committed, "expired pin must not publish Ready");
        let bad = report
            .faults
            .iter()
            .find(|f| f.id == "payload-bad-hash")
            .expect("payload-bad-hash");
        assert_eq!(bad.error_id, "EvidenceDigestMismatch");
        assert!(!bad.recoverable);
        let diff = report
            .faults
            .iter()
            .find(|f| f.id == "diff-no-advance")
            .expect("diff-no-advance");
        assert_eq!(diff.error_id, "InvalidHandle");
        assert!(diff.recoverable);
        assert!(!diff.committed);
        println!("{}", replay_text());
    }
}
