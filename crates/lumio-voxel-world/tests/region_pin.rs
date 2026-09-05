//! R-00440: region pin declaration, readiness, budgets, and release semantics.

use lumio_voxel_domain::block::{BlockId, BlockState, BlockType};
use lumio_voxel_domain::section::SectionStorage;
use lumio_voxel_ops::query::{BlockReadSection, BlockReadWorld};
use lumio_voxel_project::physics_query::{
    MaterialClass, MaterialMask, MaterialTable, PhysicsQuery, PhysicsWorld, QueryResolution, Vec3,
};
use lumio_voxel_world::world::{
    PinBudget, PinExemptionHook, PinReadiness, RegionPinError, RegionPinManager, RegionPinStatus,
    section_keys_for_region,
};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn whole_61_by_61_by_2_map_expands_to_sixteen_sections() {
    let sections =
        section_keys_for_region(0, 60, 0, 1, 0, 60, PinBudget::new(16, 16)).expect("region");
    assert_eq!(sections.len(), 16);

    let mut pins = RegionPinManager::with_budgets(16, 16);
    let pin = pins
        .declare_pin(sections.clone())
        .expect("budgeted declaration");
    assert_eq!(
        pins.status(pin).expect("status").readiness(),
        PinReadiness::NotReady
    );
    assert!(
        pins.settle(pin).is_err(),
        "gameplay must wait for readiness"
    );

    pins.mark_ready(pin).expect("ready signal");
    assert_eq!(
        pins.status(pin).expect("status").readiness(),
        PinReadiness::Ready
    );
    for section in sections {
        assert!(pins.is_pinned(&section).expect("section"));
        pins.validate_pinned_read(pin, &section, "Ready")
            .expect("ready pin read");
        pins.validate_physics_result(pin, &section, "Ready")
            .expect("ready pin physics");
        assert!(
            !pins
                .streaming_priority_eligible(&section)
                .expect("priority")
        );
    }
}

#[test]
fn readiness_gate_allows_pending_before_ready_but_rejects_pending_after_ready() {
    let mut pins = RegionPinManager::with_budgets(1, 1);
    let pin = pins.declare_pin(["s:0:0:0"]).expect("declaration");
    pins.validate_pinned_read(pin, "s:0:0:0", "Pending")
        .expect("pending is valid before ready");
    assert_eq!(
        pins.settle(pin).unwrap_err().error_id(),
        "pin_region_not_ready"
    );

    pins.mark_ready(pin).expect("ready signal");
    let error = pins
        .validate_pinned_read(pin, "s:0:0:0", "Pending")
        .expect_err("pending after ready");
    assert_eq!(error.error_id(), "pinned_read_returned_pending");
}

#[test]
fn budget_failure_is_atomic_and_does_not_partially_pin() {
    let mut pins = RegionPinManager::with_budgets(2, 3);
    let error = pins
        .declare_pin(["s:0:0:0", "s:1:0:0", "s:2:0:0"])
        .expect_err("caller budget");
    assert_eq!(error.error_id(), "residency_pin_exceeds_budget");
    assert_eq!(pins.active_pin_count(), 0);
    assert_eq!(pins.pinned_section_count(), 0);
    assert!(!pins.is_pinned("s:0:0:0").expect("section"));

    let first = pins.declare_pin(["s:0:0:0"]).expect("first declaration");
    let error = pins
        .declare_pin(["s:1:0:0", "s:2:0:0"])
        .expect_err("aggregate budget");
    assert_eq!(error.error_id(), "residency_pin_exceeds_budget");
    assert_eq!(pins.active_pin_count(), 1);
    assert_eq!(pins.pinned_section_count(), 1);
    assert!(pins.is_pinned("s:0:0:0").expect("existing pin"));
    assert_eq!(pins.status(first).expect("status").section_count(), 1);

    let mut pins = RegionPinManager::with_budgets(4, 2);
    let error = pins
        .declare_pin(["s:0:0:0", "s:1:0:0", "s:2:0:0"])
        .expect_err("host budget");
    assert_eq!(error.error_id(), "residency_pin_exceeds_budget");
    assert_eq!(pins.active_pin_count(), 0);
    assert_eq!(pins.pinned_section_count(), 0);
}

#[test]
fn declaration_stops_consuming_after_the_effective_budget() {
    let consumed = Rc::new(Cell::new(0));
    let counter = Rc::clone(&consumed);
    let input = (0..1_000_000).map(move |x| {
        counter.set(counter.get() + 1);
        format!("s:{x}:0:0")
    });
    let mut pins = RegionPinManager::with_budgets(2, 2);
    let error = pins.declare_pin(input).expect_err("budget is a hard fence");
    assert_eq!(error.error_id(), "residency_pin_exceeds_budget");
    assert!(
        consumed.get() <= 3,
        "input must not be drained before rejection"
    );
    assert_eq!(pins.active_pin_count(), 0);
}

#[test]
fn enormous_region_is_rejected_before_expansion() {
    let error = section_keys_for_region(
        i32::MIN,
        i32::MAX,
        0,
        255,
        i32::MIN,
        i32::MAX,
        PinBudget::new(262_144, 262_144),
    )
    .expect_err("region exceeds the injected pin budget");
    assert_eq!(error.error_id(), "residency_pin_exceeds_budget");
}

