//! R-00137: DurabilityAck is the only Dirty-clear path (coverage-checked root swap).

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkPage, ChunkPayload, ChunkReplacement, ChunkSlot,
    CoveredChunkAck, DirtyFrontier, DurabilityAckContext,
};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
};
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_world::world::{
    AckEvidence, AdmittedCommand, BarrierScope, VoxelWorld, WorldCommand, WorldConfigAdapter,
    WorldDescriptor, WorldError, WorldWriteLane, apply_durability_ack,
};
use std::collections::BTreeMap;
use std::path::Path;
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

fn origin_of(world: &VoxelWorld, request_id: &str) -> OriginToken {
    let guard = world.generation_guard();
    OriginToken::try_new(
        guard.world_context_id(),
        guard.generation(),
        request_id,
        0,
        BTreeMap::new(),
        "VoxelCommit",
    )
    .expect("origin")
}

fn lifecycle_cmd(world: &VoxelWorld, event: &'static str, to: &'static str) -> WorldCommand {
    WorldCommand::Lifecycle {
        event,
        to,
        origin: origin_of(world, event),
    }
}

fn admit(world: &mut VoxelWorld, command: WorldCommand) -> Result<AdmittedCommand, WorldError> {
    world.endpoint().admit(command)
}

fn drive(world: &mut VoxelWorld, steps: &[(&'static str, &'static str)]) {
    for (event, to) in steps {
        let cmd = lifecycle_cmd(world, event, to);
        admit(world, cmd).unwrap_or_else(|err| panic!("{event}->{to}: {}", err.error_id()));
        assert_eq!(world.state_view().lifecycle(), *to);
    }
}

fn descriptor(role: &str, context: &str, world_id: &str) -> WorldDescriptor {
    WorldDescriptor {
        role: role.to_string(),
        world_context_id: context.to_string(),
        capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
        config: WorldConfigAdapter {
            world_id: world_id.to_string(),
        },
    }
}

fn create_named(role: &str, context: &str, world_id: &str, label: &str) -> VoxelWorld {
    VoxelWorld::create(
        descriptor(role, context, world_id),
        approved_snapshot(label),
    )
    .unwrap_or_else(|err| panic!("create {role}: {}", err.error_id()))
}

fn drive_to_running(world: &mut VoxelWorld) {
    drive(
        world,
        &[
            ("Initialize", "Initialized"),
            ("Prime", "Ready"),
            ("Start", "Running"),
        ],
    );
}

fn identity_of(world: &VoxelWorld) -> [u8; 32] {
    world.publication_authority().capture().root().identity()
}

fn world_rev(n: u64) -> WorldRevision {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut reserved = alloc.reserve_world().unwrap();
    reserved.finalize().unwrap()
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

fn empty_replacement(base: &lumio_voxel_domain::chunk::ChunkDirectoryRoot) -> ChunkReplacement {
    ChunkDeltaBuilder::new(base)
        .freeze()
        .expect("empty replacement")
}

fn current_world_revision(world: &VoxelWorld) -> u64 {
    world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision
}

fn latest_dirty(world: &VoxelWorld, chunk_id: &str) -> Option<u64> {
    world
        .publication_authority()
        .capture()
        .dirty_frontier()
        .latest_revision(chunk_id)
        .expect("canonical chunk id")
}

fn assert_lane_free(world: &mut VoxelWorld) {
    let lease = WorldWriteLane::try_acquire(world).expect("DurabilityAck must drop occupancy");
    drop(lease);
}

fn assert_stable_error(id: &str) {
    assert!(
        STABLE_ERROR_IDS.contains(&id),
        "error id {id} is not a generated STABLE_ERROR_IDS member"
    );
}

fn seed_ready(world: &VoxelWorld, chunks: &[&str]) {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let next = before.stamp().world_revision + 1;
    let mut builder = ChunkDirectoryBuilder::new();
    let mut chunk_revision_set = BTreeMap::new();
    for id in chunks {
        builder
            .insert(id, ChunkSlot::ready(payload(id.as_bytes())))
            .expect("canonical chunk id");
        chunk_revision_set.insert((*id).to_string(), next);
    }
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: view.world_id().to_string(),
        context_id: view.world_context_id().to_string(),
        generation: view.instance_generation(),
        world_revision: next,
        chunk_revision_set,
    };
    let dirty = DirtyFrontier::new(view.world_id(), view.instance_generation()).expect("world id");
    let later = PublishedStateRoot::new(stamp, builder.freeze(), dirty);
    let mut prepared = world
        .publication_authority()
        .prepare(
            world_rev(next),
            later,
            empty_replacement(before.directory()),
        )
        .expect("seed prepare");
    world
        .publication_authority()
        .publish_once(prepared.seal().expect("seed seal"))
        .expect("seed publish");
}

