//! R-00134: immutable VoxelCaptureRef and generated Canonical codec port.

#![cfg(feature = "snapshot")]

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
};
use lumio_voxel_domain::section::{
    DirtyFrontier, SectionDeltaBuilder, SectionDirectoryBuilder, SectionPage, SectionPayload,
    SectionSlot,
};
use lumio_voxel_ops::snapshot::{
    CaptureError, CaptureReadPort, CutEvidence, ManifestAdapter, MemoryCaptureWriter, PinOrLease,
    SNAPSHOT_HEADER_SCHEMA, SNAPSHOT_PAYLOAD_SCHEMA, SnapshotError, VoxelCaptureRef,
    decode_canonical_object, encode_capture,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
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

fn approved_snapshot(label: &str, capabilities: &[&str]) -> Arc<VoxelConfigSnapshot> {
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
    let names: Vec<String> = capabilities.iter().map(|s| (*s).to_string()).collect();
    let cfg = GeneratedVoxelConfig {
        schema_id: "config-table",
        host_capability_schema_id: "host-capability",
        schema_epoch: SCHEMA_EPOCH,
        config_hash: hex32(&sha256(label.as_bytes())),
        gate_source_hashes: digests,
        host_capability: GeneratedHostCapability::from_names(names.clone()),
        start_capabilities: names,
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

fn stamp(
    world_id: &str,
    context: &str,
    generation: u64,
    world_revision: u64,
) -> GeneratedRevisionStamp {
    GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.to_string(),
        context_id: context.to_string(),
        generation,
        world_revision,
        section_revision_set: BTreeMap::new(),
    }
}

fn dummy_payload(bytes: &[u8]) -> SectionPayload {
    SectionPayload::from_pages([SectionPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .expect("valid dense uncompressed page")
}

fn dummy_root(
    world_id: &str,
    context: &str,
    generation: u64,
    world_revision: u64,
    with_payload: bool,
) -> PublishedStateRoot {
    let mut builder = SectionDirectoryBuilder::new();
    if with_payload {
        builder
            .insert(
                "s:0:0:0",
                SectionSlot::ready(dummy_payload(b"must-not-read")),
            )
            .expect("canonical dummy id");
    }
    PublishedStateRoot::new(
        stamp(world_id, context, generation, world_revision),
        builder.freeze(),
        DirtyFrontier::new(world_id, generation).expect("world id"),
    )
}

fn authority(
    label: &str,
    world_id: &str,
    context: &str,
    generation: u64,
    initial: PublishedStateRoot,
) -> PublicationAuthority {
    let pins = PinRegistry::from_approved_snapshot(
        approved_snapshot(label, &["Native", "ReferenceVoxel"]),
        16,
        context,
        generation,
    );
    PublicationAuthority::new(world_id, context, generation, pins, initial)
        .expect("initial root matches authority")
}

fn cut_evidence(view: &PublishedReadView, config_hash: &str) -> CutEvidence {
    CutEvidence {
        world_id: view.stamp().world_id.clone(),
        context_id: view.stamp().context_id.clone(),
        generation: view.stamp().generation,
        world_revision: view.stamp().world_revision,
        config_hash: config_hash.to_string(),
        artifact_hash: view.root().identity(),
    }
}

fn pin_of(view: &PublishedReadView) -> PinOrLease {
    PinOrLease::Lease(view.lease().clone())
}

fn capture_of(
    view: &PublishedReadView,
    config_hash: &str,
) -> Result<VoxelCaptureRef, CaptureError> {
    VoxelCaptureRef::new(view, pin_of(view), cut_evidence(view, config_hash))
}

fn publish_later(auth: &PublicationAuthority, view: &PublishedReadView, payload: bool) {
    let later = dummy_root("world-a", "ctx-1", 1, 1, payload);
    let mut prepared = auth
        .prepare(
            world_rev(1),
            later,
            SectionDeltaBuilder::new(view.directory())
                .freeze()
                .expect("empty replacement"),
        )
        .expect("prepare later root");
    auth.publish_once(prepared.seal().expect("seal"))
        .expect("publish later root");
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn schemas_are_generated_members() {
    assert!(SCHEMA_IDS.contains(&SNAPSHOT_HEADER_SCHEMA));
    assert!(SCHEMA_IDS.contains(&SNAPSHOT_PAYLOAD_SCHEMA));
    assert_eq!(SNAPSHOT_HEADER_SCHEMA, "snapshot-header");
    assert_eq!(SNAPSHOT_PAYLOAD_SCHEMA, "voxel-snapshot-payload");
    assert_send_sync::<VoxelCaptureRef>();
    assert_send_sync::<MemoryCaptureWriter>();
}

fn assert_port_identity(port: &impl CaptureReadPort, view: &PublishedReadView, config_hash: &str) {
    assert_eq!(port.stamp(), view.stamp());
    assert_eq!(port.root_identity(), view.root().identity());
    assert_eq!(port.world_id(), view.stamp().world_id);
    assert_eq!(port.context_id(), view.stamp().context_id);
    assert_eq!(port.instance_generation(), view.stamp().generation);
    assert_eq!(
        port.section_revision_set(),
        &view.stamp().section_revision_set
    );
    assert_eq!(port.config_hash(), config_hash);
}

#[test]
fn capture_survives_concurrent_publish_and_encode_hashes_old_cut() {
    let snap = approved_snapshot("r00134-conc", &["Native", "ReferenceVoxel"]);
    let auth = Arc::new(authority(
        "r00134-conc-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, true),
    ));
    let view = auth.capture();
    let capture = capture_of(&view, snap.config_hash()).expect("capture old cut");
    let old_stamp = capture.stamp().clone();
    let old_identity = capture.root_identity();
    let old_generation = capture.instance_generation();
    let old_config = capture.config_hash().to_string();

    let start = Arc::new(Barrier::new(2));
    let encode_start = Arc::clone(&start);
    let capture_for_thread = capture.clone();
    let join = thread::spawn(move || {
        encode_start.wait();
        let mut writer = MemoryCaptureWriter::new(8192);
        let meta = encode_capture(&capture_for_thread, &mut writer).expect("encode old cut");
        (meta, writer.as_slice().to_vec())
    });

    start.wait();
    publish_later(&auth, &view, false);
    let published = auth.capture();
    assert_ne!(published.stamp(), &old_stamp);
    assert_ne!(published.root().identity(), old_identity);

    let (meta, bytes) = join.join().expect("encode thread");
    assert_eq!(meta.root_identity(), old_identity);
    assert_eq!(meta.world_revision(), old_stamp.world_revision);
    assert_eq!(meta.generation(), old_generation);
    assert_eq!(meta.config_hash(), old_config);
    assert_eq!(capture.stamp(), &old_stamp);
    assert_eq!(capture.root_identity(), old_identity);
    assert_eq!(view.stamp(), &old_stamp);
    assert_eq!(view.root().identity(), old_identity);

    let identity_hex = hex32(&old_identity);
    let text = String::from_utf8(bytes.clone()).expect("canonical utf-8");
    assert!(text.contains(&identity_hex), "{text}");
    assert!(text.contains("\"worldRevision\":0"), "{text}");
    assert!(!text.contains("\"worldRevision\":1"), "{text}");
}

#[test]
fn drop_capture_does_not_panic_and_second_capture_of_new_view_differs() {
    let snap = approved_snapshot("r00134-drop", &["Native", "ReferenceVoxel"]);
    let auth = authority(
        "r00134-drop-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, false),
    );
    let view = auth.capture();
    let capture = capture_of(&view, snap.config_hash()).expect("first capture");
    let old_identity = capture.root_identity();
    let live_clone = capture.clone();

    publish_later(&auth, &view, true);
    drop(capture);

    let mut writer = MemoryCaptureWriter::new(8192);
    let meta = encode_capture(&live_clone, &mut writer).expect("clone still encodes");
    assert_eq!(meta.root_identity(), old_identity);
    drop(live_clone);

    let new_view = auth.capture();
    let second = capture_of(&new_view, snap.config_hash()).expect("second capture");
    assert_ne!(second.root_identity(), old_identity);
    assert_ne!(second.stamp().world_revision, 0);

    let mut writer2 = MemoryCaptureWriter::new(8192);
    let meta2 = encode_capture(&second, &mut writer2).expect("encode new cut");
    assert_ne!(meta2.root_identity(), old_identity);
    assert_ne!(writer2.as_slice(), writer.as_slice());
}

#[test]
fn two_encodes_of_same_ref_are_byte_identical_and_decode_back() {
    let snap = approved_snapshot("r00134-canon", &["Native", "ReferenceVoxel"]);
    let auth = authority(
        "r00134-canon-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, true),
    );
    let view = auth.capture();
    let capture = capture_of(&view, snap.config_hash()).expect("capture");
    assert_port_identity(&capture, &view, snap.config_hash());
    assert_eq!(capture.pin_stamp(), capture.stamp());

    let mut a = MemoryCaptureWriter::new(8192);
    let mut b = MemoryCaptureWriter::new(8192);
    let meta_a = encode_capture(&capture, &mut a).expect("encode a");
    let meta_b = encode_capture(&capture, &mut b).expect("encode b");
    assert_eq!(a.as_slice(), b.as_slice());
    assert_eq!(meta_a.payload_hash(), meta_b.payload_hash());
    assert_eq!(meta_a.root_identity(), meta_b.root_identity());
    assert_eq!(meta_a.byte_len(), a.as_slice().len());

    let manifest = ManifestAdapter::object(&capture).expect("manifest object");
    let expected = manifest.encode();
    assert_eq!(a.as_slice(), expected.as_bytes());
    assert_eq!(meta_a.payload_hash().0, sha256(expected.as_bytes()));
    assert!(expected.contains("\"schemaId\":\"voxel-snapshot-payload\""));
    assert!(expected.contains("\"headerSchemaId\":\"snapshot-header\""));

    // ADR 0011 says a snapshot written before the typed encoding still restores
    // after it. Everything above this line is self-referential — encode agreeing
    // with itself, decode inverting encode, two substrings — so all of it stays
    // green through any change to `ManifestAdapter::object`, and the claim about
    // yesterday's bytes had nothing holding it. This digest does.
    //
    // Expected value from `tools/canonical/canonical_encoding_oracle.py`
    // (`snapshot_manifest`), which encodes this member set from the written rules
    // rather than by calling the code under test. Regenerating it from a failure
    // here is the wrong move: a changed digest means manifests written before the
    // change no longer round-trip, which is an ADR decision, not a test update.
    //
    // Moved from b513120c… by the chunk→section rename (ADR 0013): the manifest carries
    // `rootIdentity`, and that fingerprint includes the Debug rendering of the directory,
    // so renaming the key type changed it. Snapshots written before the rename therefore
    // do not round-trip — which is the ADR's decision, taken deliberately, not a quiet
    // test update. The value below came out of the oracle after its `rootIdentity` input
    // was moved, not out of this test's failure output.
    const MANIFEST_SHA256: &str =
        "1893afc9f731966250394262a38e0e5a7fae90a33b35d99684ac2adcf47c98ea";
    assert_eq!(
        hex32(&sha256(expected.as_bytes())),
        MANIFEST_SHA256,
        "manifest bytes changed: {expected}"
    );

    // Encoding is only worth anything if it is invertible: decode must hand back
    // exactly the members that were encoded, not a regrouping of them.
    let decoded = decode_canonical_object(a.as_slice()).expect("decode own bytes");
    assert_eq!(decoded, manifest);
}

#[test]
fn cancel_buffer_limit_and_bad_input_fail_this_operation_only() {
    let snap = approved_snapshot("r00134-fail", &["Native", "ReferenceVoxel"]);
    let auth = authority(
        "r00134-fail-view",
        "world-a",
        "ctx-1",
        1,
        dummy_root("world-a", "ctx-1", 1, 0, true),
    );
    let view = auth.capture();
    let stamp_before = view.stamp().clone();
    let identity_before = view.root().identity();

    let other = authority(
        "r00134-fail-other",
        "world-b",
        "ctx-1",
        1,
        dummy_root("world-b", "ctx-1", 1, 0, false),
    );
    let other_view = other.capture();
    let bad_pin = VoxelCaptureRef::new(
        &view,
        pin_of(&other_view),
        cut_evidence(&view, snap.config_hash()),
    )
    .expect_err("foreign lease");
    assert_eq!(bad_pin.error_id(), "InvalidHandle");
    assert_stable_error(bad_pin.error_id());

    let mut empty = cut_evidence(&view, snap.config_hash());
    empty.world_id.clear();
    let bad_empty = VoxelCaptureRef::new(&view, pin_of(&view), empty).expect_err("empty world");
    assert_eq!(bad_empty.error_id(), "InvalidHandle");
    assert_stable_error(bad_empty.error_id());

    let mut bad_hash = cut_evidence(&view, snap.config_hash());
    bad_hash.config_hash = "not-a-hash".to_string();
    let bad_cfg = VoxelCaptureRef::new(&view, pin_of(&view), bad_hash).expect_err("bad hash");
    assert_eq!(bad_cfg.error_id(), "InvalidHandle");
    assert_stable_error(bad_cfg.error_id());

    let mut mismatch = cut_evidence(&view, snap.config_hash());
    mismatch.artifact_hash = [7u8; 32];
    let bad_art = VoxelCaptureRef::new(&view, pin_of(&view), mismatch).expect_err("bad artifact");
    assert_eq!(bad_art.error_id(), "InvalidHandle");
    assert_stable_error(bad_art.error_id());

    let capture = capture_of(&view, snap.config_hash()).expect("live capture after rejects");

    let mut tiny = MemoryCaptureWriter::new(1);
    let over = encode_capture(&capture, &mut tiny).expect_err("buffer limit");
    assert_eq!(over.error_id(), "BudgetExceeded");
    assert_stable_error(over.error_id());
    assert!(tiny.as_slice().is_empty());

    let mut cancelled = MemoryCaptureWriter::new(8192);
    cancelled.cancel();
    let cancel_err: SnapshotError = encode_capture(&capture, &mut cancelled).expect_err("cancel");
    assert_eq!(cancel_err.error_id(), "LoaderCancelled");
    assert_stable_error(cancel_err.error_id());
    assert!(cancelled.as_slice().is_empty());

    let mut ok = MemoryCaptureWriter::new(8192);
    let meta = encode_capture(&capture, &mut ok).expect("encode after failed attempts");
    assert_eq!(meta.root_identity(), identity_before);
    assert!(!ok.as_slice().is_empty());

    assert_eq!(view.stamp(), &stamp_before);
    assert_eq!(view.root().identity(), identity_before);
    assert_eq!(capture.stamp(), &stamp_before);
    assert_eq!(capture.root_identity(), identity_before);
}

#[test]
fn snapshot_sources_contain_no_fs_io() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/snapshot");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("snapshot module sources")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "expected snapshot/*.rs exclusive sources"
    );
    for path in files {
        let text = fs::read_to_string(&path).expect("read snapshot source");
        assert!(
            !text.contains("std::fs"),
            "{} must not use std::fs",
            path.display()
        );
        assert!(
            !text.contains("lumio_voxel_test_support"),
            "{} must not depend on test-support",
            path.display()
        );
        assert!(
            !text.contains("WorldWriteLane"),
            "{} must not take WorldWriteLane",
            path.display()
        );
        assert!(
            !text.contains("lumio_voxel_world"),
            "{} must not depend on world",
            path.display()
        );
    }
}
