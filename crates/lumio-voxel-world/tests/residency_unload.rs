//! R-00452: durability-fenced Section unload and the R-00440 pin seam.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, sha256};
use lumio_voxel_domain::block::{BlockId, CellOffset};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, REVISION_STAMP_SCHEMA, RevisionAllocator,
};
use lumio_voxel_domain::section::{CoveredSectionAck, DurabilityAckContext};
use lumio_voxel_domain::section::{
    SectionDeltaBuilder, SectionDirectoryBuilder, SectionPayload, SectionSlot, SectionStorage,
};
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::{MutationEntry, MutationRequest};
use lumio_voxel_world::world::{
    AckEvidence, BarrierScope, NoPinExemption, PinExemptionError, PinExemptionHook,
    RegionPinManager, VoxelWorld, WorldCommand, WorldConfigAdapter, WorldDescriptor,
    WorldWriteLane, apply_durability_ack, unload_section,
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
    let digests = P0_DECISION_GATES
        .iter()
        .map(|gate| {
            (
                (*gate).to_string(),
                hex32(&sha256(format!("approved-{gate}").as_bytes())),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let evidence: Vec<DecisionEvidence> = P0_DECISION_GATES
        .iter()
        .map(|gate| DecisionEvidence {
            gate_id: (*gate).to_string(),
            approval_status: "approved".to_string(),
            source_hashes: source.clone(),
            evidence_digest: digests[*gate].clone(),
        })
        .collect();
    VoxelConfigSnapshot::from_generated(
        &GeneratedVoxelConfig {
            schema_id: "config-table",
            host_capability_schema_id: "host-capability",
            schema_epoch: SCHEMA_EPOCH,
            config_hash: hex32(&sha256(label.as_bytes())),
            gate_source_hashes: digests,
            host_capability: GeneratedHostCapability::from_names(["Native", "ReferenceVoxel"]),
            start_capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
            key_material: None,
        },
        &evidence,
    )
    .expect("approved snapshot")
}

fn origin(world: &VoxelWorld, request_id: &str) -> OriginToken {
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

fn running_world(label: &str) -> VoxelWorld {
    let mut world = VoxelWorld::create(
        WorldDescriptor {
            role: "Authority".into(),
            world_context_id: format!("ctx-{label}"),
            capabilities: vec!["Native".into(), "ReferenceVoxel".into()],
            config: WorldConfigAdapter {
                world_id: format!("world-{label}"),
            },
        },
        approved_snapshot(label),
    )
    .expect("world");
    for (event, to) in [
        ("Initialize", "Initialized"),
        ("Prime", "Ready"),
        ("Start", "Running"),
    ] {
        let event_origin = origin(&world, event);
        world
            .endpoint()
            .admit(WorldCommand::Lifecycle {
                event,
                to,
                origin: event_origin,
            })
            .expect("lifecycle");
    }
    world
}

fn payload(bytes: &[u8]) -> SectionPayload {
    let digest = sha256(bytes);
    SectionPayload::from_storage(SectionStorage::uniform(BlockId::from_raw(
        u32::from_le_bytes(digest[..4].try_into().unwrap()),
    )))
    .expect("payload")
}

fn world_revision(n: u64) -> lumio_voxel_domain::revision::WorldRevision {
    let mut allocator = RevisionAllocator::new();
    for _ in 0..n {
        allocator.reserve_world().expect("revision").abandon();
    }
    allocator
        .reserve_world()
        .expect("revision")
        .finalize()
        .expect("revision")
}

fn seed_ready(world: &VoxelWorld) {
    let before = world.publication_authority().capture();
    let next = before.stamp().world_revision + 1;
    let mut directory = SectionDirectoryBuilder::new();
    directory
        .insert("s:0:0:0", SectionSlot::ready(payload(b"ready")))
        .expect("section key");
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: before.stamp().world_id.clone(),
        context_id: before.stamp().context_id.clone(),
        generation: before.stamp().generation,
        world_revision: next,
        section_revision_set: BTreeMap::from([("s:0:0:0".into(), next)]),
    };
    let dirty = before.dirty_frontier().clone();
    let root = PublishedStateRoot::new(stamp, directory.freeze(), dirty);
    let empty = SectionDeltaBuilder::new(before.directory())
        .freeze()
        .expect("replacement");
    let mut prepared = world
        .publication_authority()
        .prepare(world_revision(next), root, empty)
        .expect("prepare seed");
    world
        .publication_authority()
        .publish_once(prepared.seal().expect("seal seed"))
        .expect("publish seed");
}

fn mutate(world: &mut VoxelWorld) {
    let view = world.publication_authority().capture();
    let expected_section_revision = view
        .stamp()
        .section_revision_set
        .get("s:0:0:0")
        .copied()
        .unwrap_or(view.stamp().world_revision);
    let request = MutationRequest {
        txn_id: "txn-r00452".into(),
        world_id: view.stamp().world_id.clone(),
        generation: view.stamp().generation,
        entries: vec![MutationEntry::new(
            "s:0:0:0",
            CellOffset::new(0).expect("cell offset"),
            BlockId::from_raw(1),
            expected_section_revision,
        )],
    };
    let mut lease = WorldWriteLane::try_acquire(world).expect("lane");
    lease.enter(BarrierScope::Mutation).expect("mutation scope");
    let prepared = lease.prepare(&request).expect("prepare mutation");
    lease.commit(prepared).expect("commit mutation");
}

fn ack_for(world: &VoxelWorld, revision: u64) -> AckEvidence {
    let view = world.publication_authority().capture();
    AckEvidence {
        kind: "DurabilityAck".into(),
        world_id: view.stamp().world_id.clone(),
        context: DurabilityAckContext {
            context_id: view.stamp().context_id.clone(),
            generation: view.stamp().generation,
        },
        covered_world_revision: view.stamp().world_revision,
        covered_sections: vec![CoveredSectionAck {
            section_id: "s:0:0:0".into(),
            up_to_section_revision: revision,
        }],
    }
}

#[derive(Default)]
struct Hook {
    calls: Vec<String>,
}

impl PinExemptionHook for Hook {
    fn check_pin_exemption(&mut self, section_id: &str) -> Result<(), PinExemptionError> {
        self.calls.push(section_id.to_string());
        Ok(())
    }
}

struct RejectingHook;

impl PinExemptionHook for RejectingHook {
    fn check_pin_exemption(&mut self, _section_id: &str) -> Result<(), PinExemptionError> {
        Err(PinExemptionError::pinned_section_evicted())
    }
}

#[test]
fn clean_unload_converts_ready_to_unchanged_and_calls_pin_hook() {
    let mut world = running_world("clean");
    seed_ready(&world);
    let before = world.publication_authority().capture().root().identity();
    let mut hook = Hook::default();
    let receipt = unload_section(&mut world, "s:0:0:0", &mut hook).expect("unload");

    assert_eq!(hook.calls, ["s:0:0:0"]);
    assert_eq!(receipt.section_id(), "s:0:0:0");
    assert_eq!(receipt.old_root(), before);
    assert_ne!(receipt.new_root(), before);
    assert_eq!(
        world
            .publication_authority()
            .capture()
            .directory()
            .lookup("s:0:0:0")
            .expect("key")
            .expect("slot")
            .presence(),
        "Unchanged"
    );
}

#[test]
fn dirty_unload_is_rejected_and_keeps_section_resident() {
    let mut world = running_world("dirty");
    seed_ready(&world);
    mutate(&mut world);
    let before = world.publication_authority().capture().root().identity();
    let mut hook = Hook::default();
    let error = unload_section(&mut world, "s:0:0:0", &mut hook).expect_err("dirty unload");

    assert_eq!(error.error_id(), "dirty_section_not_durable");
    assert!(
        hook.calls.is_empty(),
        "pin policy is not consulted when durability fails"
    );
    assert_eq!(
        world.publication_authority().capture().root().identity(),
        before
    );
    assert_eq!(
        world
            .publication_authority()
            .capture()
            .directory()
            .lookup("s:0:0:0")
            .expect("key")
            .expect("slot")
            .presence(),
        "Ready"
    );

    let latest = world
        .publication_authority()
        .capture()
        .dirty_frontier()
        .latest_revision("s:0:0:0")
        .expect("key")
        .expect("dirty");
    let ack = ack_for(&world, latest);
    apply_durability_ack(&mut world, ack).expect("ack");
    let mut hook = Hook::default();
    unload_section(&mut world, "s:0:0:0", &mut hook).expect("durable unload");
    assert_eq!(hook.calls, ["s:0:0:0"]);
}

#[test]
fn pin_hook_rejection_keeps_clean_section_resident() {
    let mut world = running_world("pinned");
    seed_ready(&world);
    let before = world.publication_authority().capture().root().identity();
    let error = unload_section(&mut world, "s:0:0:0", &mut RejectingHook)
        .expect_err("pin hook must reject unload");

    assert_eq!(error.error_id(), "pinned_section_evicted");
    assert_eq!(
        world.publication_authority().capture().root().identity(),
        before
    );
    assert_eq!(
        world
            .publication_authority()
            .capture()
            .directory()
            .lookup("s:0:0:0")
            .expect("key")
            .expect("slot")
            .presence(),
        "Ready"
    );
}

#[test]
fn region_pin_manager_blocks_unload_until_release() {
    let mut world = running_world("manager");
    seed_ready(&world);
    let mut pins = RegionPinManager::with_budgets(1, 1);
    let pin = pins.declare_pin(["s:0:0:0"]).expect("declaration");
    pins.mark_ready(pin).expect("ready signal");

    let error = unload_section(&mut world, "s:0:0:0", &mut pins).expect_err("pinned unload");
    assert_eq!(error.error_id(), "pinned_section_evicted");
    pins.release_pin(pin).expect("release");
    unload_section(&mut world, "s:0:0:0", &mut pins).expect("released unload");
}

#[test]
fn attached_pin_manager_cannot_be_bypassed_by_a_later_residency_update() {
    let mut world = running_world("attached-manager");
    seed_ready(&world);
    let mut pins = RegionPinManager::with_budgets(1, 1);
    let pin = pins.declare_pin(["s:0:0:0"]).expect("declaration");
    pins.mark_ready(pin).expect("ready");
    world.set_region_pin_manager(pins);

    let error = unload_section(&mut world, "s:0:0:0", &mut Hook::default())
        .expect_err("attached ready pin must guard every residency path");
    assert_eq!(error.error_id(), "pinned_section_evicted");
    assert_eq!(
        world
            .publication_authority()
            .capture()
            .directory()
            .lookup("s:0:0:0")
            .expect("key")
            .expect("slot")
            .presence(),
        "Ready"
    );
}

#[test]
fn no_pin_exemption_cannot_bypass_pin_protection() {
    let mut world = running_world("no-pin-exemption");
    seed_ready(&world);
    let mut hook = NoPinExemption;
    let error = unload_section(&mut world, "s:0:0:0", &mut hook).unwrap_err();
    assert_eq!(error.error_id(), "pinned_section_evicted");
}

#[test]
fn world_ready_signal_requires_every_declared_section_to_be_ready() {
    let world = running_world("world-ready");
    let mut pins = RegionPinManager::with_budgets(1, 1);
    let pin = pins.declare_pin(["s:0:0:0"]).expect("declaration");
    let error = pins
        .mark_ready_from_world(pin, &world)
        .expect_err("missing section is not ready");
    assert_eq!(error.error_id(), "pin_region_not_ready");

    seed_ready(&world);
    pins.mark_ready_from_world(pin, &world)
        .expect("ready section signal");
    assert!(pins.status(pin).expect("status").is_ready());
}
