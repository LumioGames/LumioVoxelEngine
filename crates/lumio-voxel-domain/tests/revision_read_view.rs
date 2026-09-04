//! R-00071: immutable ReadView pin, live-pin retention, dual-world isolation.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, PinRegistry, ReadViewLease, RetentionFrontier, RevisionAllocator,
    to_generated_stamp,
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

fn stamp_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev: u64,
    sections: &[(&str, u64)],
) -> GeneratedRevisionStamp {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..world_rev {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut w = alloc.reserve_world().unwrap();
    let world = w.finalize().unwrap();
    let mut pairs = Vec::new();
    for (id, rev) in sections {
        let mut section_alloc = RevisionAllocator::new();
        for _ in 0..*rev {
            section_alloc.reserve_section().unwrap().abandon();
        }
        let mut c = section_alloc.reserve_section().unwrap();
        pairs.push((id.to_string(), c.finalize().unwrap()));
    }
    to_generated_stamp(world_id, context_id, generation, world, &pairs)
}

fn assert_generated_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
    assert!(
        id == "InvalidHandle" || id == "BudgetExceeded",
        "pin refusal must map to InvalidHandle or BudgetExceeded, got {id}"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn read_view_stamp_is_frozen_across_later_stamp_and_config_reload() {
    assert!(SCHEMA_IDS.contains(&"voxel-revision-stamp"));
    assert_send_sync::<lumio_voxel_domain::revision::RevisionPin>();
    assert_send_sync::<ReadViewLease>();

    let snap = approved_snapshot("r00071-read-view");
    let registry = PinRegistry::from_approved_snapshot(snap, 4, "ctx-1", 7);
    let stamp0 = stamp_at("world-a", "ctx-1", 7, 0, &[("s:0:0:0", 0)]);
    let pin = registry.try_pin(stamp0.clone()).unwrap();
    let view = ReadViewLease::from_pin(pin.clone());

    let stamp1 = stamp_at("world-a", "ctx-1", 7, 1, &[("s:0:0:0", 3)]);
    assert!(stamp1.world_revision > view.stamp().world_revision);

    let reloaded =
        PinRegistry::from_approved_snapshot(approved_snapshot("r00071-reload"), 4, "ctx-1", 7);
    let later = reloaded.try_pin(stamp1.clone()).unwrap();

    assert_eq!(view.stamp(), &stamp0);
    assert_eq!(pin.stamp(), &stamp0);
    assert_eq!(later.stamp(), &stamp1);
    assert_ne!(view.stamp(), later.stamp());
}

#[test]
fn retention_frontier_follows_live_pins_out_of_order_drop_keeps_pinned_stamp() {
    let snap = approved_snapshot("r00071-retention");
    let registry = PinRegistry::from_approved_snapshot(snap, 4, "ctx-1", 1);
    let old = stamp_at("world-a", "ctx-1", 1, 0, &[("s:0:0:0", 0)]);
    let mid = stamp_at("world-a", "ctx-1", 1, 2, &[("s:0:0:0", 4)]);
    let new = stamp_at("world-a", "ctx-1", 1, 5, &[("s:0:0:0", 9)]);

    let pin_old = registry.try_pin(old.clone()).unwrap();
    let pin_mid = registry.try_pin(mid.clone()).unwrap();
    let pin_new = registry.try_pin(new.clone()).unwrap();
    let frontier = RetentionFrontier::from_registry(&registry);
    assert_eq!(frontier.oldest_live(), Some(old.clone()));

    drop(pin_new);
    assert_eq!(
        frontier.oldest_live(),
        Some(old.clone()),
        "dropping a newer pin must not free the still-pinned older stamp"
    );

    drop(pin_old);
    assert_eq!(
        frontier.oldest_live(),
        Some(mid.clone()),
        "out-of-order drop of the oldest pin advances only to the next live pin"
    );

    drop(pin_mid);
    assert_eq!(frontier.oldest_live(), None);
}

#[test]
fn two_registries_do_not_share_pins_or_retention() {
    let snap = approved_snapshot("r00071-dual-world");
    let a = PinRegistry::from_approved_snapshot(snap.clone(), 1, "ctx-a", 3);
    let b = PinRegistry::from_approved_snapshot(snap, 1, "ctx-a", 3);
    let stamp = stamp_at("world-a", "ctx-a", 3, 0, &[]);

    let pin_a = a.try_pin(stamp.clone()).unwrap();
    let pin_b = b.try_pin(stamp.clone()).unwrap();
    assert_eq!(pin_a.stamp(), &stamp);
    assert_eq!(pin_b.stamp(), &stamp);

    let over_a = a.try_pin(stamp.clone()).unwrap_err();
    assert_generated_error(over_a.error_id());
    assert!(
        b.try_pin(stamp.clone()).is_err(),
        "b is independently at capacity"
    );

    drop(pin_a);
    assert_eq!(RetentionFrontier::from_registry(&a).oldest_live(), None);
    assert_eq!(
        RetentionFrontier::from_registry(&b).oldest_live(),
        Some(stamp.clone()),
        "dropping a pin on world A must not release world B"
    );

    let foreign = stamp_at("world-b", "ctx-b", 9, 0, &[]);
    let mismatch = b.try_pin(foreign).unwrap_err();
    assert_eq!(mismatch.error_id(), "InvalidHandle");
    assert_generated_error(mismatch.error_id());
}

#[test]
fn try_pin_over_max_pins_fails_with_generated_error_id() {
    let snap = approved_snapshot("r00071-budget");
    let zero = PinRegistry::from_approved_snapshot(snap.clone(), 0, "ctx-1", 1);
    let stamp = stamp_at("world-a", "ctx-1", 1, 0, &[]);
    let zero_err = zero.try_pin(stamp.clone()).unwrap_err();
    assert_generated_error(zero_err.error_id());

    let registry = PinRegistry::from_approved_snapshot(snap, 1, "ctx-1", 1);
    let held = registry.try_pin(stamp.clone()).unwrap();
    let clone = held.clone();
    let over = registry.try_pin(stamp.clone()).unwrap_err();
    assert_generated_error(over.error_id());
    assert_eq!(over.error_id(), "BudgetExceeded");

    drop(clone);
    let still_over = registry.try_pin(stamp.clone()).unwrap_err();
    assert_eq!(still_over.error_id(), "BudgetExceeded");

    drop(held);
    let reused = registry.try_pin(stamp.clone()).unwrap();
    assert_eq!(reused.stamp(), &stamp);
}

#[test]
fn destroyed_world_refuses_new_pins_old_pin_keeps_immutable_stamp() {
    let snap = approved_snapshot("r00071-destroy");
    let registry = PinRegistry::from_approved_snapshot(snap.clone(), 2, "ctx-1", 4);
    let stamp = stamp_at("world-a", "ctx-1", 4, 0, &[]);
    let held = registry.try_pin(stamp.clone()).unwrap();
    let view = ReadViewLease::from_pin(held.clone());

    registry.destroy();
    let refused = registry.try_pin(stamp.clone()).unwrap_err();
    assert_eq!(refused.error_id(), "InvalidHandle");
    assert_generated_error(refused.error_id());
    assert_eq!(held.stamp(), &stamp);
    assert_eq!(view.stamp(), &stamp);

    let reincarnated = PinRegistry::from_approved_snapshot(snap, 2, "ctx-1", 4);
    let fresh = reincarnated.try_pin(stamp.clone()).unwrap();
    assert_eq!(fresh.stamp(), &stamp);
    drop(held);
    assert_eq!(
        RetentionFrontier::from_registry(&reincarnated).oldest_live(),
        Some(stamp),
        "dropping a pin from a destroyed instance must not write into a new instance"
    );
}
