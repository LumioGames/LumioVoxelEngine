//! R-00146: V1.4 MVP vertical slice via generated voxel-world-port.

use lumio_voxel_contracts::BASELINE_ID;
use lumio_voxel_test_support::b0_harness::{B0VerificationReport, run_b0_matrix};
use lumio_voxel_test_support::b2_harness::{B2VerificationReport, run_b2_matrix};
use lumio_voxel_test_support::mvp_harness::{
    STEP_COUNT, STEP_NAMES, matrix_entry_points, run_mvp_vertical_slice,
};

#[test]
fn b0_and_b2_matrix_entry_points_typecheck() {
    let _: fn() -> B0VerificationReport = run_b0_matrix;
    let _: fn() -> B2VerificationReport = run_b2_matrix;
    let (b0, b2) = matrix_entry_points();
    let _: fn() -> B0VerificationReport = b0;
    let _: fn() -> B2VerificationReport = b2;
}

#[test]
fn mvp_vertical_slice_step_count_and_dual_instance() {
    let report = run_mvp_vertical_slice();
    assert_eq!(report.baseline, BASELINE_ID);
    assert_eq!(report.baseline, "LGE-V1.4-2026-08-27");
    assert_eq!(report.steps.len(), STEP_COUNT);
    assert_eq!(STEP_COUNT, 10);
    let names: Vec<_> = report.steps.iter().map(|step| step.name).collect();
    assert_eq!(names, STEP_NAMES);
    assert_ne!(
        report.authority_identity, report.replica_identity,
        "independent VoxelWorld identities must differ"
    );
    assert!(
        report.all_ok(),
        "MVP vertical slice failed: steps={:?}",
        report.steps
    );
    let duplicate = report
        .steps
        .iter()
        .find(|step| step.name == "duplicate_replay")
        .expect("duplicate_replay step");
    assert!(
        duplicate.detail.contains("original receipt"),
        "duplicate_replay must drive adapter.commit and keep the original receipt: {}",
        duplicate.detail
    );
    assert_eq!(
        report
            .receipts
            .iter()
            .filter(|receipt| receipt.txn_id == "txn-mvp")
            .count(),
        1,
        "duplicate commit must not record a second txn-mvp receipt"
    );
}
