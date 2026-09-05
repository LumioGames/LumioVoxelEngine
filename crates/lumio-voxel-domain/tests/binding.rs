use lumio_voxel_contracts::sha256;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::binding::{
    BindingPublication, BindingReference, BindingState, BindingTransaction, EntityRecord,
    ExplicitBlockEntityTypePolicy, SparseReferenceTable,
};
use lumio_voxel_domain::block::{BlockId, BlockType, CellOffset};
use lumio_voxel_domain::section::{
    DeltaEntry, SectionEncoding, SectionId, SectionPayloadEnvelope, SectionStorage,
};

fn policy() -> ExplicitBlockEntityTypePolicy {
    ExplicitBlockEntityTypePolicy::new().with(BlockType::new(1_000).unwrap(), "chest")
}

fn offset(raw: u16) -> CellOffset {
    CellOffset::new(raw).unwrap()
}

fn chest_state() -> (SectionStorage, CellOffset) {
    let cell = offset(17);
    let mut cells = vec![BlockId::from_raw(0); vw::SECTION_CELLS as usize];
    cells[usize::from(cell.raw())] = BlockId::from_raw(1_000 << 8);
    (SectionStorage::from_cells(&cells).unwrap(), cell)
}

#[test]
fn entity_backed_cells_use_sparse_entity_only_references() {
    let (section, cell) = chest_state();
    let table = SparseReferenceTable::from_entries([(cell, "entity-1")]).unwrap();
    assert_eq!(table.len(), 1);
    assert_eq!(table.get(cell).map(String::as_str), Some("entity-1"));
    let bytes = table.to_wire_bytes().unwrap();
    assert!(!bytes.windows("inventory".len()).any(|w| w == b"inventory"));
    BindingState::new(
        section,
        table,
        [EntityRecord::new("entity-1", "chest")],
        &policy(),
    )
    .unwrap();
}

#[test]
fn missing_or_orphan_or_type_mismatch_are_explicit() {
    let (section, cell) = chest_state();
    let missing =
        BindingState::new(section.clone(), SparseReferenceTable::new(), [], &policy()).unwrap_err();
    assert_eq!(missing.error_id(), "entity_binding_missing");

    let dead = BindingState::new(
        section.clone(),
        SparseReferenceTable::from_entries([(cell, "entity-1")]).unwrap(),
        [EntityRecord::dead("entity-1", "chest")],
        &policy(),
    )
    .unwrap_err();
    assert_eq!(dead.error_id(), "entity_binding_missing");

    let orphan = BindingState::new(
        SectionStorage::uniform(BlockId::from_raw(0)),
        SparseReferenceTable::from_entries([(cell, "entity-1")]).unwrap(),
        [EntityRecord::new("entity-1", "chest")],
        &policy(),
    )
    .unwrap_err();
    assert_eq!(orphan.error_id(), "entity_binding_orphan");

    let mismatch = BindingState::new(
        section,
        SparseReferenceTable::from_entries([(cell, "entity-1")]).unwrap(),
        [EntityRecord::new("entity-1", "furnace")],
        &policy(),
    )
    .unwrap_err();
    assert_eq!(mismatch.error_id(), "entity_binding_type_mismatch");
}

#[test]
fn sparse_shape_and_business_data_are_rejected() {
    let dense = SparseReferenceTable::from_dense(vec![
        Some("entity-1".to_owned());
        vw::SECTION_CELLS as usize
    ])
    .unwrap_err();
    assert_eq!(dense.error_id(), "entity_binding_not_sparse");
    assert_eq!(
        BindingReference::from_raw(vw::CELL_OFFSET_MAX + 1, "entity-1")
            .unwrap_err()
            .error_id(),
        "cell_offset_out_of_range"
    );

    let record = BindingReference::with_business_data(offset(2), "entity-1", b"inventory");
    let business = SparseReferenceTable::from_records([record]).unwrap_err();
    assert_eq!(business.error_id(), "entity_binding_not_sparse");

    let payload = b"count=1;inventory=secret";
    assert_eq!(
        BindingState::validate_wire_payload(payload)
            .unwrap_err()
            .error_id(),
        "entity_binding_not_sparse"
    );

    let section_payload = SectionPayloadEnvelope::from_wire_parts(
        "s:0:0:0",
        1,
        lumio_voxel_domain::section::SectionEncoding::Uniform,
        9,
        sha256(b"inventory="),
        None,
        b"inventory=".to_vec(),
    );
    assert_eq!(
        BindingState::validate_section_payload(&section_payload)
            .unwrap_err()
            .error_id(),
        "business_data_in_payload"
    );
}

#[test]
fn section_payload_validation_requires_typed_delta_revision_metadata() {
    let first_delivery = SectionPayloadEnvelope::encode_full(
        SectionId::new(0, 0, 0).unwrap(),
        1,
        &SectionStorage::uniform(BlockId::from_raw(0)),
    );
    assert!(BindingState::validate_section_payload(&first_delivery).is_ok());

    let valid_subsequent_delivery = SectionPayloadEnvelope::encode_delta(
        SectionId::new(0, 0, 0).unwrap(),
        8,
        7,
        &[DeltaEntry::new(offset(3), BlockId::from_raw(1))],
    );
    assert!(BindingState::validate_section_payload(&valid_subsequent_delivery).is_ok());

    for (section_revision, base_section_revision) in [(2, None), (7, Some(7)), (6, Some(7))] {
        let payload = valid_subsequent_delivery.payload().to_vec();
        let malformed = SectionPayloadEnvelope::from_wire_parts(
            "s:0:0:0",
            section_revision,
            SectionEncoding::Delta,
            payload.len() as u32,
            sha256(&payload),
            base_section_revision,
            payload,
        );
        assert_eq!(
            BindingState::validate_section_payload(&malformed)
                .unwrap_err()
                .error_id(),
            "business_data_in_payload"
        );
    }
}

