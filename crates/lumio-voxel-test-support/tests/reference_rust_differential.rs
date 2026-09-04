use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_test_support::reference_harness::{
    ReferenceRustDifferentialReport, run_reference_rust_differential,
};

const EXPECTED_OPERATION_ORDER: [&str; 11] = [
    "initialize",
    "prime",
    "start",
    "query",
    "query",
    "prepareMutation",
    "commit",
    "commitReplayMatrix",
    "captureRestoreMatrix",
    "applyDurabilityAckMatrix",
    "shutdown",
];
const EXPECTED_LIFECYCLE: [&str; 11] = [
    "Initialized",
    "Ready",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Running",
    "Disposed",
];
const EXPECTED_WORLD_REVISIONS: [u64; 11] = [1, 1, 1, 1, 1, 1, 2, 2, 2, 4, 4];
const EXPECTED_GENERATIONS: [u64; 11] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2];
const EXPECTED_STAMP_GENERATIONS: [u64; 11] = [1; 11];
const EXPECTED_PUBLICATION_EPOCHS: [u64; 11] = [0, 0, 0, 0, 0, 0, 1, 1, 1, 6, 6];
const EXPECTED_PROVENANCE: [(&str, &str); 5] = [
    ("architectureBaselineId", "LGE-V1.4-2026-08-27"),
    ("voxelHead", "61cb864978dedfe9bdf7b687fea08660b31469f1"),
    (
        "architectureMirrorSha256",
        "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0",
    ),
    (
        "v13DecisionGatesSha256",
        "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2",
    ),
    (
        "blueprintSha256",
        "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa",
    ),
];
const EXPECTED_CONFIG_HASH: &str =
    "4fd5f112e0d73ff0a043f34bed1804c8ae2b261215d598a251e921039faca1f4";

// Fixed golden over the independently specified, generation-normalized trace
// frame. It is intentionally not generated from either leg at runtime.
//
// Moved from 5a77a11d… by the chunk→section rename (ADR 0013): the trace frame carries
// Section ids, whose canonical text went from `c:x:y:z` to `s:x:y:z`. The two legs are
// separate implementations of the same contract and both landed on the value below,
// which is the only reason it is trustworthy — a golden that only one leg produced
// would prove nothing.
const GOLDEN_TRACE_SHA256: &str =
    "4acbb1e431a770a184eb5139137716e174b7411c39ede322b3cdeebe1cde297b";

