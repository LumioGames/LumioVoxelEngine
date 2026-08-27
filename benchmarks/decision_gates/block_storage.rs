//! VOX-D-002 measurement seam (R-00058).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Does not encode a production backend default or pull an unaudited crate.

#![forbid(unsafe_code)]

use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule};
use lumio_voxel_test_support::fault_injection::FaultPoint;
use lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};

/// Architecture-owner approval status for this gate.
pub fn approval_status() -> &'static str {
    "approved"
}

pub fn approval_reference() -> &'static str {
    "LGE-V1.4-VOX-D-P0-2026-08-28"
}

/// Adapter-internal backend. Not a generated compressor default.
pub fn selected_family() -> &'static str {
    "DenseUncompressedAdapter"
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

/// Occupancy labels used as payload bytes. Not a selected backend default.
pub fn corpus_labels() -> &'static [&'static str] {
    &["air", "repeated", "high-entropy"]
}

/// Generated schema ids used in the corpus schedule (no second Schema).
pub fn generated_schema_ids() -> &'static [&'static str] {
    &[
        "voxel-chunk-page",
        "voxel-query",
        "voxel-mutation-receipt",
    ]
}

/// Fault labels mapped onto shipped unrecoverable-after-visible-write points.
pub fn fault_matrix() -> &'static [(&'static str, FaultPoint)] {
    &[
        ("corrupt-page", FaultPoint::CorruptSnapshot),
        ("mixed-backend", FaultPoint::PostPublication),
        ("unaudited-codec", FaultPoint::LostResult),
    ]
}

/// Fixed seed for the corpus `Schedule`. Not a production config value.
pub const SCHEDULE_SEED: u64 = 0x0002_D002;

/// No backend is selected; the first `candidate_names()` row is list order only.
pub fn selected_backend() -> Option<&'static str> {
    None
}

pub fn corpus_schedule() -> Schedule {
    let mut ops = Vec::new();
    let mut seq = 0u64;
    for &label in corpus_labels() {
        for &schema_id in generated_schema_ids() {
            ops.push(GeneratedVoxelOperation {
                schema_id,
                seq,
                payload: label.as_bytes().to_vec(),
            });
            seq += 1;
        }
    }
    Schedule {
        seed: SCHEDULE_SEED,
        ops,
    }
}

/// Three identical schedule replays. Compare `Trace.snapshot` (and full `Trace`).
pub fn replay_three() -> ThreeRun {
    let schedule = corpus_schedule();
    let a = DeterministicExecutor::run(&schedule);
    let b = DeterministicExecutor::run(&schedule);
    let c = DeterministicExecutor::run(&schedule);
    ThreeRun {
        snapshots: [a.snapshot, b.snapshot, c.snapshot],
        traces_eq: a == b && b == c,
        snapshots_eq: a.snapshot == b.snapshot && b.snapshot == c.snapshot,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeRun {
    pub snapshots: [[u8; 32]; 3],
    pub traces_eq: bool,
    pub snapshots_eq: bool,
}

/// Drive `VoxelPortHarness` with each fault label. Visible-write faults stay unrecoverable.
pub fn drive_fault_matrix() -> [FaultReplay; 3] {
    let rows = fault_matrix();
    core::array::from_fn(|i| {
        let (label, point) = rows[i];
        let mut port = VoxelPortHarness::new();
        port.arm(point);
        let outcome = port.execute(&GeneratedVoxelOperation {
            schema_id: "voxel-chunk-page",
            seq: i as u64,
            payload: label.as_bytes().to_vec(),
        });
        FaultReplay {
            label,
            point,
            error: outcome.error,
            recoverable: outcome.recoverable,
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaultReplay {
    pub label: &'static str,
    pub point: FaultPoint,
    pub error: Option<&'static str>,
    pub recoverable: bool,
}

pub fn visible_write_faults_unrecoverable() -> bool {
    drive_fault_matrix().iter().all(|row| !row.recoverable)
}
