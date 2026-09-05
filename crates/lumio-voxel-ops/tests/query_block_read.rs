use lumio_voxel_domain::block::{BlockId, BlockState, BlockType, CellOffset};
use lumio_voxel_domain::section::SectionStorage;
use lumio_voxel_ops::query::{
    BlockReadSection, BlockReadWorld, MAX_CELLS_PER_READ_REQUEST, read_box, read_box_into,
    read_cell, read_cell_into, read_column, read_column_into,
};

fn block(raw: u32) -> BlockId {
    BlockId::from_parts(
        BlockType::new(raw).expect("test block type is in range"),
        BlockState::new(0),
    )
}

#[test]
fn box_and_cell_reads_preserve_y_z_x_order_and_revisions() {
    let stone = block(256);
    let mut world = BlockReadWorld::new();
    world
        .insert(
            "s:0:0:0",
            BlockReadSection::ready(12, SectionStorage::uniform(stone)),
        )
        .expect("section key");

    let before = world.clone();
    let result = read_box(&world, (0, 0, 0), (15, 15, 15)).expect("box read");
    assert_eq!(result.cell_count(), 4096);
    assert_eq!(result.segments().len(), 1);
    let segment = &result.segments()[0];
    assert_eq!(segment.section_id(), "s:0:0:0");
    assert_eq!(segment.presence(), "Ready");
    assert_eq!(segment.section_revision(), 12);
    assert_eq!(segment.block_ids().expect("ready ids").len(), 4096);
    assert_eq!(segment.cells()[0].offset(), CellOffset::new(0).unwrap());
    assert_eq!(segment.cells()[1].offset(), CellOffset::new(1).unwrap());
    assert_eq!(segment.cells()[16].offset(), CellOffset::new(16).unwrap());
    assert_eq!(segment.cells()[256].offset(), CellOffset::new(256).unwrap());
    let second = read_box(&world, (0, 0, 0), (15, 15, 15)).expect("repeat box read");
    assert_eq!(result, second, "same request must be deterministic");

    let cell = read_cell(&world, 3_i32, 7_i64, 5_i32).expect("cell read");
    assert_eq!(cell.presence(), "Ready");
    assert_eq!(cell.section_revision(), 12);
    assert_eq!(cell.block_id(), Some(stone));
    assert_eq!(world, before, "proof read must not mutate the source");

    let mut output = vec![None; 4096];
    let mut segment_output = [None; 1];
    let buffered = read_box_into(
        &world,
        (0, 0, 0),
        (15, 15, 15),
        &mut output,
        &mut segment_output,
    )
    .expect("buffered box read");
    assert_eq!(buffered.cell_count(), output.len());
    assert_eq!(buffered.segment_count(), 1);
    assert!(buffered.is_fully_resolved());
    assert_eq!(segment_output[0].unwrap().section_id().key(), "s:0:0:0");
    assert_eq!(segment_output[0].unwrap().first_cell(), 0);
    assert_eq!(segment_output[0].unwrap().cell_count(), output.len());
    assert!(output.iter().all(|value| *value == Some(stone)));
    let mut one = None;
    let one_result = read_cell_into(&world, 3_i32, 7_i64, 5_i32, &mut one).expect("buffered cell");
    assert_eq!(one_result.block_id(), one);
}

