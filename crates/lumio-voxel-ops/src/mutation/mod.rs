//! Canonical fingerprint, receipt ledger, and side-effect-free Prepare.
//!
//! Production code must not depend on `lumio-voxel-test-support`.
//! Prepare does not publish a Root, finalize a receipt, or clear Dirty.

#![forbid(unsafe_code)]

mod commit;
mod commit_finalize;
mod fingerprint;
mod plan;
mod preconditions;
mod prepare;
mod prepared_token;
mod receipt_ledger;
mod reservation;

pub use commit::{CommitEvidence, GeneratedMutationReceipt, commit};
pub use fingerprint::{
    MUTATION_RECEIPT_SCHEMA, MutationRequest, RequestFingerprint, canonical_fingerprint,
};
pub use plan::{MutationPlan, MutationPlanner};
pub use preconditions::{MutationError, MutationPreconditions};
pub use prepare::prepare;
pub use prepared_token::PreparedMutation;
pub use receipt_ledger::{
    FinalizeOutcome, LedgerError, LookupOutcome, ReceiptLedger, ReceiptStatus, ReplayDisposition,
};
pub use reservation::{GenerationBoundLeaseFamily, LEASE_FAMILY, MutationReservation};
