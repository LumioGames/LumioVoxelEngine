//! Canonical fingerprint and txn receipt ledger (R-00093).
//!
//! Production code must not depend on `lumio-voxel-test-support`.
//! The ledger does not publish chunks and does not hold a directory root.

#![forbid(unsafe_code)]

mod fingerprint;
mod receipt_ledger;
mod reservation;

pub use fingerprint::{
    MUTATION_RECEIPT_SCHEMA, MutationRequest, RequestFingerprint, canonical_fingerprint,
};
pub use receipt_ledger::{
    FinalizeOutcome, LedgerError, LookupOutcome, ReceiptLedger, ReplayDisposition,
};
pub use reservation::{GenerationBoundLeaseFamily, LEASE_FAMILY, MutationReservation};
