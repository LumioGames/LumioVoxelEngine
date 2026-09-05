use lumio_voxel_contracts::sha256;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{BlockId, CellOffset};
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::section::{
    ChunkRecord, DeltaEntry, SectionEncoding, SectionPage, SectionPayload, SectionPayloadEnvelope,
    SectionStorage,
};

fn block(raw: u32) -> BlockId {
    BlockId::from_raw(raw)
}

#[test]
fn section_encoding_names_match_the_contract_dispatch_table() {
    assert_eq!(
        [
            SectionEncoding::Uniform.name(),
            SectionEncoding::Palette.name(),
            SectionEncoding::Raw.name(),
            SectionEncoding::Delta.name(),
        ],
        vw::SECTION_PAYLOAD_ENCODINGS
    );
}

fn envelope(encoding: SectionEncoding, payload: Vec<u8>) -> SectionPayloadEnvelope {
    SectionPayloadEnvelope::from_wire_parts(
        "s:0:0:0",
        1,
        encoding,
        payload.len() as u32,
        sha256(&payload),
        None,
        payload,
    )
}

fn palette_bytes(entries: &[BlockId], indices: &[u8]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for entry in entries {
        payload.extend_from_slice(&entry.raw().to_le_bytes());
    }
    payload.extend_from_slice(indices);
    payload
}

#[test]
fn single_cell_change_rides_delta() {
    let baseline = SectionStorage::uniform(block(1_000));
    let entry = DeltaEntry::new(CellOffset::new(2_101).unwrap(), block(2_000));
    let envelope =
        SectionPayloadEnvelope::encode_delta(SectionId::new(3, 4, 5).unwrap(), 13, 12, &[entry]);

    assert_eq!(envelope.encoding(), SectionEncoding::Delta);
    assert_eq!(envelope.base_section_revision(), Some(12));
    assert_eq!(envelope.payload_length(), 6);
    let decoded = envelope.decode(Some((&baseline, 12))).unwrap();
    assert_eq!(decoded.section_revision(), 13);
    assert_eq!(decoded.storage().read(entry.offset()), block(2_000));
}

#[test]
fn first_delivery_rides_full_encoding() {
    let storage = SectionStorage::uniform(block(1_000));
    let envelope =
        SectionPayloadEnvelope::encode_full(SectionId::new(0, 0, 0).unwrap(), 1, &storage);

    assert_ne!(envelope.encoding(), SectionEncoding::Delta);
    assert_eq!(envelope.section_key(), "s:0:0:0");
    assert_eq!(envelope.section_revision(), 1);
    assert_eq!(envelope.base_section_revision(), None);
    assert_eq!(envelope.payload_length() as usize, envelope.payload().len());
    assert_eq!(envelope.payload_sha256(), sha256(envelope.payload()));
    assert_eq!(envelope.decode(None).unwrap().storage(), &storage);
}

#[test]
fn delta_rejected_then_resynced_in_full() {
    let baseline = SectionStorage::uniform(block(1_000));
    let authoritative = SectionStorage::uniform(block(2_000));
    let section_id = SectionId::new(0, 0, 0).unwrap();
    let stale_delta = SectionPayloadEnvelope::encode_delta(
        section_id,
        15,
        14,
        &[DeltaEntry::new(CellOffset::new(0).unwrap(), block(2_000))],
    );

    assert_eq!(
        stale_delta
            .decode(Some((&baseline, 12)))
            .unwrap_err()
            .error_id(),
        vw::DELTA_BASE_REVISION_MISMATCH
    );
    let full = SectionPayloadEnvelope::encode_full(section_id, 15, &authoritative);
    let decoded = full.decode(Some((&baseline, 12))).unwrap();
    assert_ne!(full.encoding(), SectionEncoding::Delta);
    assert_eq!(decoded.section_revision(), 15);
    assert_eq!(decoded.storage(), &authoritative);
}

#[test]
fn full_payload_revision_must_advance_beyond_supplied_baseline() {
    let storage = SectionStorage::uniform(block(1_000));
    let section_id = SectionId::new(0, 0, 0).unwrap();
    let stale = SectionPayloadEnvelope::encode_full(section_id, 12, &storage);
    assert_eq!(
        stale.decode(Some((&storage, 12))).unwrap_err().error_id(),
        vw::STALE_SECTION_REVISION
    );
}

#[test]
fn storage_sidecar_must_match_one_sealed_page() {
    let left = SectionStorage::uniform(block(1_000));
    let bytes = block(2_000).raw().to_le_bytes().to_vec();
    let error = SectionPayload::from_pages_with_storage(
        [SectionPage::new(
            "Dense",
            "None",
            bytes.clone(),
            sha256(&bytes),
        )],
        Some(left),
    )
    .unwrap_err();
    assert_eq!(error.error_id(), vw::SECTION_ENCODING_MISMATCH);

    let matching = block(1_000).raw().to_le_bytes().to_vec();
    let extra = block(2_000).raw().to_le_bytes().to_vec();
    let error = SectionPayload::from_pages_with_storage(
        [
            SectionPage::new("Dense", "None", matching.clone(), sha256(&matching)),
            SectionPage::new("Dense", "None", extra.clone(), sha256(&extra)),
        ],
        Some(SectionStorage::uniform(block(1_000))),
    )
    .unwrap_err();
    assert_eq!(error.error_id(), vw::SECTION_ENCODING_MISMATCH);
}

