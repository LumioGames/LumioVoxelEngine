//! R-00096: Prepare has no visible side effects.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDirectoryBuilder, ChunkDirectoryRoot, ChunkPage, ChunkPayload, ChunkSlot, DirtyFrontier,
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
    LookupOutcome, MutationRequest, PreparedMutation, ReceiptLedger, canonical_fingerprint, prepare,
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
    let snap = approved_snapshot("r00096-prepare");
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

fn visible_cut(view: &PublishedReadView) -> ([u8; 32], DirtyFrontier) {
    (view.root().identity(), view.dirty_frontier().clone())
}

#[test]
fn failed_precondition_wrong_world_leaves_ledger_vacant_and_root_unchanged() {
    assert!(SCHEMA_IDS.contains(&REVISION_STAMP_SCHEMA));
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let (hash_before, dirty_before) = visible_cut(&view);
    let req = request("txn-1", "world-b", 1, 0, &[("c:0:0:0", "edit")]);

    let err = prepare(&req, &view, &mut ledger).unwrap_err();
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(ledger.lookup(&req).unwrap(), LookupOutcome::Vacant);

    let after = auth.capture();
    let (hash_after, dirty_after) = visible_cut(&after);
    assert_eq!(hash_after, hash_before);
    assert_eq!(dirty_after, dirty_before);
    assert_eq!(after.root().identity(), view.root().identity());
}

#[test]
fn successful_prepare_is_move_only_and_does_not_publish() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let (hash_before, dirty_before) = visible_cut(&view);
    let req = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit")]);
    let expected_fp = canonical_fingerprint(&req).expect("no duplicate member");
    let config_hash = ledger.config_hash().to_string();

    let token = prepare(&req, &view, &mut ledger).expect("prepare succeeds");
    assert_eq!(token.txn_id(), "txn-1");
    assert_eq!(token.fingerprint(), expected_fp);
    assert_eq!(token.base_identity(), hash_before);
    assert_eq!(token.generation(), 1);
    assert_eq!(token.config_hash(), config_hash);
    match ledger.lookup(&req).unwrap() {
        LookupOutcome::InFlight => {}
        other => panic!("successful prepare may reserve InFlight, got {other:?}"),
    }

    let after = auth.capture();
    assert_eq!(after.root().identity(), hash_before);
    assert_eq!(after.dirty_frontier(), &dirty_before);

    fn take_moved(token: PreparedMutation) -> String {
        token.txn_id().to_string()
    }
    assert_eq!(take_moved(token), "txn-1");
}

#[test]
fn conflict_fingerprint_leaves_first_reservation_receipt_unchanged() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let (hash_before, dirty_before) = visible_cut(&view);
    let first = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit-a")]);
    let token = prepare(&first, &view, &mut ledger).expect("first prepare");
    let receipt = b"receipt-first".to_vec();
    ledger
        .finalize(&first, receipt.clone())
        .expect("test harness may finalize via public ledger API");
    drop(token);

    let conflict = request("txn-1", "world-a", 1, 0, &[("c:0:0:0", "edit-b")]);
    let err = prepare(&conflict, &view, &mut ledger).unwrap_err();
    assert_eq!(err.error_id(), "RevisionConflict");
    assert_stable_error(err.error_id());
    assert_eq!(
        err.disposition(),
        Some(lumio_voxel_ops::mutation::ReplayDisposition::Conflict)
    );

    match ledger.lookup(&first).unwrap() {
        LookupOutcome::Duplicate { receipt: stored } => assert_eq!(stored, receipt),
        other => panic!("first receipt must be unchanged, got {other:?}"),
    }
    let after = auth.capture();
    assert_eq!(after.root().identity(), hash_before);
    assert_eq!(after.dirty_frontier(), &dirty_before);
}

#[test]
fn failed_stage_invalid_chunk_id_aborts_without_publish() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let (hash_before, dirty_before) = visible_cut(&view);
    let req = request("txn-1", "world-a", 1, 0, &[("c:01:0:0", "edit")]);

    let err = prepare(&req, &view, &mut ledger).unwrap_err();
    assert_eq!(err.error_id(), "CoordinateOutOfBounds");
    assert_stable_error(err.error_id());
    assert_eq!(ledger.lookup(&req).unwrap(), LookupOutcome::Vacant);

    let after = auth.capture();
    assert_eq!(after.root().identity(), hash_before);
    assert_eq!(after.dirty_frontier(), &dirty_before);
}
