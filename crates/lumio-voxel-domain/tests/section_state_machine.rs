//! R-00073: immutable payload, four-state slot, directory COW root.

use lumio_voxel_contracts::legacy_baseline;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_contracts::voxel_world::SECTION_PRESENCE;
use lumio_voxel_contracts::{MACHINE_IDS, SCHEMA_IDS, is_stable_error_id, sha256};
use lumio_voxel_domain::section::{
    SectionDirectoryBuilder, SectionPage, SectionPayload, SectionSlot,
};

fn dense_page(bytes: &[u8]) -> SectionPage {
    SectionPage::new("Dense", "None", bytes.to_vec(), sha256(bytes))
}

fn sample_payload() -> SectionPayload {
    SectionPayload::from_pages([dense_page(b"page-0")]).expect("valid dense uncompressed page")
}

fn assert_stable_error(id: &str) {
    assert!(
        is_stable_error_id(id),
        "error id {id} is neither a contract error code nor a frozen-mirror STABLE_ERROR_IDS member"
    );
}

#[test]
fn four_slot_states_map_one_to_one_onto_section_presence() {
    assert_eq!(
        SECTION_PRESENCE,
        &["Ready", "Unchanged", "Pending", "Unavailable"]
    );
    assert!(
        MACHINE_IDS.contains(&legacy_baseline::SECTION_RESIDENCY_MACHINE_ID),
        "residency is a different generated machine and must not replace presence"
    );
    assert!(!SECTION_PRESENCE.contains(&"Unallocated"));
    assert!(!SECTION_PRESENCE.contains(&"Loading"));
    assert!(!SECTION_PRESENCE.contains(&"Dirty"));

    let ready = SectionSlot::ready(sample_payload());
    let unchanged = SectionSlot::unchanged();
    let pending = SectionSlot::pending();
    let unavailable = SectionSlot::unavailable();

    let names = [
        ready.presence(),
        unchanged.presence(),
        pending.presence(),
        unavailable.presence(),
    ];
    assert_eq!(names, SECTION_PRESENCE);
    for name in names {
        assert!(SECTION_PRESENCE.contains(&name));
    }

    assert_ne!(ready, unchanged);
    assert_ne!(ready, pending);
    assert_ne!(ready, unavailable);
    assert_ne!(unchanged, pending);
    assert_ne!(unchanged, unavailable);
    assert_ne!(pending, unavailable);

    assert!(ready.payload().is_some());
    assert!(unchanged.payload().is_none());
    assert!(pending.payload().is_none());
    assert!(unavailable.payload().is_none());

    let payload = sample_payload();
    assert!(SCHEMA_IDS.contains(&payload.schema_id()));
    assert_eq!(payload.schema_id(), legacy_baseline::SECTION_PAGE_SCHEMA_ID);
}

#[test]
fn illegal_conversion_returns_generated_error_and_leaves_previous_slot_unchanged() {
    let slot = SectionSlot::unavailable();
    let before = slot.clone();
    let err = slot.try_convert("Ready", None).unwrap_err();
    assert_eq!(err.error_id(), vw::SECTION_UNAVAILABLE);
    assert_stable_error(err.error_id());
    assert_eq!(slot, before);
    assert_eq!(slot.presence(), "Unavailable");
    assert!(slot.payload().is_none());

    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", slot.clone())
        .expect("canonical section id");
    let root = builder.freeze();
    let err = builder.convert("s:0:0:0", "Ready", None).unwrap_err();
    assert_eq!(err.error_id(), vw::SECTION_UNAVAILABLE);
    assert_stable_error(err.error_id());

    let looked = root
        .lookup("s:0:0:0")
        .expect("canonical id")
        .expect("slot remains published");
    assert_eq!(looked.presence(), "Unavailable");
    assert_eq!(looked, &before);

    let still_builder = builder
        .freeze()
        .lookup("s:0:0:0")
        .expect("canonical id")
        .expect("builder entry unchanged after failed convert")
        .clone();
    assert_eq!(still_builder, before);

    let pending = SectionSlot::pending();
    let ready = pending
        .try_convert("Ready", Some(sample_payload()))
        .expect("Pending -> Ready with payload is legal");
    assert_eq!(ready.presence(), "Ready");
    assert!(ready.payload().is_some());
}

