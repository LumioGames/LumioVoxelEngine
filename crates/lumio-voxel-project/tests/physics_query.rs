use lumio_voxel_domain::block::{BlockId, BlockState, BlockType, WorldY};
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::section::SectionStorage;
use lumio_voxel_project::physics_query::{
    Aabb, BodyId, HitTarget, MaterialClass, MaterialClassLookup, MaterialMask, MaterialTable,
    PhysicsQuery, PhysicsQueryError, PhysicsWorld, QueryResolution, Vec3, verify_deterministic,
};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
struct Materials(BTreeMap<u32, MaterialClass>);

impl Materials {
    fn with(mut self, block_id: BlockId, class: MaterialClass) -> Self {
        self.0.insert(block_id.block_type().raw(), class);
        self
    }
}

impl MaterialClassLookup for Materials {
    fn class_for(&self, block_type: BlockType) -> Option<MaterialClass> {
        self.0.get(&block_type.raw()).copied()
    }
}

fn block(raw: u32) -> BlockId {
    BlockId::from_parts(
        BlockType::new(raw).expect("test BlockType is in range"),
        BlockState::new(0),
    )
}

fn section(x: i32, y: i64, z: i32) -> SectionId {
    SectionId::new(x, y, z).unwrap()
}

fn wall_world() -> PhysicsWorld {
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(3, WorldY::new(1).unwrap(), 0, block(1));
    world.insert_ready(section(0, 0, 0), storage);
    world
}

fn materials() -> Materials {
    Materials::default()
        .with(block(1), MaterialClass::Solid)
        .with(block(2), MaterialClass::Liquid)
}

#[test]
fn raycast_uses_dda_and_returns_block_granular_hit() {
    let world = wall_world();
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);

    let result = query
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            10.0,
            MaterialMask::solid(),
        )
        .unwrap();
    let hit = match result {
        QueryResolution::Hit(hit) => hit,
        other => panic!("expected hit, got {other:?}"),
    };
    assert_eq!(hit.cell().x(), 3);
    assert_eq!(hit.cell().y(), 1);
    assert_eq!(hit.cell().z(), 0);
    assert_eq!(hit.block_id(), Some(block(1)));
    assert_eq!(hit.normal(), Vec3::new(-1.0, 0.0, 0.0));
    assert!((hit.distance() - 2.5).abs() < f32::EPSILON);
}

#[test]
fn unresolved_is_distinct_and_carries_section_key() {
    let mut world = PhysicsWorld::new();
    world.insert_ready(section(0, 0, 0), SectionStorage::uniform(block(0)));
    world.insert_pending(section(1, 0, 0));
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);
    let result = query
        .raycast(
            Vec3::new(15.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            4.0,
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(result.unresolved_section_key().as_deref(), Some("s:1:0:0"));
    assert!(matches!(result, QueryResolution::Unresolved { .. }));
    assert_ne!(result, QueryResolution::Miss);
    assert_eq!(
        result.interpret_as_miss().unwrap_err().error_id(),
        "unresolved_hit_treated_as_air"
    );
    assert_eq!(
        result.interpret_as_hit().unwrap_err().error_id(),
        "unresolved_hit_treated_as_solid"
    );
}

#[test]
fn material_filter_is_injected_and_liquid_is_queryable_but_not_solid() {
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(1, WorldY::new(1).unwrap(), 0, block(2));
    storage.write_world(2, WorldY::new(1).unwrap(), 0, block(1));
    world.insert_ready(section(0, 0, 0), storage);
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);

    let solid = query
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            5.0,
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(solid.hit().unwrap().block_id(), Some(block(1)));

    let liquid = query
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            5.0,
            MaterialMask::liquid(),
        )
        .unwrap();
    assert_eq!(liquid.hit().unwrap().block_id(), Some(block(2)));
}

