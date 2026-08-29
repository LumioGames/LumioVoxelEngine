//! Side-effect-free Prepare: private builders only; never publish or finalize.

#![forbid(unsafe_code)]

use super::fingerprint::{MutationRequest, canonical_fingerprint};
use super::plan::{MutationPlan, MutationPlanner};
use super::preconditions::{MutationError, MutationPreconditions};
use super::prepared_token::PreparedMutation;
use super::receipt_ledger::ReceiptLedger;
use super::reservation::MutationReservation;
use lumio_voxel_contracts::sha256;
use lumio_voxel_domain::chunk::{
    ChunkDeltaBuilder, ChunkPage, ChunkPayload, ChunkSlot, StagedEdit,
};
use lumio_voxel_domain::publication::PublishedReadView;

/// Capture base identity, validate, stage privately, reserve. Does not publish Root,
/// finalize a receipt, clear Dirty, do I/O, or invoke a callback.
pub fn prepare(
    request: &MutationRequest,
    base: &PublishedReadView,
    ledger: &mut ReceiptLedger,
) -> Result<PreparedMutation, MutationError> {
    let base_identity = base.root().identity();
    MutationPreconditions::check(request, base, ledger)?;
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
    let mut builder = ChunkDeltaBuilder::new(base.directory());
    for (chunk_id, edits) in plan.chunk_edits() {
        let slot = ChunkSlot::ready(payload_bytes(&edits.payload_bytes())?);
        let cells: Vec<String> = edits.cell_ids().map(str::to_string).collect();
        builder
            .stage(StagedEdit::new(chunk_id.clone(), slot).cells(cells))
            .map_err(MutationError::from_chunk)?;
    }
    let replacement = builder.freeze().map_err(MutationError::from_chunk)?;

    let mut dirty = base.dirty_frontier().clone();
    let stamp = base.stamp();
    for chunk_id in plan.chunk_ids() {
        let revision = stamp
            .chunk_revision_set
            .get(chunk_id)
            .copied()
            .unwrap_or(stamp.world_revision);
        dirty = dirty
            .record(chunk_id, revision, "mutation")
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

fn payload_bytes(bytes: &[u8]) -> Result<ChunkPayload, MutationError> {
    ChunkPayload::from_pages([ChunkPage::new(
        "Dense",
        "None",
        bytes.to_vec(),
        sha256(bytes),
    )])
    .map_err(MutationError::from_chunk)
}
