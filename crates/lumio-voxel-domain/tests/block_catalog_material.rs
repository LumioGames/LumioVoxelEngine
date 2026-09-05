use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{
    BehaviorTemplate, BlockCatalogRowInput, BlockId, BlockResolution, BlockState, BlockTables,
    BlockType, BuiltinBlockType, CollisionBehavior, FaceVisibility, LightAttenuation,
    MaterialClass, MaterialStorage, MeshBehavior, OfficialCatalog, RenderPass, RoomBehaviorInput,
    RoomLocalCatalog, RoomLocalRowInput, StateAccessError, StateFieldSpec, StateLayout,
};

fn row(block_type: u32, name: impl Into<String>) -> BlockCatalogRowInput {
    BlockCatalogRowInput {
        block_type: Some(block_type),
        name: Some(name.into()),
        material_class: Some("Solid".into()),
        behavior_template: Some("FullCube".into()),
        asset_ref: Some("asset://blocks/test".into()),
        state_layout: Some(StateLayout::empty()),
    }
}

fn dense_rows(last: u32) -> Vec<BlockCatalogRowInput> {
    (vw::FIRST_OFFICIAL_BLOCK_TYPE..=last)
        .map(|block_type| row(block_type, format!("lumio.block_{block_type}")))
        .collect()
}

fn assert_code<T: std::fmt::Debug>(
    result: Result<T, lumio_voxel_domain::block::BlockError>,
    code: &str,
) {
    assert_eq!(result.expect_err(code).code(), code);
}

fn resolution_tables() -> BlockTables {
    let official = OfficialCatalog::load(vec![row(256, "lumio.stone")], &[]).unwrap();
    let mut room_local = RoomLocalCatalog::new();
    room_local.register(player_row("player.plank")).unwrap();
    BlockTables::new(official, room_local)
}

#[test]
fn resolver_sentinel_air() {
    let tables = resolution_tables();
    assert_eq!(
        tables.resolve(BlockId::from_parts(BlockType::AIR, BlockState::new(0))),
        Ok(BlockResolution::Builtin(BuiltinBlockType::Air))
    );
}

#[test]
fn resolver_sentinel_error_block() {
    let tables = resolution_tables();
    assert_eq!(
        tables.resolve(BlockId::from_parts(BlockType::ERROR, BlockState::new(0))),
        Ok(BlockResolution::Builtin(BuiltinBlockType::ErrorBlock))
    );
}

#[test]
fn resolver_sentinel_ecs_occupancy() {
    let tables = resolution_tables();
    assert_eq!(
        BlockType::ENTITY_OCCUPANCY_PLACEHOLDER.raw(),
        2,
        "D1 occupancy placeholder is BlockType 2"
    );
    assert_eq!(
        tables.resolve(BlockId::from_parts(
            BlockType::ENTITY_OCCUPANCY_PLACEHOLDER,
            BlockState::new(0),
        )),
        Ok(BlockResolution::Builtin(
            BuiltinBlockType::EntityOccupancyPlaceholder
        ))
    );
}

#[test]
fn resolver_sentinel_structure_placeholder() {
    let tables = resolution_tables();
    assert_eq!(
        tables.resolve(BlockId::from_parts(
            BlockType::STRUCTURE_PLACEHOLDER,
            BlockState::new(0),
        )),
        Ok(BlockResolution::Builtin(
            BuiltinBlockType::StructurePlaceholder
        ))
    );
}

#[test]
fn resolver_reserved_type_is_unregistered() {
    let tables = resolution_tables();
    assert_code(
        tables.resolve(BlockId::from_parts(
            BlockType::new(4).unwrap(),
            BlockState::new(0),
        )),
        vw::UNREGISTERED_BLOCK_TYPE,
    );
}