fn mutate(world: &mut VoxelWorld, txn_id: &str, chunks: &[(&str, &str)]) {
    let view = world.state_view();
    let world_revision = current_world_revision(world);
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
    for (id, value) in chunks {
        fields.insert((*id).to_string(), (*value).to_string());
    }
    let request = MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: view.world_id().to_string(),
        generation: view.instance_generation(),
        fields,
    };
    let mut lease = WorldWriteLane::try_acquire(world).expect("lane free for mutation");
    lease
        .enter(BarrierScope::Mutation)
        .expect("Running admits Mutation");
    let prepared = lease
        .prepare(&request)
        .unwrap_or_else(|err| panic!("prepare {txn_id}: {}", err.error_id()));
    lease
        .commit(prepared)
        .unwrap_or_else(|err| panic!("commit {txn_id}: {}", err.error_id()));
}

fn ack_for(world: &VoxelWorld, chunks: &[(&str, u64)]) -> AckEvidence {
    let view = world.state_view();
    AckEvidence {
        kind: "DurabilityAck".to_string(),
        world_id: view.world_id().to_string(),
        context: DurabilityAckContext {
            context_id: view.world_context_id().to_string(),
            generation: view.instance_generation(),
        },
        covered_world_revision: current_world_revision(world),
        covered_chunks: chunks
            .iter()
            .map(|(id, rev)| CoveredChunkAck {
                chunk_id: (*id).to_string(),
                up_to_chunk_revision: *rev,
            })
            .collect(),
    }
}

fn running_world_with_dirty(label: &str, chunks: &[&str]) -> VoxelWorld {
    let mut world = create_named(
        "Authority",
        &format!("ctx-{label}"),
        &format!("world-{label}"),
        label,
    );
    drive_to_running(&mut world);
    seed_ready(&world, chunks);
    let edits: Vec<(&str, &str)> = chunks.iter().map(|id| (*id, "edit")).collect();
    mutate(&mut world, "txn-dirty", &edits);
    for id in chunks {
        assert!(
            latest_dirty(&world, id).is_some(),
            "{id} must be dirty after mutation commit"
        );
    }
    world
}

fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn walk_rs(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|err| {
        panic!("read_dir {}: {err}", dir.display());
    });
    for entry in entries {
        let entry = entry.expect("dirent");
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == "tests" {
            continue;
        }
        if path.is_dir() {
            walk_rs(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn covering_ack_clears_only_that_chunk_and_changes_root() {
    assert!(SCHEMA_IDS.contains(&"voxel-durability-ack"));
    let mut world = running_world_with_dirty("r00137-happy", &["c:0:0:0", "c:1:0:0"]);
    let covered = latest_dirty(&world, "c:0:0:0").expect("dirty after commit");
    let other = latest_dirty(&world, "c:1:0:0").expect("other dirty after commit");
    let before = identity_of(&world);
    let ack = ack_for(&world, &[("c:0:0:0", covered)]);
    let receipt = apply_durability_ack(&mut world, ack).expect("covering ack");
    assert_eq!(receipt.coverage_len(), 1);
    assert_eq!(receipt.old_root(), before);
    assert_ne!(receipt.new_root(), before);
    assert_eq!(identity_of(&world), receipt.new_root());
    assert_eq!(latest_dirty(&world, "c:0:0:0"), None);
    assert_eq!(latest_dirty(&world, "c:1:0:0"), Some(other));
    assert_lane_free(&mut world);
}

#[test]
fn older_ack_does_not_clear_newer_dirty() {
    let mut world = running_world_with_dirty("r00137-old", &["c:0:0:0"]);
    let latest = latest_dirty(&world, "c:0:0:0").expect("dirty after commit");
    assert!(
        latest > 0,
        "seeded chunk revision leaves room for an older cut"
    );
    let before = identity_of(&world);
    let ack = ack_for(&world, &[("c:0:0:0", latest - 1)]);
    let receipt = apply_durability_ack(&mut world, ack).expect("old ack is idempotent");
    assert_eq!(receipt.coverage_len(), 0);
    assert_eq!(receipt.old_root(), before);
    assert_eq!(receipt.new_root(), before);
    assert_eq!(identity_of(&world), before);
    assert_eq!(latest_dirty(&world, "c:0:0:0"), Some(latest));
    assert_lane_free(&mut world);
}

#[test]
fn duplicate_ack_after_success_is_noop() {
    let mut world = running_world_with_dirty("r00137-dup", &["c:0:0:0"]);
    let covered = latest_dirty(&world, "c:0:0:0").expect("dirty after commit");
    let ack = ack_for(&world, &[("c:0:0:0", covered)]);
    apply_durability_ack(&mut world, ack.clone()).expect("first covering ack");
    let after_first = identity_of(&world);
    assert_eq!(latest_dirty(&world, "c:0:0:0"), None);
    let receipt = apply_durability_ack(&mut world, ack).expect("duplicate ack");
    assert_eq!(receipt.coverage_len(), 0);
    assert_eq!(receipt.old_root(), after_first);
    assert_eq!(receipt.new_root(), after_first);
    assert_eq!(identity_of(&world), after_first);
    assert_eq!(latest_dirty(&world, "c:0:0:0"), None);
    assert_lane_free(&mut world);
}

#[test]
fn wrong_world_id_or_generation_keeps_identity() {
    let mut world = running_world_with_dirty("r00137-mismatch", &["c:0:0:0"]);
    let covered = latest_dirty(&world, "c:0:0:0").expect("dirty after commit");
    let before = identity_of(&world);
    let dirty_before = latest_dirty(&world, "c:0:0:0");

    let mut wrong_world = ack_for(&world, &[("c:0:0:0", covered)]);
    wrong_world.world_id = "world-other".to_string();
    let err = apply_durability_ack(&mut world, wrong_world).expect_err("wrong world");
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);
    assert_eq!(latest_dirty(&world, "c:0:0:0"), dirty_before);
    assert_lane_free(&mut world);

    let mut wrong_generation = ack_for(&world, &[("c:0:0:0", covered)]);
    wrong_generation.context.generation = world.state_view().instance_generation() + 1;
    let err = apply_durability_ack(&mut world, wrong_generation).expect_err("stale generation");
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);
    assert_eq!(latest_dirty(&world, "c:0:0:0"), dirty_before);
    assert_lane_free(&mut world);
}

#[test]
fn future_covered_world_revision_is_rejected_before_clear() {
    let mut world = running_world_with_dirty("r00137-future", &["c:0:0:0"]);
    let covered = latest_dirty(&world, "c:0:0:0").expect("dirty after commit");
    let before = identity_of(&world);
    let mut ack = ack_for(&world, &[("c:0:0:0", covered)]);
    ack.covered_world_revision = current_world_revision(&world) + 1;
    let err = apply_durability_ack(&mut world, ack).expect_err("future cut");
    assert_eq!(err.error_id(), "EvidenceDigestMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);
    assert!(latest_dirty(&world, "c:0:0:0").is_some());
    assert_lane_free(&mut world);
}

#[test]
fn partial_coverage_clears_only_listed_chunk() {
    let mut world = running_world_with_dirty("r00137-partial", &["c:0:0:0", "c:2:0:0"]);
    let first = latest_dirty(&world, "c:0:0:0").expect("first dirty");
    let second = latest_dirty(&world, "c:2:0:0").expect("second dirty");
    let before = identity_of(&world);
    let ack = ack_for(&world, &[("c:0:0:0", first)]);
    let receipt = apply_durability_ack(&mut world, ack).expect("partial ack");
    assert_eq!(receipt.coverage_len(), 1);
    assert_ne!(identity_of(&world), before);
    assert_eq!(latest_dirty(&world, "c:0:0:0"), None);
    assert_eq!(latest_dirty(&world, "c:2:0:0"), Some(second));
    assert_lane_free(&mut world);
}

#[test]
fn production_src_has_no_clear_dirty_identifier() {
    let dirty = include_str!("../../lumio-voxel-domain/src/chunk/dirty.rs");
    let delta = include_str!("../../lumio-voxel-domain/src/chunk/delta.rs");
    for src in [dirty, delta] {
        let code = strip_line_comments(src);
        assert!(
            !code.contains("fn clear_dirty"),
            "domain dirty/delta must not define clear_dirty"
        );
        assert!(
            !code.contains("clear_dirty("),
            "domain dirty/delta must not call clear_dirty"
        );
    }
    assert!(
        strip_line_comments(dirty).contains("fn except_covered"),
        "except_covered is the only frontier removal helper"
    );

    let crates_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    walk_rs(&crates_dir, &mut files);
    assert!(!files.is_empty(), "expected production rust sources");
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("read {}: {err}", path.display());
        });
        let code = strip_line_comments(&text);
        assert!(
            !code.contains("fn clear_dirty"),
            "{} must not define fn clear_dirty",
            path.display()
        );
        assert!(
            !code.contains("clear_dirty("),
            "{} must not call clear_dirty(",
            path.display()
        );
    }

    let apply = include_str!("../src/world/durability_ack.rs");
    assert!(apply.contains("apply_durability_ack"));
    assert!(apply.contains("except_covered"));
    assert!(!apply.contains("std::fs"));
    assert!(!apply.contains("fsync"));
}
