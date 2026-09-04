//! R-00121: target-World fault isolation. Other instances keep progressing.

use lumio_voxel_contracts::{
    BASELINE_ID, MACHINE_IDS, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256,
    state_transition_table,
};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
};
use lumio_voxel_domain::section::{
    DirtyFrontier, SectionDeltaBuilder, SectionDirectoryBuilder, SectionPage, SectionPayload,
    SectionReplacement, SectionSlot,
};
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_ops::query::GeneratedVoxelQueryRequest;
use lumio_voxel_world::world::{
    AdmittedCommand, FaultEvidence, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor,
    WorldError, WorldEvent, WorldEventSink, WorldFaultPort, intern_local_embedded_pair,
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

fn mutation_cmd(world: &VoxelWorld, txn_id: &str) -> WorldCommand {
    let view = world.state_view();
    WorldCommand::Mutation {
        origin: origin_of(world, txn_id),
        request: MutationRequest {
            txn_id: txn_id.to_string(),
            world_id: view.world_id().to_string(),
            generation: view.instance_generation(),
            fields: BTreeMap::new(),
        },
    }
}

fn query_cmd(world: &VoxelWorld, query_id: &str) -> WorldCommand {
    let view = world.state_view();
    WorldCommand::Query {
        origin: origin_of(world, query_id),
        request: GeneratedVoxelQueryRequest {
            query_id: query_id.to_string(),
            world_id: view.world_id().to_string(),
            context: view.world_context_id().to_string(),
            section_ids: vec!["s:0:0:0".to_string()],
            cancel: false,
        },
    }
}

fn world_rev(n: u64) -> WorldRevision {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut reserved = alloc.reserve_world().unwrap();
    reserved.finalize().unwrap()
}

fn payload(bytes: &[u8]) -> SectionPayload {
    SectionPayload::from_pages([SectionPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn empty_replacement(
    base: &lumio_voxel_domain::section::SectionDirectoryRoot,
) -> SectionReplacement {
    SectionDeltaBuilder::new(base)
        .freeze()
        .expect("empty replacement")
}

fn root_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev_n: u64,
    slot: SectionSlot,
    dirty_reason: Option<&str>,
) -> PublishedStateRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", slot)
        .expect("canonical section id");
    let directory = builder.freeze();
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.to_string(),
        context_id: context_id.to_string(),
        generation,
        world_revision: world_rev_n,
        section_revision_set: BTreeMap::from([("s:0:0:0".to_string(), world_rev_n)]),
    };
    let dirty = match dirty_reason {
        Some(reason) => DirtyFrontier::new(world_id, generation)
            .expect("world id")
            .record("s:0:0:0", world_rev_n, reason)
            .expect("record dirty"),
        None => DirtyFrontier::new(world_id, generation).expect("world id"),
    };
    PublishedStateRoot::new(stamp, directory, dirty)
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

fn session_machine() -> &'static str {
    MACHINE_IDS
        .iter()
        .copied()
        .find(|id| *id == "SimulationSession")
        .expect("SimulationSession is generated")
}

fn publish_cut(world: &VoxelWorld, label: &[u8]) {
    let view = world.state_view();
    let before = world.publication_authority().capture();
    let mut prepared = world
        .publication_authority()
        .prepare(
            world_rev(1),
            root_at(
                view.world_id(),
                view.world_context_id(),
                view.instance_generation(),
                1,
                SectionSlot::ready(payload(label)),
                Some("mutation"),
            ),
            empty_replacement(before.directory()),
        )
        .expect("prepare published cut");
    let token = prepared.seal().expect("seal");
    world
        .publication_authority()
        .publish_once(token)
        .expect("publish_once");
}

#[test]
fn trip_world_a_leaves_world_b_progressing_and_keeps_published_root() {
    assert!(SCHEMA_IDS.contains(&"failure-bundle"));
    assert!(SCHEMA_IDS.contains(&"logging-event"));
    let machine = session_machine();
    assert!(!state_transition_table().iter().any(|edge| {
        edge.machine == machine && (edge.from == "Faulted" || edge.to == "Faulted")
    }));

    let (authority_role, replica_role) =
        intern_local_embedded_pair("Authority", "Replica").expect("LocalEmbedded pair");
    let mut world_a = create_named(
        authority_role,
        "ctx-fault-a",
        "world-fault-a",
        "r00121-fault-a",
    );
    let mut world_b = create_named(
        replica_role,
        "ctx-fault-b",
        "world-fault-b",
        "r00121-fault-b",
    );
    drive_to_running(&mut world_a);
    drive_to_running(&mut world_b);

    publish_cut(&world_a, b"auth-cut-1");
    let id_a = identity_of(&world_a);
    let id_b = identity_of(&world_b);
    assert_ne!(id_a, id_b);

    let mut sink = WorldEventSink::bounded(8);
    let evidence = FaultEvidence::new("invariant-breach");
    WorldFaultPort::trip(&mut world_a, &mut sink, "MaintenanceKick", &evidence)
        .expect("trip confines the target world");
    WorldFaultPort::trip(&mut world_a, &mut sink, "MaintenanceKick", &evidence)
        .expect("trip is idempotent");

    assert_eq!(world_a.state_view().lifecycle(), "Disposed");
    assert_ne!(world_a.state_view().lifecycle(), "Faulted");
    assert_eq!(world_a.state_view().lifecycle_machine(), machine);
    assert_eq!(identity_of(&world_a), id_a);

    let mut saw_failure = false;
    for event in sink.events() {
        if let WorldEvent::Failure(bundle) = event {
            assert_eq!(bundle.schema_id(), "failure-bundle");
            assert_eq!(bundle.error_id(), "MaintenanceKick");
            assert_stable_error(bundle.error_id());
            assert_eq!(bundle.world_id(), "world-fault-a");
            saw_failure = true;
        }
    }
    assert!(saw_failure, "trip emits a FailureBundle fragment");

    assert_eq!(identity_of(&world_b), id_b);
    let q_after = query_cmd(&world_b, "q-after-a-trip");
    admit(&mut world_b, q_after).expect("B still admits query");
    let pause_b = lifecycle_cmd(&world_b, "Pause", "Paused");
    admit(&mut world_b, pause_b).expect("B still admits lifecycle");
    assert_eq!(world_b.state_view().lifecycle(), "Paused");
    let q_paused = query_cmd(&world_b, "q-paused-b");
    admit(&mut world_b, q_paused).expect("B query while paused");
    let resume_b = lifecycle_cmd(&world_b, "Resume", "Running");
    admit(&mut world_b, resume_b).expect("B resume");
    let write_b = mutation_cmd(&world_b, "txn-b-still-running");
    admit(&mut world_b, write_b).expect("B still admits writes");
    assert_eq!(identity_of(&world_b), id_b);

    let write_a = mutation_cmd(&world_a, "txn-a-after-trip");
    let err_a = admit(&mut world_a, write_a).expect_err("A rejects writes after trip");
    assert_stable_error(err_a.error_id());
    assert_eq!(identity_of(&world_a), id_a);
}
