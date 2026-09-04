//! R-00066: immutable VoxelConfigSnapshot and CapabilityView.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    CapabilityView, DecisionEvidence, GateSourceHashes, GeneratedHostCapability,
    GeneratedVoxelConfig, P0_DECISION_GATES, VoxelConfigSnapshot,
};
use std::collections::BTreeMap;
use std::sync::Arc;

const BLOCKED_FIXTURE: &str = include_str!("fixtures/p0-gates-blocked.json");

const SECRET: &str = "test-key-material-do-not-log";

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn labeled_hash(label: &str) -> String {
    hex32(&sha256(label.as_bytes()))
}

fn recorded_source_identity() -> GateSourceHashes {
    GateSourceHashes {
        architecture_baseline_id: BASELINE_ID.to_string(),
        voxel_head: "b2f0d8a3763a02f805e29cbd101560ba7fdca77b".to_string(),
        architecture_mirror_sha256:
            "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0".to_string(),
        v13_decision_gates_sha256:
            "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2".to_string(),
        blueprint_sha256: "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa"
            .to_string(),
    }
}

fn recorded_blocked_digests() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "VOX-D-001".to_string(),
            "90c65e89b00030acc9da76282171bf2a36186f814e701744e4943f7759cb1601".to_string(),
        ),
        (
            "VOX-D-002".to_string(),
            "812f1219c606a541679e9887341a4abd156204e7381d408e104d2a7b22fbce51".to_string(),
        ),
        (
            "VOX-D-003".to_string(),
            "17a8f6fffc6620ca1d5f494de05e149622091b55c0f48f2b2095245c23b3accf".to_string(),
        ),
        (
            "VOX-D-004".to_string(),
            "6fe271b63b1a00fbcba4b4984ce86e14317fbda995d10d343edf7b320ff45e1b".to_string(),
        ),
    ])
}

fn p0_allow_list() -> GeneratedHostCapability {
    GeneratedHostCapability::from_names(["Native", "ReferenceVoxel", "VoxelSnapshot"])
}

fn evidence(status: &str, digests: &BTreeMap<String, String>) -> Vec<DecisionEvidence> {
    let source = recorded_source_identity();
    P0_DECISION_GATES
        .iter()
        .map(|gate| DecisionEvidence {
            gate_id: (*gate).to_string(),
            approval_status: status.to_string(),
            source_hashes: source.clone(),
            evidence_digest: digests[*gate].clone(),
        })
        .collect()
}

fn config_with(
    hash_label: &str,
    start: &[&str],
    allow: GeneratedHostCapability,
    digests: BTreeMap<String, String>,
    secret: Option<&str>,
) -> GeneratedVoxelConfig {
    GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: labeled_hash(hash_label),
        gate_source_hashes: digests,
        host_capability: allow,
        start_capabilities: start.iter().map(|s| (*s).to_string()).collect(),
        key_material: secret.map(str::to_string),
    }
}