#[test]
fn resolver_official_registered_row_is_ordinary() {
    let tables = resolution_tables();
    let resolved = tables
        .resolve(BlockId::from_parts(
            BlockType::new(256).unwrap(),
            BlockState::new(0),
        ))
        .unwrap();
    assert!(
        matches!(resolved, BlockResolution::Ordinary(definition) if definition.name() == "lumio.stone")
    );
}

#[test]
fn resolver_official_unregistered_row_is_unregistered() {
    let tables = resolution_tables();
    assert_code(
        tables.resolve(BlockId::from_parts(
            BlockType::new(257).unwrap(),
            BlockState::new(0),
        )),
        vw::UNREGISTERED_BLOCK_TYPE,
    );
}

#[test]
fn resolver_room_local_mapped_row_is_ordinary() {
    let tables = resolution_tables();
    let resolved = tables
        .resolve(BlockId::from_parts(
            BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK).unwrap(),
            BlockState::new(0),
        ))
        .unwrap();
    assert!(
        matches!(resolved, BlockResolution::Ordinary(definition) if definition.name() == "player.plank")
    );
}

#[test]
fn resolver_room_local_unmapped_row_is_unregistered() {
    let tables = resolution_tables();
    assert_code(
        tables.resolve(BlockId::from_parts(
            BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK + 1).unwrap(),
            BlockState::new(0),
        )),
        vw::UNREGISTERED_BLOCK_TYPE,
    );
}

#[test]
fn catalog_row_is_complete_and_dense() {
    let mut catalog = OfficialCatalog::load(dense_rows(4_095), &[]).unwrap();
    let block_type = catalog
        .register(row(4_096, "lumio.new_block"))
        .expect("the next dense row is admitted");

    assert_eq!(block_type.raw(), 4_096);
    let definition = catalog.get(block_type).unwrap().unwrap();
    assert_eq!(definition.name(), "lumio.new_block");
    assert_eq!(definition.material_class(), MaterialClass::Solid);
    assert_eq!(definition.behavior_template(), BehaviorTemplate::FullCube);
    assert_eq!(definition.asset_ref(), "asset://blocks/test");
    assert!(definition.state_layout().fields().is_empty());
}

#[test]
fn catalog_skips_block_type_numbers() {
    let mut catalog = OfficialCatalog::load(dense_rows(4_095), &[]).unwrap();
    assert_code(
        catalog.register(row(5_000, "lumio.gap")),
        vw::BLOCK_CATALOG_NOT_DENSE,
    );
}

#[test]
fn catalog_reuses_a_retired_name() {
    assert_code(
        OfficialCatalog::load(vec![row(256, "lumio.retired")], &["lumio.retired"]),
        vw::BLOCK_CATALOG_NAME_REUSED,
    );
}

#[test]
fn duplicate_live_catalog_name_is_rejected() {
    assert_code(
        OfficialCatalog::load(vec![row(256, "lumio.same"), row(257, "lumio.same")], &[]),
        vw::BLOCK_CATALOG_NAME_REUSED,
    );
}

#[test]
fn official_block_allocated_in_reserved_range() {
    assert_code(
        OfficialCatalog::load(vec![row(7, "lumio.invalid")], &[]),
        vw::SYSTEM_RESERVED_TYPE_MISUSE,
    );
}

#[test]
fn room_local_type_used_as_global() {
    let catalog = OfficialCatalog::load(vec![row(256, "lumio.stone")], &[]).unwrap();
    let player_type = BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK + 7).unwrap();
    assert_code(catalog.get(player_type), vw::BLOCK_TYPE_SCOPE_VIOLATION);
}

#[test]
fn block_catalog_row_incomplete_is_checked_for_each_of_six_fields() {
    for missing in 0..6 {
        let mut input = row(256, "lumio.complete");
        match missing {
            0 => input.block_type = None,
            1 => input.name = None,
            2 => input.material_class = None,
            3 => input.behavior_template = None,
            4 => input.asset_ref = None,
            5 => input.state_layout = None,
            _ => unreachable!(),
        }
        assert_code(
            OfficialCatalog::load(vec![input], &[]),
            vw::BLOCK_CATALOG_ROW_INCOMPLETE,
        );
    }
}