#[test]
fn reference_and_rust_execute_shared_canonical_vectors() {
    let report = run_reference_rust_differential().expect("differential runner");
    assert_eq!(report.vector_count(), EXPECTED_OPERATION_ORDER.len());
    assert_eq!(report.sequence_values(), (0..11).collect::<Vec<_>>());
    assert_eq!(report.declared_operation_order(), EXPECTED_OPERATION_ORDER);
    assert_eq!(report.operation_order(), EXPECTED_OPERATION_ORDER);
    assert!(report.unique_contiguous_sequences());
    assert!(report.independent_oracle_verified());
    assert!(report.complete_state_projection());
    assert!(report.roots_and_digests_verified());
    for (
        ((reference, rust), (lifecycle, revision)),
        ((generation, stamp_generation), publication_epoch),
    ) in report
        .reference_observations()
        .iter()
        .zip(report.rust_observations())
        .zip(EXPECTED_LIFECYCLE.iter().zip(EXPECTED_WORLD_REVISIONS))
        .zip(
            EXPECTED_GENERATIONS
                .iter()
                .zip(EXPECTED_STAMP_GENERATIONS)
                .zip(EXPECTED_PUBLICATION_EPOCHS),
        )
    {
        assert_eq!(reference.lifecycle, *lifecycle);
        assert_eq!(rust.lifecycle, *lifecycle);
        assert_eq!(reference.world_revision, revision);
        assert_eq!(rust.world_revision, revision);
        assert_eq!(reference.generation, *generation);
        assert_eq!(rust.generation, *generation);
        assert_eq!(reference.stamp_generation, stamp_generation);
        assert_eq!(rust.stamp_generation, stamp_generation);
        assert_eq!(reference.publication_epoch, publication_epoch);
        assert_eq!(rust.publication_epoch, publication_epoch);
    }
    assert_eq!(report.reference_digest_hex(), GOLDEN_TRACE_SHA256);
    assert_eq!(report.rust_digest_hex(), GOLDEN_TRACE_SHA256);
    assert_eq!(report.reference_digest(), report.rust_digest());
    let observations = report.reference_observations();
    let rust_observations = report.rust_observations();
    for (reference, rust) in observations.iter().zip(rust_observations) {
        for state in [reference, rust] {
            assert_ne!(state.published_root, [0; 32]);
            assert_eq!(state.contract_root, state.state_semantic_digest);
            assert_ne!(state.directory_digest, [0; 32]);
            assert_ne!(state.dirty_digest, [0; 32]);
            assert_eq!(state.lifecycle_machine, "SimulationSession");
            assert_eq!(state.config_hash, EXPECTED_CONFIG_HASH);
            assert!(state.sections.iter().all(|section| {
                (section.presence.as_deref() == Some("Ready")) == section.payload_digest.is_some()
            }));
        }
        assert_eq!(reference.contract_root, rust.contract_root);
        assert_eq!(reference.directory_digest, rust.directory_digest);
        assert_eq!(reference.dirty_digest, rust.dirty_digest);
    }
    assert_eq!(observations[2].probes[0].label, "illegal-transition");
    assert_eq!(
        observations[2].probes[0].error_id.as_deref(),
        Some("InvalidHandle")
    );
    assert_eq!(
        rust_observations[2].probes[0].error_id.as_deref(),
        Some("InvalidHandle")
    );
    assert_eq!(
        observations[3].section_presence,
        vec![
            ("s:-10:0:0".into(), "Ready".into()),
            ("s:-1:0:0".into(), "Ready".into()),
            ("s:0:0:0".into(), "Ready".into()),
            ("s:1:0:0".into(), "Ready".into()),
            ("s:9:0:0".into(), "Unchanged".into()),
        ]
    );
    assert!(
        observations[3]
            .sections
            .iter()
            .all(|section| section.section_revision.is_some() || section.presence.is_none())
    );
    for observation in observations {
        assert_eq!(observation.baseline_id, "LGE-V1.4-2026-08-27");
        assert_eq!(observation.schema_epoch, 1);
        assert_eq!(
            observation
                .gate_source_hashes
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect::<Vec<_>>(),
            EXPECTED_PROVENANCE
        );
        assert_eq!(observation.config_hash.len(), 64);
    }
    assert_eq!(
        observations[7]
            .probes
            .iter()
            .map(|p| p.label.as_str())
            .collect::<Vec<_>>(),
        vec!["exact-replay", "conflicting-replay", "stale-replay"]
    );
    assert_eq!(
        observations[4]
            .probes
            .iter()
            .map(|probe| probe.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "cancel",
            "malformed-coordinate",
            "leading-zero-coordinate",
            "out-of-range-coordinate",
            "wrong-world",
            "wrong-context",
            "stale-generation",
            "wrong-config",
            "budget"
        ]
    );
    assert_eq!(
        observations[4].probes[0].error_id.as_deref(),
        Some("InvalidHandle")
    );
    assert_eq!(
        observations[4].probes[4].error_id.as_deref(),
        Some("SessionMismatch")
    );
    assert_eq!(
        observations[4].probes[6].error_id.as_deref(),
        Some("StaleEpoch")
    );
    assert_eq!(
        observations[4].probes[7].error_id.as_deref(),
        Some("SessionMismatch")
    );
    assert_eq!(
        observations[4].probes[8].error_id.as_deref(),
        Some("BudgetExceeded")
    );
    assert_eq!(
        observations[5]
            .probes
            .iter()
            .map(|probe| probe.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "stale-revision",
            "wrong-world",
            "stale-generation",
            "abort",
            "wrong-config",
            "malformed-section"
        ]
    );
    assert!(observations[5].probes[3].ok);
    assert_eq!(
        observations[5].probes[4].error_id.as_deref(),
        Some("SessionMismatch")
    );
    // "malformed-section" 送的是 s:01:0:0——前导零不是规范写法(契约 key.canonical)。
    assert_eq!(
        observations[5].probes[5].error_id.as_deref(),
        Some(vw::UNKNOWN_SECTION_KEY)
    );
    assert_eq!(
        observations[7].probes[0]
            .receipt
            .as_ref()
            .unwrap()
            .disposition,
        "Duplicate"
    );
    assert_eq!(
        observations[7].probes[1].error_id.as_deref(),
        Some("RevisionConflict")
    );
    assert!(
        observations[6]
            .receipt
            .as_ref()
            .is_some_and(|receipt| receipt.wire_verified && receipt.fingerprint != [0; 32])
    );
    assert_eq!(
        observations[6].receipt.as_ref().unwrap().disposition,
        "Original"
    );
    assert_eq!(
        observations[7].receipt.as_ref().unwrap().fingerprint,
        observations[6].receipt.as_ref().unwrap().fingerprint
    );
    assert_eq!(
        observations[7].receipt.as_ref().unwrap().receipt_hash,
        observations[6].receipt.as_ref().unwrap().receipt_hash
    );
    assert_eq!(
        observations[7].receipt.as_ref().unwrap().receipt_len,
        observations[6].receipt.as_ref().unwrap().receipt_len
    );
    let capture = observations[8].capture.as_ref().unwrap();
    assert_eq!(capture.cut_id, "cut-differential");
    assert_eq!(capture.world_id, "world-differential");
    assert_eq!(capture.context_id, "ctx-differential");
    assert_eq!(capture.config_hash, EXPECTED_CONFIG_HASH);
    assert_eq!(capture.artifact_hash, observations[8].published_root);
    assert_eq!(
        observations[8]
            .probes
            .iter()
            .map(|probe| (probe.label.as_str(), probe.error_id.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("capture-root-mismatch", Some("InvalidHandle")),
            ("truncated", Some("InvalidHandle")),
            ("wrong-world", Some("SessionMismatch")),
            ("wrong-config", Some("EvidenceDigestMismatch")),
            ("stale-generation", Some("StaleEpoch"))
        ]
    );
    // probes[1] = s:-0:0:0,probes[2] = s:01:0:0:两者都是写法非规范,不是坐标越界。
    assert_eq!(
        observations[4].probes[1].error_id.as_deref(),
        Some(vw::UNKNOWN_SECTION_KEY)
    );
    assert_eq!(
        observations[4].probes[2].error_id.as_deref(),
        Some(vw::UNKNOWN_SECTION_KEY)
    );
    // probes[3] = s:2147483648:0:0:这一条才是真的越出 int32。
    assert_eq!(
        observations[4].probes[3].error_id.as_deref(),
        Some(vw::COORDINATE_OUT_OF_BOUNDS)
    );
    assert_eq!(
        observations[9]
            .probes
            .iter()
            .map(|p| p.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "stale",
            "partial",
            "replayed-old",
            "current",
            "duplicate",
            "wrong-world",
            "wrong-context",
            "stale-generation",
            "future-cut",
            "duplicate-section",
            "malformed-section",
            "wrong-kind"
        ]
    );
    assert_eq!(
        observations[9].probes[0].ack.as_ref().unwrap().coverage_len,
        0
    );
    assert_eq!(
        observations[9].probes[1].ack.as_ref().unwrap().coverage_len,
        1
    );
    assert_eq!(
        observations[9].probes[2].ack.as_ref().unwrap().coverage_len,
        0
    );
    assert_eq!(
        observations[9].probes[3].ack.as_ref().unwrap().coverage_len,
        2
    );
    assert_eq!(
        observations[9].probes[4].ack.as_ref().unwrap().coverage_len,
        0
    );
    for probe in observations[9].probes.iter().take(5) {
        let ack = probe.ack.as_ref().unwrap();
        assert_eq!(ack.world_id, "world-differential");
        assert_eq!(ack.context_id, "ctx-differential");
        assert_eq!(ack.generation, 1);
        assert_ne!(ack.old_root, [0; 32]);
        assert_ne!(ack.new_root, [0; 32]);
    }
    assert_eq!(
        observations[9].probes[5].error_id.as_deref(),
        Some("SessionMismatch")
    );
    assert_eq!(
        observations[9].probes[6].error_id.as_deref(),
        Some("SessionMismatch")
    );
    assert_eq!(
        observations[9].probes[7].error_id.as_deref(),
        Some("StaleEpoch")
    );
    assert_eq!(
        observations[9].probes[8].error_id.as_deref(),
        Some("EvidenceDigestMismatch")
    );
    assert_eq!(
        observations[9].probes[9].error_id.as_deref(),
        Some("InvalidHandle")
    );
    // 落盘回执里的 s:-0:0:0 同样是写法非规范。
    assert_eq!(
        observations[9].probes[10].error_id.as_deref(),
        Some(vw::UNKNOWN_SECTION_KEY)
    );
    assert_eq!(
        observations[9].probes[11].error_id.as_deref(),
        Some("InvalidHandle")
    );
    assert!(
        observations[9].probes[2]
            .state
            .dirty_frontier
            .iter()
            .find(|entry| entry.section_id == "s:0:0:0")
            .is_some_and(|entry| entry.latest_revision == Some(2))
    );
    assert!(
        observations[9].probes[3]
            .state
            .dirty_frontier
            .iter()
            .all(|entry| entry.latest_revision.is_none())
    );
    let restore = observations[9].restore.as_ref().unwrap();
    assert_eq!(restore.cut_id, "cut-restore-valid");
    assert_eq!(restore.world_id, "world-differential");
    assert_eq!(restore.context_id, "ctx-differential");
    assert_eq!(restore.config_hash, EXPECTED_CONFIG_HASH);
    assert_eq!(restore.new_root, observations[9].published_root);
    assert_eq!(observations[10].operation, "shutdown");
    assert!(observations[10].route.contains("shutdown"));
    assert!(!observations[10].route.contains("destroy"));
    assert_eq!(rust_observations[10].operation, "shutdown");
    assert!(!rust_observations[10].route.contains("destroy"));
    assert!(
        ReferenceRustDifferentialReport::callable_coverage()
            .iter()
            .all(|row| row.status == "BLOCKED_UPSTREAM")
    );
    assert_eq!(
        ReferenceRustDifferentialReport::unknown_error_disposition(),
        "BLOCKED_UPSTREAM"
    );
    assert!(
        ReferenceRustDifferentialReport::callable_coverage()
            .iter()
            .find(|row| row.method == "destroy")
            .is_some_and(|row| row.local_route.is_none())
    );
    let repeated = run_reference_rust_differential().expect("repeat differential runner");
    assert!(repeated.observations_match());
    assert_eq!(repeated.reference_digest_hex(), GOLDEN_TRACE_SHA256);
    assert_eq!(repeated.rust_digest_hex(), GOLDEN_TRACE_SHA256);
    assert!(
        report.observations_match(),
        "differential mismatch: {report:?}"
    );
}
