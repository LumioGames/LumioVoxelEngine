use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_domain::block::{BlockId, CellOffset};
use lumio_voxel_domain::key::SectionId;
use lumio_voxel_domain::section::{
    DeltaEntry, RoomModificationLayer, SectionDeliveryState, SectionDispatch, SectionEncoding,
    SectionStorage,
};

const DELTA_ENTRY_BYTES: usize = vw::DELTA_BYTES_PER_ENTRY as usize;

fn section(x: i32) -> SectionId {
    SectionId::new(x, 0, 0).unwrap()
}

fn block(raw: u32) -> BlockId {
    BlockId::from_raw(raw)
}

fn layer_with_delta(entry_count: u16) -> (RoomModificationLayer, SectionId, SectionStorage) {
    let section_id = section(0);
    let baseline = SectionStorage::uniform(block(1_000));
    let entries: Vec<_> = (0..entry_count)
        .map(|offset| {
            DeltaEntry::new(
                CellOffset::new(offset).unwrap(),
                block(2_000 + u32::from(offset)),
            )
        })
        .collect();
    let mut layer = RoomModificationLayer::new();
    layer.set_ready(section_id, 12, baseline.clone());
    layer.record_delta(section_id, 13, &entries).unwrap();
    (layer, section_id, baseline)
}

#[test]
fn unchanged_section_is_zero_bytes() {
    let layer = RoomModificationLayer::new();
    let response = Some(layer.dispatch_section(section(7), SectionDeliveryState::FirstDelivery));
    let dispatch = response
        .as_ref()
        .expect("Unchanged is a response, not silence");

    assert_eq!(dispatch.presence(), "Unchanged");
    assert_eq!(
        dispatch.payload_length(),
        Some(vw::SHORT_TICKET_PAYLOAD_LENGTH)
    );
    assert_eq!(dispatch.payload(), Some(&[][..]));
    assert!(dispatch.envelope().is_none());
    dispatch.require_available().unwrap();

    let decoded = SectionDispatch::validate_unchanged_ticket(
        section(7),
        vw::SHORT_TICKET_PAYLOAD_LENGTH,
        &[],
    )
    .unwrap();
    assert_eq!(decoded, *dispatch);
}

#[test]
fn unchanged_answered_with_full_payload() {
    let section_id = section(1);
    let full = lumio_voxel_domain::section::SectionPayloadEnvelope::encode_full(
        section_id,
        1,
        &SectionStorage::uniform(block(1_000)),
    );

    let error = SectionDispatch::validate_unchanged_ticket(
        section_id,
        full.payload_length(),
        full.payload(),
    )
    .unwrap_err();

    assert_eq!(error.error_id(), vw::SECTION_ENCODING_MISMATCH);
    assert_eq!(
        SectionDispatch::validate_unchanged_ticket(section_id, 1, &[])
            .unwrap_err()
            .error_id(),
        vw::SECTION_ENCODING_MISMATCH
    );
}

#[test]
fn pending_section_rendered_as_air() {
    let ready_id = section(0);
    let pending_id = section(1);
    let mut layer = RoomModificationLayer::new();
    layer.set_ready(ready_id, 1, SectionStorage::uniform(block(1_000)));

    let responses = [
        layer.dispatch_section(ready_id, SectionDeliveryState::FirstDelivery),
        layer.dispatch_section(pending_id, SectionDeliveryState::Pending),
    ];
    let pending = &responses[1];

    assert_eq!(responses.len(), 2, "Pending must not be omitted");
    assert_eq!(layer.len(), 1, "Pending is not another modification layer");
    assert_eq!(pending.presence(), "Pending");
    assert_eq!(pending.payload_length(), None);
    assert_eq!(pending.payload(), None);
    assert_eq!(
        pending.require_available().unwrap_err().error_id(),
        vw::SECTION_UNAVAILABLE
    );
}

