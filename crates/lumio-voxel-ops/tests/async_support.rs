//! R-00068: OriginToken, bounded port from approved snapshot, completion dispositions.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_ops::async_support::{
    APPLY_PHASES, BoundedJobPort, CompletionDisposition, OriginEnvelope, OriginToken,
    full_load_action, validate_completion,
};
use std::collections::BTreeMap;

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn approved_snapshot() -> std::sync::Arc<VoxelConfigSnapshot> {
    let source = GateSourceHashes {
        architecture_baseline_id: BASELINE_ID.to_string(),
        voxel_head: "b2f0d8a3763a02f805e29cbd101560ba7fdca77b".to_string(),
        architecture_mirror_sha256:
            "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0".to_string(),
        v13_decision_gates_sha256:
            "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2".to_string(),
        blueprint_sha256: "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa"
            .to_string(),
    };
    let digests: BTreeMap<String, String> = P0_DECISION_GATES
        .iter()
        .map(|g| {
            (
                (*g).to_string(),
                hex32(&sha256(format!("approved-{g}").as_bytes())),
            )
        })
        .collect();
    let ev: Vec<DecisionEvidence> = P0_DECISION_GATES
        .iter()
        .map(|g| DecisionEvidence {
            gate_id: (*g).to_string(),
            approval_status: "approved".to_string(),
            source_hashes: source.clone(),
            evidence_digest: digests[*g].clone(),
        })
        .collect();
    let cfg = GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: hex32(&sha256(b"r00068-approved")),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
        start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        key_material: None,
    };
    VoxelConfigSnapshot::from_generated(&cfg, &ev).expect("approved P0 snapshot")
}

fn origin(
    ctx: &str,
    generation: u64,
    req: &str,
    world_rev: u64,
    phase: &'static str,
) -> OriginToken {
    OriginToken::try_new(ctx, generation, req, world_rev, BTreeMap::new(), phase).unwrap()
}

#[test]
fn origin_requires_generated_phase_and_nonempty_fields() {
    assert!(APPLY_PHASES.contains(&"VoxelCommit"));
    assert!(SCHEMA_IDS.contains(&"voxel-revision-stamp"));
    assert!(OriginToken::try_new("", 1, "r", 0, BTreeMap::new(), "VoxelCommit").is_err());
    assert!(OriginToken::try_new("w", 1, "", 0, BTreeMap::new(), "VoxelCommit").is_err());
    assert!(OriginToken::try_new("w", 1, "r", 0, BTreeMap::new(), "NotAPhase").is_err());
    let ok = OriginToken::try_new("w", 1, "r", 0, BTreeMap::new(), "VoxelCommit").unwrap();
    assert_eq!(ok.apply_phase(), "VoxelCommit");
}

#[test]
fn port_from_approved_snapshot_is_bounded_queue_full() {
    let snap = approved_snapshot();
    let mut port = BoundedJobPort::from_approved_snapshot(snap.clone(), 1).unwrap();
    assert_eq!(full_load_action(), "QueueFull");
    assert!(STABLE_ERROR_IDS.contains(&full_load_action()));
    let job = OriginEnvelope {
        origin: origin("world-a", 1, "req-1", 0, "VoxelCommit"),
        config_hash: snap.config_hash().to_string(),
        payload: 1u8,
    };
    port.try_submit(job).unwrap();
    let full = OriginEnvelope {
        origin: origin("world-a", 1, "req-2", 0, "VoxelCommit"),
        config_hash: snap.config_hash().to_string(),
        payload: 2u8,
    };
    let err = port.try_submit(full).unwrap_err();
    assert_eq!(err.error_id(), "QueueFull");
}

#[test]
fn dual_world_ports_do_not_share_queue_state() {
    let snap = approved_snapshot();
    let mut a = BoundedJobPort::from_approved_snapshot(snap.clone(), 1).unwrap();
    let mut b: BoundedJobPort<()> =
        BoundedJobPort::from_approved_snapshot(snap.clone(), 1).unwrap();
    let job = OriginEnvelope {
        origin: origin("world-a", 1, "req-a", 0, "VoxelCommit"),
        config_hash: snap.config_hash().to_string(),
        payload: (),
    };
    a.try_submit(job).unwrap();
    assert!(a.pop().is_some());
    assert!(b.pop().is_none());
}

#[test]
fn completion_dispositions_are_exclusive() {
    let basis = origin("world-a", 2, "basis", 5, "VoxelCommit");
    let ok = origin("world-a", 2, "job-1", 5, "VoxelCommit");
    assert_eq!(
        validate_completion(&basis, &ok),
        CompletionDisposition::Accept
    );
    let dup = origin("world-a", 2, "basis", 5, "VoxelCommit");
    assert_eq!(
        validate_completion(&basis, &dup),
        CompletionDisposition::Duplicate
    );
    let stale = origin("world-a", 1, "job-2", 5, "VoxelCommit");
    assert_eq!(
        validate_completion(&basis, &stale),
        CompletionDisposition::Stale
    );
    let late = origin("world-a", 2, "job-3", 4, "VoxelCommit");
    assert_eq!(
        validate_completion(&basis, &late),
        CompletionDisposition::Late
    );
    let wrong = origin("world-b", 2, "job-4", 5, "VoxelCommit");
    assert_eq!(
        validate_completion(&basis, &wrong),
        CompletionDisposition::WrongWorld
    );
}
