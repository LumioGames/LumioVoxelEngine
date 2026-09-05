use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{BlockId, CellOffset, WorldY};
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::section::{
    PaletteCapacityAction, SectionEncoding, SectionPayloadEnvelope, SectionStorage,
};
use std::mem::size_of;
use std::thread;

fn block(raw: u32) -> BlockId {
    BlockId::from_raw(raw)
}

fn cells_with_kinds(kind_count: u32) -> Vec<BlockId> {
    (0..vw::SECTION_CELLS)
        .map(|cell| block(1_000 + cell % kind_count))
        .collect()
}

#[test]
fn uniform_section_is_one_value() {
    let stone = block(1_000);
    let storage = SectionStorage::uniform(stone);

    assert_eq!(storage.encoding(), SectionEncoding::Uniform);
    assert_eq!(storage.resident_cell_bytes(), 0);
    assert!(size_of::<SectionStorage>() < vw::SECTION_CELLS as usize);
    assert_eq!(storage.read(CellOffset::new(0).unwrap()), stone);
    assert_eq!(
        storage.read(CellOffset::new(vw::CELL_OFFSET_MAX).unwrap()),
        stone
    );

    let envelope =
        SectionPayloadEnvelope::encode_full(SectionId::new(0, 0, 0).unwrap(), 1, &storage);
    assert_eq!(envelope.encoding(), SectionEncoding::Uniform);
    assert_eq!(envelope.payload_length(), 4);
}

#[test]
fn palette_section_at_cap() {
    let storage = SectionStorage::from_cells(&cells_with_kinds(256)).unwrap();

    assert_eq!(storage.encoding(), SectionEncoding::Palette);
    assert_eq!(storage.palette_entry_count(), Some(256));
    assert_eq!(storage.resident_cell_bytes(), vw::SECTION_CELLS as usize);
}

#[test]
fn raw_section_above_cap() {
    let storage = SectionStorage::from_cells(&cells_with_kinds(257)).unwrap();

    assert_eq!(storage.encoding(), SectionEncoding::Raw);
    assert_eq!(storage.palette_entry_count(), None);
    assert_eq!(
        storage.resident_cell_bytes(),
        vw::SECTION_CELLS as usize * size_of::<BlockId>()
    );
    let envelope =
        SectionPayloadEnvelope::encode_full(SectionId::new(0, 0, 0).unwrap(), 1, &storage);
    assert_eq!(envelope.encoding(), SectionEncoding::Raw);
    assert_eq!(envelope.payload_length(), 16_384);
    assert_eq!(envelope.decode(None).unwrap().storage(), &storage);
}

#[test]
fn raw_storage_downgrades_transparently_when_kinds_fall_to_palette_capacity() {
    let mut storage = SectionStorage::from_cells(&cells_with_kinds(257)).unwrap();
    for offset in 0..vw::SECTION_CELLS as u16 {
        if offset % 257 == 256 {
            storage.write(CellOffset::new(offset).unwrap(), block(1_000));
        }
    }

    assert_eq!(storage.encoding(), SectionEncoding::Palette);
    for offset in 0..vw::SECTION_CELLS as u16 {
        let kind = u32::from(offset % 257);
        let expected = if kind == 256 { 1_000 } else { 1_000 + kind };
        assert_eq!(
            storage.read(CellOffset::new(offset).unwrap()),
            block(expected)
        );
    }
}

#[test]
fn repeated_place_and_remove_of_three_hundred_kinds_stays_palette() {
    let base = block(500);
    let mut storage = SectionStorage::uniform(base);
    let offset = CellOffset::new(0).unwrap();

    for raw in 1_000..1_300 {
        storage.write(offset, block(raw));
        storage.write(offset, base);
    }

    assert_eq!(storage.encoding(), SectionEncoding::Palette);
    assert_eq!(storage.palette_entry_count(), Some(256));
    assert_eq!(storage.read(offset), base);
}

#[test]
fn dead_palette_slot_reused_at_capacity() {
    let mut storage = SectionStorage::from_cells(&cells_with_kinds(256)).unwrap();
    for offset in 0..vw::SECTION_CELLS as u16 {
        let slot = offset % 256;
        if (1..=3).contains(&slot) || (slot == 4 && offset != 4) {
            storage.write(CellOffset::new(offset).unwrap(), block(1_000));
        }
    }

    let outcome = storage.write(CellOffset::new(4).unwrap(), block(99_999));

    assert_eq!(storage.encoding(), SectionEncoding::Palette);
    assert_eq!(storage.resident_cell_bytes(), vw::SECTION_CELLS as usize);
    assert_eq!(
        outcome.palette_capacity_action(),
        PaletteCapacityAction::ReusedDeadSlot {
            cell_index_changed: false,
        }
    );
}

#[test]
fn escalate_to_raw_only_when_all_alive() {
    let mut storage = SectionStorage::from_cells(&cells_with_kinds(256)).unwrap();

    let outcome = storage.write(CellOffset::new(0).unwrap(), block(99_999));

    assert_eq!(storage.encoding(), SectionEncoding::Raw);
    assert_eq!(
        outcome.palette_capacity_action(),
        PaletteCapacityAction::EscalatedAfterFullScan
    );
    assert_eq!(storage.read(CellOffset::new(0).unwrap()), block(99_999));
}

#[test]
fn serialized_palette_has_no_dead_entry() {
    let mut storage = SectionStorage::uniform(block(1_000));
    for raw in 1_001_u32..1_004 {
        storage.write(CellOffset::new((raw - 1_000) as u16).unwrap(), block(raw));
    }
    storage.write(CellOffset::new(1).unwrap(), block(1_000));
    storage.write(CellOffset::new(2).unwrap(), block(1_000));
    assert_eq!(storage.palette_entry_count(), Some(4));

    let envelope =
        SectionPayloadEnvelope::encode_full(SectionId::new(2, 3, 4).unwrap(), 8, &storage);
    let decoded = envelope.decode(None).unwrap();

    assert_eq!(envelope.encoding(), SectionEncoding::Palette);
    assert_eq!(decoded.storage().palette_entry_count(), Some(2));
    assert_eq!(
        decoded.storage().read(CellOffset::new(3).unwrap()),
        block(1_003)
    );
}

#[test]
fn world_reads_and_writes_use_the_foundation_y_z_x_cell_offset() {
    let base = block(1_000);
    let placed = block(2_000);
    let mut storage = SectionStorage::uniform(base);
    let y = WorldY::new(200).unwrap();

    storage.write_world(37, y, 19, placed);

    assert_eq!(CellOffset::from_world(37, y, 19).raw(), 2_101);
    assert_eq!(storage.read_world(37, y, 19), placed);
    assert_eq!(storage.read(CellOffset::new(2_101).unwrap()), placed);
    assert_eq!(storage.read(CellOffset::new(1_381).unwrap()), base);
}

#[test]
fn copy_on_write_snapshot_reads_remain_coherent_during_escalation() {
    let original_cells = cells_with_kinds(256);
    let mut writer = SectionStorage::from_cells(&original_cells).unwrap();
    let reader_snapshot = writer.clone();

    let reader = thread::spawn(move || {
        (0..vw::SECTION_CELLS as u16)
            .map(|offset| reader_snapshot.read(CellOffset::new(offset).unwrap()))
            .collect::<Vec<_>>()
    });
    writer.write(CellOffset::new(0).unwrap(), block(99_999));

    assert_eq!(reader.join().unwrap(), original_cells);
    assert_eq!(writer.encoding(), SectionEncoding::Raw);
    assert_eq!(writer.read(CellOffset::new(0).unwrap()), block(99_999));
}
