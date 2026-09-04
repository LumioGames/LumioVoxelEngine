//! R-00135: Snapshot Cut validation and CaptureCut barrier.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::section::DirtyFrontier;
use lumio_voxel_ops::async_support::{OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_ops::snapshot::{MemoryCaptureWriter, encode_capture};
use lumio_voxel_world::world::{
    AdmittedCommand, ForbiddenWork, RuntimeSnapshotCut, VoxelWorld, WorldCommand,
    WorldConfigAdapter, WorldDescriptor, WorldError, WorldRouter, WorldWriteLane, capture,
    reject_forbidden,
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

fn dirty_of(world: &VoxelWorld) -> DirtyFrontier {
    world
        .publication_authority()
        .capture()
        .dirty_frontier()
        .clone()
}

fn mutation_request(world: &VoxelWorld, txn_id: &str, world_revision: u64) -> MutationRequest {
    let view = world.state_view();
    let mut fields = BTreeMap::new();
    fields.insert("world_revision".to_string(), world_revision.to_string());
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: view.world_id().to_string(),
        generation: view.instance_generation(),
        fields,
    }
}

fn mutation_envelope(
    world: &VoxelWorld,
    txn_id: &str,
    world_revision: u64,
) -> OriginEnvelope<MutationRequest> {
    OriginEnvelope {
        origin: origin_of(world, txn_id),
        config_hash: String::new(),
        payload: mutation_request(world, txn_id, world_revision),
    }
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn assert_identity_unchanged(
    world: &VoxelWorld,
    identity: [u8; 32],
    dirty: &DirtyFrontier,
    lifecycle: &str,
) {
    assert_eq!(identity_of(world), identity);
    assert_eq!(&dirty_of(world), dirty);
    assert_eq!(world.state_view().lifecycle(), lifecycle);
}

#[test]
fn compatible_cut_capture_ref_matches_view_and_encode_runs_after_barrier() {
    let mut world = create_named(
        "Authority",
        "ctx-capture-ok",
        "world-capture-ok",
        "r00135-ok",
    );
    drive_to_running(&mut world);
    let view = world.publication_authority().capture();
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-ok");
    let (captured, evidence) = capture(&mut world, &cut).expect("compatible cut");
    assert_eq!(captured.stamp(), view.stamp());
    assert_eq!(captured.root_identity(), view.root().identity());
    assert_eq!(captured.pin_stamp(), captured.stamp());
    assert_eq!(evidence.cut_id, "cut-ok");
    assert_eq!(evidence.voxel_stamp, captured.stamp().clone());
    assert_eq!(evidence.root_hash, captured.root_identity());
    assert!(
        evidence.barrier_released,
        "CaptureCut occupancy must be released before returning"
    );

    let mut writer = MemoryCaptureWriter::new(8192);
    let meta = encode_capture(&captured, &mut writer).expect("encode after barrier");
    assert_eq!(meta.root_identity(), captured.root_identity());
    assert_eq!(meta.generation(), captured.instance_generation());
    assert_eq!(meta.world_revision(), captured.stamp().world_revision);
}

#[test]
fn try_acquire_succeeds_after_capture_releases_barrier() {
    let mut world = create_named(
        "Authority",
        "ctx-capture-lease",
        "world-capture-lease",
        "r00135-lease",
    );
    drive_to_running(&mut world);
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-lease");
    let (_captured, evidence) = capture(&mut world, &cut).expect("capture");
    assert!(evidence.barrier_released);
    let lease = WorldWriteLane::try_acquire(&mut world)
        .expect("capture must drop the write lane before returning");
    drop(lease);
}

#[test]
fn stale_generation_and_wrong_world_fail_without_changing_identity() {
    let mut world = create_named(
        "Authority",
        "ctx-capture-reject",
        "world-capture-reject",
        "r00135-reject",
    );
    drive_to_running(&mut world);
    let identity = identity_of(&world);
    let dirty = dirty_of(&world);
    let lifecycle = world.state_view().lifecycle();

    let mut stale = RuntimeSnapshotCut::from_live(&world, "cut-stale");
    stale.generation = stale.generation.wrapping_add(1);
    let err = capture(&mut world, &stale).expect_err("stale generation");
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
    assert_identity_unchanged(&world, identity, &dirty, lifecycle);

    let mut wrong = RuntimeSnapshotCut::from_live(&world, "cut-wrong-world");
    wrong.world_id = "world-other".to_string();
    let err = capture(&mut world, &wrong).expect_err("wrong world_id");
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());
    assert_identity_unchanged(&world, identity, &dirty, lifecycle);

    let mut stamp = RuntimeSnapshotCut::from_live(&world, "cut-stamp");
    stamp.world_revision = stamp.world_revision.wrapping_add(1);
    let err = capture(&mut world, &stamp).expect_err("stamp mismatch");
    assert_eq!(err.error_id(), "InvalidHandle");
    assert_stable_error(err.error_id());
    assert_identity_unchanged(&world, identity, &dirty, lifecycle);

    let lease = WorldWriteLane::try_acquire(&mut world)
        .expect("failed capture must not keep the write lane");
    drop(lease);
}

#[test]
fn capture_then_mutation_keeps_old_cut_identity() {
    let mut world = create_named(
        "Authority",
        "ctx-capture-mut",
        "world-capture-mut",
        "r00135-mut",
    );
    drive_to_running(&mut world);
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-before-mut");
    let (captured, evidence) = capture(&mut world, &cut).expect("capture old cut");
    assert!(evidence.barrier_released);
    let old_stamp = captured.stamp().clone();
    let old_identity = captured.root_identity();

    let env = mutation_envelope(&world, "txn-after-capture", 0);
    let prepared = WorldRouter::prepare(&mut world, env).expect("prepare after capture");
    let receipt = WorldRouter::commit(&mut world, prepared).expect("commit after capture");
    let new_identity = identity_of(&world);
    assert_ne!(new_identity, old_identity);
    assert_eq!(receipt.payload.evidence.old_root, old_identity);
    assert_eq!(receipt.payload.evidence.new_root, new_identity);
    assert_eq!(captured.stamp(), &old_stamp);
    assert_eq!(captured.root_identity(), old_identity);
    assert_eq!(evidence.voxel_stamp, old_stamp);
    assert_eq!(evidence.root_hash, old_identity);

    let mut writer = MemoryCaptureWriter::new(8192);
    let meta = encode_capture(&captured, &mut writer).expect("encode still old identity");
    assert_eq!(meta.root_identity(), old_identity);
    assert_eq!(meta.world_revision(), old_stamp.world_revision);
}

#[test]
fn reject_forbidden_io_does_not_publish_or_change_identity() {
    let mut world = create_named(
        "Replica",
        "ctx-capture-forbid",
        "world-capture-forbid",
        "r00135-forbid",
    );
    drive_to_running(&mut world);
    let identity = identity_of(&world);
    let dirty = dirty_of(&world);
    let lifecycle = world.state_view().lifecycle();

    let err = reject_forbidden(ForbiddenWork::Io);
    assert_eq!(err.error_id(), "LoaderTimeout");
    assert_stable_error(err.error_id());
    assert_identity_unchanged(&world, identity, &dirty, lifecycle);

    let cut = RuntimeSnapshotCut::from_live(&world, "cut-after-forbid");
    let (captured, evidence) = capture(&mut world, &cut).expect("capture is not I/O");
    assert!(evidence.barrier_released);
    assert_eq!(captured.root_identity(), identity);
    assert_identity_unchanged(&world, identity, &dirty, lifecycle);
}

#[test]
fn duplicate_compatible_cut_returns_same_root_hash() {
    let mut world = create_named(
        "Authority",
        "ctx-capture-dup",
        "world-capture-dup",
        "r00135-dup",
    );
    drive_to_running(&mut world);
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-dup");
    let (first, first_ev) = capture(&mut world, &cut).expect("first compatible cut");
    let (second, second_ev) = capture(&mut world, &cut).expect("duplicate compatible cut");
    assert_eq!(first.root_identity(), second.root_identity());
    assert_eq!(first.stamp(), second.stamp());
    assert_eq!(first_ev.root_hash, second_ev.root_hash);
    assert_eq!(first_ev.voxel_stamp, second_ev.voxel_stamp);
    assert!(first_ev.barrier_released);
    assert!(second_ev.barrier_released);
}
