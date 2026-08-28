//! R-00104: commit linearizes one PreparedMutation through publish_once.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkDirectoryRoot, ChunkPage, ChunkPayload,
    ChunkSlot, DirtyFrontier,
};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::{
    PublicationAuthority, PublishedReadView, PublishedStateRoot,
};
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, PinRegistry, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
    to_generated_stamp,
};
use lumio_voxel_ops::mutation::{
    LookupOutcome, MutationRequest, ReceiptLedger, canonical_fingerprint, commit, prepare,
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn approved_snapshot(label: &str) -> Arc<VoxelConfigSnapshot> {
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
        config_hash: hex32(&sha256(label.as_bytes())),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
        start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        key_material: None,
    };
    VoxelConfigSnapshot::from_generated(&cfg, &ev).expect("approved P0 snapshot")
}

fn world_rev(n: u64) -> WorldRevision {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut reserved = alloc.reserve_world().unwrap();
    reserved.finalize().unwrap()
}

fn stamp_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev_n: u64,
    chunks: &[(&str, u64)],
) -> GeneratedRevisionStamp {
    let world = world_rev(world_rev_n);
    let mut pairs = Vec::new();
    for (id, rev) in chunks {
        let mut chunk_alloc = RevisionAllocator::new();
        for _ in 0..*rev {
            chunk_alloc.reserve_chunk().unwrap().abandon();
        }
        let mut c = chunk_alloc.reserve_chunk().unwrap();
        pairs.push((id.to_string(), c.finalize().unwrap()));
    }
    to_generated_stamp(world_id, context_id, generation, world, &pairs)
}

