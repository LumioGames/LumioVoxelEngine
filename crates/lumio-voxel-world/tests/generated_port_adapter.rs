//! R-00142: generated voxel-world-port total adapter.

use lumio_voxel_contracts::{
    BASELINE_ID, BINDINGS, BoundedBuffer, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS,
    is_stable_error_id, sha256,
};
use lumio_voxel_domain::block::{BlockId, CellOffset};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{GeneratedRevisionStamp, WorldRevision};
use lumio_voxel_domain::section::{
    DirtyFrontier, DurabilityAckContext, SectionDeltaBuilder, SectionDirectoryBuilder,
    SectionPayload, SectionSlot, SectionStorage, StagedEdit,
};
use lumio_voxel_ops::async_support::{OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::{MutationEntry, MutationRequest};
use lumio_voxel_ops::query::{BlockReadWorld, GeneratedVoxelQueryRequest};
use lumio_voxel_ops::snapshot::{
    MemoryCaptureWriter, RestorePreflight, RestoreShadowBuilder, encode_capture,
};
use lumio_voxel_project::physics_query::{
    MaterialClass, MaterialMask, MaterialTable, PhysicsQuery, PhysicsWorld, QueryResolution, Vec3,
};
use lumio_voxel_world::port::{
    GENERATED_PORT_METHODS, GeneratedVoxelWorldPortAdapter, MutationStatus, OwnedResultBuffer,
    map_internal_error,
};
use lumio_voxel_world::world::{
    AckEvidence, AdmittedCommand, RuntimeSnapshotCut, VoxelWorld, WorldCommand, WorldConfigAdapter,
    WorldDescriptor, WorldError, WorldRouter,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn drive_adapter(world: &mut VoxelWorld, steps: &[(&'static str, &'static str)]) {
    for (event, to) in steps {
        let cmd = lifecycle_cmd(world, event, to);
        GeneratedVoxelWorldPortAdapter::new(world)
            .admit(cmd)
            .unwrap_or_else(|err| panic!("{event}->{to}: {}", err.error_id()));
        assert_eq!(world.state_view().lifecycle(), *to);
    }
}

fn drive_to_running(world: &mut VoxelWorld) {
    drive_adapter(
        world,
        &[
            ("Initialize", "Initialized"),
            ("Prime", "Ready"),
            ("Start", "Running"),
        ],
    );
}

fn intern_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == "voxel-world-port")
        .expect("voxel-world-port must exist in generated SCHEMA_IDS")
}

fn intern_binding_rust_type() -> &'static str {
    BINDINGS
        .iter()
        .find(|binding| {
            binding.schema_id == "voxel-world-port" && binding.rust_type == "VoxelWorldPort"
        })
        .map(|binding| binding.rust_type)
        .expect("generated BINDINGS must intern VoxelWorldPort")
}

fn query_envelope(
    world: &VoxelWorld,
    query_id: &str,
) -> OriginEnvelope<GeneratedVoxelQueryRequest> {
    let view = world.state_view();
    OriginEnvelope {
        origin: origin_of(world, query_id),
        config_hash: world.config_hash().to_string(),
        payload: GeneratedVoxelQueryRequest {
            query_id: query_id.to_string(),
            world_id: view.world_id().to_string(),
            context: view.world_context_id().to_string(),
            section_ids: vec!["s:0:0:0".to_string()],
            cancel: false,
        },
    }
}

fn mutation_envelope(world: &VoxelWorld, txn_id: &str) -> OriginEnvelope<MutationRequest> {
    let view = world.state_view();
    OriginEnvelope {
        origin: origin_of(world, txn_id),
        config_hash: world.config_hash().to_string(),
        payload: MutationRequest {
            txn_id: txn_id.to_string(),
            world_id: view.world_id().to_string(),
            generation: view.instance_generation(),
            entries: Vec::new(),
        },
    }
}

fn empty_ack(world: &VoxelWorld) -> AckEvidence {
    let view = world.state_view();
    AckEvidence {
        kind: "DurabilityAck".to_string(),
        world_id: view.world_id().to_string(),
        context: DurabilityAckContext {
            context_id: view.world_context_id().to_string(),
            generation: view.instance_generation(),
        },
        covered_world_revision: world
            .publication_authority()
            .capture()
            .stamp()
            .world_revision,
        covered_sections: Vec::new(),
    }
}

