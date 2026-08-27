//! VOX-D-007 measurement seam (R-00063).
//!
//! Not a workspace member. Do not add this path to Cargo.toml.
//! Spatial / mesh / collision is a projection over generated `voxel-query`
//! ops; this file is not production spatial code and does not invent a
//! public spatial Schema. NativeCore kernel artifact hashes stay unfrozen.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::SCHEMA_IDS;
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{
    GeneratedVoxelOperation, GeneratedVoxelOutcome, VoxelPortHarness,
};

/// Architecture-owner approval status. Stays blocked until the owner decides.
pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn gate_id() -> &'static str {
    "VOX-D-007"
}

pub fn card_id() -> &'static str {
    "R-00063"
}

/// Consumed harness card. Do not invent a substitute.
pub fn harness_requirement() -> &'static str {
    "R-00047"
}

pub fn harness_requirement_met() -> bool {
    true
}

/// Generated schema id used by every op. Spatial is a projection over query.
pub fn query_schema_id() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == "voxel-query")
        .expect("generated SCHEMA_IDS must include voxel-query")
}

/// No public spatial Schema exists; do not invent one.
pub fn public_spatial_schema_id() -> Option<&'static str> {
    None
}

/// Candidate identifiers only. Order is not a ranking; the first id is not a default.
pub fn candidate_ids() -> &'static [&'static str] {
    &[
        "reference-voxel-kernel",
        "nativecore-spatial-adapter",
        "unaudited-oss-kernel",
    ]
}

pub fn selected_default_candidate() -> Option<&'static str> {
    None
}

/// Unpublished NativeCore kernel artifact. Must not be frozen as a production default.
pub fn nativecore_kernel_artifact_hash() -> Option<&'static str> {
    None
}

pub fn native_differential_skip_reason() -> &'static str {
    "NativeCore spatial/collision kernel artifact unpublished; adapter hash not frozen as a production default"
}

/// Executed corpus. Spatial projection over `voxel-query`; no new schema.
pub fn corpus_ids() -> &'static [&'static str] {
    &[
        "candidate-set",
        "occlusion-miss",
        "cache-key-with-world-revision",
        "cancel-before-complete",
    ]
}

pub const REPEAT_RUNS: usize = 3;
pub const MEASUREMENT_SEED: u64 = 0x0000_0007;

/// Research axes still open for architecture-owner numeric policy.
pub fn planned_measurement_axes() -> &'static [&'static str] {
    &[
        "candidate-projection",
        "occlusion",
        "mesh",
        "collision",
        "cache",
    ]
}

