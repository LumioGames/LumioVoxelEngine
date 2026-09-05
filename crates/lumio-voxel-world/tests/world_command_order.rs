//! R-00119: per-World command linearization and completion fencing.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_ops::async_support::{
    CompletionDisposition, OriginEnvelope, OriginToken, validate_completion,
};
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_world::world::{
    AdmittedCommand, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor, WorldError,
    WorldRouter, WorldWriteLane,
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

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn prepare_and_commit(
    world: &mut VoxelWorld,
    txn_id: &str,
    world_revision: u64,
) -> OriginEnvelope<lumio_voxel_ops::mutation::GeneratedMutationReceipt> {
    let env = mutation_envelope(world, txn_id, world_revision);
    let prepared = WorldRouter::prepare(world, env)
        .unwrap_or_else(|err| panic!("prepare {txn_id}: {}", err.error_id()));
    WorldRouter::commit(world, prepared)
        .unwrap_or_else(|err| panic!("commit {txn_id}: {}", err.error_id()))
}

#[test]
fn same_world_mutation_commits_serialize_on_new_identity() {
    let mut world = create_named(
        "Authority",
        "ctx-order-same",
        "world-order-same",
        "r00119-order-same",
    );
    drive_to_running(&mut world);
    let id0 = identity_of(&world);
    let first = prepare_and_commit(&mut world, "txn-first", 0);
    let id1 = identity_of(&world);
    assert_ne!(id1, id0);
    assert_eq!(first.payload.evidence.old_root, id0);
    assert_eq!(first.payload.evidence.new_root, id1);

    let second = prepare_and_commit(&mut world, "txn-second", 1);
    let id2 = identity_of(&world);
    assert_ne!(id2, id1);
    assert_eq!(second.payload.evidence.old_root, id1);
    assert_eq!(second.payload.evidence.new_root, id2);
}

#[test]
fn two_worlds_commit_independently() {
    let mut world_a = create_named(
        "Authority",
        "ctx-order-a",
        "world-order-a",
        "r00119-order-a",
    );
    let mut world_b = create_named("Replica", "ctx-order-b", "world-order-b", "r00119-order-b");
    drive_to_running(&mut world_a);
    drive_to_running(&mut world_b);
    {
        let lease_a = WorldWriteLane::try_acquire(&mut world_a).expect("A lane");
        let lease_b = WorldWriteLane::try_acquire(&mut world_b).expect("B lane independent of A");
        drop(lease_a);
        drop(lease_b);
    }

    let id_a0 = identity_of(&world_a);
    let id_b0 = identity_of(&world_b);
    assert_ne!(id_a0, id_b0);
    let _ = prepare_and_commit(&mut world_a, "txn-a", 0);
    let id_a1 = identity_of(&world_a);
    let id_b1 = identity_of(&world_b);
    assert_ne!(id_a1, id_a0);
    assert_eq!(id_b1, id_b0);

    let _ = prepare_and_commit(&mut world_b, "txn-b", 0);
    assert_ne!(identity_of(&world_b), id_b0);
    assert_eq!(identity_of(&world_a), id_a1);
}

#[test]
fn stale_or_wrong_generation_completion_does_not_publish() {
    let mut world = create_named(
        "Authority",
        "ctx-order-stale",
        "world-order-stale",
        "r00119-order-stale",
    );
    drive_to_running(&mut world);
    let before = identity_of(&world);
    let guard = world.generation_guard();
    let basis = OriginToken::try_new(
        guard.world_context_id(),
        guard.generation(),
        "write-lane-basis",
        0,
        BTreeMap::new(),
        "VoxelCommit",
    )
    .expect("basis");

    let stale_env = mutation_envelope(&world, "txn-stale", 0);
    let prepared =
        WorldRouter::prepare(&mut world, stale_env).expect("prepare before stale completion");
    let OriginEnvelope {
        origin: good_origin,
        config_hash,
        payload,
    } = prepared;
    let stale = OriginToken::try_new(
        good_origin.world_context_id(),
        good_origin.instance_generation().wrapping_sub(1),
        good_origin.request_id(),
        good_origin.input_world_revision(),
        BTreeMap::new(),
        good_origin.apply_phase(),
    )
    .expect("stale origin constructs");
    assert_eq!(
        validate_completion(&basis, &stale),
        CompletionDisposition::Stale
    );
    let err = WorldRouter::commit(
        &mut world,
        OriginEnvelope {
            origin: stale,
            config_hash,
            payload,
        },
    )
    .expect_err("stale completion must not publish");
    assert_eq!(err.error_id(), "StaleEpoch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);

    let wrong_ctx_env = mutation_envelope(&world, "txn-wrong-ctx", 0);
    let prepared = WorldRouter::prepare(&mut world, wrong_ctx_env)
        .expect("prepare before wrong-world completion");
    let OriginEnvelope {
        origin: good_origin,
        config_hash,
        payload,
    } = prepared;
    let wrong_world = OriginToken::try_new(
        "other-context",
        good_origin.instance_generation(),
        good_origin.request_id(),
        good_origin.input_world_revision(),
        BTreeMap::new(),
        good_origin.apply_phase(),
    )
    .expect("wrong-world origin constructs");
    assert_eq!(
        validate_completion(&basis, &wrong_world),
        CompletionDisposition::WrongWorld
    );
    let err = WorldRouter::commit(
        &mut world,
        OriginEnvelope {
            origin: wrong_world,
            config_hash,
            payload,
        },
    )
    .expect_err("wrong world must not publish");
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);

    let wrong_gen_env = mutation_envelope(&world, "txn-wrong-gen", 0);
    let prepared = WorldRouter::prepare(&mut world, wrong_gen_env)
        .expect("prepare before wrong-generation completion");
    let OriginEnvelope {
        origin: good_origin,
        config_hash,
        payload,
    } = prepared;
    let wrong_gen = OriginToken::try_new(
        good_origin.world_context_id(),
        good_origin.instance_generation().wrapping_add(1),
        good_origin.request_id(),
        good_origin.input_world_revision(),
        BTreeMap::new(),
        good_origin.apply_phase(),
    )
    .expect("higher generation origin constructs");
    assert_eq!(
        validate_completion(&basis, &wrong_gen),
        CompletionDisposition::WrongWorld
    );
    let err = WorldRouter::commit(
        &mut world,
        OriginEnvelope {
            origin: wrong_gen,
            config_hash,
            payload,
        },
    )
    .expect_err("wrong generation must not publish");
    assert_eq!(err.error_id(), "SessionMismatch");
    assert_stable_error(err.error_id());
    assert_eq!(identity_of(&world), before);
}