fn seed_structured_section(world: &VoxelWorld, section_id: &str) {
    let storage = SectionStorage::uniform(BlockId::from_raw(0));
    let payload = SectionPayload::from_storage(storage).expect("structured payload");
    publish_section_slot(world, section_id, SectionSlot::ready(payload));
}

fn publish_section_slot(world: &VoxelWorld, section_id: &str, slot: SectionSlot) {
    let before = world.publication_authority().capture();
    let world_revision = before.stamp().world_revision + 1;
    let section_revision = world_revision;

    let mut directory = SectionDirectoryBuilder::new();
    for (id, current) in before.directory().iter() {
        directory.insert(&id.key(), current.clone()).unwrap();
    }
    directory.insert(section_id, slot.clone()).unwrap();
    let mut revisions = before.stamp().section_revision_set.clone();
    revisions.insert(section_id.to_string(), section_revision);
    let stamp = GeneratedRevisionStamp {
        schema_id: before.stamp().schema_id,
        world_id: before.stamp().world_id.clone(),
        context_id: before.stamp().context_id.clone(),
        generation: before.stamp().generation,
        world_revision,
        section_revision_set: revisions,
    };
    let root = PublishedStateRoot::new(
        stamp,
        directory.freeze(),
        DirtyFrontier::new(&before.stamp().world_id, before.stamp().generation).unwrap(),
    );
    let mut delta = SectionDeltaBuilder::new(before.directory());
    delta
        .stage(StagedEdit::new(section_id, slot))
        .expect("stage seed");
    let replacement = delta.freeze().expect("freeze seed");
    let mut prepared = world
        .publication_authority()
        .prepare(WorldRevision::from_raw(world_revision), root, replacement)
        .expect("prepare seed");
    let token = prepared.seal().expect("seal seed");
    world
        .publication_authority()
        .publish_once(token)
        .expect("publish seed");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

#[test]
fn bindings_and_schema_ids_intern_voxel_world_port() {
    let interned_schema = intern_schema();
    let interned_binding = intern_binding_rust_type();
    assert!(SCHEMA_IDS.contains(&"voxel-world-port"));
    assert!(BINDINGS.iter().any(|binding| {
        binding.schema_id == "voxel-world-port" && binding.rust_type == "VoxelWorldPort"
    }));

    let mut world = create_named(
        "Authority",
        "ctx-port-intern",
        "world-port-intern",
        "r00142-intern",
    );
    let adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
    assert!(std::ptr::eq(adapter.schema_id(), interned_schema));
    let evidence = adapter.evidence();
    assert!(std::ptr::eq(evidence.schema_id, interned_schema));
    assert!(std::ptr::eq(evidence.binding_rust_type, interned_binding));
    assert_eq!(evidence.binding_rust_type, "VoxelWorldPort");
}

#[test]
fn adapter_query_matches_direct_world_stamp() {
    let mut world = create_named(
        "Authority",
        "ctx-port-query",
        "world-port-query",
        "r00142-query",
    );
    drive_to_running(&mut world);
    let envelope = query_envelope(&world, "q-port");
    let via = GeneratedVoxelWorldPortAdapter::new(&mut world)
        .query(envelope.clone())
        .expect("adapter query");
    let direct = WorldRouter::query(&mut world, envelope).expect("direct query");
    assert_eq!(
        via.payload.evidence().read_stamp(),
        direct.payload.evidence().read_stamp()
    );
    assert_eq!(
        via.payload.evidence().read_stamp(),
        world.publication_authority().capture().stamp()
    );
}

#[test]
fn adapter_mutation_matches_direct_receipt_txn() {
    let mut via_world = create_named(
        "Authority",
        "ctx-port-mut-a",
        "world-port-mut-a",
        "r00142-mut-a",
    );
    drive_to_running(&mut via_world);
    let via_env = mutation_envelope(&via_world, "txn-port");
    let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut via_world);
    let prepared = adapter.prepare_mutation(via_env).expect("adapter prepare");
    let via_receipt = adapter.commit(prepared).expect("adapter commit");
    assert_eq!(via_receipt.payload.txn_id, "txn-port");
    assert_eq!(via_receipt.payload.evidence.txn_id, "txn-port");

    let mut direct_world = create_named(
        "Replica",
        "ctx-port-mut-b",
        "world-port-mut-b",
        "r00142-mut-b",
    );
    drive_to_running(&mut direct_world);
    let direct_env = mutation_envelope(&direct_world, "txn-port");
    let prepared = WorldRouter::prepare(&mut direct_world, direct_env).expect("direct prepare");
    let direct_receipt = WorldRouter::commit(&mut direct_world, prepared).expect("direct commit");
    assert_eq!(via_receipt.payload.txn_id, direct_receipt.payload.txn_id);
    assert_eq!(
        via_receipt.payload.evidence.txn_id,
        direct_receipt.payload.evidence.txn_id
    );
}