/// Research fault names mapped onto shipped `FaultPoint` values.
pub fn planned_fault_axes() -> &'static [&'static str] {
    &[
        "cross-world-cache-hit",
        "missing-neighbor-chunk",
        "cancel-after-visible",
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappedFault {
    pub name: &'static str,
    pub point: FaultPoint,
}

pub fn mapped_faults() -> &'static [MappedFault] {
    &[
        MappedFault {
            name: "cross-world-cache-hit",
            point: FaultPoint::CorruptSnapshot,
        },
        MappedFault {
            name: "missing-neighbor-chunk",
            point: FaultPoint::LostResult,
        },
        MappedFault {
            name: "cancel-after-visible",
            point: FaultPoint::PostPublication,
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatMeasurement {
    pub corpus_id: &'static str,
    pub traces: [Trace; REPEAT_RUNS],
}

impl RepeatMeasurement {
    pub fn identical(&self) -> bool {
        self.traces.windows(2).all(|w| w[0] == w[1])
    }

    pub fn snapshot_hexes(&self) -> [String; REPEAT_RUNS] {
        [
            snapshot_hex(&self.traces[0].snapshot),
            snapshot_hex(&self.traces[1].snapshot),
            snapshot_hex(&self.traces[2].snapshot),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultRepeatMeasurement {
    pub name: &'static str,
    pub point: FaultPoint,
    pub outcomes: [GeneratedVoxelOutcome; REPEAT_RUNS],
}

impl FaultRepeatMeasurement {
    pub fn identical(&self) -> bool {
        self.outcomes.windows(2).all(|w| w[0] == w[1])
    }
}

/// True after the shipped harness three-run compare on this seam's corpus.
pub fn measurements_executed() -> bool {
    corpus_repeat_measurements()
        .iter()
        .all(RepeatMeasurement::identical)
        && fault_repeat_measurements()
            .iter()
            .all(FaultRepeatMeasurement::identical)
        && cache_key_identity_isolated()
        && cancel_before_complete_does_not_insert()
}

pub fn measurements_skip_reason() -> Option<&'static str> {
    None
}

pub fn query_op(seq: u64, payload: &str) -> GeneratedVoxelOperation {
    GeneratedVoxelOperation {
        schema_id: query_schema_id(),
        seq,
        payload: payload.as_bytes().to_vec(),
    }
}

/// Test-private identity fields on a query payload. Not a public spatial Schema.
pub fn projection_payload(
    corpus: &str,
    world: &str,
    generation: u64,
    world_revision: u64,
    chunk_revision: u64,
    extra: &str,
) -> String {
    format!(
        "corpus={corpus}|worldContext={world}|generation={generation}|worldRevision={world_revision}|chunkRevision={chunk_revision}|{extra}"
    )
}

pub fn corpus_schedule(corpus_id: &str) -> Schedule {
    let seed = MEASUREMENT_SEED;
    let ops = match corpus_id {
        "candidate-set" => vec![
            query_op(
                0,
                &projection_payload(
                    "candidate-set",
                    "alpha",
                    1,
                    10,
                    3,
                    "kind=candidate|coord=0,0,0|count=4|order=stable",
                ),
            ),
            query_op(
                1,
                &projection_payload(
                    "candidate-set",
                    "alpha",
                    1,
                    10,
                    3,
                    "kind=candidate|coord=1,0,0|count=4|order=stable",
                ),
            ),
        ],
        "occlusion-miss" => vec![query_op(
            0,
            &projection_payload(
                "occlusion-miss",
                "alpha",
                1,
                10,
                3,
                "kind=occlusion|neighbor=Pending|notEmptyWorld=1",
            ),
        )],
        "cache-key-with-world-revision" => vec![
            query_op(
                0,
                &projection_payload(
                    "cache-key-with-world-revision",
                    "alpha",
                    1,
                    10,
                    3,
                    "kind=cache|coord=0,0,0",
                ),
            ),
            query_op(
                1,
                &projection_payload(
                    "cache-key-with-world-revision",
                    "alpha",
                    1,
                    11,
                    3,
                    "kind=cache|coord=0,0,0",
                ),
            ),
            query_op(
                2,
                &projection_payload(
                    "cache-key-with-world-revision",
                    "beta",
                    1,
                    10,
                    3,
                    "kind=cache|coord=0,0,0",
                ),
            ),
        ],
        "cancel-before-complete" => vec![
            query_op(
                0,
                &projection_payload(
                    "cancel-before-complete",
                    "alpha",
                    1,
                    10,
                    3,
                    "kind=project|status=started",
                ),
            ),
            query_op(
                1,
                &projection_payload(
                    "cancel-before-complete",
                    "alpha",
                    1,
                    10,
                    3,
                    "kind=cancel|beforeComplete=1",
                ),
            ),
        ],
        other => panic!("unknown corpus id: {other}"),
    };
    Schedule { seed, ops }
}

pub fn corpus_schedules() -> Vec<Schedule> {
    corpus_ids().iter().map(|id| corpus_schedule(id)).collect()
}

pub fn repeat_runs(schedule: &Schedule) -> [Trace; REPEAT_RUNS] {
    [
        DeterministicExecutor::run(schedule),
        DeterministicExecutor::run(schedule),
        DeterministicExecutor::run(schedule),
    ]
}

pub fn corpus_repeat_measurements() -> Vec<RepeatMeasurement> {
    corpus_ids()
        .iter()
        .map(|id| RepeatMeasurement {
            corpus_id: id,
            traces: repeat_runs(&corpus_schedule(id)),
        })
        .collect()
}

pub fn execute_mapped_fault(
    fault: MappedFault,
    op: &GeneratedVoxelOperation,
) -> GeneratedVoxelOutcome {
    let mut port = VoxelPortHarness::new();
    port.arm(fault.point);
    port.execute(op)
}

pub fn fault_repeat_measurements() -> Vec<FaultRepeatMeasurement> {
    mapped_faults()
        .iter()
        .copied()
        .map(|fault| {
            let op = query_op(1, &format!("kind=fault|name={}", fault.name));
            FaultRepeatMeasurement {
                name: fault.name,
                point: fault.point,
                outcomes: [
                    execute_mapped_fault(fault, &op),
                    execute_mapped_fault(fault, &op),
                    execute_mapped_fault(fault, &op),
                ],
            }
        })
        .collect()
}

/// Same coordinates, different World / Revision → different snapshot hashes.
pub fn cache_key_identity_isolated() -> bool {
    let hashes = cache_key_identity_snapshots();
    hashes.same_world_revision != hashes.revision_bump
        && hashes.same_world_revision != hashes.other_world
        && hashes.revision_bump != hashes.other_world
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheKeySnapshots {
    pub same_world_revision: [u8; 32],
    pub revision_bump: [u8; 32],
    pub other_world: [u8; 32],
}

pub fn cache_key_identity_snapshots() -> CacheKeySnapshots {
    let coord = "kind=cache|coord=0,0,0";
    let run = |world: &str, world_revision: u64| {
        DeterministicExecutor::run(&Schedule {
            seed: MEASUREMENT_SEED,
            ops: vec![query_op(
                0,
                &projection_payload(
                    "cache-key-with-world-revision",
                    world,
                    1,
                    world_revision,
                    3,
                    coord,
                ),
            )],
        })
        .snapshot
    };
    CacheKeySnapshots {
        same_world_revision: run("alpha", 10),
        revision_bump: run("alpha", 11),
        other_world: run("beta", 10),
    }
}

/// PrePublication cancel does not insert into the harness committed set.
pub fn cancel_before_complete_does_not_insert() -> bool {
    let mut port = VoxelPortHarness::new();
    let empty = port.snapshot_hash();
    port.arm(FaultPoint::PrePublication);
    let out = port.execute(&query_op(
        0,
        &projection_payload(
            "cancel-before-complete",
            "alpha",
            1,
            10,
            3,
            "kind=cancel|beforeComplete=1",
        ),
    ));
    out.error == Some(FaultInjector::error_id(FaultPoint::PrePublication))
        && out.recoverable
        && port.snapshot_hash() == empty
}

pub fn snapshot_hex(snapshot: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for &b in snapshot {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_remains_blocked() {
        assert_eq!(approval_status(), "blocked");
        assert_eq!(gate_id(), "VOX-D-007");
        assert_eq!(card_id(), "R-00063");
        assert!(harness_requirement_met());
        assert_eq!(harness_requirement(), "R-00047");
        assert!(measurements_executed());
        assert_eq!(measurements_skip_reason(), None);
        assert!(candidate_ids().len() >= 2);
        assert!(
            candidate_ids()
                .iter()
                .any(|id| *id == "unaudited-oss-kernel"),
            "held-out unaudited OSS row must remain listed, not selected"
        );
        assert_eq!(selected_default_candidate(), None);
        assert_eq!(nativecore_kernel_artifact_hash(), None);
        assert_eq!(public_spatial_schema_id(), None);
    }

    #[test]
    fn ops_use_generated_voxel_query_schema() {
        assert_eq!(query_schema_id(), "voxel-query");
        assert!(SCHEMA_IDS.contains(&query_schema_id()));
        for schedule in corpus_schedules() {
            assert!(!schedule.ops.is_empty());
            for op in &schedule.ops {
                assert_eq!(op.schema_id, "voxel-query");
                assert!(SCHEMA_IDS.contains(&op.schema_id));
            }
        }
    }

    #[test]
    fn three_repeat_runs_are_byte_identical() {
        for measured in corpus_repeat_measurements() {
            assert!(
                measured.identical(),
                "corpus {} traces must match across three runs",
                measured.corpus_id
            );
            let hexes = measured.snapshot_hexes();
            assert_eq!(hexes[0], hexes[1]);
            assert_eq!(hexes[1], hexes[2]);
        }
    }

    #[test]
    fn cache_key_includes_world_and_revision() {
        assert!(cache_key_identity_isolated());
        let hashes = cache_key_identity_snapshots();
        assert_ne!(hashes.same_world_revision, hashes.revision_bump);
        assert_ne!(hashes.same_world_revision, hashes.other_world);
        assert_ne!(hashes.revision_bump, hashes.other_world);
    }

    #[test]
    fn occlusion_miss_is_not_empty_world_payload() {
        let schedule = corpus_schedule("occlusion-miss");
        let payload = String::from_utf8(schedule.ops[0].payload.clone()).unwrap();
        assert!(payload.contains("neighbor=Pending"));
        assert!(payload.contains("notEmptyWorld=1"));
        assert!(!payload.contains("emptyWorld=1"));
    }

    #[test]
    fn cancel_before_complete_does_not_hit_cache() {
        assert!(cancel_before_complete_does_not_insert());
        let schedule = corpus_schedule("cancel-before-complete");
        let last = String::from_utf8(schedule.ops.last().unwrap().payload.clone()).unwrap();
        assert!(last.contains("beforeComplete=1"));
        assert!(!last.contains("status=completed"));
    }

    #[test]
    fn faults_map_to_shipped_fault_points() {
        let expected = [
            (
                "cross-world-cache-hit",
                FaultPoint::CorruptSnapshot,
                "EvidenceDigestMismatch",
                false,
            ),
            (
                "missing-neighbor-chunk",
                FaultPoint::LostResult,
                "EvidenceMissing",
                false,
            ),
            (
                "cancel-after-visible",
                FaultPoint::PostPublication,
                "PartialLoadRolledBack",
                false,
            ),
        ];
        let mapped = mapped_faults();
        assert_eq!(mapped.len(), expected.len());
        for (fault, (name, point, error, recoverable)) in mapped.iter().zip(expected) {
            assert_eq!(fault.name, name);
            assert_eq!(fault.point, point);
            assert_eq!(FaultInjector::error_id(fault.point), error);
            assert_eq!(FaultInjector::recoverable(fault.point), recoverable);
            assert!(
                !recoverable,
                "{name} is a visible or identity fault and must not look like a generic retry"
            );
        }
        for measured in fault_repeat_measurements() {
            assert!(
                measured.identical(),
                "fault {} outcomes must match across three runs",
                measured.name
            );
            let error = FaultInjector::error_id(measured.point);
            for out in &measured.outcomes {
                assert_eq!(out.schema_id, "voxel-query");
                assert_eq!(out.error, Some(error));
                assert!(!out.recoverable);
            }
        }
    }

    #[test]
    fn nativecore_kernel_hash_is_not_frozen() {
        assert_eq!(nativecore_kernel_artifact_hash(), None);
        assert!(native_differential_skip_reason().contains("not frozen"));
    }

    #[test]
    fn print_raw_trace_hashes() {
        println!("approval_status={}", approval_status());
        println!(
            "nativecore_kernel_artifact_hash={:?}",
            nativecore_kernel_artifact_hash()
        );
        println!("public_spatial_schema_id={:?}", public_spatial_schema_id());
        for measured in corpus_repeat_measurements() {
            let hexes = measured.snapshot_hexes();
            println!(
                "corpus={} identical={} run1={} run2={} run3={}",
                measured.corpus_id,
                measured.identical(),
                hexes[0],
                hexes[1],
                hexes[2]
            );
        }
        let keys = cache_key_identity_snapshots();
        println!(
            "cache-key same={} rev={} world={}",
            snapshot_hex(&keys.same_world_revision),
            snapshot_hex(&keys.revision_bump),
            snapshot_hex(&keys.other_world)
        );
        for measured in fault_repeat_measurements() {
            println!(
                "fault={} point={:?} error={:?} recoverable={} identical={}",
                measured.name,
                measured.point,
                measured.outcomes[0].error,
                measured.outcomes[0].recoverable,
                measured.identical()
            );
        }
        println!(
            "cancel_before_complete_does_not_insert={}",
            cancel_before_complete_does_not_insert()
        );
    }
}