#[test]
fn overlap_reports_actual_count_and_truncation_without_allocating_results() {
    let mut world = PhysicsWorld::new();
    world.insert_ready(section(0, 0, 0), SectionStorage::uniform(block(1)));
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);
    let mut buffer = [Default::default(); 3];
    let result = query
        .overlap(
            Aabb::new(Vec3::new(2.0, 0.5, 2.0), Vec3::new(2.0, 0.5, 2.0)),
            MaterialMask::solid(),
            &mut buffer,
        )
        .unwrap();
    assert_eq!(result.actual_count(), 16);
    assert_eq!(result.written_count(), 3);
    assert!(result.truncated());
    assert_eq!(
        result.require_complete().unwrap_err().error_id(),
        "query_buffer_overflow"
    );
}

#[test]
fn sweep_returns_fraction_in_range_and_does_not_apply_response() {
    let world = wall_world();
    let before = world.clone();
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);
    let result = query
        .sweep(
            Aabb::new(Vec3::new(0.5, 1.5, 0.5), Vec3::new(0.4, 0.4, 0.4)),
            Vec3::new(4.0, 0.0, 0.0),
            MaterialMask::solid(),
        )
        .unwrap();
    let hit = result.hit().expect("wall collision");
    assert!((0.0..=1.0).contains(&hit.fraction()));
    assert!((hit.fraction() - 0.525).abs() < 0.001);
    assert_eq!(world, before);
}

#[test]
fn sweep_orders_proven_hits_before_farther_unresolved_sections() {
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(2, WorldY::new(1).unwrap(), 0, block(1));
    world.insert_ready(section(0, 0, 0), storage);
    world.insert_pending(section(1, 0, 0));
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);

    let result = query
        .sweep(
            Aabb::new(Vec3::new(0.5, 1.5, 0.5), Vec3::new(0.4, 0.4, 0.4)),
            Vec3::new(20.0, 0.0, 0.0),
            MaterialMask::solid(),
        )
        .unwrap();
    let hit = result
        .hit()
        .expect("known near hit must win over farther pending section");
    assert_eq!(hit.cell().x(), 2);

    let mut unresolved_world = PhysicsWorld::new();
    unresolved_world.insert_pending(section(0, 0, 0));
    let unresolved_query = PhysicsQuery::new(&unresolved_world, &materials);
    let unresolved = unresolved_query
        .sweep(
            Aabb::new(Vec3::new(0.5, 1.5, 0.5), Vec3::new(0.4, 0.4, 0.4)),
            Vec3::new(20.0, 0.0, 0.0),
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(
        unresolved.unresolved_section_key().as_deref(),
        Some("s:0:0:0")
    );
}

#[test]
fn repeated_queries_and_independent_instances_are_deterministic() {
    let world = wall_world();
    let other = world.clone();
    let materials = materials();
    let query_a = PhysicsQuery::new(&world, &materials);
    let query_b = PhysicsQuery::new(&other, &materials);
    let a = query_a
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            10.0,
            MaterialMask::solid(),
        )
        .unwrap();
    let b = query_b
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            10.0,
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(a, b);
    verify_deterministic(&a, &b).unwrap();
}

#[test]
fn query_rejection_codes_are_contract_stable() {
    assert_eq!(
        PhysicsQueryError::query_result_divergence().error_id(),
        "query_result_divergence"
    );
    assert_eq!(
        PhysicsQueryError::query_mutates_world().error_id(),
        "query_mutates_world"
    );
}

#[test]
fn custom_block_behavior_is_supplied_by_material_lookup() {
    let custom = block(0x00ab_cdef);
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(1, WorldY::new(1).unwrap(), 0, custom);
    world.insert_ready(section(0, 0, 0), storage);
    let materials = Materials::default().with(custom, MaterialClass::Solid);
    let query = PhysicsQuery::new(&world, &materials);
    let result = query
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            3.0,
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(result.hit().unwrap().block_id(), Some(custom));
}

#[test]
fn material_table_classifies_all_states_by_block_type() {
    let block_type = BlockType::new(77).unwrap();
    let state_zero = BlockId::from_parts(block_type, BlockState::new(0));
    let state_one = BlockId::from_parts(block_type, BlockState::new(1));
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(1, WorldY::new(1).unwrap(), 0, state_one);
    world.insert_ready(section(0, 0, 0), storage);
    let materials = MaterialTable::default().with(state_zero.block_type(), MaterialClass::Solid);
    let query = PhysicsQuery::new(&world, &materials);
    assert!(
        query
            .raycast(
                Vec3::new(0.5, 1.5, 0.5),
                Vec3::new(1.0, 0.0, 0.0),
                3.0,
                MaterialMask::solid(),
            )
            .unwrap()
            .is_hit()
    );
}