#[test]
fn adapter_capture_restore_and_durability_ack_are_callable() {
    let snap = approved_snapshot("r00142-cra");
    let mut world = VoxelWorld::create(
        descriptor("Authority", "ctx-port-cra", "world-port-cra"),
        snap.clone(),
    )
    .unwrap_or_else(|err| panic!("create: {}", err.error_id()));
    drive_to_running(&mut world);
    let cut = RuntimeSnapshotCut::from_live(&world, "cut-port");
    let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
    let (captured, evidence) = adapter.capture(&cut).expect("adapter capture");
    assert!(evidence.barrier_released);
    let mut writer = MemoryCaptureWriter::new(8192);
    encode_capture(&captured, &mut writer).expect("encode after capture");
    let bytes = writer.as_slice().to_vec();
    drop(captured);

    let decoded = RestorePreflight::validate(
        &bytes,
        world.state_view().world_id(),
        world.state_view().instance_generation(),
        snap.as_ref(),
    )
    .expect("preflight");
    let candidate = RestoreShadowBuilder::build(&decoded).expect("shadow");
    let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
    adapter.restore(candidate).expect("adapter restore");

    let ack = empty_ack(&world);
    GeneratedVoxelWorldPortAdapter::new(&mut world)
        .apply_durability_ack(ack)
        .expect("adapter durability ack");
}