fn payload(bytes: &[u8]) -> ChunkPayload {
    ChunkPayload::from_pages([ChunkPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn directory_ready() -> ChunkDirectoryRoot {
    let mut builder = ChunkDirectoryBuilder::new();
    builder
        .insert("c:0:0:0", ChunkSlot::ready(payload(b"base-ready")))
        .expect("canonical chunk id");
    builder
        .insert("c:1:0:0", ChunkSlot::not_loaded())
        .expect("canonical chunk id");
    builder
        .insert("c:2:0:0", ChunkSlot::pending())
        .expect("canonical chunk id");
    builder
        .insert("c:3:0:0", ChunkSlot::unavailable())
        .expect("canonical chunk id");
    builder.freeze()
}

fn published_world(
    world_id: &str,
    generation: u64,
) -> (PublicationAuthority, Arc<VoxelConfigSnapshot>) {
    let snap = approved_snapshot("r00104-commit");
    let stamp = stamp_at(
        world_id,
        "ctx-1",
        generation,
        0,
        &[
            ("c:0:0:0", 0),
            ("c:1:0:0", 0),
            ("c:2:0:0", 0),
            ("c:3:0:0", 0),
        ],
    );
    let root = PublishedStateRoot::new(
        stamp,
        directory_ready(),
        DirtyFrontier::new(world_id, generation).expect("world id"),
    );
    let pins = PinRegistry::from_approved_snapshot(snap.clone(), 16, "ctx-1", generation);
    let auth = PublicationAuthority::new(world_id, "ctx-1", generation, pins, root)
        .expect("initial root matches authority");
    (auth, snap)
}

fn request(
    txn_id: &str,
    world_id: &str,
    generation: u64,
    world_revision: u64,
    extra: &[(&str, &str)],
) -> MutationRequest {
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
    for (k, v) in extra {
        fields.insert((*k).to_string(), (*v).to_string());
    }
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: world_id.to_string(),
        generation,
        fields,
    }
}

fn assert_stable_error(id: &str) {
    assert!(
        STABLE_ERROR_IDS.contains(&id),
        "error id {id} is not a generated STABLE_ERROR_IDS member"
    );
}

fn assert_consistent_cut(view: &PublishedReadView) {
    assert_eq!(view.stamp(), view.root().stamp());
    assert_eq!(view.directory(), view.root().directory());
    assert_eq!(view.dirty_frontier(), view.root().dirty_frontier());
}

fn empty_replacement(
    directory: &ChunkDirectoryRoot,
) -> lumio_voxel_domain::chunk::ChunkReplacement {
    ChunkDeltaBuilder::new(directory)
        .freeze()
        .expect("empty replacement")
}

#[test]
fn happy_path_prepare_then_commit_is_atomic_cut() {
    assert!(SCHEMA_IDS.contains(&REVISION_STAMP_SCHEMA));
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let before = auth.capture();
    let hash_before = before.root().identity();
    let dirty_before = before.dirty_frontier().clone();
    let req = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit")]);
    let expected_fp = canonical_fingerprint(&req);

    let prepared = prepare(&req, &before, &mut ledger).expect("prepare succeeds");
    assert_eq!(prepared.fingerprint(), expected_fp);
    let receipt = commit(prepared, &auth, &mut ledger).expect("commit succeeds");
    assert_eq!(receipt.txn_id, "txn-1");
    assert!(!receipt.receipt.is_empty());
    assert_eq!(receipt.evidence.txn_id, "txn-1");
    assert_eq!(receipt.evidence.old_root, hash_before);

    let after = auth.capture();
    assert_consistent_cut(&before);
    assert_consistent_cut(&after);
    assert_eq!(before.root().identity(), hash_before);
    assert_eq!(before.dirty_frontier(), &dirty_before);
    assert_eq!(before.stamp().world_revision, 0);
    assert_ne!(after.root().identity(), hash_before);
    assert_eq!(after.stamp().world_revision, 1);
    assert_ne!(before.stamp(), after.stamp());
    assert_ne!(before.directory(), after.directory());
    assert_eq!(
        after.dirty_frontier().reason("c:0:0:0").unwrap(),
        Some("mutation")
    );
    match ledger.lookup(&req).unwrap() {
        LookupOutcome::Duplicate { receipt: stored } => assert_eq!(stored, receipt.receipt),
        other => panic!("commit must finalize a receipt, got {other:?}"),
    }
}

#[test]
fn duplicate_txn_returns_original_receipt_without_second_publish() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let req = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit")]);
    let prepared = prepare(&req, &view, &mut ledger).expect("prepare");
    let first = commit(prepared, &auth, &mut ledger).expect("first commit");
    let identity = auth.capture().root().identity();
    match ledger.lookup(&req).unwrap() {
        LookupOutcome::Duplicate { receipt } => assert_eq!(receipt, first.receipt),
        other => panic!("first commit stores a receipt, got {other:?}"),
    }

    let (auth2, snap2) = published_world("world-a", 1);
    let mut ledger2 = ReceiptLedger::from_approved_snapshot(snap2, 4).unwrap();
    let view2 = auth2.capture();
    let hash2 = view2.root().identity();
    let req2 = request("txn-dup", "world-a", 1, 0, &[("c:0:0:0", "edit-2")]);
    let prepared2 = prepare(&req2, &view2, &mut ledger2).expect("second prepare");
    ledger2
        .finalize(&req2, first.receipt.clone())
        .expect("test harness may finalize via public ledger API");
    let replayed = commit(prepared2, &auth2, &mut ledger2).expect("duplicate commit");
    assert_eq!(replayed.receipt, first.receipt);
    assert_eq!(auth2.capture().root().identity(), hash2);
    assert_eq!(auth.capture().root().identity(), identity);

    let prepared_same =
        prepare(&req, &auth.capture(), &mut ledger).expect("same-fingerprint prepare after commit");
    let replayed_same = commit(prepared_same, &auth, &mut ledger).expect("same-world duplicate");
    assert_eq!(replayed_same.receipt, first.receipt);
    assert_eq!(replayed_same.txn_id, first.txn_id);
    assert_eq!(auth.capture().root().identity(), identity);
}

#[test]
fn stale_base_fails_snapshot_base_mismatch_without_swap() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let hash_before = view.root().identity();
    let req = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit")]);
    let prepared = prepare(&req, &view, &mut ledger).expect("prepare");

    let injected = PublishedStateRoot::new(
        stamp_at("world-a", "ctx-1", 1, 1, &[("c:0:0:0", 1)]),
        {
            let mut builder = ChunkDirectoryBuilder::new();
            builder
                .insert("c:0:0:0", ChunkSlot::ready(payload(b"injected")))
                .expect("canonical chunk id");
            builder.freeze()
        },
        DirtyFrontier::new("world-a", 1).expect("world id"),
    );
    let mut injected_prep = auth
        .prepare(
            world_rev(1),
            injected,
            empty_replacement(auth.capture().directory()),
        )
        .expect("inject a different published root");
    let published = auth
        .publish_once(injected_prep.seal().expect("seal injected"))
        .expect("injected publish");
    let injected_hash = published.root().identity();
    assert_ne!(injected_hash, hash_before);

    let err = commit(prepared, &auth, &mut ledger).unwrap_err();
    assert_eq!(err.error_id(), "SnapshotBaseMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(auth.capture().root().identity(), injected_hash);
    match ledger.lookup(&req).unwrap() {
        LookupOutcome::InFlight => {}
        other => panic!("stale commit must not finalize, got {other:?}"),
    }
}

#[test]
fn conflict_fingerprint_same_txn_does_not_swap() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let first_req = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit-a")]);
    let prepared = prepare(&first_req, &view, &mut ledger).expect("first prepare");
    let first = commit(prepared, &auth, &mut ledger).expect("first commit");
    let identity = auth.capture().root().identity();

    let conflict = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit-b")]);
    let err = prepare(&conflict, &auth.capture(), &mut ledger).unwrap_err();
    assert_eq!(err.error_id(), "RevisionConflict");
    assert_stable_error(err.error_id());
    assert_eq!(
        err.disposition(),
        Some(lumio_voxel_ops::mutation::ReplayDisposition::Conflict)
    );
    assert_eq!(auth.capture().root().identity(), identity);
    match ledger.lookup(&first_req).unwrap() {
        LookupOutcome::Duplicate { receipt } => assert_eq!(receipt, first.receipt),
        other => panic!("first receipt must be unchanged, got {other:?}"),
    }
}
