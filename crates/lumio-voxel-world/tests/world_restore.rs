//! R-00136: preflight + shadow root + atomic restore publish.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkDirectoryBuilder, ChunkPage, ChunkPayload, ChunkReplacement, ChunkSlot,
    DirtyFrontier,
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
use lumio_voxel_ops::snapshot::{
    CutEvidence, MemoryCaptureWriter, PinOrLease, RestorePreflight, RestoreShadowBuilder,
    VoxelCaptureRef, encode_capture,
};
use lumio_voxel_world::world::{
    AdmittedCommand, BarrierScope, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor,
    WorldError, WorldWriteLane, restore,
};
use std::collections::BTreeMap;
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

fn root_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev_n: u64,
    slot: ChunkSlot,
    dirty_reason: Option<&str>,
) -> PublishedStateRoot {
    let mut builder = ChunkDirectoryBuilder::new();
    builder.insert("c:0:0:0", slot).expect("canonical chunk id");
    let directory = builder.freeze();
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.to_string(),
        context_id: context_id.to_string(),
        generation,
        world_revision: world_rev_n,
        chunk_revision_set: BTreeMap::from([("c:0:0:0".to_string(), world_rev_n)]),
    };
    let dirty = match dirty_reason {
        Some(reason) => DirtyFrontier::new(world_id, generation)
            .expect("world id")
            .record("c:0:0:0", world_rev_n, reason)
            .expect("record dirty"),
        None => DirtyFrontier::new(world_id, generation).expect("world id"),
    };
    PublishedStateRoot::new(stamp, directory, dirty)
}

fn capture_of(world: &VoxelWorld, config_hash: &str) -> VoxelCaptureRef {
    let view = world.publication_authority().capture();
    VoxelCaptureRef::new(
        &view,
        PinOrLease::Lease(view.lease().clone()),
        CutEvidence {
            world_id: view.stamp().world_id.clone(),
            context_id: view.stamp().context_id.clone(),
            generation: view.stamp().generation,
            world_revision: view.stamp().world_revision,
            config_hash: config_hash.to_string(),
            artifact_hash: view.root().identity(),
        },
    )
    .expect("capture")
}

fn encode_bytes(capture: &VoxelCaptureRef) -> Vec<u8> {
    let mut writer = MemoryCaptureWriter::new(8192);
    encode_capture(capture, &mut writer).expect("encode");
    writer.as_slice().to_vec()
}

fn publish_ready_chunk(world: &VoxelWorld) {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let later = root_at(
        view.world_id(),
        view.world_context_id(),
        view.instance_generation(),
        1,
        ChunkSlot::ready(payload(b"restore-src")),
        Some("mutation"),
    );
    let mut prepared = world
        .publication_authority()
        .prepare(world_rev(1), later, empty_replacement(before.directory()))
        .expect("prepare cut");
    world
        .publication_authority()
        .publish_once(prepared.seal().expect("seal"))
        .expect("publish cut");
}

fn assert_stable_error(id: &str) {
    assert!(
        STABLE_ERROR_IDS.contains(&id),
        "error id {id} is not a generated STABLE_ERROR_IDS member"
    );
}