#[test]
fn stale_generation_handle_maps_to_stale_epoch() {
    let mut world = create_named(
        "Authority",
        "ctx-port-stale",
        "world-port-stale",
        "r00142-stale",
    );
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
    let err = GeneratedVoxelWorldPortAdapter::new(&mut world)
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
fn owned_result_buffer_double_release_is_handle_double_release() {
    let mut buffer = OwnedResultBuffer::new(4);
    assert_eq!(buffer.as_slice().expect("live buffer"), b"");
    let transferred: BoundedBuffer = buffer.transfer().expect("transfer");
    assert_eq!(transferred.as_slice(), b"");
    assert_eq!(
        buffer
            .as_slice()
            .expect_err("use after transfer")
            .error_id(),
        "InvalidHandle"
    );
    assert_stable_error("InvalidHandle");

    let mut buffer = OwnedResultBuffer::new(1);
    buffer.release().expect("first release");
    let err = buffer.release().expect_err("double release");
    assert_eq!(err.error_id(), "HandleDoubleRelease");
    assert_stable_error(err.error_id());
    assert_eq!(
        buffer.as_slice().expect_err("use after release").error_id(),
        "InvalidHandle"
    );
}

#[test]
fn unknown_error_id_mapping_cannot_succeed() {
    let unknown = "TotallyUnknownError";
    assert!(!STABLE_ERROR_IDS.contains(&unknown));
    let mapped = map_internal_error(unknown);
    assert_eq!(mapped.error_id(), unknown);
    assert!(!STABLE_ERROR_IDS.contains(&mapped.error_id()));
    for id in STABLE_ERROR_IDS {
        assert_eq!(
            map_internal_error(id).error_id(),
            *id,
            "known id {id} must intern to itself"
        );
    }
    let world_err: WorldError = VoxelWorld::create(
        WorldDescriptor {
            role: "Authority".into(),
            world_context_id: String::new(),
            capabilities: vec!["Native".into()],
            config: WorldConfigAdapter {
                world_id: "world-x".into(),
            },
        },
        approved_snapshot("r00142-map"),
    )
    .expect_err("empty context");
    assert_eq!(
        map_internal_error(world_err.error_id()).error_id(),
        world_err.error_id()
    );
}

#[test]
fn generated_port_exposes_all_frozen_methods() {
    assert_eq!(
        GENERATED_PORT_METHODS,
        &[
            "createWorld",
            "query",
            "prepareMutation",
            "commit",
            "abort",
            "status",
            "capture",
            "applyDurabilityAck",
            "restore",
            "quiesce",
            "destroy",
        ]
    );

    let snapshot = approved_snapshot("r00142-surface");
    let mut world = GeneratedVoxelWorldPortAdapter::create_world(
        descriptor("Authority", "ctx-port-surface", "world-port-surface"),
        snapshot,
    )
    .expect("createWorld");
    drive_to_running(&mut world);

    {
        let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
        assert_eq!(
            adapter.status("missing-txn").expect("status"),
            MutationStatus::Unknown
        );
        adapter.quiesce("maintenance").expect("quiesce");
    }
    assert_eq!(world.state_view().lifecycle(), "Paused");
    GeneratedVoxelWorldPortAdapter::new(&mut world)
        .destroy()
        .expect("destroy");
    assert_eq!(world.state_view().lifecycle(), "Disposed");
}

#[test]
fn status_tracks_prepared_and_applied_mutations() {
    let mut world = create_named(
        "Authority",
        "ctx-port-status",
        "world-port-status",
        "r00142-status",
    );
    drive_to_running(&mut world);
    let request = mutation_envelope(&world, "txn-status");
    let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
    assert_eq!(
        adapter.status("txn-status").expect("unknown"),
        MutationStatus::Unknown
    );
    let prepared = adapter.prepare_mutation(request).expect("prepare");
    assert_eq!(
        adapter.status("txn-status").expect("prepared"),
        MutationStatus::Prepared
    );
    adapter.commit(prepared).expect("commit");
    assert_eq!(
        adapter.status("txn-status").expect("applied"),
        MutationStatus::Applied
    );
}

#[test]
fn committed_storage_is_shared_by_block_reads_and_physics() {
    let mut world = create_named(
        "Authority",
        "ctx-published-storage",
        "world-published-storage",
        "published-storage-views",
    );
    drive_to_running(&mut world);
    seed_structured_section(&world, "s:0:0:0");

    let generation = world.state_view().instance_generation();
    let revision = world
        .publication_authority()
        .capture()
        .stamp()
        .section_revision_set["s:0:0:0"];
    let stone = BlockId::from_raw(256);
    let request = OriginEnvelope {
        origin: origin_of(&world, "txn-shared-storage"),
        config_hash: world.config_hash().to_string(),
        payload: MutationRequest {
            txn_id: "txn-shared-storage".into(),
            world_id: "world-published-storage".into(),
            generation,
            entries: vec![MutationEntry::new(
                "s:0:0:0",
                CellOffset::new(0).unwrap(),
                stone,
                revision,
            )],
        },
    };
    let mut adapter = GeneratedVoxelWorldPortAdapter::new(&mut world);
    let prepared = adapter.prepare_mutation(request).expect("prepare mutation");
    adapter.commit(prepared).expect("commit mutation");

    let published = world.publication_authority().capture();
    let reads = BlockReadWorld::from_published_view(&published).expect("block read view");
    let cell = reads.read_cell(0, 0, 0).expect("published cell read");
    assert_eq!(cell.block_id(), Some(stone));
    assert_eq!(cell.section_revision(), revision + 1);

    let physics = PhysicsWorld::from_published_view(&published).expect("physics view");
    let materials = MaterialTable::default().with(stone.block_type(), MaterialClass::Solid);
    let hit = PhysicsQuery::new(&physics, &materials)
        .raycast(
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            MaterialMask::solid(),
        )
        .expect("physics query");
    assert!(matches!(hit, QueryResolution::Hit(_)));
}

#[test]
fn unchanged_published_slots_require_and_share_the_original_map_baseline() {
    let mut world = create_named(
        "Authority",
        "ctx-published-baseline",
        "world-published-baseline",
        "published-baseline-views",
    );
    drive_to_running(&mut world);
    publish_section_slot(&world, "s:0:0:0", SectionSlot::unchanged());

    let published = world.publication_authority().capture();
    assert_eq!(
        BlockReadWorld::from_published_view(&published)
            .expect_err("Unchanged has no inline BlockId storage")
            .error_id(),
        "section_encoding_mismatch"
    );
    assert_eq!(
        PhysicsWorld::from_published_view(&published)
            .expect_err("physics cannot infer the original map")
            .error_id(),
        "section_encoding_mismatch"
    );

    let stone = BlockId::from_raw(256 << 8);
    let baseline = SectionStorage::uniform(stone);
    let resolve_baseline =
        |id: &lumio_voxel_domain::key::SectionId| (id.key() == "s:0:0:0").then(|| baseline.clone());
    let reads = BlockReadWorld::from_published_view_with_baseline(&published, &resolve_baseline)
        .expect("resolved block read view");
    let cell = reads.read_cell(0, 0, 0).expect("baseline cell read");
    assert_eq!(cell.presence(), "Unchanged");
    assert_eq!(cell.block_id(), Some(stone));

    let physics = PhysicsWorld::from_published_view_with_baseline(&published, &resolve_baseline)
        .expect("resolved physics view");
    let materials = MaterialTable::default().with(stone.block_type(), MaterialClass::Solid);
    let hit = PhysicsQuery::new(&physics, &materials)
        .raycast(
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            MaterialMask::solid(),
        )
        .expect("baseline physics query");
    assert!(matches!(hit, QueryResolution::Hit(_)));
}

#[test]
fn no_ffi_runtime_persistence_crate_and_adapter_has_no_pinvoke() {
    let root = workspace_root();
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(&root)
        .output()
        .expect("cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = String::from_utf8_lossy(&out.stdout);
    let idx = json
        .find("\"workspace_members\"")
        .expect("workspace_members");
    let slice = &json[idx..];
    let start = slice.find('[').unwrap();
    let end = slice.find(']').unwrap();
    let members = &slice[start..=end];
    let count = members.matches("lumio-voxel-").count();
    assert_eq!(count, 7, "workspace_members={members}");
    assert!(!members.contains("ffi"));
    assert!(!members.contains("runtime"));
    assert!(!members.contains("persistence"));
    assert!(!json.contains("lumio-voxel-ffi"));
    assert!(!json.contains("lumio-voxel-runtime"));
    assert!(!json.contains("lumio-voxel-persistence"));

    let sources = [
        include_str!("../src/port/mod.rs"),
        include_str!("../src/port/adapter.rs"),
        include_str!("../src/port/error_mapping.rs"),
        include_str!("../src/port/ownership.rs"),
    ];
    for src in sources {
        assert!(!src.contains("extern \"C\""));
        assert!(!src.contains("extern \"c\""));
        assert!(!src.contains("DllImport"));
        assert!(!src.contains("P/Invoke"));
        assert!(!src.contains("PInvoke"));
    }
}

#[test]
fn adapter_admit_returns_admitted_lifecycle() {
    let mut world = create_named(
        "Replica",
        "ctx-port-admit",
        "world-port-admit",
        "r00142-admit",
    );
    let cmd = lifecycle_cmd(&world, "Initialize", "Initialized");
    let admitted = GeneratedVoxelWorldPortAdapter::new(&mut world)
        .admit(cmd)
        .expect("legal Initialize");
    match admitted {
        AdmittedCommand::Lifecycle { from, event, to } => {
            assert_eq!(from, "Created");
            assert_eq!(event, "Initialize");
            assert_eq!(to, "Initialized");
        }
        other => panic!("expected lifecycle admission, got {other:?}"),
    }
    assert_eq!(world.state_view().lifecycle(), "Initialized");
}