fn p0_start_config(hash_label: &str, digests: BTreeMap<String, String>) -> GeneratedVoxelConfig {
    config_with(
        hash_label,
        &["Native", "ReferenceVoxel"],
        p0_allow_list(),
        digests,
        Some(SECRET),
    )
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

#[test]
fn generated_schema_ids_are_from_contracts() {
    assert!(SCHEMA_IDS.contains(&"config-table"));
    assert!(SCHEMA_IDS.contains(&"host-capability"));
    assert_eq!(
        P0_DECISION_GATES,
        &["VOX-D-001", "VOX-D-002", "VOX-D-003", "VOX-D-004"]
    );
}

#[test]
fn blocked_p0_gates_refuse_snapshot_listing_vox_d_001_through_004() {
    assert!(BLOCKED_FIXTURE.contains("\"approvalStatus\": \"blocked\""));
    for gate in P0_DECISION_GATES {
        assert!(
            BLOCKED_FIXTURE.contains(&format!("\"{gate}\"")),
            "{gate} missing from fixture"
        );
    }

    let digests = recorded_blocked_digests();
    let cfg = p0_start_config("blocked-p0", digests.clone());
    let ev = evidence("blocked", &digests);
    let result = VoxelConfigSnapshot::from_generated(&cfg, &ev);
    let err = result.expect_err("blocked P0 gates must not materialize a snapshot");
    assert_stable_error(err.error_id());
    assert_eq!(err.error_id(), "TrustPolicyRejected");
    assert_eq!(
        err.blocked_gates(),
        &["VOX-D-001", "VOX-D-002", "VOX-D-003", "VOX-D-004"]
    );
    let rendered = err.to_string();
    for gate in P0_DECISION_GATES {
        assert!(
            rendered.contains(gate),
            "error must list {gate}: {rendered}"
        );
    }
}

#[test]
fn hash_mismatch_and_missing_gate_evidence_are_errors_without_snapshot() {
    let mut digests = recorded_blocked_digests();
    let ev = evidence("approved", &digests);
    digests.insert(
        "VOX-D-002".to_string(),
        labeled_hash("tampered-vox-d-002-digest"),
    );
    let cfg = p0_start_config("hash-mismatch", digests);
    let err = VoxelConfigSnapshot::from_generated(&cfg, &ev)
        .expect_err("digest mismatch must not yield a snapshot");
    assert_stable_error(err.error_id());
    assert_eq!(err.error_id(), "EvidenceDigestMismatch");
    assert!(err.blocked_gates().is_empty());

    let digests = recorded_blocked_digests();
    let mut missing = evidence("approved", &digests);
    missing.retain(|e| e.gate_id != "VOX-D-003");
    let cfg = p0_start_config("missing-gate", digests);
    let err = VoxelConfigSnapshot::from_generated(&cfg, &missing)
        .expect_err("missing P0 gate evidence must not yield a snapshot");
    assert_stable_error(err.error_id());
    assert_eq!(err.error_id(), "EvidenceMissing");
    assert_eq!(err.blocked_gates(), &["VOX-D-003"]);
}

#[test]
fn unknown_capability_name_cannot_expand() {
    let digests = recorded_blocked_digests();
    let cfg = config_with(
        "unknown-cap",
        &["Native", "NotAGeneratedCapability"],
        p0_allow_list(),
        digests.clone(),
        None,
    );
    let ev = evidence("approved", &digests);
    let err = VoxelConfigSnapshot::from_generated(&cfg, &ev)
        .expect_err("unknown capability must not expand the generated allow-list");
    assert_stable_error(err.error_id());
    assert_eq!(err.error_id(), "CapabilityMissing");
    assert!(err.to_string().contains("NotAGeneratedCapability"));
}

#[test]
fn capability_view_can_only_narrow_generated_allow_list() {
    let digests: BTreeMap<String, String> = P0_DECISION_GATES
        .iter()
        .map(|g| ((*g).to_string(), labeled_hash(&format!("approved-{g}"))))
        .collect();
    let cfg = p0_start_config("narrow-ok", digests.clone());
    let ev = evidence("approved", &digests);
    let snapshot = VoxelConfigSnapshot::from_generated(&cfg, &ev).expect("approved P0 evidence");

    let narrowed = GeneratedHostCapability::from_names(["Native"]);
    let view = CapabilityView::derive(&narrowed, &snapshot).expect("narrowing must succeed");
    assert!(view.contains("Native"));
    assert!(!view.contains("ReferenceVoxel"));

    let expanded =
        GeneratedHostCapability::from_names(["Native", "ReferenceVoxel", "VoxelSpatial"]);
    let err = CapabilityView::derive(&expanded, &snapshot).expect_err("expansion must fail");
    assert_stable_error(err.error_id());
    assert_eq!(err.error_id(), "ClaimNotGranted");
    assert!(err.to_string().contains("VoxelSpatial"));
}

#[test]
fn snapshot_is_immutable_and_distinct_hashes_are_independent() {
    let src = include_str!("../src/config_snapshot.rs");
    assert!(
        !src.contains("fn set_"),
        "VoxelConfigSnapshot must not expose setters"
    );
    for needle in ["section_size", "page_size", "batch_limit", "lease_ms"] {
        assert!(
            !src.contains(needle),
            "must not invent VOX-D numeric default field {needle}"
        );
    }

    let digests_a: BTreeMap<String, String> = P0_DECISION_GATES
        .iter()
        .map(|g| ((*g).to_string(), labeled_hash(&format!("a-{g}"))))
        .collect();
    let digests_b: BTreeMap<String, String> = P0_DECISION_GATES
        .iter()
        .map(|g| ((*g).to_string(), labeled_hash(&format!("b-{g}"))))
        .collect();
    let mut cfg_a = p0_start_config("config-a", digests_a.clone());
    let cfg_b = p0_start_config("config-b", digests_b.clone());
    let ev_a = evidence("approved", &digests_a);
    let ev_b = evidence("approved", &digests_b);

    let snap_a = VoxelConfigSnapshot::from_generated(&cfg_a, &ev_a).unwrap();
    let snap_b = VoxelConfigSnapshot::from_generated(&cfg_b, &ev_b).unwrap();
    assert_ne!(snap_a.config_hash(), snap_b.config_hash());
    assert!(!Arc::ptr_eq(&snap_a, &snap_b));
    assert_eq!(snap_a.baseline_id(), BASELINE_ID);
    assert_eq!(snap_a.schema_epoch(), SCHEMA_EPOCH);
    assert_eq!(snap_a.gate_source_hashes(), &digests_a);
    assert_eq!(snap_b.gate_source_hashes(), &digests_b);

    let original_a = snap_a.config_hash().to_string();
    cfg_a.config_hash = labeled_hash("mutated-after-capture");
    assert_eq!(
        snap_a.config_hash(),
        original_a,
        "mutating the input must not write back into a captured snapshot"
    );

    let debug = format!("{snap_a:?}");
    let audit = snap_a.audit_summary();
    assert!(
        !debug.contains(SECRET),
        "Debug must redact key material: {debug}"
    );
    assert!(
        !audit.contains(SECRET),
        "audit must redact key material: {audit}"
    );
    assert!(
        !debug.to_lowercase().contains("key_material"),
        "Debug must not expose a key material field: {debug}"
    );

    let ha = snap_a.config_hash().to_string();
    let hb = snap_b.config_hash().to_string();
    let ta = std::thread::spawn(move || snap_a.config_hash().to_string());
    let tb = std::thread::spawn(move || snap_b.config_hash().to_string());
    assert_eq!(ta.join().unwrap(), ha);
    assert_eq!(tb.join().unwrap(), hb);
}
