//! Sole visible linearization: `publish_once`, then ledger finalize of prebuilt bytes.

#![forbid(unsafe_code)]

use super::fingerprint::MutationRequest;
use super::preconditions::MutationError;
use super::receipt_ledger::ReceiptLedger;
use lumio_voxel_domain::publication::{PublicationAuthority, PublicationToken};

pub(super) fn publish_once_and_finalize(
    authority: &PublicationAuthority,
    ledger: &mut ReceiptLedger,
    token: PublicationToken,
    request: MutationRequest,
    receipt_bytes: Vec<u8>,
) -> Result<Vec<u8>, MutationError> {
    authority
        .publish_once(token)
        .map_err(MutationError::from_publish)?;
    let outcome = ledger
        .finalize(&request, receipt_bytes)
        .map_err(MutationError::from_ledger)?;
    Ok(outcome.receipt)
}
