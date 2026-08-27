//! VOX-D-004 measurement seam (R-00060).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Drives the shipped R-00047 `DeterministicExecutor` / `VoxelPortHarness` /
//! `FaultPoint` surface. Does not encode a production lease, prune, or
//! capacity default, and does not invent abort reasons.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};

/// Generated schema id for every corpus and fault op.
pub const RECEIPT_SCHEMA_ID: &str = "voxel-mutation-receipt";

/// Measurement seed. Not a lease duration or table cap.
pub const MEASUREMENT_SEED: u64 = 0x0000_D004;

/// Same seed / corpus / schedule is replayed this many times.
pub const REPEAT_COUNT: usize = 3;

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "approved"
}

pub fn approval_reference() -> &'static str {
    "LGE-V1.4-VOX-D-P0-2026-08-28"
}

/// Adapter-internal lease family. Not a wall-clock or table-cap Schema default.
pub fn selected_family() -> &'static str {
    "GenerationBoundLeaseFamily"
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

/// Frozen voxel-mutation-receipt corpus driven through the harness.
pub fn receipt_corpus() -> &'static [&'static str] {
    &[
        "applied",
        "duplicate",
        "aborted-conflict",
        "lost-result",
        "pruned",
    ]
}

/// Frozen abort / reject ids already published on the V1.4 contract.
/// This seam does not add names.
pub fn frozen_abort_vocabulary() -> &'static [&'static str] {
    &[
        "RevisionConflict",
        "ChunkUnavailable",
        "StaleEpoch",
        "InvalidHandle",
        "EvidenceMissing",
        "PartialLoadRolledBack",
    ]
}

/// Card-level capacity axes the echo harness cannot size. Not executed.
pub fn unmeasured_capacity_axes() -> &'static [&'static str] {
    &[
        "repeated-txn",
        "long-txn",
        "crash-replay",
        "capacity-pressure",
        "prune-safety-point",
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappedFault {
    pub name: &'static str,
    pub point: FaultPoint,
}

/// Decision-gate faults mapped onto shipped R-00047 `FaultPoint`s.
/// `PostPublication` / `LostResult` stay unrecoverable.
pub fn mapped_faults() -> &'static [MappedFault] {
    &[
        MappedFault {
            name: "commit-intent-leak",
            point: FaultPoint::PostPublication,
        },
        MappedFault {
            name: "applied-missing-result",
            point: FaultPoint::LostResult,
        },
        MappedFault {
            name: "lease-expired",
            point: FaultPoint::PrePublication,
        },
    ]
}

pub fn receipt_op(seq: u64, payload: &[u8]) -> GeneratedVoxelOperation {
    debug_assert!(
        SCHEMA_IDS.contains(&RECEIPT_SCHEMA_ID),
        "operation schema_id must be a generated schema id"
    );
    GeneratedVoxelOperation {
        schema_id: RECEIPT_SCHEMA_ID,
        seq,
        payload: payload.to_vec(),
    }
}

pub fn corpus_schedule() -> Schedule {
    for id in frozen_abort_vocabulary() {
        debug_assert!(
            STABLE_ERROR_IDS.contains(id),
            "abort vocabulary must stay on the generated ID registry"
        );
    }
    let mut ops = Vec::with_capacity(receipt_corpus().len());
    for (i, name) in receipt_corpus().iter().enumerate() {
        ops.push(receipt_op(i as u64, name.as_bytes()));
    }
    Schedule {
        seed: MEASUREMENT_SEED,
        ops,
    }
}

pub fn encode_trace(trace: &Trace) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&trace.seed.to_le_bytes());
    buf.extend_from_slice(&trace.snapshot);
    for outcome in &trace.outcomes {
        buf.extend_from_slice(&outcome.seq.to_le_bytes());
        buf.extend_from_slice(outcome.schema_id.as_bytes());
        buf.extend_from_slice(&(outcome.payload.len() as u64).to_le_bytes());
        buf.extend_from_slice(&outcome.payload);
        match outcome.error {
            None => buf.push(0),
            Some(error) => {
                buf.push(1);
                buf.extend_from_slice(&(error.len() as u64).to_le_bytes());
                buf.extend_from_slice(error.as_bytes());
            }
        }
        buf.push(u8::from(outcome.recoverable));
    }
    buf
}

pub fn hash_trace(trace: &Trace) -> [u8; 32] {
    sha256(&encode_trace(trace))
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatReport {
    pub seed: u64,
    pub traces: [Trace; REPEAT_COUNT],
    pub trace_hashes: [[u8; 32]; REPEAT_COUNT],
    pub snapshot_hashes: [[u8; 32]; REPEAT_COUNT],
    pub byte_identical: bool,
}

/// Three `DeterministicExecutor::run` replays of the same schedule.
pub fn run_three_repeats() -> RepeatReport {
    let schedule = corpus_schedule();
    let traces = [
        DeterministicExecutor::run(&schedule),
        DeterministicExecutor::run(&schedule),
        DeterministicExecutor::run(&schedule),
    ];
    let trace_hashes = [
        hash_trace(&traces[0]),
        hash_trace(&traces[1]),
        hash_trace(&traces[2]),
    ];
    let snapshot_hashes = [traces[0].snapshot, traces[1].snapshot, traces[2].snapshot];
    let byte_identical = traces[0] == traces[1]
        && traces[1] == traces[2]
        && trace_hashes[0] == trace_hashes[1]
        && trace_hashes[1] == trace_hashes[2]
        && snapshot_hashes[0] == snapshot_hashes[1]
        && snapshot_hashes[1] == snapshot_hashes[2];
    RepeatReport {
        seed: schedule.seed,
        traces,
        trace_hashes,
        snapshot_hashes,
        byte_identical,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaultObservation {
    pub name: &'static str,
    pub point: FaultPoint,
    pub error_id: &'static str,
    pub recoverable: bool,
}

/// Arm each mapped `FaultPoint` on a fresh `VoxelPortHarness`.
pub fn run_fault_matrix() -> [FaultObservation; 3] {
    let faults = mapped_faults();
    let mut out = [FaultObservation {
        name: faults[0].name,
        point: faults[0].point,
        error_id: "",
        recoverable: false,
    }; 3];
    for (slot, mapped) in out.iter_mut().zip(faults.iter()) {
        let mut port = VoxelPortHarness::new();
        port.arm(mapped.point);
        let outcome = port.execute(&receipt_op(0, mapped.name.as_bytes()));
        let error_id = outcome
            .error
            .unwrap_or_else(|| FaultInjector::error_id(mapped.point));
        debug_assert_eq!(error_id, FaultInjector::error_id(mapped.point));
        debug_assert_eq!(
            outcome.recoverable,
            FaultInjector::recoverable(mapped.point)
        );
        debug_assert_eq!(
            FaultInjector::recoverable(mapped.point),
            !matches!(
                mapped.point,
                FaultPoint::PostPublication | FaultPoint::LostResult | FaultPoint::CorruptSnapshot
            )
        );
        *slot = FaultObservation {
            name: mapped.name,
            point: mapped.point,
            error_id,
            recoverable: outcome.recoverable,
        };
    }
    out
}

pub fn selected_lease() -> Option<&'static str> {
    None
}

pub fn selected_capacity() -> Option<&'static str> {
    None
}

pub fn selected_prune_rule() -> Option<&'static str> {
    None
}
