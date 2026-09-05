use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{
    BlockId, BlockScope, BlockState, BlockType, CellOffset, StateAccessError, StateFieldSpec,
    StateLayout, StateLayoutError,
};
use lumio_voxel_domain::key::WorldY;

#[test]
fn block_id_round_trips_unsigned_parts() {
    let block_type = BlockType::new(10_007).expect("24-bit block type");
    let block_state = BlockState::new(13);
    let id = BlockId::from_parts(block_type, block_state);

    assert_eq!(id.raw(), (10_007_u32 << 8) | 13);
    assert_eq!(id.block_type(), block_type);
    assert_eq!(id.block_state(), block_state);
}

#[test]
fn room_local_block_ids_remain_unsigned() {
    let block_type = BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK + 7).unwrap();
    let id = BlockId::from_parts(block_type, BlockState::new(vw::BLOCK_STATE_MAX as u8));

    assert!(id.raw() > i32::MAX as u32);
    assert_eq!(id.block_type(), block_type);
}

#[test]
fn scope_bit_selects_global_or_room_local_and_extracts_local_index() {
    let global = BlockType::new(4_096).unwrap();
    let room_local = BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK + 7).unwrap();

    assert_eq!(global.scope(), BlockScope::Global);
    assert_eq!(global.room_local_index(), None);
    assert_eq!(room_local.scope(), BlockScope::RoomLocal);
    assert_eq!(room_local.room_local_index(), Some(7));
    assert!(BlockType::new(vw::BLOCK_TYPE_MAX + 1).is_none());
}

#[test]
fn world_y_is_shared_with_section_key_conventions() {
    let y = WorldY::new(200).unwrap();
    assert_eq!(y.section_y(), 12);
    assert_eq!(y.cell_y(), 8);
    assert_eq!(
        WorldY::new(-3).unwrap_err().error_id(),
        vw::WORLD_Y_OUT_OF_RANGE
    );
    assert_eq!(
        WorldY::new(256).unwrap_err().error_id(),
        vw::WORLD_Y_OUT_OF_RANGE
    );
}

#[test]
fn cell_offset_uses_y_z_x_strides_and_inverts() {
    let y = WorldY::new(200).unwrap();
    let offset = CellOffset::from_world(37, y, 19);

    assert_eq!(offset.raw(), 2_101);
    assert_eq!((offset.y(), offset.z(), offset.x()), (8, 3, 5));
    assert_eq!(
        CellOffset::validate_for_world(offset.raw(), 37, y, 19).unwrap(),
        offset
    );
}

#[test]
fn cell_offset_is_bounded_to_one_section() {
    let min = CellOffset::from_world(0, WorldY::new(0).unwrap(), 0);
    let max = CellOffset::from_world(15, WorldY::new(15).unwrap(), 15);

    assert_eq!(min.raw(), vw::CELL_OFFSET_MIN);
    assert_eq!(max.raw(), vw::CELL_OFFSET_MAX);
    assert_eq!(
        CellOffset::new(vw::CELL_OFFSET_MAX + 1).unwrap_err().code(),
        vw::CELL_OFFSET_OUT_OF_RANGE
    );
}

#[test]
fn cell_offset_rejects_a_different_stride_for_the_same_coordinates() {
    let y = WorldY::new(200).unwrap();
    let wrong = ((37_i32 & 15) * 256 + (19_i32 & 15) * 16 + (200_i32 & 15)) as u16;

    assert_eq!(
        CellOffset::validate_for_world(wrong, 37, y, 19)
            .unwrap_err()
            .code(),
        vw::CELL_OFFSET_OUT_OF_RANGE
    );
}

#[test]
fn dynamic_state_layout_tracks_offsets_and_reads_and_writes_fields() {
    let layout = StateLayout::new(&[
        StateFieldSpec::new("facing", 2),
        StateFieldSpec::new("open", 1),
        StateFieldSpec::new("hinge", 1),
        StateFieldSpec::new("upper", 1),
    ])
    .unwrap();

    assert_eq!(layout.field("facing").unwrap().offset(), 0);
    assert_eq!(layout.field("open").unwrap().offset(), 2);
    assert_eq!(layout.field("hinge").unwrap().offset(), 3);
    assert_eq!(layout.field("upper").unwrap().offset(), 4);

    let state = layout.write(BlockState::new(0), "facing", 3).unwrap();
    let state = layout.write(state, "open", 1).unwrap();
    assert_eq!(layout.read(state, "facing").unwrap(), 3);
    assert_eq!(layout.read(state, "open").unwrap(), 1);
    assert_eq!(
        layout.write(state, "facing", 4).unwrap_err(),
        StateAccessError::ValueOutOfRange
    );
    assert_eq!(
        layout.read(state, "missing").unwrap_err(),
        StateAccessError::UnknownField
    );
}

#[test]
fn state_layout_rejects_invalid_field_declarations() {
    assert_eq!(
        StateLayout::new(&[StateFieldSpec::new("", 1)]).unwrap_err(),
        StateLayoutError::EmptyName
    );
    assert_eq!(
        StateLayout::new(&[
            StateFieldSpec::new("level", 4),
            StateFieldSpec::new("level", 1),
        ])
        .unwrap_err(),
        StateLayoutError::DuplicateName
    );
    assert_eq!(
        StateLayout::new(&[StateFieldSpec::new("empty", 0)]).unwrap_err(),
        StateLayoutError::ZeroWidth
    );
    assert_eq!(
        StateLayout::new(&[StateFieldSpec::new("too_wide", 9)]).unwrap_err(),
        StateLayoutError::WidthOverflow
    );
}
