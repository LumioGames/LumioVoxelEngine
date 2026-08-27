//! R-00145: B2 matrix against shipped Query / Mutation / World / Restore APIs.

use lumio_voxel_contracts::{BASELINE_ID, CHUNK_PRESENCE, STABLE_ERROR_IDS};
use lumio_voxel_test_support::b2_harness::{
    MATRIX_ROWS, case_capture_encode_outside_barrier, case_commit_atomic_duplicate,
    case_dual_world_fault_isolation, case_durability_ack_covers_latest,
    case_fault_injector_recoverable, case_port_adapter_routes, case_prepare_does_not_publish,
    case_prepare_wrong_world, case_query_cancel_budget, case_query_four_state,
    case_query_single_cut_plan_hash, case_restore_preflight_and_swap, run_b2_matrix,
};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};

fn assert_case_ok(case: lumio_voxel_test_support::b2_harness::B2CaseResult) {
    assert!(case.ok, "{} {}: {}", case.id, case.name, case.detail);
}

#[test]
fn run_b2_matrix_covers_twelve_rows() {
    let report = run_b2_matrix();
    assert_eq!(report.baseline, BASELINE_ID);
    assert_eq!(report.baseline, "LGE-V1.4-2026-08-27");
    assert_eq!(report.cases.len(), MATRIX_ROWS);
    assert_eq!(MATRIX_ROWS, 12);
    assert!(
        report.all_ok(),
        "B2 matrix failed: cases={:?}",
        report.cases
    );
    let ids: Vec<_> = report.cases.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"
        ]
    );
}

#[test]
fn query_single_cut_plan_hash_and_stamp_mismatch() {
    assert_case_ok(case_query_single_cut_plan_hash());
}

#[test]
fn query_four_state_maps_absent_to_not_loaded() {
    assert_eq!(
        CHUNK_PRESENCE,
        &["Ready", "NotLoaded", "Pending", "Unavailable"]
    );
    assert_case_ok(case_query_four_state());
}

#[test]
fn query_cancel_and_budget_leave_identity() {
    assert_case_ok(case_query_cancel_budget());
}

#[test]
fn prepare_wrong_world_and_success_do_not_publish() {
    assert_case_ok(case_prepare_wrong_world());
    assert_case_ok(case_prepare_does_not_publish());
}

#[test]
fn commit_is_atomic_and_duplicate_returns_same_receipt() {
    assert_case_ok(case_commit_atomic_duplicate());
}

#[test]
fn dual_world_trip_a_leaves_b_identity() {
    assert_case_ok(case_dual_world_fault_isolation());
}

#[test]
fn capture_restore_and_durability_ack() {
    assert_case_ok(case_capture_encode_outside_barrier());
    assert_case_ok(case_restore_preflight_and_swap());
    assert_case_ok(case_durability_ack_covers_latest());
}

#[test]
fn port_adapter_query_prepare_commit_capture() {
    assert_case_ok(case_port_adapter_routes());
}

#[test]
fn fault_injector_pre_publication_is_recoverable() {
    assert!(FaultInjector::recoverable(FaultPoint::PrePublication));
    assert!(!FaultInjector::recoverable(FaultPoint::PostPublication));
    assert!(STABLE_ERROR_IDS.contains(&FaultInjector::error_id(FaultPoint::PrePublication)));
    assert!(STABLE_ERROR_IDS.contains(&FaultInjector::error_id(FaultPoint::PostPublication)));
    assert_case_ok(case_fault_injector_recoverable());
}