#[test]
fn catalog_missing_material_class_is_structural() {
    let mut input = row(256, "lumio.stone");
    input.material_class = None;
    assert_code(
        OfficialCatalog::load(vec![input], &[]),
        vw::BLOCK_CATALOG_ROW_INCOMPLETE,
    );
}

#[test]
fn catalog_empty_material_class_is_structural() {
    let mut input = row(256, "lumio.stone");
    input.material_class = Some("  ".into());
    assert_code(
        OfficialCatalog::load(vec![input], &[]),
        vw::BLOCK_CATALOG_ROW_INCOMPLETE,
    );
}

#[test]
fn catalog_unknown_non_empty_material_class_is_semantic() {
    let mut input = row(256, "lumio.granite");
    input.material_class = Some("Granite".into());
    assert_code(
        OfficialCatalog::load(vec![input], &[]),
        vw::UNKNOWN_MATERIAL_CLASS,
    );
}

#[test]
fn catalog_missing_field_precedes_unknown_material_class() {
    let mut input = row(256, "lumio.stone");
    input.material_class = Some("Crystal".into());
    input.asset_ref = None;
    assert_code(
        OfficialCatalog::load(vec![input], &[]),
        vw::BLOCK_CATALOG_ROW_INCOMPLETE,
    );
}

#[test]
fn catalog_complete_known_material_class_is_valid() {
    let catalog = OfficialCatalog::load(vec![row(256, "lumio.stone")], &[]).unwrap();
    assert_eq!(
        catalog
            .get(BlockType::new(256).unwrap())
            .unwrap()
            .unwrap()
            .material_class(),
        MaterialClass::Solid
    );
}

#[test]
fn behavior_template_resolves_to_registry() {
    let catalog = OfficialCatalog::load(vec![row(256, "lumio.stone")], &[]).unwrap();
    let definition = catalog.get(BlockType::new(256).unwrap()).unwrap().unwrap();

    assert_eq!(definition.behavior_template(), BehaviorTemplate::FullCube);
    assert!(definition.state_layout().fields().is_empty());
}

#[test]
fn v1_behavior_templates_declare_their_state_layouts() {
    let full_cube = BehaviorTemplate::FullCube.state_layout();
    assert!(full_cube.fields().is_empty());

    let liquid = BehaviorTemplate::Liquid.state_layout();
    let level = liquid
        .field("level")
        .expect("Liquid declares its level field");
    assert_eq!(level.offset(), 0);
    assert_eq!(level.width(), 4);
}

#[test]
fn catalog_references_unlisted_template() {
    let mut input = row(256, "lumio.stairs");
    input.behavior_template = Some("Stairs".into());
    assert_code(
        OfficialCatalog::load(vec![input], &[]),
        vw::UNKNOWN_BEHAVIOR_TEMPLATE,
    );
}

#[test]
fn block_type_without_material_class() {
    let mut input = row(256, "lumio.granite");
    input.material_class = Some("Granite".into());
    assert_code(
        OfficialCatalog::load(vec![input], &[]),
        vw::UNKNOWN_MATERIAL_CLASS,
    );
}

#[test]
fn metal_and_wood_share_one_class() {
    let catalog =
        OfficialCatalog::load(vec![row(256, "lumio.metal"), row(257, "lumio.wood")], &[]).unwrap();

    for raw in [256, 257] {
        assert_eq!(
            catalog
                .get(BlockType::new(raw).unwrap())
                .unwrap()
                .unwrap()
                .material_class(),
            MaterialClass::Solid
        );
    }
}