#[test]
fn valid_binding_cases_use_policy_without_reserved_type_assumptions() {
    let chest_type = BlockType::new(1_000).unwrap();
    let chest = BlockId::from_parts(chest_type, lumio_voxel_domain::block::BlockState::new(3));
    let mut cells = vec![BlockId::from_raw(0); vw::SECTION_CELLS as usize];
    cells[17] = chest;
    cells[18] = chest;
    let section = SectionStorage::from_cells(&cells).unwrap();
    let table =
        SparseReferenceTable::from_entries([(offset(17), "left"), (offset(18), "right")]).unwrap();
    let state = BindingState::new(
        section,
        table,
        [
            EntityRecord::new("left", "numeric-policy-type"),
            EntityRecord::new("right", "numeric-policy-type"),
        ],
        &(|block: BlockType| (block == chest_type).then(|| "numeric-policy-type".to_owned())),
    );
    assert!(state.is_ok());

    let plain = BindingState::new(
        SectionStorage::uniform(BlockId::from_raw(0)),
        SparseReferenceTable::new(),
        [],
        &policy(),
    );
    assert!(plain.is_ok());

    let initial = BindingState::new(
        {
            let mut cells = vec![BlockId::from_raw(0); vw::SECTION_CELLS as usize];
            cells[17] = chest;
            SectionStorage::from_cells(&cells).unwrap()
        },
        SparseReferenceTable::from_entries([(offset(17), "portable")]).unwrap(),
        [EntityRecord::new("portable", "numeric-policy-type")],
        &(|block: BlockType| (block == chest_type).then(|| "numeric-policy-type".to_owned())),
    )
    .unwrap();
    let publication = BindingPublication::new(initial.clone());
    let mut move_tx = BindingTransaction::begin(&initial);
    move_tx.remove(offset(17), BlockId::from_raw(0)).unwrap();
    move_tx
        .place(
            offset(18),
            chest,
            EntityRecord::new("portable", "numeric-policy-type"),
        )
        .unwrap();
    let moved = publication
        .publish(
            move_tx,
            &(|block: BlockType| (block == chest_type).then(|| "numeric-policy-type".to_owned())),
        )
        .unwrap();
    assert_eq!(
        moved
            .state()
            .references()
            .get(offset(18))
            .map(String::as_str),
        Some("portable")
    );
    assert!(!moved.state().references().contains(offset(17)));
    assert_eq!(moved.state().entities().len(), 1);
}

#[test]
fn placement_removal_and_failure_are_single_publication_units() {
    let initial = BindingState::empty(SectionStorage::uniform(BlockId::from_raw(0)));
    let publication = BindingPublication::new(initial.clone());
    let cell = offset(9);
    let block = BlockId::from_raw(1_000 << 8);

    let mut place = BindingTransaction::begin(&initial);
    place
        .place(cell, block, EntityRecord::new("entity-1", "chest"))
        .unwrap();
    publication.publish(place, &policy()).unwrap();
    let placed = publication.capture();
    assert_eq!(placed.state().references().len(), 1);
    assert_eq!(placed.state().entities().len(), 1);
    assert_eq!(placed.state().section().read(cell), block);

    let mut replace = BindingTransaction::begin(placed.state());
    replace
        .place(cell, block, EntityRecord::new("entity-2", "chest"))
        .unwrap();
    publication.publish(replace, &policy()).unwrap();
    let replaced = publication.capture();
    assert!(replaced.state().entity("entity-1").is_none());
    assert!(replaced.state().entity("entity-2").is_some());

    let mut remove = BindingTransaction::begin(replaced.state());
    remove.remove(cell, BlockId::from_raw(0)).unwrap();
    publication.publish(remove, &policy()).unwrap();
    let removed = publication.capture();
    assert!(removed.state().references().is_empty());
    assert!(removed.state().entities().is_empty());
    assert_eq!(removed.state().section().read(cell), BlockId::from_raw(0));

    let before = removed.state().clone();
    let mut failed = BindingTransaction::begin(&before);
    failed.set_block_commit(1, cell, block).unwrap();
    failed.set_reference_commit(2, cell, "entity-2").unwrap();
    let error = publication.publish(failed, &policy()).unwrap_err();
    assert_eq!(error.error_id(), "binding_commit_split");
    assert_eq!(publication.capture().state(), &before);
}

#[test]
fn stale_binding_transaction_cannot_erase_a_newer_publication() {
    let initial = BindingState::empty(SectionStorage::uniform(BlockId::from_raw(0)));
    let publication = BindingPublication::new(initial.clone());
    let mut first = BindingTransaction::begin(&initial);
    first
        .set_block_commit(1, offset(1), BlockId::from_raw(11))
        .unwrap();
    let mut stale = BindingTransaction::begin(&initial);
    stale
        .set_block_commit(2, offset(2), BlockId::from_raw(22))
        .unwrap();

    publication.publish(first, &policy()).unwrap();
    let error = publication.publish(stale, &policy()).unwrap_err();
    assert_eq!(error.error_id(), "binding_commit_split");
    let current = publication.capture();
    assert_eq!(
        current.state().section().read(offset(1)),
        BlockId::from_raw(11)
    );
    assert_eq!(
        current.state().section().read(offset(2)),
        BlockId::from_raw(0)
    );
}
