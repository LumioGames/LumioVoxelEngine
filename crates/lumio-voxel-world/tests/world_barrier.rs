//! R-00119: serial write lease, typed Barrier scopes, and forbidden-work probes.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_ops::async_support::{OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_ops::query::GeneratedVoxelQueryRequest;
use lumio_voxel_world::world::{
    AdmittedCommand, BarrierScope, ForbiddenWork, PinBudget, RegionPinManager, VoxelWorld,
    WorldCommand, WorldConfigAdapter, WorldDescriptor, WorldError, WorldRouter, WorldWriteLane,
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

fn mutation_request(world: &VoxelWorld, txn_id: &str, _world_revision: u64) -> MutationRequest {
    let view = world.state_view();
    MutationRequest {
        txn_id: txn_id.to_string(),
        world_id: view.world_id().to_string(),
        generation: view.instance_generation(),
        entries: Vec::new(),
    }
}

fn query_request(world: &VoxelWorld, query_id: &str) -> GeneratedVoxelQueryRequest {
    let view = world.state_view();
    GeneratedVoxelQueryRequest {
        query_id: query_id.to_string(),
        world_id: view.world_id().to_string(),
        context: view.world_context_id().to_string(),
        section_ids: vec!["s:0:0:0".to_string()],
        cancel: false,
    }
}

fn mutation_envelope(
    world: &VoxelWorld,
    txn_id: &str,
    world_revision: u64,
) -> OriginEnvelope<MutationRequest> {
    OriginEnvelope {
        origin: origin_of(world, txn_id),
        config_hash: world.config_hash().to_string(),
        payload: mutation_request(world, txn_id, world_revision),
    }
}

fn query_envelope(
    world: &VoxelWorld,
    query_id: &str,
) -> OriginEnvelope<GeneratedVoxelQueryRequest> {
    OriginEnvelope {
        origin: origin_of(world, query_id),
        config_hash: world.config_hash().to_string(),
        payload: query_request(world, query_id),
    }
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

#[test]
fn router_rejects_an_empty_config_hash() {
    let mut world = create_named(
        "Authority",
        "ctx-empty-config-hash",
        "world-empty-config-hash",
        "empty-config-hash",
    );
    drive_to_running(&mut world);

    let mut envelope = query_envelope(&world, "q-empty-config-hash");
    envelope.config_hash.clear();
    let error = WorldRouter::query(&mut world, envelope)
        .expect_err("config identity is mandatory on every routed command");
    assert_eq!(error.error_id(), "SessionMismatch");
}

#[test]
fn mutation_barrier_prepare_commit_changes_capture_identity() {
    let mut world = create_named(
        "Authority",
        "ctx-barrier-mut",
        "world-barrier-mut",
        "r00119-barrier-mut",
    );
    drive_to_running(&mut world);
    let before = identity_of(&world);
    let request = mutation_request(&world, "txn-same-lease", 0);
    {
        let mut lease =
            WorldWriteLane::try_acquire(&mut world).expect("write lane is free before mutation");
        lease
            .enter(BarrierScope::Mutation)
            .expect("Running admits Mutation");
        let prepared = lease.prepare(&request).expect("prepare under Mutation");
        let receipt = lease.commit(prepared).expect("commit under the same lease");
        assert_eq!(receipt.evidence.old_root, before);
        assert_ne!(receipt.evidence.new_root, before);
    }
    let after = identity_of(&world);
    assert_ne!(after, before);
}

#[test]
fn query_capture_uses_old_or_new_cut_not_mixed() {
    let mut world = create_named(
        "Authority",
        "ctx-barrier-query",
        "world-barrier-query",
        "r00119-barrier-query",
    );
    drive_to_running(&mut world);
    let cut0 = world.publication_authority().capture();
    let q0_env = query_envelope(&world, "q-before");
    let q0 = WorldRouter::query(&mut world, q0_env).expect("query pins a single cut");
    assert_eq!(q0.payload.evidence().read_stamp(), cut0.stamp());
    let presence0: Vec<&str> = q0
        .payload
        .items()
        .iter()
        .map(|item| item.presence())
        .collect();

    let cut_env = mutation_envelope(&world, "txn-cut", 0);
    let prepared = WorldRouter::prepare(&mut world, cut_env).expect("prepare mutation");
    let receipt = WorldRouter::commit(&mut world, prepared).expect("publish new cut");
    let cut1 = world.publication_authority().capture();
    assert_ne!(cut1.root().identity(), cut0.root().identity());
    assert_eq!(receipt.payload.evidence.new_root, cut1.root().identity());

    let q1_env = query_envelope(&world, "q-after");
    let q1 = WorldRouter::query(&mut world, q1_env).expect("query after publish");
    assert_eq!(q1.payload.evidence().read_stamp(), cut1.stamp());
    assert_ne!(
        q0.payload.evidence().read_stamp(),
        q1.payload.evidence().read_stamp()
    );
    assert_eq!(q0.payload.evidence().read_stamp(), cut0.stamp());
    let presence1: Vec<&str> = q1
        .payload
        .items()
        .iter()
        .map(|item| item.presence())
        .collect();
    assert_eq!(presence0.len(), q0.payload.items().len());
    assert_eq!(presence1.len(), q1.payload.items().len());
    assert_ne!(
        q0.payload.evidence().plan_hash(),
        q1.payload.evidence().plan_hash()
    );
}

#[test]
fn forbidden_work_probes_return_generated_error_and_do_not_publish() {
    let mut world = create_named(
        "Replica",
        "ctx-barrier-forbid",
        "world-barrier-forbid",
        "r00119-barrier-forbid",
    );
    drive_to_running(&mut world);
    let probes = [
        (ForbiddenWork::Io, "LoaderTimeout"),
        (ForbiddenWork::Sleep, "LoaderTimeout"),
        (ForbiddenWork::UnboundedLoop, "BudgetExceeded"),
        (ForbiddenWork::Callback, "InvalidHandle"),
    ];
    for work in probes {
        assert_eq!(reject_forbidden(work.0).error_id(), work.1);
        assert_stable_error(work.1);
    }

    let scopes = [
        BarrierScope::Mutation,
        BarrierScope::CaptureCut,
        BarrierScope::DurabilityAck,
        BarrierScope::Restore,
        BarrierScope::StreamingApply,
    ];
    for scope in scopes {
        let before = identity_of(&world);
        {
            let mut lease = WorldWriteLane::try_acquire(&mut world).expect("lane free for probe");
            lease.enter(scope).unwrap_or_else(|err| {
                panic!("{scope:?} admission: {}", err.error_id());
            });
            for work in probes {
                let err = reject_forbidden(work.0);
                assert_eq!(err.error_id(), work.1);
                assert_stable_error(err.error_id());
            }
        }
        assert_eq!(
            identity_of(&world),
            before,
            "{scope:?} forbidden probes must not publish"
        );
    }
}

#[test]
fn query_path_does_not_keep_the_write_lane() {
    let mut world = create_named(
        "Authority",
        "ctx-barrier-lane",
        "world-barrier-lane",
        "r00119-barrier-lane",
    );
    drive_to_running(&mut world);
    let drop_env = query_envelope(&world, "q-drop-lease");
    let outcome =
        WorldRouter::query(&mut world, drop_env).expect("query returns without holding the lane");
    assert!(!outcome.origin.request_id().is_empty());
    assert!(!outcome.config_hash.is_empty());
    let lease = WorldWriteLane::try_acquire(&mut world)
        .expect("query must drop the write lane before returning");
    drop(lease);
}

#[test]
fn world_query_rejects_a_non_ready_result_for_an_attached_ready_pin() {
    let mut world = create_named(
        "Authority",
        "ctx-barrier-pinned-query",
        "world-barrier-pinned-query",
        "r00119-barrier-pinned-query",
    );
    drive_to_running(&mut world);
    let mut pins = RegionPinManager::from_budget(PinBudget::new(1, 1));
    let pin = pins.declare_pin(["s:0:0:0"]).expect("pin");
    pins.mark_ready(pin).expect("ready");
    world.set_region_pin_manager(pins);

    let envelope = query_envelope(&world, "q-pinned");
    let error = WorldRouter::query(&mut world, envelope)
        .expect_err("attached ready pins must guard generated query outcomes");
    assert_eq!(error.error_id(), "pinned_read_returned_pending");
}
