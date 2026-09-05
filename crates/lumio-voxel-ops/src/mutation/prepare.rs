//! Side-effect-free Prepare: private builders only; never publish or finalize.

#![forbid(unsafe_code)]

use super::fingerprint::{MutationRequest, canonical_fingerprint};
use super::plan::{MutationPlan, MutationPlanner};
use super::preconditions::{MutationError, MutationPreconditions};
use super::prepared_token::PreparedMutation;
use super::receipt_ledger::{LookupOutcome, ReceiptLedger};
use super::reservation::MutationReservation;
use lumio_voxel_contracts::sha256;
use lumio_voxel_domain::publication::PublishedReadView;
use lumio_voxel_domain::section::{
    SectionDeltaBuilder, SectionPage, SectionPayload, SectionPayloadEnvelope, SectionSlot,
    SectionStorage, StagedEdit,
};
use std::collections::BTreeMap;

/// Capture base identity, validate, stage privately, reserve. Does not publish Root,
/// finalize a receipt, clear Dirty, do I/O, or invoke a callback.
pub fn prepare(
    request: &MutationRequest,
    base: &PublishedReadView,
    ledger: &mut ReceiptLedger,
) -> Result<PreparedMutation, MutationError> {
    let base_identity = base.root().identity();
    MutationPreconditions::check(request, base, ledger)?;

    // A completed transaction is a replay, not a new storage read. Keep the
    // stored receipt on the prepared token so commit can replay it after the
    // original Section has been unloaded.
    if let LookupOutcome::Duplicate { receipt } =
        ledger.lookup(request).map_err(MutationError::from_ledger)?
    {
        let reservation = ledger
            .reserve(request)
            .map_err(MutationError::from_ledger)?;
        let evidence = ledger
            .evidence(request)
            .map_err(MutationError::from_ledger)?;
        let replacement = SectionDeltaBuilder::new(base.directory())
            .freeze()
            .map_err(MutationError::from_section)?;
        return Ok(PreparedMutation::bind_replay(
            request.clone(),
            reservation.fingerprint(),
            base_identity,
            request.generation,
            ledger.config_hash().to_string(),
            reservation,
            replacement,
            base.dirty_frontier().clone(),
            base.stamp().world_revision,
            receipt,
            evidence,
        ));
    }

    let plan = MutationPlanner::build(request)?;

    let reservation = ledger
        .reserve(request)
        .map_err(MutationError::from_ledger)?;
    match seal_private(request, base, ledger, &plan, base_identity, reservation) {
        Ok(token) => Ok(token),
        Err(err) => {
            let _ = ledger.abort(request);
            Err(err)
        }
    }
}

fn seal_private(
    request: &MutationRequest,
    base: &PublishedReadView,
    ledger: &ReceiptLedger,
    plan: &MutationPlan,
    base_identity: [u8; 32],
    reservation: MutationReservation,
) -> Result<PreparedMutation, MutationError> {
    let mut builder = SectionDeltaBuilder::new(base.directory());
    for (section_id, edits) in plan.section_edits() {
        let current_revision = base
            .stamp()
            .section_revision_set
            .get(section_id)
            .copied()
            .unwrap_or(base.stamp().world_revision);
        let next_revision = current_revision
            .checked_add(1)
            .ok_or_else(MutationError::invalid_handle)?;
        let (slot, cells) = payload_for_entries(section_id, next_revision, base, edits.entries())?;
        builder
            .stage(StagedEdit::new(section_id.clone(), slot).cells(cells))
            .map_err(MutationError::from_section)?;
    }
    let replacement = builder.freeze().map_err(MutationError::from_section)?;

    let mut dirty = base.dirty_frontier().clone();
    let stamp = base.stamp();
    for section_id in plan.section_ids() {
        let revision = stamp
            .section_revision_set
            .get(section_id)
            .copied()
            .unwrap_or(stamp.world_revision);
        let next_revision = revision
            .checked_add(1)
            .ok_or_else(MutationError::invalid_handle)?;
        dirty = dirty
            .record(section_id, next_revision, "mutation")
            .map_err(MutationError::from_dirty)?;
    }

    let target_world_revision = stamp
        .world_revision
        .checked_add(1)
        .ok_or_else(MutationError::invalid_handle)?;

    let fingerprint =
        canonical_fingerprint(request).map_err(|_| MutationError::invalid_handle())?;
    Ok(PreparedMutation::bind(
        request.clone(),
        fingerprint,
        base_identity,
        request.generation,
        ledger.config_hash().to_string(),
        reservation,
        replacement,
        dirty,
        target_world_revision,
    ))
}

fn payload_for_entries(
    section_id: &str,
    section_revision: u64,
    base: &PublishedReadView,
    entries: &[super::fingerprint::MutationEntry],
) -> Result<(SectionSlot, Vec<String>), MutationError> {
    let id = lumio_voxel_domain::key::SectionId::parse(section_id)
        .map_err(|_| MutationError::unstructured_mutation_entry())?;
    // The section payload is rebuilt privately from the canonical storage adapter. The
    // request fingerprint preserves submission order; collapsing here keeps only each
    // cell's last value before invoking the storage writer.
    let mut storage = base_storage(base, section_id)?;
    let mut final_writes = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        final_writes.insert(entry.cell_offset, (index, entry.block_id));
    }
    let mut final_writes: Vec<_> = final_writes
        .into_iter()
        .map(|(offset, (index, block_id))| (offset, index, block_id))
        .collect();
    final_writes.sort_unstable_by_key(|(_, index, _)| *index);
    let mut cells = Vec::with_capacity(final_writes.len());
    for (offset, _, block_id) in final_writes {
        storage.write(offset, block_id);
        cells.push(offset.raw().to_string());
    }
    let envelope = SectionPayloadEnvelope::encode_full(id, section_revision, &storage);
    let payload = SectionPayload::from_pages_with_storage(
        [SectionPage::new(
            "Dense",
            "None",
            envelope.payload().to_vec(),
            sha256(envelope.payload()),
        )],
        Some(storage),
    )
    .map_err(MutationError::from_section)?;
    Ok((SectionSlot::ready(payload), cells))
}

fn base_storage(
    base: &PublishedReadView,
    section_id: &str,
) -> Result<SectionStorage, MutationError> {
    let slot = base
        .directory()
        .lookup(section_id)
        .map_err(MutationError::from_section)?;
    let payload = slot
        .and_then(|slot| slot.payload())
        .ok_or_else(MutationError::section_unavailable)?;
    payload
        .storage()
        .cloned()
        .ok_or_else(MutationError::unstructured_mutation_entry)
}
