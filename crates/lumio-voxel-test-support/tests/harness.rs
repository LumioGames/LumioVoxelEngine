use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule};
use lumio_voxel_test_support::fault_injection::FaultPoint;
use lumio_voxel_test_support::fixture_runner::run_fixture;
use lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};
use lumio_voxel_test_support::reference_harness::run_reference_rust_differential;
use std::path::PathBuf;

fn op(seq: u64, payload: &[u8]) -> GeneratedVoxelOperation {
    GeneratedVoxelOperation {
        schema_id: "voxel-query",
        seq,
        payload: payload.to_vec(),
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn hashmap_fold_is_not_schedule_order() {
    let ops: Vec<_> = (0..32).map(|i| op(i, &[i as u8])).collect();
    let vec_fold = DeterministicExecutor::vec_fold_payloads(&ops);
    let map_fold = DeterministicExecutor::hashmap_fold_payloads(&ops);
    assert_ne!(
        vec_fold, map_fold,
        "HashMap iteration must not be treated as the schedule"
    );
}

#[test]
fn same_seed_and_schedule_repeat_byte_identical() {
    let schedule = Schedule {
        seed: 0xA11CE,
        ops: (0..8).map(|i| op(i, &[i as u8, 7])).collect(),
    };
    let a = DeterministicExecutor::run(&schedule);
    let b = DeterministicExecutor::run(&schedule);
    assert_eq!(a, b);
    assert_eq!(a.snapshot, b.snapshot);
}

#[test]
fn five_fault_points() {
    let cases = [
        (FaultPoint::PrePublication, true, "InvalidHandle"),
        (FaultPoint::PostPublication, false, "PartialLoadRolledBack"),
        (FaultPoint::LostResult, false, "EvidenceMissing"),
        (FaultPoint::CorruptSnapshot, false, "EvidenceDigestMismatch"),
        (FaultPoint::StaleCompletion, true, "StaleEpoch"),
    ];
    for (point, recoverable, error) in cases {
        let mut port = VoxelPortHarness::new();
        port.arm(point);
        let out = port.execute(&op(1, b"x"));
        assert_eq!(out.error, Some(error), "{point:?}");
        assert_eq!(
            out.recoverable, recoverable,
            "{point:?} must not look like a generic retry"
        );
        if !recoverable {
            assert!(!out.recoverable);
        }
    }
}

#[test]
fn fixture_runner_positive_and_unknown_schema() {
    let mut port = VoxelPortHarness::new();
    let ok = run_fixture(&fixtures_dir().join("positive-query.json"), &mut port).unwrap();
    assert!(ok.passed, "{ok:?}");
    assert_eq!(ok.seed, 1);
    assert_eq!(ok.trace.outcomes.len(), 2);

    let mut port = VoxelPortHarness::new();
    let bad = run_fixture(&fixtures_dir().join("unknown-schema.json"), &mut port);
    assert!(bad.unwrap_err().contains("unknown schema_id"));
}

#[test]
fn production_crates_do_not_depend_on_test_support() {
    let graph = lumio_voxel_test_support::crate_dag::live_graph(
        &lumio_voxel_test_support::workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR")),
    )
    .expect("live graph");
    let v = lumio_voxel_test_support::crate_dag::violations(&graph);
    assert!(v.is_empty(), "{v:?}");
    for (krate, deps) in &graph {
        if krate != "lumio-voxel-test-support" {
            assert!(
                !deps.iter().any(|d| d == "lumio-voxel-test-support"),
                "{krate} must not depend on test-support"
            );
        }
    }
}

#[test]
fn reference_and_rust_run_the_same_real_mutation_revision_snapshot_sequence() {
    let report = run_reference_rust_differential().expect("reference/Rust differential");
    assert!(!report.steps.is_empty(), "differential must execute real steps");
    assert_eq!(
        report.rust_trace_hash, report.reference_trace_hash,
        "Rust and Reference trace hashes diverged: {report:?}"
    );
    for step in &report.steps {
        assert_eq!(
            step.rust_world_revision, step.reference_world_revision,
            "revision mismatch at {}",
            step.operation
        );
        assert_eq!(
            step.rust_snapshot_hash, step.reference_snapshot_hash,
            "snapshot hash mismatch at {}",
            step.operation
        );
    }
    assert!(report.evidence().contains("adapter.commit"));
    assert!(report.evidence().contains("encode_capture"));
}
