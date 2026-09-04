//! R-00078: one atomic PublishedState root; capture never mixes cuts.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, is_stable_error_id, sha256};
use lumio_voxel_domain::config_snapshot::{
    DecisionEvidence, GateSourceHashes, GeneratedHostCapability, GeneratedVoxelConfig,
    P0_DECISION_GATES, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::{
    PublicationAuthority, PublishedReadView, PublishedStateRoot,
};
use lumio_voxel_domain::revision::{
    GeneratedRevisionStamp, PinRegistry, REVISION_STAMP_SCHEMA, RevisionAllocator, WorldRevision,
    to_generated_stamp,
};
use lumio_voxel_domain::section::{
    DirtyFrontier, SectionDeltaBuilder, SectionDirectoryBuilder, SectionDirectoryRoot, SectionPage,
    SectionPayload, SectionReplacement, SectionSlot,
};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use std::collections::BTreeMap;
use std::sync::{Arc, Barrier};
use std::thread;

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

fn world_rev(n: u64) -> WorldRevision {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc.reserve_world().unwrap().abandon();
    }
    let mut reserved = alloc.reserve_world().unwrap();
    reserved.finalize().unwrap()
}

fn stamp_at(
    world_id: &str,
    context_id: &str,
    generation: u64,
    world_rev_n: u64,
    sections: &[(&str, u64)],
) -> GeneratedRevisionStamp {
    let world = world_rev(world_rev_n);
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

fn payload(bytes: &[u8]) -> SectionPayload {
    SectionPayload::from_pages([SectionPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn directory_with(slot: SectionSlot) -> SectionDirectoryRoot {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", slot)
        .expect("canonical section id");
    builder.freeze()
}

fn frontier(world_id: &str, generation: u64) -> DirtyFrontier {
    DirtyFrontier::new(world_id, generation).expect("non-empty world id")
}

fn empty_replacement(base: &SectionDirectoryRoot) -> SectionReplacement {
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
    let directory = directory_with(slot);
    let stamp = stamp_at(
        world_id,
        context_id,
        generation,
        world_rev_n,
        &[("s:0:0:0", world_rev_n)],
    );
    let dirty = match dirty_reason {
        Some(reason) => frontier(world_id, generation)
            .record("s:0:0:0", world_rev_n, reason)
            .expect("record dirty"),
        None => frontier(world_id, generation),
    };
    PublishedStateRoot::new(stamp, directory, dirty)
}

fn authority(
    label: &str,
    world_id: &str,
    context_id: &str,
    generation: u64,
    initial: PublishedStateRoot,
) -> PublicationAuthority {
    let pins =
        PinRegistry::from_approved_snapshot(approved_snapshot(label), 16, context_id, generation);
    PublicationAuthority::new(world_id, context_id, generation, pins, initial)
        .expect("initial root matches authority world")
}

fn presence(view: &PublishedReadView) -> &str {
    view.directory()
        .lookup("s:0:0:0")
        .expect("canonical id")
        .expect("slot published")
        .presence()
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

fn assert_consistent_cut(view: &PublishedReadView) {
    assert_eq!(view.stamp(), view.root().stamp());
    assert_eq!(view.stamp(), view.lease().stamp());
    assert_eq!(view.directory(), view.root().directory());
    assert_eq!(view.dirty_frontier(), view.root().dirty_frontier());
    match view.stamp().world_revision {
        0 => assert_eq!(presence(view), "Unchanged"),
        1 => assert_eq!(presence(view), "Ready"),
        other => panic!("unexpected published world revision {other}"),
    }
}

#[test]
fn publish_swaps_stamp_and_directory_together() {
    assert!(SCHEMA_IDS.contains(&REVISION_STAMP_SCHEMA));
    assert_send_sync::<PublicationAuthority>();
    assert_send_sync::<PublishedReadView>();
    assert_send_sync::<PublishedStateRoot>();

    let initial = root_at("world-a", "ctx-1", 1, 0, SectionSlot::unchanged(), None);
    let auth = authority("r00078-atomic", "world-a", "ctx-1", 1, initial);
    let before = auth.capture();
    assert_consistent_cut(&before);
    assert_eq!(before.stamp().world_revision, 0);
    assert_eq!(presence(&before), "Unchanged");
    assert!(before.root().indexes().is_empty());
    let hash_before = before.root().identity();

    let new_root = root_at(
        "world-a",
        "ctx-1",
        1,
        1,
        SectionSlot::ready(payload(b"cut-1")),
        Some("mutation"),
    );
    let mut prepared = auth
        .prepare(
            world_rev(1),
            new_root,
            empty_replacement(before.directory()),
        )
        .expect("prepare against current cut");
    let token = prepared.seal().expect("first seal");
    let published = auth.publish_once(token).expect("first visible swap");

    assert_consistent_cut(&published);
    assert_eq!(published.stamp().world_revision, 1);
    assert_eq!(presence(&published), "Ready");
    assert_ne!(published.root().identity(), hash_before);
    assert_eq!(
        published.dirty_frontier().reason("s:0:0:0").unwrap(),
        Some("mutation")
    );

    let after = auth.capture();
    assert_consistent_cut(&after);
    assert_eq!(after.stamp().world_revision, 1);
    assert_eq!(presence(&after), "Ready");
    assert_eq!(after.root().identity(), published.root().identity());

    assert_eq!(before.stamp().world_revision, 0);
    assert_eq!(presence(&before), "Unchanged");
    assert_eq!(before.root().identity(), hash_before);
    assert_ne!(before.stamp(), after.stamp());
    assert_ne!(presence(&before), presence(&after));
}

#[test]
fn stale_wrong_world_and_double_token_leave_root_hash_unchanged() {
    let initial = root_at("world-a", "ctx-1", 1, 0, SectionSlot::unchanged(), None);
    let auth = authority("r00078-reject", "world-a", "ctx-1", 1, initial);
    let hash0 = auth.capture().root().identity();

    let mut first = auth
        .prepare(
            world_rev(1),
            root_at(
                "world-a",
                "ctx-1",
                1,
                1,
                SectionSlot::ready(payload(b"cut-1")),
                Some("one"),
            ),
            empty_replacement(auth.capture().directory()),
        )
        .expect("first prepare");
    let token_ok = first.seal().expect("seal ok");

    let mut stale_prep = auth
        .prepare(
            world_rev(1),
            root_at(
                "world-a",
                "ctx-1",
                1,
                1,
                SectionSlot::ready(payload(b"stale")),
                Some("stale"),
            ),
            empty_replacement(auth.capture().directory()),
        )
        .expect("stale prepare still sees the original base");
    let token_stale = stale_prep.seal().expect("stale seal");

    let published = auth.publish_once(token_ok).expect("winning publish");
    let hash1 = published.root().identity();
    assert_ne!(hash1, hash0);

    let stale_err = auth.publish_once(token_stale).unwrap_err();
    assert_eq!(stale_err.error_id(), "SnapshotBaseMismatch");
    assert_stable_error(stale_err.error_id());
    assert_eq!(auth.capture().root().identity(), hash1);

    let other_initial = root_at("world-b", "ctx-2", 9, 0, SectionSlot::unchanged(), None);
    let other = authority("r00078-other", "world-b", "ctx-2", 9, other_initial);
    let mut foreign_prep = other
        .prepare(
            world_rev(1),
            root_at(
                "world-b",
                "ctx-2",
                9,
                1,
                SectionSlot::ready(payload(b"foreign")),
                Some("foreign"),
            ),
            empty_replacement(other.capture().directory()),
        )
        .expect("foreign prepare");
    let foreign_token = foreign_prep.seal().expect("foreign seal");
    let wrong_world = auth.publish_once(foreign_token).unwrap_err();
    assert_eq!(wrong_world.error_id(), "SessionMismatch");
    assert_stable_error(wrong_world.error_id());
    assert_eq!(auth.capture().root().identity(), hash1);
    assert_eq!(presence(&auth.capture()), "Ready");
    assert_eq!(other.capture().stamp().world_revision, 0);
    assert_eq!(presence(&other.capture()), "Unchanged");

    let gen_initial = root_at("world-a", "ctx-1", 2, 0, SectionSlot::unchanged(), None);
    let newer_gen = authority("r00078-gen", "world-a", "ctx-1", 2, gen_initial);
    let mut gen_prep = newer_gen
        .prepare(
            world_rev(1),
            root_at(
                "world-a",
                "ctx-1",
                2,
                1,
                SectionSlot::ready(payload(b"gen")),
                Some("gen"),
            ),
            empty_replacement(newer_gen.capture().directory()),
        )
        .expect("same world, new generation");
    let gen_token = gen_prep.seal().expect("gen seal");
    let wrong_gen = auth.publish_once(gen_token).unwrap_err();
    assert_eq!(wrong_gen.error_id(), "StaleEpoch");
    assert_stable_error(wrong_gen.error_id());
    assert_eq!(auth.capture().root().identity(), hash1);

    let mut double = auth
        .prepare(
            world_rev(2),
            root_at(
                "world-a",
                "ctx-1",
                1,
                2,
                SectionSlot::pending(),
                Some("double"),
            ),
            empty_replacement(auth.capture().directory()),
        )
        .expect("prepare after successful cut");
    let first_seal = double.seal().expect("first seal of this prepare");
    let reused = double.seal().unwrap_err();
    assert_eq!(reused.error_id(), "HandleDoubleRelease");
    assert_stable_error(reused.error_id());
    assert_eq!(auth.capture().root().identity(), hash1);
    drop(first_seal);
    assert_eq!(auth.capture().root().identity(), hash1);

    let foreign_stamp = root_at("world-b", "ctx-9", 1, 1, SectionSlot::pending(), None);
    let rejected_prepare = auth
        .prepare(
            world_rev(1),
            foreign_stamp,
            empty_replacement(auth.capture().directory()),
        )
        .unwrap_err();
    assert_eq!(rejected_prepare.error_id(), "SessionMismatch");
    assert_stable_error(rejected_prepare.error_id());
    assert_eq!(auth.capture().root().identity(), hash1);
}

#[test]
fn two_authorities_do_not_share_root_arc() {
    let a = authority(
        "r00078-a",
        "world-a",
        "ctx-a",
        3,
        root_at("world-a", "ctx-a", 3, 0, SectionSlot::unchanged(), None),
    );
    let b = authority(
        "r00078-b",
        "world-b",
        "ctx-b",
        4,
        root_at("world-b", "ctx-b", 4, 0, SectionSlot::unavailable(), None),
    );

    let view_a = a.capture();
    let view_b = b.capture();
    assert!(!Arc::ptr_eq(&view_a.root_arc(), &view_b.root_arc()));
    assert_ne!(view_a.root().identity(), view_b.root().identity());
    assert_eq!(presence(&view_a), "Unchanged");
    assert_eq!(
        view_b
            .directory()
            .lookup("s:0:0:0")
            .unwrap()
            .unwrap()
            .presence(),
        "Unavailable"
    );

    let mut prepared = a
        .prepare(
            world_rev(1),
            root_at(
                "world-a",
                "ctx-a",
                3,
                1,
                SectionSlot::ready(payload(b"only-a")),
                Some("a"),
            ),
            empty_replacement(view_a.directory()),
        )
        .expect("prepare A");
    let token = prepared.seal().expect("seal A");
    a.publish_once(token).expect("publish A");

    let after_a = a.capture();
    let after_b = b.capture();
    assert_consistent_cut(&after_a);
    assert_eq!(after_a.stamp().world_revision, 1);
    assert_eq!(presence(&after_a), "Ready");
    assert_eq!(after_b.stamp().world_revision, 0);
    assert_eq!(
        after_b
            .directory()
            .lookup("s:0:0:0")
            .unwrap()
            .unwrap()
            .presence(),
        "Unavailable"
    );
    assert!(!Arc::ptr_eq(&after_a.root_arc(), &after_b.root_arc()));
    assert_eq!(after_b.root().identity(), view_b.root().identity());
}

#[test]
fn seal_tokens_are_unique_and_used_ids_cannot_publish() {
    let auth = authority(
        "r00078-token",
        "world-a",
        "ctx-1",
        1,
        root_at("world-a", "ctx-1", 1, 0, SectionSlot::unchanged(), None),
    );
    let hash0 = auth.capture().root().identity();
    let base = auth.capture();

    let mut prep_a = auth
        .prepare(
            world_rev(1),
            root_at(
                "world-a",
                "ctx-1",
                1,
                1,
                SectionSlot::ready(payload(b"token-a")),
                Some("a"),
            ),
            empty_replacement(base.directory()),
        )
        .expect("prepare A");
    let mut prep_b = auth
        .prepare(
            world_rev(1),
            root_at("world-a", "ctx-1", 1, 1, SectionSlot::pending(), Some("b")),
            empty_replacement(base.directory()),
        )
        .expect("prepare B");

    let token_a = prep_a.seal().expect("seal A");
    let token_b = prep_b.seal().expect("seal B");
    assert_ne!(
        token_a.id(),
        token_b.id(),
        "each seal must mint a distinct token id"
    );
    let used_id = token_a.id();

    let published = auth.publish_once(token_a).expect("consume token A");
    let hash1 = published.root().identity();
    assert_ne!(hash1, hash0);

    let stale = auth.publish_once(token_b).unwrap_err();
    assert_eq!(stale.error_id(), "SnapshotBaseMismatch");
    assert_stable_error(stale.error_id());
    assert_eq!(auth.capture().root().identity(), hash1);

    let mut prep_c = auth
        .prepare(
            world_rev(2),
            root_at(
                "world-a",
                "ctx-1",
                1,
                2,
                SectionSlot::unavailable(),
                Some("c"),
            ),
            empty_replacement(auth.capture().directory()),
        )
        .expect("prepare C");
    let token_c = prep_c.seal().expect("seal C");
    assert_ne!(token_c.id(), used_id);
    let _published_c = auth
        .publish_once(token_c)
        .expect("unique unused id publishes");
    assert_ne!(auth.capture().root().identity(), hash1);
}

#[test]
fn concurrent_captures_see_complete_old_or_complete_new() {
    let auth = Arc::new(authority(
        "r00078-race",
        "world-a",
        "ctx-1",
        1,
        root_at("world-a", "ctx-1", 1, 0, SectionSlot::unchanged(), None),
    ));
    let start = Arc::new(Barrier::new(5));
    let mut readers = Vec::new();
    for _ in 0..4 {
        let auth = Arc::clone(&auth);
        let start = Arc::clone(&start);
        readers.push(thread::spawn(move || {
            start.wait();
            let mut seen = Vec::new();
            for _ in 0..64 {
                let view = auth.capture();
                assert_consistent_cut(&view);
                seen.push(view.stamp().world_revision);
            }
            seen
        }));
    }

    let writer = {
        let auth = Arc::clone(&auth);
        let start = Arc::clone(&start);
        thread::spawn(move || {
            start.wait();
            let mut prepared = auth
                .prepare(
                    world_rev(1),
                    root_at(
                        "world-a",
                        "ctx-1",
                        1,
                        1,
                        SectionSlot::ready(payload(b"racy")),
                        Some("race"),
                    ),
                    empty_replacement(auth.capture().directory()),
                )
                .expect("prepare under race");
            let token = prepared.seal().expect("seal under race");
            auth.publish_once(token).expect("swap under race")
        })
    };

    let published = writer.join().expect("writer thread");
    assert_consistent_cut(&published);
    for handle in readers {
        let revisions = handle.join().expect("reader thread");
        assert!(revisions.iter().all(|r| *r == 0 || *r == 1));
    }
}

/// Drive one prepare → seal → publish cycle with a fault point armed.
///
/// `PrePublication` fires while the token is still unsealed, so the visible
/// write never happens; any later point fires after `publish_once`, so the new
/// cut is already visible and must survive.
fn publish_under_fault(
    auth: &PublicationAuthority,
    injector: &mut FaultInjector,
    target: WorldRevision,
    new_root: PublishedStateRoot,
    replacement: SectionReplacement,
) -> Result<(), &'static str> {
    let mut prepared = auth
        .prepare(target, new_root, replacement)
        .map_err(|e| e.error_id())?;
    if let Some(point @ FaultPoint::PrePublication) = injector.take() {
        return Err(FaultInjector::error_id(point));
    }
    let token = prepared.seal().map_err(|e| e.error_id())?;
    auth.publish_once(token).map_err(|e| e.error_id())?;
    Ok(())
}

fn next_cut(base: &PublishedReadView) -> (PublishedStateRoot, SectionReplacement) {
    (
        root_at(
            "world-a",
            "ctx-1",
            1,
            1,
            SectionSlot::ready(payload(b"cut-1")),
            Some("mutation"),
        ),
        empty_replacement(base.directory()),
    )
}

#[test]
fn injected_pre_publication_fault_leaves_the_published_cut_untouched() {
    let initial = root_at("world-a", "ctx-1", 1, 0, SectionSlot::unchanged(), None);
    let auth = authority("r00078-fault-pre", "world-a", "ctx-1", 1, initial);
    let before = auth.capture();
    let hash_before = before.root().identity();

    let mut injector = FaultInjector::new();
    injector.arm(FaultPoint::PrePublication);
    let (root, replacement) = next_cut(&before);
    let err = publish_under_fault(&auth, &mut injector, world_rev(1), root, replacement)
        .expect_err("armed pre-publication fault must abort the cycle");
    assert_eq!(err, "InvalidHandle");
    assert_stable_error(err);
    assert!(FaultInjector::recoverable(FaultPoint::PrePublication));

    // Nothing became visible: the old cut is still whole and still current.
    let after = auth.capture();
    assert_consistent_cut(&after);
    assert_eq!(after.root().identity(), hash_before);
    assert_eq!(after.stamp().world_revision, 0);

    // Recoverable means the same publication succeeds on a retry.
    let (root, replacement) = next_cut(&after);
    publish_under_fault(&auth, &mut injector, world_rev(1), root, replacement)
        .expect("retry after a recoverable fault");
    let retried = auth.capture();
    assert_consistent_cut(&retried);
    assert_eq!(retried.stamp().world_revision, 1);
    assert_ne!(retried.root().identity(), hash_before);
}

#[test]
fn injected_post_publication_fault_does_not_roll_the_visible_cut_back() {
    let initial = root_at("world-a", "ctx-1", 1, 0, SectionSlot::unchanged(), None);
    let auth = authority("r00078-fault-post", "world-a", "ctx-1", 1, initial);
    let before = auth.capture();
    let hash_before = before.root().identity();

    let mut injector = FaultInjector::new();
    injector.arm(FaultPoint::PostPublication);
    let (root, replacement) = next_cut(&before);
    publish_under_fault(&auth, &mut injector, world_rev(1), root, replacement)
        .expect("post-publication fault fires after the visible swap");
    // An already-visible write is never recoverable and must not be undone.
    assert!(!FaultInjector::recoverable(FaultPoint::PostPublication));
    assert_stable_error(FaultInjector::error_id(FaultPoint::PostPublication));

    let after = auth.capture();
    assert_consistent_cut(&after);
    assert_eq!(after.stamp().world_revision, 1);
    assert_ne!(after.root().identity(), hash_before);
}