#[test]
fn unavailable_section_treated_as_deleted() {
    let unavailable_id = section(2);
    let layer = RoomModificationLayer::new();

    let response = Some(layer.dispatch_section(unavailable_id, SectionDeliveryState::Unavailable));
    let unavailable = response
        .as_ref()
        .expect("Unavailable is explicit, not an omitted deletion");

    assert_eq!(unavailable.presence(), "Unavailable");
    assert!(layer.is_empty(), "Unavailable is not a modification entry");
    assert_eq!(unavailable.payload(), None);
    assert_eq!(
        unavailable.require_available().unwrap_err().error_id(),
        vw::SECTION_UNAVAILABLE
    );
}

#[test]
fn first_delivery_and_resync_use_full_after_layer_has_delta() {
    let (layer, section_id, _) = layer_with_delta(1);

    for delivery in [
        SectionDeliveryState::FirstDelivery,
        SectionDeliveryState::Resync,
        SectionDeliveryState::Ready(11),
    ] {
        let dispatch = layer.dispatch_section(section_id, delivery);
        let envelope = dispatch
            .envelope()
            .expect("Ready carries the shared envelope");
        assert_eq!(dispatch.presence(), "Ready");
        assert_ne!(envelope.encoding(), SectionEncoding::Delta);
        assert_eq!(envelope.base_section_revision(), None);
        assert_eq!(envelope.section_revision(), 13);
        assert_eq!(
            envelope
                .decode(None)
                .unwrap()
                .storage()
                .read(CellOffset::new(0).unwrap()),
            block(2_000)
        );
    }
}

#[test]
fn later_ready_change_uses_existing_delta_envelope() {
    let (layer, section_id, baseline) = layer_with_delta(1);

    let dispatch = layer.dispatch_section(section_id, SectionDeliveryState::Ready(12));
    let envelope = dispatch
        .envelope()
        .expect("Ready carries the shared envelope");

    assert_eq!(dispatch.presence(), "Ready");
    assert_eq!(envelope.encoding(), SectionEncoding::Delta);
    assert_eq!(envelope.base_section_revision(), Some(12));
    assert_eq!(envelope.payload_length(), vw::DELTA_BYTES_PER_ENTRY);
    assert_eq!(
        envelope
            .decode(Some((&baseline, 12)))
            .unwrap()
            .storage()
            .read(CellOffset::new(0).unwrap()),
        block(2_000)
    );
}

#[test]
fn delta_entries_are_emitted_in_deterministic_order() {
    let section_id = section(3);
    let entries = [
        DeltaEntry::new(CellOffset::new(9).unwrap(), block(2_009)),
        DeltaEntry::new(CellOffset::new(2).unwrap(), block(2_002)),
    ];
    let mut layer = RoomModificationLayer::new();
    layer.set_ready(section_id, 12, SectionStorage::uniform(block(1_000)));
    layer.record_delta(section_id, 13, &entries).unwrap();

    let payload = layer
        .dispatch_section(section_id, SectionDeliveryState::Ready(12))
        .payload()
        .unwrap()
        .to_vec();
    let offsets: Vec<_> = payload
        .as_chunks::<DELTA_ENTRY_BYTES>()
        .0
        .iter()
        .map(|entry| u16::from_le_bytes([entry[0], entry[1]]))
        .collect();
    assert_eq!(offsets, [2, 9]);

    let mut permuted = RoomModificationLayer::new();
    permuted.set_ready(section_id, 12, SectionStorage::uniform(block(1_000)));
    permuted
        .record_delta(
            section_id,
            13,
            &[*entries.last().unwrap(), *entries.first().unwrap()],
        )
        .unwrap();
    assert_eq!(
        permuted
            .dispatch_section(section_id, SectionDeliveryState::Ready(12))
            .payload(),
        Some(payload.as_slice()),
        "unique-cell Delta bytes must not depend on submission order"
    );
}

