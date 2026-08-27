//! R-00073: immutable payload, four-state slot, directory COW root.

use lumio_voxel_contracts::{CHUNK_PRESENCE, MACHINE_IDS, SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_domain::chunk::{ChunkDirectoryBuilder, ChunkPage, ChunkPayload, ChunkSlot};

fn dense_page(bytes: &[u8]) -> ChunkPage {
    ChunkPage::new("Dense", "None", bytes.to_vec(), sha256(bytes))
}

fn sample_payload() -> ChunkPayload {
    ChunkPayload::from_pages([dense_page(b"page-0")]).expect("valid dense uncompressed page")
}

fn assert_stable_error(id: &str) {
    assert!(
        STABLE_ERROR_IDS.contains(&id),
        "error id {id} is not a generated STABLE_ERROR_IDS member"
    );
}

#[test]
fn four_slot_states_map_one_to_one_onto_chunk_presence() {
    assert_eq!(
        CHUNK_PRESENCE,
        &["Ready", "NotLoaded", "Pending", "Unavailable"]
    );
    assert!(
        MACHINE_IDS.contains(&"VoxelChunkResidency"),
        "residency is a different generated machine and must not replace presence"
    );
    assert!(!CHUNK_PRESENCE.contains(&"Unallocated"));
    assert!(!CHUNK_PRESENCE.contains(&"Loading"));
    assert!(!CHUNK_PRESENCE.contains(&"Dirty"));

    let ready = ChunkSlot::ready(sample_payload());
    let not_loaded = ChunkSlot::not_loaded();
    let pending = ChunkSlot::pending();
    let unavailable = ChunkSlot::unavailable();

    let names = [
        ready.presence(),
        not_loaded.presence(),
        pending.presence(),
        unavailable.presence(),
    ];
    assert_eq!(names, CHUNK_PRESENCE);
    for name in names {
        assert!(CHUNK_PRESENCE.contains(&name));
    }

    assert_ne!(ready, not_loaded);
    assert_ne!(ready, pending);
    assert_ne!(ready, unavailable);
    assert_ne!(not_loaded, pending);
    assert_ne!(not_loaded, unavailable);
    assert_ne!(pending, unavailable);

    assert!(ready.payload().is_some());
    assert!(not_loaded.payload().is_none());
    assert!(pending.payload().is_none());
    assert!(unavailable.payload().is_none());

    let payload = sample_payload();
    assert!(SCHEMA_IDS.contains(&payload.schema_id()));
    assert_eq!(payload.schema_id(), "voxel-chunk-page");
}

#[test]
fn illegal_conversion_returns_generated_error_and_leaves_previous_slot_unchanged() {
    let slot = ChunkSlot::unavailable();
    let before = slot.clone();
    let err = slot.try_convert("Ready", None).unwrap_err();
    assert_eq!(err.error_id(), "ChunkUnavailable");
    assert_stable_error(err.error_id());
    assert_eq!(slot, before);
    assert_eq!(slot.presence(), "Unavailable");
    assert!(slot.payload().is_none());

    let mut builder = ChunkDirectoryBuilder::new();
    builder
        .insert("c:0:0:0", slot.clone())
        .expect("canonical chunk id");
    let root = builder.freeze();
    let err = builder.convert("c:0:0:0", "Ready", None).unwrap_err();
    assert_eq!(err.error_id(), "ChunkUnavailable");
    assert_stable_error(err.error_id());

    let looked = root
        .lookup("c:0:0:0")
        .expect("canonical id")
        .expect("slot remains published");
    assert_eq!(looked.presence(), "Unavailable");
    assert_eq!(looked, &before);

    let still_builder = builder
        .freeze()
        .lookup("c:0:0:0")
        .expect("canonical id")
        .expect("builder entry unchanged after failed convert")
        .clone();
    assert_eq!(still_builder, before);

    let pending = ChunkSlot::pending();
    let ready = pending
        .try_convert("Ready", Some(sample_payload()))
        .expect("Pending -> Ready with payload is legal");
    assert_eq!(ready.presence(), "Ready");
    assert!(ready.payload().is_some());
}

#[test]
fn freeze_then_builder_insert_does_not_mutate_frozen_root() {
    let mut builder = ChunkDirectoryBuilder::new();
    builder
        .insert("c:0:0:0", ChunkSlot::not_loaded())
        .expect("canonical chunk id");
    let root_a = builder.freeze();
    let root_a_again = builder.freeze();

    builder
        .insert("c:1:0:0", ChunkSlot::pending())
        .expect("canonical chunk id");
    builder
        .insert("c:0:0:0", ChunkSlot::unavailable())
        .expect("replace on builder only");

    assert_eq!(
        root_a
            .lookup("c:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "NotLoaded"
    );
    assert!(root_a.lookup("c:1:0:0").expect("canonical id").is_none());
    assert_eq!(
        root_a_again
            .lookup("c:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "NotLoaded"
    );

    let root_b = builder.freeze();
    assert_eq!(
        root_b
            .lookup("c:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Unavailable"
    );
    assert_eq!(
        root_b
            .lookup("c:1:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Pending"
    );

    let mut other = ChunkDirectoryBuilder::new();
    other
        .insert("c:0:0:0", ChunkSlot::ready(sample_payload()))
        .expect("independent builder");
    let root_other = other.freeze();
    assert_eq!(
        root_other
            .lookup("c:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Ready"
    );
    assert_eq!(
        root_a
            .lookup("c:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "NotLoaded"
    );
    assert!(root_a.lookup("c:2:2:2").expect("canonical miss").is_none());
}

#[test]
fn bad_chunk_id_or_payload_hash_mismatch_fails_with_generated_error() {
    let mut builder = ChunkDirectoryBuilder::new();
    let slot = ChunkSlot::not_loaded();

    for bad in [
        "0:0:0",
        "c:0:0",
        "c:0:0:0:1",
        "c:01:0:0",
        "c:+1:0:0",
        "c:-0:0:0",
        "c:1.0:0:0",
        "c:2147483648:0:0",
        "c:-2147483649:0:0",
        "C:0:0:0",
        "c:x:y:z",
        "",
    ] {
        let err = builder.insert(bad, slot.clone()).unwrap_err();
        assert_eq!(err.error_id(), "CoordinateOutOfBounds", "id {bad}");
        assert_stable_error(err.error_id());
        let err = ChunkDirectoryBuilder::new()
            .freeze()
            .lookup(bad)
            .unwrap_err();
        assert_eq!(err.error_id(), "CoordinateOutOfBounds", "lookup {bad}");
    }

    builder
        .insert("c:-1:2:-3", ChunkSlot::pending())
        .expect("negative i32 coords are in range");
    builder
        .insert("c:-2147483648:2147483647:0", ChunkSlot::not_loaded())
        .expect("i32 extremes are in range");
    let root = builder.freeze();
    assert_eq!(
        root.lookup("c:-1:2:-3")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Pending"
    );

    let bytes = b"dense-page".to_vec();
    let mut bad_digest = sha256(&bytes);
    bad_digest[0] ^= 0xff;
    let err =
        ChunkPayload::from_pages([ChunkPage::new("Dense", "None", bytes, bad_digest)]).unwrap_err();
    assert_eq!(err.error_id(), "EvidenceDigestMismatch");
    assert_stable_error(err.error_id());
}

#[test]
fn chunk_module_source_has_no_io() {
    let sources = [
        include_str!("../src/chunk/mod.rs"),
        include_str!("../src/chunk/payload.rs"),
        include_str!("../src/chunk/slot.rs"),
        include_str!("../src/chunk/directory.rs"),
    ];
    for src in sources {
        assert!(
            !src.contains("std::fs"),
            "chunk module must not use std::fs"
        );
        assert!(!src.contains("::File"), "chunk module must not use File");
        assert!(
            !src.contains("lumio_voxel_world"),
            "chunk module must not reference world"
        );
        assert!(
            !src.contains("crate::revision"),
            "chunk must not call revision as a service"
        );
    }
}
