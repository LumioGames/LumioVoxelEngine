//! R-00081: single-cut query execute; no mixed stamp, no implicit Load.

use lumio_voxel_contracts::{
    BASELINE_ID, CHUNK_PRESENCE, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256,
};
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
use lumio_voxel_ops::query::{
    ChunkAccessResult, GeneratedVoxelQueryOutcome, GeneratedVoxelQueryRequest, QUERY_SCHEMA,
    QueryEvidence, QueryExecutor, QueryPlanner,
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
    payload_bytes: &[u8],
) -> PublishedStateRoot {
    let mut builder = ChunkDirectoryBuilder::new();
    builder
        .insert("c:0:0:0", ChunkSlot::ready(dummy_payload(payload_bytes)))
        .expect("canonical dummy id");
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

fn directory_hash(view: &PublishedReadView) -> [u8; 32] {
    sha256(format!("{:?}", view.directory()).as_bytes())
}

fn stamp_identity(stamp: &GeneratedRevisionStamp) -> (&str, &str, u64, u64) {
    (
        stamp.world_id.as_str(),
        stamp.context_id.as_str(),
        stamp.generation,
        stamp.world_revision,
    )
}

#[test]
fn concurrent_commit_execute_keeps_old_cut_and_rejects_new_view() {
    assert!(SCHEMA_IDS.contains(&QUERY_SCHEMA));
    let snap = approved_snapshot("r00081-cut", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 4).expect("planner");
    let auth = authority(
        "r00081-cut-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, b"cut-a"),
    );
    let view_a = auth.capture();
    let plan = planner
        .plan(&request(&["c:0:0:0"], false), &view_a, snap.as_ref())
        .expect("plan against A");
    let captured = plan.read_stamp().clone();
    let dir_a = directory_hash(&view_a);

    let later = dummy_root("world-a", "ctx-1", 1, 1, b"cut-b");
    assert_ne!(later.stamp(), plan.read_stamp());
    let mut prepared = auth
        .prepare(
            world_rev(1),
            later,
            ChunkDeltaBuilder::new(view_a.directory())
                .freeze()
                .expect("empty replacement"),
        )
        .expect("prepare later root");
    let view_b = auth
        .publish_once(prepared.seal().expect("seal"))
        .expect("publish later root");
    assert_ne!(view_b.stamp(), &captured);
    assert_eq!(plan.read_stamp(), &captured);
    assert_eq!(plan.read_stamp(), view_a.stamp());

    let outcome: GeneratedVoxelQueryOutcome =
        QueryExecutor::execute(&plan, &view_a).expect("cut A");
    let evidence: &QueryEvidence = outcome.evidence();
    assert_eq!(evidence.read_stamp(), &captured);
    assert_eq!(evidence.read_stamp(), view_a.stamp());
    assert_ne!(evidence.read_stamp(), view_b.stamp());
    assert_eq!(
        stamp_identity(evidence.read_stamp()),
        stamp_identity(&captured)
    );
    assert_eq!(evidence.plan_hash(), plan.plan_hash());
    assert_eq!(directory_hash(&view_a), dir_a);
    assert_eq!(outcome.items().len(), 1);
    assert_eq!(outcome.items()[0].chunk_id(), "c:0:0:0");
    assert_eq!(outcome.items()[0].presence(), "Ready");

    let err = QueryExecutor::execute(&plan, &view_b).expect_err("new view stamp mismatch");
    assert_eq!(err.error_id(), "InvalidHandle");
    assert_stable_error(err.error_id());
    assert_eq!(directory_hash(&view_a), dir_a);
    assert_eq!(view_a.stamp(), &captured);
}

#[test]
fn budget_exhaust_second_walk_is_budget_exceeded() {
    let snap = approved_snapshot("r00081-budget", &["Native", "ReferenceVoxel"]);
    let n = 2;
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), n).expect("planner");
    let auth = authority(
        "r00081-budget-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, b"must-not-read"),
    );
    let view = auth.capture();
    let dir_before = directory_hash(&view);
    let stamp_before = stamp_debug_hash(&view);
    let root_before = view.root().identity();
    let plan = planner
        .plan(
            &request(&["c:0:0:0", "c:1:0:0"], false),
            &view,
            snap.as_ref(),
        )
        .expect("plan of N");
    assert_eq!(plan.budget(), n);
    assert_eq!(plan.canonical_chunks().len(), n);

    let outcome = QueryExecutor::execute(&plan, &view).expect("first walk within budget");
    assert_eq!(outcome.evidence().budget_used(), n);
    assert_eq!(outcome.items().len(), n);
    assert_eq!(directory_hash(&view), dir_before);
    assert_eq!(stamp_debug_hash(&view), stamp_before);

    let err = QueryExecutor::walk(&plan, &view, outcome.evidence().budget_used())
        .expect_err("second walk exceeds budget");
    assert_eq!(err.error_id(), "BudgetExceeded");
    assert_stable_error(err.error_id());
    assert_eq!(directory_hash(&view), dir_before);
    assert_eq!(stamp_debug_hash(&view), stamp_before);
    assert_eq!(view.root().identity(), root_before);
}

