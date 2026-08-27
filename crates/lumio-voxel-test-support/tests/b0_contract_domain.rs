//! R-00143: B0 matrix against shipped Artifact / Revision / Chunk / Publication / Port APIs.

use lumio_voxel_contracts::{
    BASELINE_ID, BINDINGS, CHUNK_PRESENCE, SCHEMA_IDS, STABLE_ERROR_IDS, verify_artifact_hashes,
};
use lumio_voxel_domain::chunk::ChunkSlot;
use lumio_voxel_domain::revision::RevisionAllocator;
use lumio_voxel_test_support::b0_harness::{
    MATRIX_ROWS, case_artifact_hash_lock, case_chunk_four_state, case_deterministic_executor,
    case_dirty_frontier_pure, case_dual_voxel_world, case_pin_reclaim, case_port_schema_intern,
    case_publication_old_or_new, case_revision_monotonic, case_seven_crate_dag, run_b0_matrix,
};
use lumio_voxel_test_support::crate_dag::{self, SEVEN_CRATES};
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule};
use lumio_voxel_test_support::reference_harness::GeneratedVoxelOperation;

fn assert_case_ok(case: lumio_voxel_test_support::b0_harness::B0CaseResult) {
    assert!(case.ok, "{} {}: {}", case.id, case.name, case.detail);
}

#[test]
fn run_b0_matrix_covers_ten_rows() {
    let report = run_b0_matrix();
    assert_eq!(report.baseline, BASELINE_ID);
    assert_eq!(report.baseline, "LGE-V1.4-2026-08-27");
    assert_eq!(report.cases.len(), MATRIX_ROWS);
    assert_eq!(MATRIX_ROWS, 10);
    assert!(
        report.all_ok(),
        "B0 matrix failed: artifact_ok={} dag_ok={} cases={:?}",
        report.artifact_ok,
        report.dag_ok,
        report.cases
    );
    assert!(report.artifact_ok);
    assert!(report.dag_ok);
    let ids: Vec<_> = report.cases.iter().map(|c| c.id).collect();
    assert_eq!(ids, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]);
}

#[test]
fn artifact_hashes_verify_ok() {
    verify_artifact_hashes().expect("shipped verify_artifact_hashes");
    assert_case_ok(case_artifact_hash_lock());
}

#[test]
fn seven_crate_dag_legal_empty_forbidden_token() {
    assert_eq!(SEVEN_CRATES.len(), 7);
    let legal = crate_dag::parse_fixture_graph(include_str!(
        "../../../tools/architecture/fixtures/dag-legal.json"
    ));
    let legal_v = crate_dag::violations(&legal);
    assert!(legal_v.is_empty(), "legal graph: {legal_v:?}");
    let extra = crate_dag::parse_fixture_graph(include_str!(
        "../../../tools/architecture/fixtures/dag-forbidden-persistence.json"
    ));
    let extra_v = crate_dag::violations(&extra);
    assert!(
        extra_v.iter().any(|s| s.contains("禁止的额外 crate 名")),
        "expected forbidden extra crate token, got {extra_v:?}"
    );
    assert_case_ok(case_seven_crate_dag());
}

#[test]
fn revision_allocator_is_monotonic_with_abandon_hole() {
    let mut alloc = RevisionAllocator::new();
    let mut first = alloc.reserve_world().expect("reserve 0");
    first.abandon();
    assert_eq!(first.finalize().unwrap_err().error_id(), "InvalidHandle");
    assert!(STABLE_ERROR_IDS.contains(&"InvalidHandle"));
    let mut next = alloc.reserve_world().expect("reserve after hole");
    assert_eq!(next.value().value(), 1, "abandoned 0 is a hole");
    next.finalize().expect("finalize 1");
    assert_case_ok(case_revision_monotonic());
}

#[test]
fn chunk_presence_is_interned_and_illegal_convert_fails() {
    assert_eq!(
        CHUNK_PRESENCE,
        &["Ready", "NotLoaded", "Pending", "Unavailable"]
    );
    let slot = ChunkSlot::unavailable();
    let before = slot.clone();
    let err = slot
        .try_convert("Ready", None)
        .expect_err("illegal convert");
    assert_eq!(err.error_id(), "ChunkUnavailable");
    assert!(STABLE_ERROR_IDS.contains(&err.error_id()));
    assert_eq!(slot, before);
    let interned = slot.presence();
    assert!(
        CHUNK_PRESENCE
            .iter()
            .any(|item| std::ptr::eq(*item, interned))
    );
    assert_case_ok(case_chunk_four_state());
}

#[test]
fn publication_capture_is_old_or_new_never_mixed() {
    assert_case_ok(case_publication_old_or_new());
}

#[test]
fn port_adapter_interns_schema_and_binding() {
    assert!(SCHEMA_IDS.contains(&"voxel-world-port"));
    assert!(BINDINGS.iter().any(|binding| {
        binding.schema_id == "voxel-world-port" && binding.rust_type == "VoxelWorldPort"
    }));
    let interned = SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == "voxel-world-port")
        .expect("voxel-world-port");
    assert!(std::ptr::eq(
        interned,
        SCHEMA_IDS
            .iter()
            .copied()
            .find(|id| *id == "voxel-world-port")
            .unwrap()
    ));
    assert_case_ok(case_port_schema_intern());
}

#[test]
fn pin_dirty_dual_world_and_executor_cases_pass() {
    assert_case_ok(case_pin_reclaim());
    assert_case_ok(case_dirty_frontier_pure());
    assert_case_ok(case_dual_voxel_world());
    assert_case_ok(case_deterministic_executor());
}

#[test]
fn deterministic_executor_two_seeds_hashmap_fold_is_not_vec_fold() {
    let ops: Vec<_> = (0..32)
        .map(|i| GeneratedVoxelOperation {
            schema_id: "voxel-query",
            seq: i,
            payload: vec![i as u8],
        })
        .collect();
    assert_ne!(
        DeterministicExecutor::vec_fold_payloads(&ops),
        DeterministicExecutor::hashmap_fold_payloads(&ops)
    );
    for seed in [0x00A1_1CE0, 0x00B0_5EED] {
        let schedule = Schedule {
            seed,
            ops: ops.clone(),
        };
        let a = DeterministicExecutor::run(&schedule);
        let b = DeterministicExecutor::run(&schedule);
        assert_eq!(a, b, "seed {seed:#x}");
        assert_eq!(a.snapshot, b.snapshot);
    }
}