#[test]
fn column_and_pending_segments_are_explicit_and_budget_is_hard() {
    let dirt = block(257);
    let mut world = BlockReadWorld::new();
    world
        .insert(
            "s:0:0:0",
            BlockReadSection::unchanged(8, SectionStorage::uniform(dirt)),
        )
        .expect("section key");
    world
        .insert("s:0:1:0", BlockReadSection::pending(9))
        .expect("section key");

    let column = read_column(&world, 2, 3, 0..=31).expect("column read");
    assert_eq!(column.cell_count(), 32);
    assert_eq!(column.segments().len(), 2);
    assert_eq!(column.segments()[0].presence(), "Unchanged");
    assert_eq!(column.segments()[0].section_revision(), 8);
    assert_eq!(column.segments()[0].block_ids().unwrap().len(), 16);
    assert_eq!(column.segments()[1].presence(), "Pending");
    assert_eq!(column.segments()[1].section_revision(), 9);
    assert!(column.segments()[1].block_ids().is_none());
    assert!(
        column.segments()[1]
            .cells()
            .iter()
            .all(|cell| cell.block_id().is_none())
    );

    let mut column_output = vec![None; 32];
    let mut column_segments = [None; 2];
    let buffered = read_column_into(
        &world,
        2,
        3,
        0..=31,
        &mut column_output,
        &mut column_segments,
    )
    .expect("buffered column read");
    assert_eq!(buffered.segment_count(), 2);
    assert!(!buffered.is_fully_resolved());
    assert!(column_output[..16].iter().all(|value| *value == Some(dirt)));
    assert!(column_output[16..].iter().all(Option::is_none));
    let mut untouched = vec![Some(dirt); 32];
    let mut undersized_segments = [None; 1];
    assert!(
        read_column_into(
            &world,
            2,
            3,
            0..=31,
            &mut untouched,
            &mut undersized_segments,
        )
        .is_err()
    );
    assert!(untouched.iter().all(|value| *value == Some(dirt)));
    let mut too_small = vec![None; 31];
    assert!(read_column_into(&world, 2, 3, 0..=31, &mut too_small, &mut column_segments,).is_err());

    let unavailable = read_cell(&world, 16_i32, 0_i64, 0_i32).expect("unavailable segment");
    assert_eq!(unavailable.presence(), "Unavailable");
    assert_eq!(unavailable.section_revision(), 0);
    assert_eq!(unavailable.block_id(), None);

    let err = read_box(&world, (0, 0, 0), (63, 63, 255)).expect_err("over budget");
    assert_eq!(err.error_id(), "read_budget_exceeded");
    assert_eq!(MAX_CELLS_PER_READ_REQUEST, 262_144);
    let mut rejected_output = vec![Some(dirt); MAX_CELLS_PER_READ_REQUEST];
    let err = read_box_into(
        &world,
        (0, 0, 0),
        (63, 63, 255),
        &mut rejected_output,
        &mut [],
    )
    .expect_err("over budget rejects before buffer write");
    assert_eq!(err.error_id(), "read_budget_exceeded");
    assert!(rejected_output.iter().all(|value| *value == Some(dirt)));

    let missing_revision =
        BlockReadSection::from_parts("Ready", None, Some(SectionStorage::uniform(dirt)))
            .expect_err("revision is mandatory");
    assert_eq!(missing_revision.error_id(), "read_result_missing_revision");
}

#[test]
fn duplicate_insert_rejects_without_replacing_the_existing_section() {
    let first = BlockId::from_raw(101);
    let second = BlockId::from_raw(202);
    let mut world = BlockReadWorld::new();
    world
        .insert(
            "s:0:0:0",
            BlockReadSection::ready(1, SectionStorage::uniform(first)),
        )
        .unwrap();
    assert!(
        world
            .insert(
                "s:0:0:0",
                BlockReadSection::ready(2, SectionStorage::uniform(second))
            )
            .is_err()
    );
    let cell = read_cell(&world, 0_i32, 0_i64, 0_i32).unwrap();
    assert_eq!(cell.section_revision(), 1);
    assert_eq!(cell.block_id(), Some(first));
}

#[test]
fn unchanged_section_without_baseline_storage_is_rejected() {
    let error = BlockReadSection::from_parts("Unchanged", Some(7), None)
        .expect_err("Unchanged reads require resolved baseline storage");
    assert_eq!(error.error_id(), "InvalidHandle");
}

#[test]
fn missing_or_unknown_presence_returns_the_contract_error_without_output() {
    let storage = SectionStorage::uniform(block(257));
    for presence in ["", "Unknown"] {
        let error = BlockReadSection::from_parts(presence, Some(7), Some(storage.clone()))
            .expect_err("a cell read must not infer presence from payload");
        assert_eq!(error.error_id(), "cell_read_missing_presence");
    }
}

#[test]
fn invalid_box_y_is_rejected_before_the_output_buffer_is_modified() {
    let sentinel = block(511);
    let mut output = vec![Some(sentinel); 257];
    let mut segments = [None; 17];
    let error = read_box_into(
        &BlockReadWorld::new(),
        (0, 0, 0),
        (0, 256, 0),
        &mut output,
        &mut segments,
    )
    .expect_err("world y=256 is outside the contract domain");
    assert_eq!(error.error_id(), "world_y_out_of_range");
    assert!(output.iter().all(|value| *value == Some(sentinel)));
}