#[test]
fn oversized_overlap_is_rejected_before_unbounded_iteration() {
    let world = PhysicsWorld::new();
    let materials = MaterialTable::default();
    let mut buffer = [Default::default(); 1];
    let error = PhysicsQuery::new(&world, &materials)
        .overlap(
            Aabb::new(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10_000.0, 10_000.0, 10_000.0),
            ),
            MaterialMask::all(),
            &mut buffer,
        )
        .unwrap_err();
    assert_eq!(error.error_id(), "query_buffer_overflow");
}

#[test]
fn registered_non_voxel_body_is_identified_separately() {
    let mut world = PhysicsWorld::new();
    world.insert_ready(section(0, 0, 0), SectionStorage::uniform(block(0)));
    world.register_body(
        BodyId::new(7),
        Aabb::new(Vec3::new(2.0, 1.5, 0.5), Vec3::new(0.25, 0.25, 0.25)),
    );
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);
    let result = query
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            5.0,
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(
        result.hit().unwrap().target(),
        HitTarget::Body(BodyId::new(7))
    );
    assert_eq!(result.hit().unwrap().block_id(), None);
}

#[test]
fn negative_boundary_ray_enters_the_lower_cell() {
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(0, WorldY::new(1).unwrap(), 0, block(1));
    world.insert_ready(section(0, 0, 0), storage);
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);
    let result = query
        .raycast(
            Vec3::new(1.0, 1.5, 0.5),
            Vec3::new(-1.0, 0.0, 0.0),
            2.0,
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(result.hit().unwrap().cell().x(), 0);
    assert_eq!(result.hit().unwrap().normal(), Vec3::new(1.0, 0.0, 0.0));
}

#[test]
fn raycast_from_above_enters_the_world_y_slab() {
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(0, WorldY::new(255).unwrap(), 0, block(1));
    world.insert_ready(section(0, 15, 0), storage);
    let materials = materials();

    let result = PhysicsQuery::new(&world, &materials)
        .raycast(
            Vec3::new(0.5, 256.5, 0.5),
            Vec3::new(0.0, -1.0, 0.0),
            2.0,
            MaterialMask::solid(),
        )
        .unwrap();
    let hit = result.hit().expect("ray enters the top world cell");
    assert_eq!(hit.cell().y(), 255);
    assert!((hit.distance() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn all_queries_leave_the_world_unchanged() {
    let world = wall_world();
    let before = world.clone();
    let materials = materials();
    let query = PhysicsQuery::new(&world, &materials);
    let _ = query
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            4.0,
            MaterialMask::solid(),
        )
        .unwrap();
    let mut buffer = [Default::default(); 4];
    let _ = query
        .overlap(
            Aabb::new(Vec3::new(1.5, 1.5, 0.5), Vec3::new(1.0, 0.5, 0.5)),
            MaterialMask::solid(),
            &mut buffer,
        )
        .unwrap();
    let _ = query
        .sweep(
            Aabb::new(Vec3::new(0.5, 1.5, 0.5), Vec3::new(0.4, 0.4, 0.4)),
            Vec3::new(4.0, 0.0, 0.0),
            MaterialMask::solid(),
        )
        .unwrap();
    assert_eq!(world, before);
}

#[test]
fn missing_material_mapping_is_rejected_instead_of_treated_as_empty() {
    let mut world = PhysicsWorld::new();
    let mut storage = SectionStorage::uniform(block(0));
    storage.write_world(1, WorldY::new(1).unwrap(), 0, block(99));
    world.insert_ready(section(0, 0, 0), storage);

    let error = PhysicsQuery::new(&world, &MaterialTable::default())
        .raycast(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(1.0, 0.0, 0.0),
            3.0,
            MaterialMask::all(),
        )
        .expect_err("an undeclared BlockType must not be interpreted as empty");
    assert_eq!(error.error_id(), "unknown_material_class");
}
