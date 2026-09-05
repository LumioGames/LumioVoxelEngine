//! R-00104: a PreparedMutation batch is one visible cut, or no swap.

use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, is_stable_error_id, sha256};
use lumio_voxel_domain::block::{BlockId, CellOffset};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::{
    PublicationAuthority, PublishedReadView, PublishedStateRoot,
};
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, PinRegistry, RevisionAllocator, WorldRevision, to_generated_stamp,
};
use lumio_voxel_domain::section::{
    DirtyFrontier, SectionDirectoryBuilder, SectionDirectoryRoot, SectionPayload, SectionSlot,
    SectionStorage,
};
use lumio_voxel_ops::mutation::{
    LookupOutcome, MAX_WRITE_BATCH_ENTRIES, MutationEntry, MutationRequest, ReceiptLedger, commit,
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

fn directory_two_ready() -> SectionDirectoryRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", SectionSlot::ready(payload(b"base-a")))
        .expect("canonical section id");
    builder
        .insert("s:4:0:0", SectionSlot::ready(payload(b"base-b")))
        .expect("canonical section id");
    builder.freeze()
}

fn published_world(
    world_id: &str,
    generation: u64,
    label: &str,
) -> (PublicationAuthority, Arc<VoxelConfigSnapshot>) {
    let snap = approved_snapshot(label);
    let stamp = stamp_at(
        world_id,
        "ctx-1",
        generation,
        0,
        &[("s:0:0:0", 0), ("s:4:0:0", 0)],
    );
    let root = PublishedStateRoot::new(
        stamp,
        directory_two_ready(),
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

fn presence(view: &PublishedReadView, section_id: &str) -> &'static str {
    view.directory()
        .lookup(section_id)
        .expect("canonical id")
        .expect("slot present")
        .presence()
}

#[test]
fn batch_sections_visible_together_old_capture_unchanged() {
    let (auth, snap) = published_world("world-a", 1, "r00104-batch");
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let before = auth.capture();
    let hash_before = before.root().identity();
    let req = request(
        "txn-batch",
        "world-a",
        1,
        0,
        &[("s:0:0:0", "edit-a"), ("s:4:0:0", "edit-b")],
    );

    let prepared = prepare(&req, &before, &mut ledger).expect("prepare batch");
    let _receipt = commit(prepared, &auth, &mut ledger).expect("commit batch");

    let after = auth.capture();
    assert_consistent_cut(&before);
    assert_consistent_cut(&after);
    assert_eq!(before.root().identity(), hash_before);
    assert_ne!(after.root().identity(), hash_before);
    assert_eq!(presence(&after, "s:0:0:0"), "Ready");
    assert_eq!(presence(&after, "s:4:0:0"), "Ready");
    assert_ne!(
        format!("{:?}", before.directory().lookup("s:0:0:0")),
        format!("{:?}", after.directory().lookup("s:0:0:0"))
    );
    assert_ne!(
        format!("{:?}", before.directory().lookup("s:4:0:0")),
        format!("{:?}", after.directory().lookup("s:4:0:0"))
    );
    assert_eq!(after.stamp().world_revision, 1);
    assert_eq!(before.stamp().world_revision, 0);
    assert_eq!(
        after.dirty_frontier().reason("s:0:0:0").unwrap(),
        Some("mutation")
    );
    assert_eq!(
        after.dirty_frontier().reason("s:4:0:0").unwrap(),
        Some("mutation")
    );
}

#[test]
fn failure_before_swap_leaves_ledger_dirty_and_root_unchanged() {
    let (auth_a, snap) = published_world("world-a", 1, "r00104-fail-a");
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let view_a = auth_a.capture();
    let hash_a = view_a.root().identity();
    let dirty_a = view_a.dirty_frontier().clone();
    let req = request(
        "txn-fail",
        "world-a",
        1,
        0,
        &[("s:0:0:0", "edit-a"), ("s:4:0:0", "edit-b")],
    );
    let prepared_world = prepare(&req, &view_a, &mut ledger).expect("prepare against world-a");

    let (auth_b, _) = published_world("world-b", 1, "r00104-fail-b");
    let hash_b = auth_b.capture().root().identity();
    let err = commit(prepared_world, &auth_b, &mut ledger).unwrap_err();
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(auth_a.capture().root().identity(), hash_a);
    assert_eq!(auth_a.capture().dirty_frontier(), &dirty_a);
    assert_eq!(auth_b.capture().root().identity(), hash_b);
    match ledger.lookup(&req).unwrap() {
        LookupOutcome::InFlight => {}
        other => panic!("failed commit must not finalize, got {other:?}"),
    }

    let (auth_stale, _) = published_world("world-a", 2, "r00104-fail-stale");
    let (auth_gen1, snap_gen1) = published_world("world-a", 1, "r00104-fail-stale-src");
    let mut ledger_gen1 = ReceiptLedger::from_approved_snapshot(snap_gen1, 4).unwrap();
    let view_gen1 = auth_gen1.capture();
    let hash_src = view_gen1.root().identity();
    let dirty_src = view_gen1.dirty_frontier().clone();
    let hash_stale = auth_stale.capture().root().identity();
    let req_stale = request(
        "txn-stale",
        "world-a",
        1,
        0,
        &[("s:0:0:0", "edit-a"), ("s:4:0:0", "edit-b")],
    );
    let prepared = prepare(&req_stale, &view_gen1, &mut ledger_gen1).expect("prepare gen 1");
    let err = commit(prepared, &auth_stale, &mut ledger_gen1).unwrap_err();
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
    assert_eq!(auth_gen1.capture().root().identity(), hash_src);
    assert_eq!(auth_gen1.capture().dirty_frontier(), &dirty_src);
    assert_eq!(auth_stale.capture().root().identity(), hash_stale);
    match ledger_gen1.lookup(&req_stale).unwrap() {
        LookupOutcome::InFlight => {}
        other => panic!("stale commit must not finalize, got {other:?}"),
    }
}

#[test]
fn stale_section_revision_rejects_the_entire_batch_before_reservation() {
    let (auth, snap) = published_world("world-a", 1, "r00438-stale-section");
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let before = auth.capture();
    let identity = before.root().identity();
    let request = MutationRequest {
        txn_id: "txn-stale-section".into(),
        world_id: "world-a".into(),
        generation: 1,
        entries: vec![
            MutationEntry::new(
                "s:0:0:0",
                CellOffset::new(0).unwrap(),
                BlockId::from_raw(7),
                1,
            ),
            MutationEntry::new(
                "s:4:0:0",
                CellOffset::new(1).unwrap(),
                BlockId::from_raw(8),
                0,
            ),
        ],
    };
    let error = prepare(&request, &before, &mut ledger).unwrap_err();
    assert_eq!(error.error_id(), "stale_section_revision");
    assert_eq!(auth.capture().root().identity(), identity);
    assert!(matches!(
        ledger.lookup(&request).unwrap(),
        LookupOutcome::Vacant
    ));
}

#[test]
fn write_batch_limit_rejects_65537_entries_without_side_effects() {
    let (auth, snap) = published_world("world-a", 1, "r00438-limit");
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let before = auth.capture();
    let identity = before.root().identity();
    let entries = (0..=vw::MAX_ENTRIES_PER_WRITE_BATCH)
        .map(|index| {
            MutationEntry::new(
                "s:0:0:0",
                CellOffset::new((index % 4096) as u16).unwrap(),
                BlockId::from_raw(index),
                0,
            )
        })
        .collect();
    let request = MutationRequest {
        txn_id: "txn-too-large".into(),
        world_id: "world-a".into(),
        generation: 1,
        entries,
    };
    let error = prepare(&request, &before, &mut ledger).unwrap_err();
    assert_eq!(error.error_id(), "write_batch_too_large");
    assert_eq!(auth.capture().root().identity(), identity);
    assert!(matches!(
        ledger.lookup(&request).unwrap(),
        LookupOutcome::Vacant
    ));
}

#[test]
fn write_batch_limit_tracks_contract_constant() {
    assert_eq!(
        MAX_WRITE_BATCH_ENTRIES,
        vw::MAX_ENTRIES_PER_WRITE_BATCH as usize
    );
}

#[test]
fn duplicate_cell_writes_are_ordered_last_write_wins() {
    let (auth, snap) = published_world("world-a", 1, "r00438-order");
    let mut ledger = ReceiptLedger::from_approved_snapshot(snap, 4).unwrap();
    let before = auth.capture();
    let request = MutationRequest {
        txn_id: "txn-order".into(),
        world_id: "world-a".into(),
        generation: 1,
        entries: vec![
            MutationEntry::new(
                "s:0:0:0",
                CellOffset::new(7).unwrap(),
                BlockId::from_raw(11),
                0,
            ),
            MutationEntry::new(
                "s:0:0:0",
                CellOffset::new(7).unwrap(),
                BlockId::from_raw(22),
                0,
            ),
        ],
    };
    let prepared = prepare(&request, &before, &mut ledger).unwrap();
    let first = commit(prepared, &auth, &mut ledger).unwrap();
    let identity = auth.capture().root().identity();
    let replay = prepare(&request, &auth.capture(), &mut ledger).unwrap();
    let second = commit(replay, &auth, &mut ledger).unwrap();
    assert_eq!(first.receipt, second.receipt);
    assert_eq!(auth.capture().root().identity(), identity);
}