#[test]
fn material_profiles_declare_all_four_axes_in_one_table() {
    let solid = MaterialClass::Solid.profile();
    assert_eq!(solid.mesh(), MeshBehavior::Solid);
    assert_eq!(solid.render_pass(), RenderPass::Opaque);
    assert_eq!(solid.collision(), CollisionBehavior::Solid);
    assert_eq!(solid.light_attenuation(), LightAttenuation::Opaque);

    let liquid = MaterialClass::Liquid.profile();
    assert_eq!(liquid.mesh(), MeshBehavior::Liquid);
    assert_eq!(liquid.render_pass(), RenderPass::Transparent);
    assert_eq!(liquid.collision(), CollisionBehavior::Passable);
    assert_eq!(liquid.light_attenuation(), LightAttenuation::Attenuating);
}

#[test]
fn water_face_against_air_only() {
    let liquid_mesh = MaterialClass::Liquid.profile().mesh();
    assert_eq!(
        liquid_mesh.face_against(Some(MaterialClass::Liquid)),
        FaceVisibility::Hidden
    );
    assert_eq!(
        liquid_mesh.face_against(Some(MaterialClass::Solid)),
        FaceVisibility::Hidden
    );
    assert_eq!(liquid_mesh.face_against(None), FaceVisibility::Visible);
}

#[test]
fn water_passable_but_queryable() {
    let liquid = MaterialClass::Liquid.profile();
    assert_eq!(liquid.collision(), CollisionBehavior::Passable);
    assert!(liquid.queryable());
}

#[test]
fn light_attenuates_through_water() {
    assert_eq!(
        MaterialClass::Liquid.profile().light_attenuation(),
        LightAttenuation::Attenuating
    );
    assert_eq!(
        MaterialClass::Solid.profile().light_attenuation(),
        LightAttenuation::Opaque
    );
}

#[test]
fn liquid_level_rides_block_state() {
    let layout = StateLayout::new(&[StateFieldSpec::new("level", 4)]).unwrap();
    let state = layout
        .write(BlockState::new(0), "level", 12)
        .expect("four bits hold levels 0 through 15");

    assert_eq!(layout.read(state, "level").unwrap(), 12);
    assert_eq!(state.raw(), 12);
    assert_eq!(
        layout.write(state, "level", 16).unwrap_err(),
        StateAccessError::ValueOutOfRange
    );
}

#[test]
fn dynamic_state_layout_maintains_offsets() {
    let door = StateLayout::new(&[
        StateFieldSpec::new("facing", 2),
        StateFieldSpec::new("open", 1),
        StateFieldSpec::new("hinge", 1),
        StateFieldSpec::new("upper", 1),
    ])
    .unwrap();
    assert_eq!(door.field("facing").unwrap().offset(), 0);
    assert_eq!(door.field("open").unwrap().offset(), 2);
    assert_eq!(door.field("hinge").unwrap().offset(), 3);
    assert_eq!(door.field("upper").unwrap().offset(), 4);

    let state = door.write(BlockState::new(0), "facing", 3).unwrap();
    let state = door.write(state, "open", 1).unwrap();
    assert_eq!(door.read(state, "facing").unwrap(), 3);
    assert_eq!(door.read(state, "open").unwrap(), 1);
}

#[test]
fn new_class_for_texture_only_difference() {
    assert_code(MaterialClass::parse("Granite"), vw::UNKNOWN_MATERIAL_CLASS);
}

#[test]
fn material_class_stored_per_cell() {
    assert_code(
        MaterialStorage::PerCellLane.validate(),
        vw::MATERIAL_CLASS_NOT_A_CELL_LANE,
    );
    MaterialStorage::BlockTypeTable.validate().unwrap();
}

#[test]
fn greedy_merge_across_solid_and_liquid() {
    assert_code(
        MaterialClass::Solid.validate_greedy_merge(MaterialClass::Liquid),
        vw::CROSS_MATERIAL_FACE_MERGE,
    );
    MaterialClass::Solid
        .validate_greedy_merge(MaterialClass::Solid)
        .unwrap();
}

