//! VOX-D-001 measurement seam (R-00057).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Occupancy and coordinates are encoded as payload bytes + seq only.
//! This file does not freeze chunk extent, world bounds, page size, or
//! overflow policy, and it does not encode production numeric defaults.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::SCHEMA_IDS;
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};

/// Fixed schedule seed for VOX-D-001 / R-00057. Measurement-only; not a production RNG default.
pub const MEASURE_SEED: u64 = 0x0000_D001_0000_0057;

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
    &["IsolatedCubicExtentFamily", "CoupledPageAxisExtentFamily"]
}

pub fn corpus_names() -> &'static [&'static str] {
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

pub fn fault_matrix_names() -> &'static [&'static str] {
    &[
        "illegal-dimension",
        "extreme-coordinate",
        "memory-pressure",
        "cross-profile-misread",
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NegativeObservation {
    pub scenario: &'static str,
    pub fault_point: &'static str,
    pub error_id: &'static str,
    pub recoverable: bool,
    pub visible_write: bool,
    pub outcome_matches_injector: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureReport {
    pub seed: u64,
    pub repeats: usize,
    pub traces_byte_identical: bool,
    pub snapshot: [u8; 32],
    pub corpus: &'static [&'static str],
    pub op_count: usize,
    pub negative: Vec<NegativeObservation>,
    pub visible_writes_unrecoverable: bool,
    pub selected_candidate: Option<&'static str>,
    pub approval_status: &'static str,
}

/// Drive the shipped harness: three identical `DeterministicExecutor::run`
/// repeats plus a `VoxelPortHarness::arm` negative matrix.
pub fn measure() -> MeasureReport {
    let schedule = measure_schedule();
    let op_count = schedule.ops.len();
    let a = DeterministicExecutor::run(&schedule);
    let b = DeterministicExecutor::run(&schedule);
    let c = DeterministicExecutor::run(&schedule);
    let traces_byte_identical = traces_byte_identical(&a, &b, &c);

    let negative = negative_matrix();
    let visible_writes_unrecoverable = negative
        .iter()
        .filter(|row| row.visible_write)
        .all(|row| !row.recoverable && row.outcome_matches_injector);

    MeasureReport {
        seed: MEASURE_SEED,
        repeats: 3,
        traces_byte_identical,
        snapshot: a.snapshot,
        corpus: corpus_names(),
        op_count,
        negative,
        visible_writes_unrecoverable,
        selected_candidate: None,
        approval_status: approval_status(),
    }
}

pub fn measure_schedule() -> Schedule {
    Schedule {
        seed: MEASURE_SEED,
        ops: corpus_ops(),
    }
}

fn traces_byte_identical(a: &Trace, b: &Trace, c: &Trace) -> bool {
    a == b && b == c
}

fn generated_schema(id: &str) -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|member| *member == id)
        .expect("schema_id must be a generated SCHEMA_IDS member")
}

fn le3(x: i32, y: i32, z: i32) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0..4].copy_from_slice(&x.to_le_bytes());
    out[4..8].copy_from_slice(&y.to_le_bytes());
    out[8..12].copy_from_slice(&z.to_le_bytes());
    out
}

/// Occupancy / coordinate samples live only in payload bytes; `seq` is the
/// schedule order. Integers here are corpus labels, not production defaults.
fn payload(kind: &[u8], coords: [u8; 12], occupancy: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(kind.len() + 1 + coords.len() + occupancy.len());
    out.extend_from_slice(kind);
    out.push(0);
    out.extend_from_slice(&coords);
    out.extend_from_slice(occupancy);
    out
}

fn op(schema_id: &'static str, seq: u64, payload: Vec<u8>) -> GeneratedVoxelOperation {
    GeneratedVoxelOperation {
        schema_id,
        seq,
        payload,
    }
}

fn corpus_ops() -> Vec<GeneratedVoxelOperation> {
    let page = generated_schema("voxel-chunk-page");
    let query = generated_schema("voxel-query");
    let mutation = generated_schema("voxel-mutation-receipt");
    let stamp = generated_schema("voxel-revision-stamp");

    vec![
        op(page, 1, payload(b"sparse", le3(0, 0, 0), &[0x01])),
        op(
            page,
            2,
            payload(
                b"dense",
                le3(0, 0, 0),
                &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            ),
        ),
        op(query, 3, payload(b"boundary-coords", le3(1, 0, -1), &[])),
        op(query, 4, payload(b"negative-coords", le3(-1, -2, -3), &[])),
        op(
            query,
            5,
            payload(b"extreme-coords", le3(i32::MIN, i32::MAX, 0), &[]),
        ),
        op(query, 6, payload(b"cold-read", le3(0, 1, 0), &[])),
        op(query, 7, payload(b"hot-read", le3(0, 1, 0), &[])),
        op(
            mutation,
            8,
            payload(b"bulk-edit", le3(2, -1, 1), &[0x11, 0x22, 0x33]),
        ),
        op(
            mutation,
            9,
            payload(b"bulk-edit", le3(3, -1, 1), &[0x44, 0x55]),
        ),
        op(stamp, 10, payload(b"bulk-edit", le3(3, -1, 1), &[0x66])),
    ]
}

fn fault_point_name(point: FaultPoint) -> &'static str {
    match point {
        FaultPoint::PrePublication => "PrePublication",
        FaultPoint::PostPublication => "PostPublication",
        FaultPoint::LostResult => "LostResult",
        FaultPoint::CorruptSnapshot => "CorruptSnapshot",
        FaultPoint::StaleCompletion => "StaleCompletion",
    }
}

fn is_visible_write(point: FaultPoint) -> bool {
    matches!(
        point,
        FaultPoint::PostPublication | FaultPoint::LostResult | FaultPoint::CorruptSnapshot
    )
}

fn negative_matrix() -> Vec<NegativeObservation> {
    let probe_schema = generated_schema("voxel-chunk-page");
    let cases: [(&'static str, FaultPoint); 5] = [
        ("illegal-dimension", FaultPoint::PrePublication),
        ("extreme-coordinate", FaultPoint::StaleCompletion),
        ("memory-pressure", FaultPoint::LostResult),
        ("memory-pressure", FaultPoint::PostPublication),
        ("cross-profile-misread", FaultPoint::CorruptSnapshot),
    ];

    let mut out = Vec::with_capacity(cases.len());
    for (i, (scenario, point)) in cases.iter().copied().enumerate() {
        let error_id = FaultInjector::error_id(point);
        let recoverable = FaultInjector::recoverable(point);
        let visible_write = is_visible_write(point);

        let mut port = VoxelPortHarness::new();
        port.arm(point);
        let probe = op(
            probe_schema,
            100 + i as u64,
            payload(scenario.as_bytes(), le3(0, 0, 0), &[0xEE]),
        );
        let outcome = port.execute(&probe);
        let outcome_matches_injector =
            outcome.error == Some(error_id) && outcome.recoverable == recoverable;

        out.push(NegativeObservation {
            scenario,
            fault_point: fault_point_name(point),
            error_id,
            recoverable,
            visible_write,
            outcome_matches_injector,
        });
    }
    out
}