#[test]
fn duplicate_delta_cells_use_the_final_submitted_value() {
    let section_id = section(5);
    let baseline = SectionStorage::uniform(block(3_000));
    let mut layer = RoomModificationLayer::new();
    layer.set_ready(section_id, 12, baseline.clone());
    layer
        .record_delta(
            section_id,
            13,
            &[
                DeltaEntry::new(CellOffset::new(7).unwrap(), block(2_000)),
                DeltaEntry::new(CellOffset::new(7).unwrap(), block(1_000)),
            ],
        )
        .unwrap();

    let dispatch = layer.dispatch_section(section_id, SectionDeliveryState::Ready(12));
    let envelope = dispatch
        .envelope()
        .expect("ready section carries its delta envelope");
    assert_eq!(
        envelope.payload_length(),
        2 * vw::DELTA_BYTES_PER_ENTRY,
        "duplicate writes remain ordered Delta entries"
    );
    assert_eq!(
        envelope
            .decode(Some((&baseline, 12)))
            .unwrap()
            .storage()
            .read(CellOffset::new(7).unwrap()),
        block(1_000)
    );
}

#[test]
fn delta_is_not_recorded_for_a_section_that_is_not_ready() {
    let section_id = section(4);
    let entry = DeltaEntry::new(CellOffset::new(0).unwrap(), block(2_000));
    let mut layer = RoomModificationLayer::new();

    assert_eq!(
        layer
            .record_delta(section_id, 2, &[entry])
            .unwrap_err()
            .error_id(),
        vw::SECTION_UNAVAILABLE
    );
    assert_eq!(
        layer
            .dispatch_section(section_id, SectionDeliveryState::Pending)
            .presence(),
        "Pending"
    );
    assert_eq!(
        layer
            .dispatch_section(section_id, SectionDeliveryState::Unavailable)
            .presence(),
        "Unavailable"
    );
    assert!(layer.is_empty());
}

#[test]
fn dispatch_payload_bytes_follow_changes_not_requested_map_area() {
    let (one_change, section_id, _) = layer_with_delta(1);
    let (hundred_changes, _, _) = layer_with_delta(100);
    let entry_bytes = vw::DELTA_BYTES_PER_ENTRY as usize;

    let one = one_change.dispatch_section(section_id, SectionDeliveryState::Ready(12));
    let hundred = hundred_changes.dispatch_section(section_id, SectionDeliveryState::Ready(12));
    assert_eq!(one_change.len(), 1);
    assert_eq!(one.payload().unwrap().len(), entry_bytes);
    assert_eq!(hundred.payload().unwrap().len(), 100 * entry_bytes);

    let payload_bytes = |extent: i32| {
        (0..extent)
            .map(|x| {
                let delivery = if x == 0 {
                    SectionDeliveryState::Ready(12)
                } else {
                    SectionDeliveryState::FirstDelivery
                };
                one_change
                    .dispatch_section(section(x), delivery)
                    .payload()
                    .map_or(0, <[u8]>::len)
            })
            .sum::<usize>()
    };

    assert_eq!(payload_bytes(8), entry_bytes);
    assert_eq!(payload_bytes(16), entry_bytes);
}

#[test]
fn one_layer_keeps_only_the_latest_section_change() {
    let section_id = section(0);
    let mut layer = RoomModificationLayer::new();
    layer.set_ready(section_id, 1, SectionStorage::uniform(block(1_000)));
    layer
        .record_delta(
            section_id,
            2,
            &[DeltaEntry::new(CellOffset::new(0).unwrap(), block(2_000))],
        )
        .unwrap();
    layer
        .record_delta(
            section_id,
            3,
            &[DeltaEntry::new(CellOffset::new(1).unwrap(), block(3_000))],
        )
        .unwrap();

    assert_eq!(layer.len(), 1);
    assert_eq!(
        layer
            .dispatch_section(section_id, SectionDeliveryState::Ready(2))
            .envelope()
            .unwrap()
            .base_section_revision(),
        Some(2)
    );
    assert_ne!(
        layer
            .dispatch_section(section_id, SectionDeliveryState::Ready(1))
            .envelope()
            .unwrap()
            .encoding(),
        SectionEncoding::Delta,
        "the layer must not expose a stack of historical patches"
    );

    layer.mark_unchanged(section_id);
    assert!(layer.is_empty());
    assert_eq!(
        layer
            .dispatch_section(section_id, SectionDeliveryState::FirstDelivery)
            .presence(),
        "Unchanged"
    );
}