#[test]
fn cancel_or_invalid_handle_leaves_capture_unchanged() {
    let snap = approved_snapshot("r00081-cancel", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 4).expect("planner");
    let auth = authority(
        "r00081-cancel-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, b"must-not-read"),
    );
    let view = auth.capture();
    let stamp_before = stamp_debug_hash(&view);
    let dir_before = directory_hash(&view);
    let root_before = view.root().identity();
    let plan = planner
        .plan(&request(&["c:0:0:0"], false), &view, snap.as_ref())
        .expect("plan");

    let cancelled = QueryExecutor::execute_cancelled(&plan, &view).expect_err("cancelled");
    assert_eq!(cancelled.error_id(), "LoaderCancelled");
    assert_stable_error(cancelled.error_id());

    let other = authority(
        "r00081-cancel-other",
        "world-b",
        "ctx-1",
        1,
        dummy_root("world-b", "ctx-1", 1, 0, b"other"),
    );
    let view_other = other.capture();
    let invalid = QueryExecutor::execute(&plan, &view_other).expect_err("foreign view");
    assert_eq!(invalid.error_id(), "InvalidHandle");
    assert_stable_error(invalid.error_id());

    assert_eq!(stamp_debug_hash(&view), stamp_before);
    assert_eq!(directory_hash(&view), dir_before);
    assert_eq!(view.root().identity(), root_before);
}

#[test]
fn outcome_items_expose_presence_and_schema_id_only() {
    let snap = approved_snapshot("r00081-payload", &["Native", "ReferenceVoxel"]);
    let planner = QueryPlanner::from_approved_snapshot(snap.clone(), 4).expect("planner");
    let auth = authority(
        "r00081-payload-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, b"must-not-read"),
    );
    let view = auth.capture();
    let plan = planner
        .plan(
            &request(&["c:0:0:0", "c:1:0:0"], false),
            &view,
            snap.as_ref(),
        )
        .expect("plan");
    let outcome = QueryExecutor::execute(&plan, &view).expect("execute");
    assert_eq!(outcome.items().len(), 2);

    let ready: &ChunkAccessResult = &outcome.items()[0];
    assert_eq!(ready.chunk_id(), "c:0:0:0");
    assert_eq!(ready.presence(), "Ready");
    assert!(CHUNK_PRESENCE.contains(&ready.presence()));
    let schema = ready.schema_id().expect("Ready carries schema_id");
    assert!(SCHEMA_IDS.contains(&schema));
    let debug = format!("{ready:?}");
    assert!(!debug.contains("ChunkPayload"));
    assert!(!debug.contains("ChunkPage"));
    assert!(!debug.contains("must-not-read"));
    assert!(!debug.contains("Arc"));

    let missing: &ChunkAccessResult = &outcome.items()[1];
    assert_eq!(missing.chunk_id(), "c:1:0:0");
    assert_eq!(missing.presence(), "NotLoaded");
    assert!(CHUNK_PRESENCE.contains(&missing.presence()));
    assert_eq!(missing.schema_id(), None);
    let missing_debug = format!("{missing:?}");
    assert!(!missing_debug.contains("ChunkPayload"));
    assert!(!missing_debug.contains("0x"));
}
