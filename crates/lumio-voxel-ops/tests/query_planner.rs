//! R-00080: deterministic query planner and adapter-internal budget admission.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkPage, ChunkPayload, ChunkSlot, DirtyFrontier,
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
};
use lumio_voxel_ops::query::{GeneratedVoxelQueryRequest, QUERY_SCHEMA, QueryPlanner};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
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

fn approved_snapshot(label: &str, capabilities: &[&str]) -> Arc<VoxelConfigSnapshot> {
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
    let names: Vec<String> = capabilities.iter().map(|s| (*s).to_string()).collect();
    let cfg = GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: hex32(&sha256(label.as_bytes())),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(names.clone()),
        start_capabilities: names,
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

fn stamp(
    world_id: &str,
    context: &str,
    generation: u64,
    world_revision: u64,
) -> GeneratedRevisionStamp {
    GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.to_string(),
        context_id: context.to_string(),
        generation,
        world_revision,
        chunk_revision_set: BTreeMap::new(),
    }
}

fn dummy_payload(bytes: &[u8]) -> ChunkPayload {
    ChunkPayload::from_pages([ChunkPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn dummy_root(
    world_id: &str,
    context: &str,
    generation: u64,
    world_revision: u64,
    with_payload: bool,
) -> PublishedStateRoot {
    let mut builder = ChunkDirectoryBuilder::new();
    if with_payload {
        builder
            .insert("c:0:0:0", ChunkSlot::ready(dummy_payload(b"must-not-read")))
            .expect("canonical dummy id");
    }
    PublishedStateRoot::new(
        stamp(world_id, context, generation, world_revision),
        builder.freeze(),
        DirtyFrontier::new(world_id, generation).expect("world id"),
    )
}

fn authority(
    label: &str,
    world_id: &str,
    context: &str,
    generation: u64,
    initial: PublishedStateRoot,
) -> PublicationAuthority {
    let pins = PinRegistry::from_approved_snapshot(
        approved_snapshot(label, &["Native", "ReferenceVoxel"]),
        16,
        context,
        generation,
    );
    PublicationAuthority::new(world_id, context, generation, pins, initial)
        .expect("initial root matches authority")
}

fn request(chunks: &[&str], cancel: bool) -> GeneratedVoxelQueryRequest {
    GeneratedVoxelQueryRequest {
        query_id: "q-1".to_string(),
        world_id: "world-a".to_string(),
        context: "ctx-1".to_string(),
        chunk_ids: chunks.iter().map(|c| (*c).to_string()).collect(),
        cancel,
    }
}

fn assert_stable_error(id: &str) {
    assert!(
        STABLE_ERROR_IDS.contains(&id),
        "error id {id} is not a generated STABLE_ERROR_IDS member"
    );
}

fn stamp_debug_hash(view: &PublishedReadView) -> [u8; 32] {
    sha256(format!("{:?}", view.stamp()).as_bytes())
}

#[test]
fn permutation_independent_plan_hash_and_captured_identities() {
    assert!(SCHEMA_IDS.contains(&QUERY_SCHEMA));
    assert_eq!(QUERY_SCHEMA, "voxel-query");

    let snap = approved_snapshot("r00080-perm", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 8).expect("planner");
    let auth = authority(
        "r00080-perm-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, false),
    );
    let view = auth.capture();

    let a = planner
        .plan(
            &request(&["c:1:0:0", "c:0:0:0", "c:-1:0:0"], false),
            &view,
            snap.as_ref(),
        )
        .expect("plan a");
    let b = planner
        .plan(
            &request(&["c:-1:0:0", "c:1:0:0", "c:0:0:0"], false),
            &view,
            snap.as_ref(),
        )
        .expect("plan b");

    assert_eq!(a.plan_hash(), b.plan_hash());
    assert_eq!(a.read_stamp(), b.read_stamp());
    assert_eq!(a.read_stamp(), view.stamp());
    assert_eq!(a.config_hash(), b.config_hash());
    assert_eq!(a.config_hash(), snap.config_hash());
    assert_eq!(
        a.canonical_chunks(),
        &[
            "c:-1:0:0".to_string(),
            "c:0:0:0".to_string(),
            "c:1:0:0".to_string()
        ]
    );
    assert_eq!(a.canonical_chunks(), b.canonical_chunks());
    assert_eq!(a.budget(), 8);
    assert_eq!(a.cancel_token(), "q-1");
}

#[test]
fn over_max_chunks_is_budget_exceeded_without_touching_dummy_root() {
    let snap = approved_snapshot("r00080-budget", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 1).expect("planner");
    let auth = authority(
        "r00080-budget-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, true),
    );
    let view = auth.capture();
    let stamp_before = stamp_debug_hash(&view);
    let root_before = view.root().identity();
    let dir_before = sha256(format!("{:?}", view.directory()).as_bytes());

    let err = planner
        .plan(
            &request(&["c:0:0:0", "c:1:0:0"], false),
            &view,
            snap.as_ref(),
        )
        .expect_err("over max_chunks");
    assert_eq!(err.error_id(), "BudgetExceeded");
    assert_stable_error(err.error_id());

    assert_eq!(stamp_debug_hash(&view), stamp_before);
    assert_eq!(view.root().identity(), root_before);
    assert_eq!(
        sha256(format!("{:?}", view.directory()).as_bytes()),
        dir_before
    );
}

#[test]
fn plan_binds_stamp_identity_independent_of_later_root() {
    let snap_a = approved_snapshot("r00080-bind-a", &["Native", "ReferenceVoxel"]);
    let snap_b = approved_snapshot("r00080-bind-b", &["Native", "ReferenceVoxel"]);
    assert_ne!(snap_a.config_hash(), snap_b.config_hash());

    let planner = QueryPlanner::from_approved_snapshot(snap_a.clone(), 4).expect("planner");
    let initial = dummy_root("world-a", "ctx-1", 1, 0, false);
    let auth = authority("r00080-bind-view", "world-a", "ctx-1", 1, initial);
    let view = auth.capture();
    let plan = planner
        .plan(&request(&["c:0:0:0"], false), &view, snap_a.as_ref())
        .expect("first plan");
    let captured = plan.read_stamp().clone();
    let captured_hash = plan.config_hash().to_string();
    let captured_plan_hash = plan.plan_hash();

    let later = dummy_root("world-a", "ctx-1", 1, 1, false);
    assert_ne!(later.stamp(), plan.read_stamp());

    let mut prepared = auth
        .prepare(
            world_rev(1),
            later,
            ChunkDeltaBuilder::new(view.directory())
                .freeze()
                .expect("empty replacement"),
        )
        .expect("prepare later root");
    let published = auth
        .publish_once(prepared.seal().expect("seal"))
        .expect("publish later root");
    assert_ne!(published.stamp(), &captured);
    assert_eq!(plan.read_stamp(), &captured);
    assert_eq!(plan.read_stamp(), view.stamp());
    assert_eq!(plan.config_hash(), captured_hash);
    assert_eq!(plan.plan_hash(), captured_plan_hash);

    let later_plan = planner
        .plan(&request(&["c:0:0:0"], false), &published, snap_b.as_ref())
        .expect("later plan uses new stamp/config");
    assert_ne!(later_plan.read_stamp(), plan.read_stamp());
    assert_ne!(later_plan.config_hash(), plan.config_hash());
    assert_eq!(plan.read_stamp(), &captured);
    assert_eq!(plan.config_hash(), captured_hash);
}

#[test]
fn query_sources_contain_no_fs_io() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/query");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("query module sources")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "expected query/*.rs exclusive sources");
    for path in files {
        let text = fs::read_to_string(&path).expect("read query source");
        assert!(
            !text.contains("std::fs"),
            "{} must not use std::fs",
            path.display()
        );
        assert!(
            !text.contains("lumio_voxel_test_support"),
            "{} must not depend on test-support",
            path.display()
        );
    }
}

#[test]
fn cancel_before_plan_is_generated_error_without_io() {
    let snap = approved_snapshot("r00080-cancel", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 4).expect("planner");
    let auth = authority(
        "r00080-cancel-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, true),
    );
    let view = auth.capture();
    let stamp_before = stamp_debug_hash(&view);
    let err = planner
        .plan(&request(&["c:0:0:0"], true), &view, snap.as_ref())
        .expect_err("cancel-before-plan");
    assert_eq!(err.error_id(), "InvalidHandle");
    assert_stable_error(err.error_id());
    assert_eq!(stamp_debug_hash(&view), stamp_before);
}

#[test]
fn illegal_chunk_id_is_coordinate_out_of_bounds() {
    let snap = approved_snapshot("r00080-coord", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 8).expect("planner");
    let auth = authority(
        "r00080-coord-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, false),
    );
    let view = auth.capture();
    for bad in [
        "0:0:0",
        "c:0:0",
        "c:0:0:0:1",
        "c:01:0:0",
        "c:+1:0:0",
        "c:-0:0:0",
        "c:x:y:z",
        "C:0:0:0",
        "",
    ] {
        let err = planner
            .plan(&request(&[bad], false), &view, snap.as_ref())
            .expect_err(bad);
        assert_eq!(err.error_id(), "CoordinateOutOfBounds", "id {bad}");
        assert_stable_error(err.error_id());
    }
}

#[test]
fn disabled_capability_is_claim_not_granted() {
    let snap = approved_snapshot("r00080-cap", &[]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 4).expect("planner");
    let auth = authority(
        "r00080-cap-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, false),
    );
    let view = auth.capture();
    let err = planner
        .plan(&request(&["c:0:0:0"], false), &view, snap.as_ref())
        .expect_err("disabled capability");
    assert_eq!(err.error_id(), "ClaimNotGranted");
    assert_stable_error(err.error_id());
}
