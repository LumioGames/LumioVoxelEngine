//! R-00116: SimulationSession lifecycle, generation fencing, and command admission.

use lumio_voxel_contracts::legacy_baseline;
use lumio_voxel_contracts::{
    BASELINE_ID, MACHINE_IDS, SCHEMA_EPOCH, SCHEMA_IDS, Transition, VOXEL_WORLD_ROLES,
    is_stable_error_id, sha256, state_transition_table,
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
    AdmittedCommand, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor, WorldError,
    intern_local_embedded_pair, intern_role,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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

fn session_machine() -> &'static str {
    MACHINE_IDS
        .iter()
        .copied()
        .find(|id| *id == "SimulationSession")
        .expect("SimulationSession is generated")
}

fn session_edges() -> Vec<&'static Transition> {
    let machine = session_machine();
    state_transition_table()
        .iter()
        .filter(|t| t.machine == machine)
        .collect()
}

fn path_from_created(target: &str) -> Vec<(&'static str, &'static str)> {
    if target == "Created" {
        return Vec::new();
    }
    let mut q = VecDeque::new();
    q.push_back(("Created", Vec::new()));
    let mut seen = BTreeSet::new();
    seen.insert("Created");
    while let Some((node, path)) = q.pop_front() {
        for edge in session_edges() {
            if edge.from != node || !seen.insert(edge.to) {
                continue;
            }
            let mut next = path.clone();
            next.push((edge.event, edge.to));
            if edge.to == target {
                return next;
            }
            q.push_back((edge.to, next));
        }
    }
    panic!("no SimulationSession path from Created to {target}");
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

fn mutation_cmd(world: &VoxelWorld, txn_id: &str) -> WorldCommand {
    let view = world.state_view();
    WorldCommand::Mutation {
        origin: origin_of(world, txn_id),
        request: MutationRequest {
            txn_id: txn_id.to_string(),
            world_id: view.world_id().to_string(),
            generation: view.instance_generation(),
            entries: Vec::new(),
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

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn assert_not_host_lifecycle(name: &str) {
    assert_ne!(name, "WorldSlotHost");
    assert_ne!(name, legacy_baseline::SECTION_RESIDENCY_MACHINE_ID);
    assert!(!matches!(
        name,
        "Allocated"
            | "Bootstrapping"
            | "NativeReady"
            | "ManagedReady"
            | "LoadingSession"
            | "Quiescing"
            | "Stopping"
            | "Destroyed"
            | "Unallocated"
            | "Loading"
            | "Dirty"
            | "Evicting"
            | "Unloaded"
    ));
}

#[test]
fn legal_simulation_session_edges_succeed() {
    assert!(SCHEMA_IDS.contains(&"voxel-world-port"));
    assert_eq!(VOXEL_WORLD_ROLES, &["Authority", "Replica"]);
    assert_eq!(
        intern_role("Authority").expect("intern Authority"),
        "Authority"
    );
    assert_eq!(intern_role("Replica").expect("intern Replica"), "Replica");
    let machine = session_machine();
    let edges = session_edges();
    assert!(!edges.is_empty());

    for edge in &edges {
        let mut world = create_named(
            "Authority",
            "ctx-legal",
            "world-legal",
            &format!("r00116-legal-{}-{}", edge.from, edge.to),
        );
        drive(&mut world, &path_from_created(edge.from));
        assert_eq!(world.state_view().lifecycle(), edge.from);
        assert_eq!(world.state_view().lifecycle_machine(), machine);
        let cmd = lifecycle_cmd(&world, edge.event, edge.to);
        let admitted = admit(&mut world, cmd).expect("legal SimulationSession edge");
        match admitted {
            AdmittedCommand::Lifecycle { from, event, to } => {
                assert_eq!(from, edge.from);
                assert_eq!(event, edge.event);
                assert_eq!(to, edge.to);
            }
            other => panic!("expected lifecycle admission, got {other:?}"),
        }
        assert_eq!(world.state_view().lifecycle(), edge.to);
    }
}

#[test]
fn illegal_created_start_running_fails_and_state_unchanged() {
    let mut world = create_named("Replica", "ctx-illegal", "world-illegal", "r00116-illegal");
    let before = world.state_view().lifecycle();
    assert_eq!(before, "Created");
    let cmd = lifecycle_cmd(&world, "Start", "Running");
    let err =
        admit(&mut world, cmd).expect_err("Created --Start--> Running is not on SimulationSession");
    assert_stable_error(err.error_id());
    assert_eq!(world.state_view().lifecycle(), before);
    assert_eq!(world.state_view().lifecycle_machine(), session_machine());
}

#[test]
fn stale_generation_command_does_not_change_lifecycle() {
    let mut world = create_named("Authority", "ctx-stale", "world-stale", "r00116-stale");
    let guard = world.generation_guard();
    let stale = OriginToken::try_new(
        guard.world_context_id(),
        guard.generation().wrapping_add(1),
        "Initialize",
        0,
        BTreeMap::new(),
        "VoxelCommit",
    )
    .expect("stale origin constructs");
    let before = world.state_view().lifecycle();
    let err = world
        .endpoint()
        .admit(WorldCommand::Lifecycle {
            event: "Initialize",
            to: "Initialized",
            origin: stale,
        })
        .expect_err("stale generation must not apply");
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
    assert_eq!(world.state_view().lifecycle(), before);
}

#[test]
fn pause_rejects_mutation_admit_resume_allows_without_commit() {
    let mut world = create_named("Authority", "ctx-pause", "world-pause", "r00116-pause");
    drive(
        &mut world,
        &[
            ("Initialize", "Initialized"),
            ("Prime", "Ready"),
            ("Start", "Running"),
            ("Pause", "Paused"),
        ],
    );
    let paused = world.state_view().lifecycle();
    let paused_cmd = mutation_cmd(&world, "txn-paused");
    let err =
        admit(&mut world, paused_cmd).expect_err("Paused must reject writes before any write path");
    assert_stable_error(err.error_id());
    assert_eq!(world.state_view().lifecycle(), paused);

    let resume = lifecycle_cmd(&world, "Resume", "Running");
    admit(&mut world, resume).expect("Resume is legal");
    let running_cmd = mutation_cmd(&world, "txn-running");
    admit(&mut world, running_cmd).expect("Running mutation admit only");
    assert_eq!(world.state_view().lifecycle(), "Running");
}

#[test]
fn query_admit_ready_running_paused_and_rejected_on_created_draining() {
    let mut world = create_named("Replica", "ctx-query", "world-query", "r00116-query");
    let created = query_cmd(&world, "q-created");
    assert!(admit(&mut world, created).is_err());

    drive(
        &mut world,
        &[("Initialize", "Initialized"), ("Prime", "Ready")],
    );
    let q_ready = query_cmd(&world, "q-ready");
    admit(&mut world, q_ready).expect("Ready query");

    drive(&mut world, &[("Start", "Running")]);
    let q_running = query_cmd(&world, "q-running");
    admit(&mut world, q_running).expect("Running query");

    drive(&mut world, &[("Pause", "Paused")]);
    let q_paused = query_cmd(&world, "q-paused");
    admit(&mut world, q_paused).expect("Paused query");
    let paused_write = mutation_cmd(&world, "txn-paused-query");
    assert!(admit(&mut world, paused_write).is_err());

    drive(&mut world, &[("Resume", "Running"), ("Drain", "Draining")]);
    let q_drain = query_cmd(&world, "q-draining");
    assert!(admit(&mut world, q_drain).is_err());
    let drain_write = mutation_cmd(&world, "txn-draining");
    assert!(admit(&mut world, drain_write).is_err());
}

#[test]
fn authority_and_replica_instances_have_independent_captures() {
    let (authority_role, replica_role) =
        intern_local_embedded_pair("Authority", "Replica").expect("LocalEmbedded pair");
    let authority = create_named(authority_role, "ctx-auth", "world-auth", "r00116-auth-snap");
    let replica = create_named(replica_role, "ctx-repl", "world-repl", "r00116-repl-snap");
    assert_ne!(
        authority.generation_guard().generation(),
        replica.generation_guard().generation()
    );

    let id_auth_0 = authority
        .publication_authority()
        .capture()
        .root()
        .identity();
    let id_repl_0 = replica.publication_authority().capture().root().identity();
    assert_ne!(id_auth_0, id_repl_0);

    let view = authority.state_view();
    let before = authority.publication_authority().capture();
    let mut prepared = authority
        .publication_authority()
        .prepare(
            world_rev(1),
            root_at(
                view.world_id(),
                view.world_context_id(),
                view.instance_generation(),
                1,
                SectionSlot::ready(payload(b"auth-cut-1")),
                Some("mutation"),
            ),
            empty_replacement(before.directory()),
        )
        .expect("prepare on authority only");
    let token = prepared.seal().expect("seal");
    authority
        .publication_authority()
        .publish_once(token)
        .expect("publish_once on authority only");

    let id_auth_1 = authority
        .publication_authority()
        .capture()
        .root()
        .identity();
    let id_repl_1 = replica.publication_authority().capture().root().identity();
    assert_ne!(id_auth_1, id_auth_0);
    assert_eq!(id_repl_1, id_repl_0);
    assert_ne!(id_auth_1, id_repl_1);
}

#[test]
fn world_state_view_does_not_expose_host_slot_or_session_types() {
    let world = create_named("Authority", "ctx-view", "world-view", "r00116-view");
    let view = world.state_view();
    let lifecycle: &'static str = view.lifecycle();
    let machine: &'static str = view.lifecycle_machine();
    let role: &'static str = view.role();
    let _ctx: &str = view.world_context_id();
    let _gen: u64 = view.instance_generation();
    let _world_id: &str = view.world_id();
    assert_eq!(lifecycle, "Created");
    assert_eq!(machine, session_machine());
    assert_not_host_lifecycle(machine);
    assert_not_host_lifecycle(lifecycle);
    assert!(VOXEL_WORLD_ROLES.contains(&role));
    assert!(!MACHINE_IDS.contains(&lifecycle));
}

#[test]
fn create_rejects_empty_ids_and_unknown_role() {
    let snap = approved_snapshot("r00116-validate");
    let empty_ctx = VoxelWorld::create(
        WorldDescriptor {
            role: "Authority".into(),
            world_context_id: String::new(),
            capabilities: vec!["Native".into()],
            config: WorldConfigAdapter {
                world_id: "world-x".into(),
            },
        },
        snap.clone(),
    )
    .unwrap_err();
    assert_eq!(empty_ctx.error_id(), "InvalidHandle");
    assert_stable_error(empty_ctx.error_id());

    let unknown = VoxelWorld::create(
        WorldDescriptor {
            role: "WorldSlotHost".into(),
            world_context_id: "ctx-x".into(),
            capabilities: vec!["Native".into()],
            config: WorldConfigAdapter {
                world_id: "world-x".into(),
            },
        },
        snap,
    )
    .unwrap_err();
    assert!(unknown.error_id() == "RoleMismatch" || unknown.error_id() == "ClaimNotGranted");
    assert_stable_error(unknown.error_id());
}