#[test]
fn delta_applied_over_mismatched_base() {
    let baseline = SectionStorage::uniform(block(1_000));
    let unchanged = baseline.clone();
    let envelope = SectionPayloadEnvelope::encode_delta(
        SectionId::new(0, 0, 0).unwrap(),
        15,
        14,
        &[DeltaEntry::new(CellOffset::new(0).unwrap(), block(2_000))],
    );

    let error = envelope.decode(Some((&baseline, 12))).unwrap_err();

    assert_eq!(error.error_id(), vw::DELTA_BASE_REVISION_MISMATCH);
    assert_eq!(baseline, unchanged);
}

#[test]
fn delta_target_revision_must_advance_beyond_its_base() {
    let baseline = SectionStorage::uniform(block(1_000));
    for target in [12, 11] {
        let envelope = SectionPayloadEnvelope::encode_delta(
            SectionId::new(0, 0, 0).unwrap(),
            target,
            12,
            &[DeltaEntry::new(CellOffset::new(0).unwrap(), block(2_000))],
        );
        assert_eq!(
            envelope
                .decode(Some((&baseline, 12)))
                .unwrap_err()
                .error_id(),
            vw::DELTA_BASE_REVISION_MISMATCH
        );
    }
}

#[test]
fn delta_sent_as_first_delivery() {
    let envelope = SectionPayloadEnvelope::encode_delta(
        SectionId::new(0, 0, 0).unwrap(),
        2,
        1,
        &[DeltaEntry::new(CellOffset::new(0).unwrap(), block(2_000))],
    );

    assert_eq!(
        envelope.decode(None).unwrap_err().error_id(),
        vw::DELTA_USED_FOR_FIRST_DELIVERY
    );
}

#[test]
fn chunk_record_carries_section_bytes() {
    assert_eq!(
        ChunkRecord::validate("c:1:2", &[1], None)
            .unwrap_err()
            .error_id(),
        vw::CHUNK_CARRIES_DATA_ERROR
    );
}

#[test]
fn chunk_declares_own_revision() {
    assert_eq!(
        ChunkRecord::validate("c:1:2", &[], Some(7))
            .unwrap_err()
            .error_id(),
        vw::CHUNK_CARRIES_DATA_ERROR
    );
    assert_eq!(
        ChunkRecord::validate("c:1:2", &[], None)
            .unwrap()
            .id()
            .key(),
        "c:1:2"
    );
}

#[test]
fn palette_with_257_entries() {
    let entries: Vec<_> = (0..257).map(|raw| block(1_000 + raw)).collect();
    let payload = palette_bytes(&entries, &vec![0; vw::SECTION_CELLS as usize]);

    assert_eq!(
        envelope(SectionEncoding::Palette, payload)
            .decode(None)
            .unwrap_err()
            .error_id(),
        vw::PALETTE_OVERFLOW
    );
}

#[test]
fn uniform_encoding_with_two_kinds() {
    let mut payload = block(1_000).raw().to_le_bytes().to_vec();
    payload.extend_from_slice(&block(2_000).raw().to_le_bytes());

    assert_eq!(
        envelope(SectionEncoding::Uniform, payload)
            .decode(None)
            .unwrap_err()
            .error_id(),
        vw::SECTION_ENCODING_MISMATCH
    );
}

#[test]
fn payload_interpreted_before_digest_check() {
    let malformed_palette = vec![1, 1];
    let incoming = SectionPayloadEnvelope::from_wire_parts(
        "s:0:0:0",
        1,
        SectionEncoding::Palette,
        malformed_palette.len() as u32,
        [0; 32],
        None,
        malformed_palette,
    );

    assert_eq!(
        incoming.decode(None).unwrap_err().error_id(),
        vw::SECTION_DIGEST_MISMATCH
    );
}

#[test]
fn escalate_to_raw_without_reclaim() {
    assert_eq!(
        SectionStorage::validate_raw_escalation(false)
            .unwrap_err()
            .error_id(),
        vw::PALETTE_RECLAIM_BEFORE_ESCALATION
    );
    SectionStorage::validate_raw_escalation(true).unwrap();
}

#[test]
fn payload_palette_contains_dead_entry() {
    let entries = [block(1_000), block(2_000), block(3_000)];
    let mut indices = vec![0; vw::SECTION_CELLS as usize];
    indices[0] = 1;
    let payload = palette_bytes(&entries, &indices);

    assert_eq!(
        envelope(SectionEncoding::Palette, payload)
            .decode(None)
            .unwrap_err()
            .error_id(),
        vw::DEAD_PALETTE_ENTRY_IN_PAYLOAD
    );
}
