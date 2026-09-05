//! R-00104: commit linearizes one PreparedMutation through publish_once.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256};
use lumio_voxel_domain::block::{BlockId, CellOffset};
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
use lumio_voxel_domain::section::{
    DirtyFrontier, SectionDeltaBuilder, SectionDirectoryBuilder, SectionDirectoryRoot,
    SectionPayload, SectionSlot, SectionStorage,
};
use lumio_voxel_ops::canonical::CanonicalObject;
use lumio_voxel_ops::mutation::{
    LookupOutcome, MutationEntry, MutationRequest, ReceiptLedger, canonical_fingerprint, commit,
    prepare,
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
    sections: &[(&str, u64)],
) -> GeneratedRevisionStamp {
    let world = world_rev(world_rev_n);
    let mut pairs = Vec::new();
    for (id, rev) in sections {
        let mut section_alloc = RevisionAllocator::new();
        for _ in 0..*rev {
            section_alloc.reserve_section().unwrap().abandon();
        }
        let mut c = section_alloc.reserve_section().unwrap();
        pairs.push((id.to_string(), c.finalize().unwrap()));
    }
    to_generated_stamp(world_id, context_id, generation, world, &pairs)
}

fn payload(bytes: &[u8]) -> SectionPayload {
    let digest = sha256(bytes);
    SectionPayload::from_storage(SectionStorage::uniform(BlockId::from_raw(
        u32::from_le_bytes(digest[..4].try_into().unwrap()),
    )))
    .expect("valid dense uncompressed page")
}

fn directory_ready() -> SectionDirectoryRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", SectionSlot::ready(payload(b"base-ready")))
        .expect("canonical section id");
    builder
        .insert("s:1:0:0", SectionSlot::unchanged())
        .expect("canonical section id");
    builder
        .insert("s:2:0:0", SectionSlot::pending())
        .expect("canonical section id");
    builder
        .insert("s:3:0:0", SectionSlot::unavailable())
        .expect("canonical section id");
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
            ("s:0:0:0", 0),
            ("s:1:0:0", 0),
            ("s:2:0:0", 0),
            ("s:3:0:0", 0),
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
    let entries = extra
        .iter()
        .map(|(key, value)| {
            let (section, cell) = key.split_once('/').unwrap_or((key, "0"));
            MutationEntry::new(
                section,
                CellOffset::new(cell.parse().unwrap_or(0)).unwrap(),
                BlockId::from_raw(value.parse().unwrap_or_else(|_| hash_value(value))),
                world_revision,
            )
        })
        .collect();
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: world_id.to_string(),
        generation,
        entries,
    }
}

