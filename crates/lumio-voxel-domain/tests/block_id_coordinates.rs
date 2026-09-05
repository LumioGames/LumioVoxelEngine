use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{BlockId, BlockScope, BlockState, BlockType, CellOffset, WorldY};

#[test]
fn block_id_composition() {
    let block_type = BlockType::new(10_007).expect("within uint24");
    let state = BlockState::new(13);
    let id = BlockId::from_parts(block_type, state);

    assert_eq!(id.raw(), (10_007_u32 << 8) | 13);
    assert_eq!(id.block_type(), block_type);
    assert_eq!(id.block_state(), state);
}

#[test]
fn entity_occupancy_placeholder() {
    assert_eq!(BlockType::AIR.raw(), 0);
    assert_eq!(BlockType::ERROR.raw(), 1);
    assert_eq!(BlockType::ENTITY_OCCUPIED.raw(), 2);
    assert_eq!(BlockType::STRUCTURE_PLACEHOLDER.raw(), 3);
}

#[test]
fn scope_bit_separates_official_and_player() {
    let official = BlockType::new(4_096).unwrap();
    let player = BlockType::new(8_388_615).unwrap();

    assert_eq!(official.scope(), BlockScope::Global);
    assert_eq!(official.room_local_index(), None);
    assert_eq!(player.scope(), BlockScope::RoomLocal);
    assert_eq!(player.room_local_index(), Some(7));
}

#[test]
fn official_blocks_start_at_256() {
    assert_eq!(vw::FIRST_OFFICIAL_BLOCK_TYPE, 256);
    assert_eq!(BlockType::new(256).unwrap().scope(), BlockScope::Global);
}

#[test]
fn block_id_is_unsigned() {
    let player = BlockType::new(vw::BLOCK_TYPE_SCOPE_MASK + 7).unwrap();
    let id = BlockId::from_parts(player, BlockState::new(255));

    assert_eq!(id.raw(), 0x8000_07ff);
    assert!(id.raw() > i32::MAX as u32);
    assert_eq!(id.block_type(), player);
}

#[test]
fn world_y_splits_into_section_and_cell() {
    let y = WorldY::new(200).unwrap();
    assert_eq!(y.section_y(), 12);
    assert_eq!(y.cell_y(), 8);
}

#[test]
fn negative_world_y() {
    let err = WorldY::new(-3).expect_err("world y is unsigned");
    assert_eq!(err.error_id(), vw::WORLD_Y_OUT_OF_RANGE);
}

#[test]
fn world_y_above_255() {
    let err = WorldY::new(256).expect_err("world y is uint8");
    assert_eq!(err.error_id(), vw::WORLD_Y_OUT_OF_RANGE);
}

#[test]
fn cell_offset_round_trip() {
    let y = WorldY::new(200).unwrap();
    let offset = CellOffset::from_world(37, y, 19);

    assert_eq!(offset.raw(), 2_101);
    assert_eq!((offset.y(), offset.z(), offset.x()), (8, 3, 5));
}

#[test]
fn cell_offset_bounds() {
    let min = CellOffset::from_world(0, WorldY::new(0).unwrap(), 0);
    let max = CellOffset::from_world(15, WorldY::new(15).unwrap(), 15);

    assert_eq!(min.raw(), 0);
    assert_eq!(max.raw(), 4_095);
    assert_eq!((max.y(), max.z(), max.x()), (15, 15, 15));
}

#[test]
fn cell_offset_out_of_range() {
    let err = CellOffset::new(4_096).expect_err("offset is twelve bits");
    assert_eq!(err.code(), vw::CELL_OFFSET_OUT_OF_RANGE);
}

#[test]
fn cell_offset_with_wrong_stride() {
    let y = WorldY::new(200).unwrap();
    let wrong = ((37_i32 & 15) * 256 + (19_i32 & 15) * 16 + (200_i32 & 15)) as u16;
    let err = CellOffset::validate_for_world(wrong, 37, y, 19)
        .expect_err("x/y-swapped stride must not be accepted for these coordinates");

    assert_eq!(err.code(), vw::CELL_OFFSET_OUT_OF_RANGE);
}