#[test]
fn voxel_system_asked_to_flow_water() {
    assert_code(
        MaterialClass::Liquid.validate_auto_propagation_request(),
        vw::LIQUID_AUTO_PROPAGATION_UNSUPPORTED,
    );
}

#[test]
fn material_class_resolved_per_palette_entry() {
    let catalog = OfficialCatalog::load(dense_rows(295), &[]).unwrap();
    let palette: Vec<BlockId> = (256..296)
        .map(|raw| {
            BlockId::from_parts(
                BlockType::new(raw).unwrap(),
                BlockState::new((raw - 256) as u8),
            )
        })
        .collect();
    let resolved = catalog.resolve_palette(&palette).unwrap();

    assert_eq!(resolved.len(), 40);
    assert!(
        resolved
            .iter()
            .all(|entry| entry.material_class() == MaterialClass::Solid)
    );
}

fn player_row(name: &str) -> RoomLocalRowInput {
    RoomLocalRowInput {
        name: name.into(),
        material_class: "Solid".into(),
        behavior: RoomBehaviorInput::Template("FullCube".into()),
        asset_ref: "asset://player/block".into(),
        state_layout: StateLayout::empty(),
    }
}

#[test]
fn player_block_picks_official_template() {
    let mut catalog = RoomLocalCatalog::new();
    let block_type = catalog.register(player_row("player.plank")).unwrap();
    let definition = catalog.get(block_type).unwrap();

    assert_eq!(block_type.room_local_index(), Some(0));
    assert_eq!(definition.behavior_template(), BehaviorTemplate::FullCube);
    assert_eq!(definition.material_class(), MaterialClass::Solid);
}

#[test]
fn two_dense_tables_resolve_block_ids_by_scope() {
    let official = OfficialCatalog::load(vec![row(256, "lumio.stone")], &[]).unwrap();
    let mut room_local = RoomLocalCatalog::new();
    let player_type = room_local.register(player_row("player.plank")).unwrap();
    let tables = BlockTables::new(official, room_local);

    let official_id = BlockId::from_parts(BlockType::new(256).unwrap(), BlockState::new(3));
    let player_id = BlockId::from_parts(player_type, BlockState::new(4));
    assert_eq!(tables.resolve(official_id).unwrap().name(), "lumio.stone");
    assert_eq!(tables.resolve(player_id).unwrap().name(), "player.plank");
}

#[test]
fn save_references_unmapped_room_local_type() {
    let catalog = RoomLocalCatalog::new();
    let block_type = BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK + 7).unwrap();
    assert_code(
        catalog.get_for_save_mapping(block_type),
        vw::ROOM_LOCAL_TYPE_WITHOUT_MAPPING,
    );
    assert_code(catalog.get(block_type), vw::UNREGISTERED_BLOCK_TYPE);

    let tables = resolution_tables();
    assert_code(
        tables.resolve_for_save_mapping(block_type),
        vw::ROOM_LOCAL_TYPE_WITHOUT_MAPPING,
    );
}

#[test]
fn player_block_declares_new_behavior() {
    let mut input = player_row("player.custom");
    input.behavior = RoomBehaviorInput::Custom;
    let mut catalog = RoomLocalCatalog::new();
    assert_code(catalog.register(input), vw::PLAYER_TYPE_DECLARES_BEHAVIOR);
}

#[test]
fn room_local_type_remapped_on_import() {
    let mut source = RoomLocalCatalog::new();
    let old = source.register(player_row("player.imported")).unwrap();
    let mut target = RoomLocalCatalog::new();
    target.register(player_row("player.existing")).unwrap();

    let remap = target.import_from(&source).unwrap();
    assert_eq!(remap.len(), 1);
    assert_eq!(remap[0].0, old);
    assert_eq!(remap[0].1.room_local_index(), Some(1));
    assert_eq!(target.get(remap[0].1).unwrap().name(), "player.imported");
}