fn hash_value(value: &str) -> u32 {
    let digest = sha256(value.as_bytes());
    u32::from_le_bytes(digest[..4].try_into().unwrap())
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn assert_consistent_cut(view: &PublishedReadView) {
    assert_eq!(view.stamp(), view.root().stamp());
    assert_eq!(view.directory(), view.root().directory());
    assert_eq!(view.dirty_frontier(), view.root().dirty_frontier());
}

fn empty_replacement(
    directory: &SectionDirectoryRoot,
) -> lumio_voxel_domain::section::SectionReplacement {
    SectionDeltaBuilder::new(directory)
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
    let req = request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let expected_fp = canonical_fingerprint(&req).expect("no duplicate member");

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
        after.dirty_frontier().reason("s:0:0:0").unwrap(),
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
    let req = request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
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
    let req2 = request("txn-dup", "world-a", 1, 0, &[("s:0:0:0", "edit-2")]);
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
fn completed_opaque_legacy_receipt_replays_without_decoding() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let req = request("txn-opaque", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let opaque = b"legacy-receipt-without-canonical-fields".to_vec();

    let prepared = prepare(&req, &auth.capture(), &mut ledger).expect("prepare");
    ledger
        .finalize(&req, opaque.clone())
        .expect("legacy storage may finalize through the public API");

    let replay = commit(
        prepare(&req, &auth.capture(), &mut ledger).expect("duplicate prepare"),
        &auth,
        &mut ledger,
    )
    .expect("opaque duplicate replay");
    drop(prepared);
    assert_eq!(replay.receipt, opaque);
    assert_eq!(replay.txn_id, req.txn_id);
    assert_eq!(replay.evidence.txn_id, req.txn_id);
    assert_eq!(replay.evidence.old_root, auth.capture().root().identity());
    assert_eq!(replay.evidence.new_root, auth.capture().root().identity());
}

#[test]
fn canonical_receipt_fields_cannot_override_prepared_identity() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let req = request("txn-canonical", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let base_identity = auth.capture().root().identity();
    let mut forged = CanonicalObject::new();
    forged.insert_text("txn_id", "forged-txn").unwrap();
    forged
        .insert_text(
            "fingerprint",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .unwrap();
    forged
        .insert_text(
            "old_root",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .unwrap();
    forged
        .insert_text(
            "new_root",
            "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        )
        .unwrap();
    let forged = forged.encode_bytes();

    prepare(&req, &auth.capture(), &mut ledger).expect("prepare");
    ledger
        .finalize(&req, forged.clone())
        .expect("test harness may finalize via the public API");
    let replay = commit(
        prepare(&req, &auth.capture(), &mut ledger).expect("duplicate prepare"),
        &auth,
        &mut ledger,
    )
    .expect("canonical duplicate replay");

    assert_eq!(replay.receipt, forged);
    assert_eq!(replay.txn_id, req.txn_id);
    assert_eq!(replay.evidence.txn_id, req.txn_id);
    assert_eq!(replay.evidence.old_root, base_identity);
    assert_eq!(replay.evidence.new_root, base_identity);
}

#[test]
fn duplicate_replay_after_section_unload_reuses_original_receipt_evidence() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let req = request("txn-unloaded", "world-a", 1, 0, &[("s:0:0:0", "edit")]);

    let first = commit(
        prepare(&req, &auth.capture(), &mut ledger).expect("first prepare"),
        &auth,
        &mut ledger,
    )
    .expect("first commit");
    let original_evidence = first.evidence.clone();

    let current = auth.capture();
    let mut unloaded_directory = SectionDirectoryBuilder::new();
    unloaded_directory
        .insert("s:0:0:0", SectionSlot::unchanged())
        .expect("canonical section id");
    let unloaded_root = PublishedStateRoot::new(
        stamp_at("world-a", "ctx-1", 1, 2, &[("s:0:0:0", 1)]),
        unloaded_directory.freeze(),
        current.dirty_frontier().clone(),
    );
    let mut unloaded = auth
        .prepare(
            world_rev(2),
            unloaded_root,
            empty_replacement(current.directory()),
        )
        .expect("prepare unloaded root");
    auth.publish_once(unloaded.seal().expect("seal unloaded root"))
        .expect("publish unloaded root");
    let unloaded_identity = auth.capture().root().identity();
    assert_eq!(
        auth.capture()
            .directory()
            .lookup("s:0:0:0")
            .unwrap()
            .unwrap()
            .presence(),
        "Unchanged"
    );

    let replay = commit(
        prepare(&req, &auth.capture(), &mut ledger)
            .expect("duplicate prepare must not read storage"),
        &auth,
        &mut ledger,
    )
    .expect("duplicate commit");
    assert_eq!(replay.receipt, first.receipt);
    assert_eq!(replay.evidence, original_evidence);
    assert_eq!(auth.capture().root().identity(), unloaded_identity);
}

#[test]
fn stale_base_fails_snapshot_base_mismatch_without_swap() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view = auth.capture();
    let hash_before = view.root().identity();
    let req = request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let prepared = prepare(&req, &view, &mut ledger).expect("prepare");

    let injected = PublishedStateRoot::new(
        stamp_at("world-a", "ctx-1", 1, 1, &[("s:0:0:0", 1)]),
        {
            let mut builder = SectionDirectoryBuilder::new();
            builder
                .insert("s:0:0:0", SectionSlot::ready(payload(b"injected")))
                .expect("canonical section id");
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
    let first_req = request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit-a")]);
    let prepared = prepare(&first_req, &view, &mut ledger).expect("first prepare");
    let first = commit(prepared, &auth, &mut ledger).expect("first commit");
    let identity = auth.capture().root().identity();

    let conflict = request("txn-1", "world-a", 1, 0, &[("s:0:0:0", "edit-b")]);
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

#[test]
fn replaying_the_same_transaction_one_hundred_times_is_stable() {
    let (auth, snap) = published_world("world-a", 1);
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let request = request("txn-replay-100", "world-a", 1, 0, &[("s:0:0:0", "edit")]);
    let first = commit(
        prepare(&request, &auth.capture(), &mut ledger).unwrap(),
        &auth,
        &mut ledger,
    )
    .unwrap();
    let identity = auth.capture().root().identity();
    for _ in 0..100 {
        let replay = commit(
            prepare(&request, &auth.capture(), &mut ledger).unwrap(),
            &auth,
            &mut ledger,
        )
        .unwrap();
        assert_eq!(replay.receipt, first.receipt);
        assert_eq!(auth.capture().root().identity(), identity);
    }
}
