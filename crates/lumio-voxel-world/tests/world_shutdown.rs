//! R-00121: ordered World shutdown and generation fencing.

use lumio_voxel_contracts::{
    BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256, state_transition_table,
};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_world::world::{
    AdmittedCommand, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor,
    WorldDiagnostics, WorldError, WorldEvent, WorldEventSink, WorldShutdown,
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

fn mutation_cmd_with_origin(world: &VoxelWorld, origin: OriginToken, txn_id: &str) -> WorldCommand {
    let view = world.state_view();
    let generation = origin.instance_generation();
    WorldCommand::Mutation {
        origin,
        request: MutationRequest {
            txn_id: txn_id.to_string(),
            world_id: view.world_id().to_string(),
            generation,
            fields: BTreeMap::new(),
        },
    }
}

fn identity_of(world: &VoxelWorld) -> [u8; 32] {
    world.publication_authority().capture().root().identity()
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn assert_not_faulted_state() {
    assert!(!state_transition_table().iter().any(|edge| {
        edge.machine == "SimulationSession" && (edge.from == "Faulted" || edge.to == "Faulted")
    }));
}

#[test]
fn begin_drain_finalize_order_and_second_finalize_is_idempotent() {
    assert!(SCHEMA_IDS.contains(&"logging-event"));
    assert_not_faulted_state();
    let mut world = create_named(
        "Authority",
        "ctx-shutdown-order",
        "world-shutdown-order",
        "r00121-shutdown-order",
    );
    drive_to_running(&mut world);
    let root0 = identity_of(&world);
    let gen0 = world.generation_guard().generation();
    let mut sink = WorldEventSink::bounded(8);

    let drain_early = WorldShutdown::drain(&mut world, &mut sink).expect_err("drain before begin");
    assert_eq!(drain_early.error_id(), "InvalidHandle");
    assert_stable_error(drain_early.error_id());
    let finalize_early =
        WorldShutdown::finalize(&mut world, &mut sink).expect_err("finalize before drain");
    assert_eq!(finalize_early.error_id(), "InvalidHandle");
    assert_eq!(world.state_view().lifecycle(), "Running");
    assert_eq!(identity_of(&world), root0);

    WorldShutdown::begin(&mut world, &mut sink).expect("begin Drain");
    assert_eq!(world.state_view().lifecycle(), "Draining");
    WorldShutdown::begin(&mut world, &mut sink).expect("begin is idempotent");
    let view = WorldDiagnostics::snapshot(&world);
    assert_eq!(view.lifecycle(), "Draining");
    assert_eq!(view.generation(), gen0);
    assert_eq!(view.published_root(), root0);

    let finalize_mid =
        WorldShutdown::finalize(&mut world, &mut sink).expect_err("finalize before drain");
    assert_eq!(finalize_mid.error_id(), "InvalidHandle");

    WorldShutdown::drain(&mut world, &mut sink).expect("drain FinalSnapshotTaken");
    assert_eq!(world.state_view().lifecycle(), "Snapshotted");
    WorldShutdown::drain(&mut world, &mut sink).expect("drain is idempotent");
    assert_eq!(
        WorldDiagnostics::snapshot(&world).in_flight_reservations(),
        0
    );

    WorldShutdown::finalize(&mut world, &mut sink).expect("finalize Dispose");
    assert_eq!(world.state_view().lifecycle(), "Disposed");
    let gen1 = world.generation_guard().generation();
    assert_ne!(gen1, gen0);
    WorldShutdown::finalize(&mut world, &mut sink).expect("second finalize is idempotent");
    assert_eq!(world.generation_guard().generation(), gen1);
    assert_eq!(world.state_view().lifecycle(), "Disposed");
    assert_ne!(world.state_view().lifecycle(), "Faulted");
    assert_eq!(identity_of(&world), root0);
}

#[test]
fn old_generation_origin_after_finalize_is_stale_epoch() {
    let mut world = create_named(
        "Replica",
        "ctx-shutdown-stale",
        "world-shutdown-stale",
        "r00121-shutdown-stale",
    );
    drive_to_running(&mut world);
    let old_origin = origin_of(&world, "txn-old-gen");
    let old_gen = old_origin.instance_generation();
    let mut sink = WorldEventSink::bounded(4);
    WorldShutdown::begin(&mut world, &mut sink).expect("begin");
    WorldShutdown::drain(&mut world, &mut sink).expect("drain");
    WorldShutdown::finalize(&mut world, &mut sink).expect("finalize");
    assert_ne!(world.generation_guard().generation(), old_gen);

    let stale = mutation_cmd_with_origin(&world, old_origin, "txn-old-gen");
    let err = admit(&mut world, stale).expect_err("old generation must not apply");
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
}

#[test]
fn sink_overflow_does_not_change_root_identity() {
    let mut world = create_named(
        "Authority",
        "ctx-shutdown-overflow",
        "world-shutdown-overflow",
        "r00121-shutdown-overflow",
    );
    drive_to_running(&mut world);
    let root0 = identity_of(&world);
    let mut sink = WorldEventSink::bounded(1);
    WorldShutdown::begin(&mut world, &mut sink).expect("begin fills the only slot");
    assert_eq!(sink.len(), 1);
    for i in 0..8 {
        sink.emit(WorldEvent::Logging {
            schema_id: "logging-event",
            event: "Drain",
            lifecycle: "Draining",
            generation: i as u64,
        });
    }
    assert!(sink.dropped() >= 8);
    assert_eq!(sink.len(), 1);
    WorldShutdown::drain(&mut world, &mut sink).expect("drain emit is dropped");
    WorldShutdown::finalize(&mut world, &mut sink).expect("finalize emit is dropped");
    assert_eq!(identity_of(&world), root0);
    assert_eq!(world.state_view().lifecycle(), "Disposed");
    assert!(sink.dropped() >= 10);
}