#[test]
fn pin_hook_blocks_clean_unload_until_release_then_restores_ordinary_residency() {
    let mut pins = RegionPinManager::with_budgets(1, 1);
    let pin = pins.declare_pin(["s:0:0:0"]).expect("declaration");
    pins.mark_ready(pin).expect("ready signal");

    let error = pins
        .check_pin_exemption("s:0:0:0")
        .expect_err("active pin is stronger than clean unload");
    assert_eq!(error.error_id(), "pinned_section_evicted");

    pins.release_pin(pin).expect("release");
    assert_eq!(
        pins.status(pin).expect("status").readiness(),
        PinReadiness::Released
    );
    pins.check_pin_exemption("s:0:0:0")
        .expect("released pin restores normal residency");
    assert!(
        pins.streaming_priority_eligible("s:0:0:0")
            .expect("priority")
    );
}

#[test]
fn status_reports_declared_section_count_and_duplicate_keys_are_rejected() {
    let mut pins = RegionPinManager::with_budgets(4, 4);
    let error = pins
        .declare_pin(["s:0:0:0", "s:0:0:0"])
        .expect_err("duplicate section");
    assert_eq!(error.error_id(), "InvalidHandle");
    assert_eq!(pins.active_pin_count(), 0);

    let pin = pins
        .declare_pin(["s:0:0:0", "s:1:0:0"])
        .expect("declaration");
    let status: RegionPinStatus = pins.status(pin).expect("status");
    assert_eq!(status.section_count(), 2);
    assert_eq!(status.readiness(), PinReadiness::NotReady);
    assert_eq!(pins.pinned_section_count(), 2);
    assert_eq!(
        RegionPinError::pin_region_not_ready().error_id(),
        "pin_region_not_ready"
    );
}

#[test]
fn ready_pin_guards_same_tick_block_and_physics_reads_after_residency_changes() {
    let section_id = "s:0:0:0";
    let block = BlockId::from_parts(BlockType::new(1).unwrap(), BlockState::new(0));
    let storage = SectionStorage::uniform(block);
    let mut pins = RegionPinManager::with_budgets(1, 1);
    let pin = pins.declare_pin([section_id]).expect("pin declaration");
    pins.mark_ready(pin).expect("ready pin");

    // Both consumers observe the same ready Section cut and accept it.
    let reads =
        BlockReadWorld::from_sections([(section_id, BlockReadSection::ready(4, storage.clone()))])
            .expect("block read world");
    let mut physical = PhysicsWorld::new();
    physical.insert_ready(
        lumio_voxel_domain::key::SectionId::new(0, 0, 0).unwrap(),
        storage,
    );
    let materials = MaterialTable::default().with(block.block_type(), MaterialClass::Solid);
    reads
        .read_cell_with_presence_guard(0_i32, 0_i64, 0_i32, &pins)
        .expect("ready pinned cell read");
    let result = PhysicsQuery::with_presence_guard(&physical, &materials, &pins)
        .raycast(
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            MaterialMask::solid(),
        )
        .expect("ready pinned physics read");
    assert!(matches!(result, QueryResolution::Hit(_)));

    // A later residency update cannot be surfaced as a missing result for a
    // Section whose pin was already ready; both paths reject it atomically.
    let pending_reads = BlockReadWorld::from_sections([(section_id, BlockReadSection::pending(5))])
        .expect("pending block read world");
    assert_eq!(
        pending_reads
            .read_cell_with_presence_guard(0_i32, 0_i64, 0_i32, &pins)
            .unwrap_err()
            .error_id(),
        "pinned_read_returned_pending"
    );
    let mut pending_physics = PhysicsWorld::new();
    pending_physics.insert_pending(lumio_voxel_domain::key::SectionId::new(0, 0, 0).unwrap());
    let error = PhysicsQuery::with_presence_guard(&pending_physics, &materials, &pins)
        .raycast(
            Vec3::new(0.5, 0.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            1.0,
            MaterialMask::solid(),
        )
        .unwrap_err();
    assert_eq!(error.error_id(), "pinned_read_returned_pending");
}

#[test]
fn r00440_same_tick_61_by_61_by_2_read_and_physics_stay_ready() {
    let sections = section_keys_for_region(0, 60, 0, 1, 0, 60, PinBudget::new(16, 16))
        .expect("R-00440 region");
    let block = BlockId::from_parts(BlockType::new(1).unwrap(), BlockState::new(0));
    let storage = SectionStorage::uniform(block);
    let mut pins = RegionPinManager::with_budgets(16, 16);
    let pin = pins.declare_pin(sections.clone()).expect("region pin");
    pins.mark_ready(pin).expect("all sections are ready");

    let read_sections = sections
        .iter()
        .map(|id| (id.as_str(), BlockReadSection::ready(7, storage.clone())));
    let reads = BlockReadWorld::from_sections(read_sections).expect("read cut");
    let read = reads
        .read_box_with_presence_guard((0, 0, 0), (60, 1, 60), &pins)
        .expect("pinned block read");
    assert_eq!(read.cell_count(), 61 * 2 * 61);
    assert!(read.is_fully_resolved());

    let mut physics = PhysicsWorld::new();
    for id in &sections {
        let parsed = lumio_voxel_domain::key::SectionId::parse(id).unwrap();
        physics.insert_ready(parsed, storage.clone());
    }
    let materials = MaterialTable::default().with(block.block_type(), MaterialClass::Solid);
    let query = PhysicsQuery::with_presence_guard(&physics, &materials, &pins);
    let mut output = [Default::default(); 1];
    let result = query
        .overlap(
            lumio_voxel_project::physics_query::Aabb::new(
                Vec3::new(30.5, 1.0, 30.5),
                Vec3::new(30.49, 1.0, 30.49),
            ),
            MaterialMask::solid(),
            &mut output,
        )
        .expect("pinned physics read");
    assert_eq!(result.actual_count(), 61 * 2 * 61);
    assert!(result.truncated());
    assert!(result.resolution().is_hit());
}