#[test]
fn freeze_then_builder_insert_does_not_mutate_frozen_root() {
    let mut builder = SectionDirectoryBuilder::new();
    builder
        .insert("s:0:0:0", SectionSlot::unchanged())
        .expect("canonical section id");
    let root_a = builder.freeze();
    let root_a_again = builder.freeze();

    builder
        .insert("s:1:0:0", SectionSlot::pending())
        .expect("canonical section id");
    builder
        .insert("s:0:0:0", SectionSlot::unavailable())
        .expect("replace on builder only");

    assert_eq!(
        root_a
            .lookup("s:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Unchanged"
    );
    assert!(root_a.lookup("s:1:0:0").expect("canonical id").is_none());
    assert_eq!(
        root_a_again
            .lookup("s:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Unchanged"
    );

    let root_b = builder.freeze();
    assert_eq!(
        root_b
            .lookup("s:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Unavailable"
    );
    assert_eq!(
        root_b
            .lookup("s:1:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Pending"
    );

    let mut other = SectionDirectoryBuilder::new();
    other
        .insert("s:0:0:0", SectionSlot::ready(sample_payload()))
        .expect("independent builder");
    let root_other = other.freeze();
    assert_eq!(
        root_other
            .lookup("s:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Ready"
    );
    assert_eq!(
        root_a
            .lookup("s:0:0:0")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Unchanged"
    );
    assert!(root_a.lookup("s:2:2:2").expect("canonical miss").is_none());
}

#[test]
fn bad_section_id_or_payload_hash_mismatch_fails_with_generated_error() {
    let mut builder = SectionDirectoryBuilder::new();
    let slot = SectionSlot::unchanged();

    // 前缀 / 元数 / 规范写法不合 → unknown_section_key。
    for bad in [
        "0:0:0",
        "s:0:0",
        "s:0:0:0:1",
        "s:01:0:0",
        "s:+1:0:0",
        "s:-0:0:0",
        "s:1.0:0:0",
        "C:0:0:0",
        "s:x:y:z",
        "",
    ] {
        let err = builder.insert(bad, slot.clone()).unwrap_err();
        assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY, "id {bad}");
        assert_stable_error(err.error_id());
        let err = SectionDirectoryBuilder::new()
            .freeze()
            .lookup(bad)
            .unwrap_err();
        assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY, "lookup {bad}");
    }

    // 旧式三坐标 c: 键在目录这一层也必须显式拒绝,不得被当成任何一层的键。
    let err = builder.insert("c:0:0:0", slot.clone()).unwrap_err();
    assert_eq!(err.error_id(), vw::UNKNOWN_SECTION_KEY, "legacy c:x:y:z");
    assert!(
        SectionDirectoryBuilder::new()
            .freeze()
            .lookup("c:0:0:0")
            .is_err(),
        "legacy key must not resolve"
    );

    // x/z 越出 int32 → coordinate_out_of_bounds。
    for bad in ["s:2147483648:0:0", "s:-2147483649:0:0"] {
        let err = builder.insert(bad, slot.clone()).unwrap_err();
        assert_eq!(err.error_id(), vw::COORDINATE_OUT_OF_BOUNDS, "id {bad}");
        assert_stable_error(err.error_id());
    }

    // 层号越出 0~15 → section_y_out_of_range。
    for bad in ["s:0:16:0", "s:0:-1:0", "s:0:2147483647:0"] {
        let err = builder.insert(bad, slot.clone()).unwrap_err();
        assert_eq!(err.error_id(), vw::SECTION_Y_OUT_OF_RANGE, "id {bad}");
        assert_stable_error(err.error_id());
    }

    builder
        .insert("s:-1:2:-3", SectionSlot::pending())
        .expect("negative i32 coords are in range");
    builder
        .insert("s:-2147483648:15:2147483647", SectionSlot::unchanged())
        .expect("i32 extremes and the top section layer are in range");
    let root = builder.freeze();
    assert_eq!(
        root.lookup("s:-1:2:-3")
            .expect("canonical id")
            .expect("present")
            .presence(),
        "Pending"
    );

    let bytes = b"dense-page".to_vec();
    let mut bad_digest = sha256(&bytes);
    bad_digest[0] ^= 0xff;
    let err = SectionPayload::from_pages([SectionPage::new("Dense", "None", bytes, bad_digest)])
        .unwrap_err();
    // 契约 payload.digest-before-interpretation:摘要必须先于任何解释校验。
    assert_eq!(err.error_id(), vw::SECTION_DIGEST_MISMATCH);
    assert_stable_error(err.error_id());
}

#[test]
fn section_module_source_has_no_io() {
    let sources = [
        include_str!("../src/section/mod.rs"),
        include_str!("../src/section/payload.rs"),
        include_str!("../src/section/slot.rs"),
        include_str!("../src/section/directory.rs"),
    ];
    for src in sources {
        assert!(
            !src.contains("std::fs"),
            "section module must not use std::fs"
        );
        assert!(!src.contains("::File"), "section module must not use File");
        assert!(
            !src.contains("lumio_voxel_world"),
            "section module must not reference world"
        );
        assert!(
            !src.contains("crate::revision"),
            "section must not call revision as a service"
        );
    }
}