#[test]
fn restore_roundtrip_replaces_identity_and_matches_decoded_stamp() {
    let snap = approved_snapshot("r00136-happy");
    let mut world = create_named(
        "Authority",
        "ctx-restore-happy",
        "world-restore-happy",
        "r00136-happy",
    );
    drive_to_running(&mut world);
    publish_ready_chunk(&world);
    let before = identity_of(&world);
    let lifecycle_before = world.state_view().lifecycle();
    let capture = capture_of(&world, snap.config_hash());
    let bytes = encode_bytes(&capture);
    drop(capture);

    let decoded = RestorePreflight::validate(
        &bytes,
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect("preflight");
    assert_eq!(decoded.world_id(), "world-restore-happy");
    assert_eq!(
        decoded.generation(),
        world.state_view().instance_generation()
    );
    assert_eq!(decoded.world_revision(), 1);
    let candidate = RestoreShadowBuilder::build(&decoded).expect("shadow");
    assert!(candidate.hash_matches());

    let receipt = restore(&mut world, candidate).expect("restore");
    assert_eq!(receipt.old_root(), before);
    assert_ne!(receipt.new_root(), before);
    assert_eq!(identity_of(&world), receipt.new_root());
    assert_ne!(identity_of(&world), before);
    assert_eq!(world.state_view().lifecycle(), lifecycle_before);

    let view = world.publication_authority().capture();
    assert_eq!(view.stamp().world_id, decoded.world_id());
    assert_eq!(view.stamp().generation, decoded.generation());
    assert_eq!(view.stamp().world_revision, decoded.world_revision());
    assert_eq!(
        view.stamp().chunk_revision_set,
        *decoded.chunk_revision_set()
    );
    assert_eq!(
        view.directory()
            .lookup("c:0:0:0")
            .expect("lookup")
            .expect("slot")
            .presence(),
        "NotLoaded"
    );
    assert_eq!(
        view.dirty_frontier(),
        &DirtyFrontier::new(decoded.world_id(), decoded.generation()).expect("empty dirty")
    );

    let lease = WorldWriteLane::try_acquire(&mut world).expect("restore dropped occupancy");
    drop(lease);
}

#[test]
fn preflight_rejects_truncated_empty_and_wrong_world_without_touching_world() {
    let snap = approved_snapshot("r00136-preflight");
    let mut world = create_named(
        "Authority",
        "ctx-restore-pre",
        "world-restore-pre",
        "r00136-preflight",
    );
    drive_to_running(&mut world);
    let before = identity_of(&world);
    let capture = capture_of(&world, snap.config_hash());
    let bytes = encode_bytes(&capture);
    drop(capture);

    let empty = RestorePreflight::validate(
        b"",
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect_err("empty");
    assert_eq!(empty.error_id(), "InvalidHandle");
    assert_stable_error(empty.error_id());

    let truncated = RestorePreflight::validate(
        &bytes[..bytes.len() / 2],
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect_err("truncated");
    assert_eq!(truncated.error_id(), "InvalidHandle");
    assert_stable_error(truncated.error_id());

    let wrong_world = RestorePreflight::validate(
        &bytes,
        "world-other",
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect_err("wrong world");
    assert_eq!(wrong_world.error_id(), "SessionMismatch");
    assert_stable_error(wrong_world.error_id());

    assert_eq!(identity_of(&world), before);
    let lease =
        WorldWriteLane::try_acquire(&mut world).expect("preflight must not occupy the write lane");
    drop(lease);
}

#[test]
fn stale_generation_restore_keeps_old_identity() {
    let snap = approved_snapshot("r00136-stale");
    let mut source = create_named(
        "Authority",
        "ctx-restore-stale",
        "world-restore-stale",
        "r00136-stale",
    );
    drive_to_running(&mut source);
    let source_gen = source.state_view().instance_generation();
    let capture = capture_of(&source, snap.config_hash());
    let bytes = encode_bytes(&capture);
    drop(capture);
    let decoded =
        RestorePreflight::validate(&bytes, "world-restore-stale", source_gen, snap.as_ref())
            .expect("preflight source generation");
    let candidate = RestoreShadowBuilder::build(&decoded).expect("shadow");

    let mut live = create_named(
        "Authority",
        "ctx-restore-stale",
        "world-restore-stale",
        "r00136-stale",
    );
    drive_to_running(&mut live);
    assert_ne!(live.state_view().instance_generation(), source_gen);
    let before = identity_of(&live);
    let dirty_before = live
        .publication_authority()
        .capture()
        .dirty_frontier()
        .clone();
    let lifecycle_before = live.state_view().lifecycle();
    let err = restore(&mut live, candidate).expect_err("stale generation");
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&live), before);
    assert_eq!(live.state_view().lifecycle(), lifecycle_before);
    assert_eq!(
        live.publication_authority().capture().dirty_frontier(),
        &dirty_before
    );
}

#[test]
fn restore_and_mutation_occupancy_are_serial() {
    let snap = approved_snapshot("r00136-occ");
    let mut world = create_named(
        "Authority",
        "ctx-restore-occ",
        "world-restore-occ",
        "r00136-occ",
    );
    drive_to_running(&mut world);
    let capture = capture_of(&world, snap.config_hash());
    let bytes = encode_bytes(&capture);
    drop(capture);
    let decoded = RestorePreflight::validate(
        &bytes,
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect("preflight");
    let candidate = RestoreShadowBuilder::build(&decoded).expect("shadow");

    {
        let mut lease = WorldWriteLane::try_acquire(&mut world).expect("mutation occupancy");
        lease
            .enter(BarrierScope::Mutation)
            .expect("Running admits Mutation");
        let err = lease.enter(BarrierScope::Restore);
        assert_eq!(
            err.expect_err("already entered").error_id(),
            "InvalidHandle"
        );
        assert_stable_error("HandleDoubleRelease");
    }

    restore(&mut world, candidate).expect("restore after mutation lease drop");
    let lease = WorldWriteLane::try_acquire(&mut world)
        .expect("restore must drop occupancy before returning");
    drop(lease);
}

#[test]
fn restore_rejected_when_not_running_leaves_identity() {
    let snap = approved_snapshot("r00136-paused");
    let mut world = create_named(
        "Replica",
        "ctx-restore-paused",
        "world-restore-paused",
        "r00136-paused",
    );
    drive(
        &mut world,
        &[("Initialize", "Initialized"), ("Prime", "Ready")],
    );
    let capture = capture_of(&world, snap.config_hash());
    let bytes = encode_bytes(&capture);
    drop(capture);
    let decoded = RestorePreflight::validate(
        &bytes,
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect("preflight off live world");
    let candidate = RestoreShadowBuilder::build(&decoded).expect("shadow");
    let before = identity_of(&world);
    let err = restore(&mut world, candidate).expect_err("Ready is not write admissible");
    assert_eq!(err.error_id(), "ClaimNotGranted");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);
    assert_eq!(world.state_view().lifecycle(), "Ready");
}

#[test]
fn restore_sources_contain_no_forbidden_imports() {
    let ops = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../lumio-voxel-ops/src/snapshot");
    let world_restore = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/world/restore.rs");
    let files = [
        ops.join("decode.rs"),
        ops.join("restore_preflight.rs"),
        ops.join("restore_shadow.rs"),
        world_restore,
    ];
    for path in files {
        let text = std::fs::read_to_string(&path).expect("read restore source");
        assert!(
            !text.contains("lumio_voxel_ops::streaming"),
            "{} must not import streaming",
            path.display()
        );
        assert!(
            !text.contains("crate::streaming"),
            "{} must not import streaming",
            path.display()
        );
        assert!(
            !text.contains("std::fs"),
            "{} must not use std::fs",
            path.display()
        );
    }
}

#[test]
fn preflight_rejects_bad_schema_epoch_and_config_hash() {
    let snap = approved_snapshot("r00136-hash");
    let mut world = create_named(
        "Authority",
        "ctx-restore-hash",
        "world-restore-hash",
        "r00136-hash",
    );
    drive_to_running(&mut world);
    let before = identity_of(&world);
    let capture = capture_of(&world, snap.config_hash());
    let bytes = encode_bytes(&capture);
    drop(capture);

    let tampered_epoch = String::from_utf8(bytes.clone())
        .expect("utf8")
        .replace("\"schemaEpoch\":1", "\"schemaEpoch\":9");
    let epoch_err = RestorePreflight::validate(
        tampered_epoch.as_bytes(),
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect_err("schemaEpoch");
    assert_eq!(epoch_err.error_id(), "ManifestUnsupportedVersion");
    assert_stable_error(epoch_err.error_id());

    let other = approved_snapshot("r00136-hash-other");
    let hash_err = RestorePreflight::validate(
        &bytes,
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        other.as_ref(),
    )
    .expect_err("config hash");
    assert_eq!(hash_err.error_id(), "EvidenceDigestMismatch");
    assert_stable_error(hash_err.error_id());

    assert_eq!(identity_of(&world), before);
}
