//! Read-only mutation preconditions. Never reserve, publish, or finalize.

#![forbid(unsafe_code)]

use super::fingerprint::MutationRequest;
use super::plan::MutationPlanner;
use super::receipt_ledger::{LedgerError, LookupOutcome, ReceiptLedger, ReplayDisposition};
use lumio_voxel_contracts::{CHUNK_PRESENCE, STABLE_ERROR_IDS};
use lumio_voxel_domain::chunk::{ChunkError, DirtyError};
use lumio_voxel_domain::publication::PublishedReadView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationError {
    error_id: &'static str,
    disposition: Option<ReplayDisposition>,
}

impl MutationError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub fn disposition(&self) -> Option<ReplayDisposition> {
        self.disposition
    }

    pub(crate) fn invalid_handle() -> Self {
        Self {
            error_id: stable("InvalidHandle"),
            disposition: None,
        }
    }

    pub(crate) fn session_mismatch() -> Self {
        Self {
            error_id: stable("SessionMismatch"),
            disposition: None,
        }
    }

    pub(crate) fn stale_epoch() -> Self {
        Self {
            error_id: stable("StaleEpoch"),
            disposition: None,
        }
    }

    pub(crate) fn revision_conflict() -> Self {
        Self {
            error_id: stable("RevisionConflict"),
            disposition: None,
        }
    }

    pub(crate) fn chunk_unavailable() -> Self {
        Self {
            error_id: stable("ChunkUnavailable"),
            disposition: None,
        }
    }

    pub(crate) fn from_ledger(err: LedgerError) -> Self {
        Self {
            error_id: err.error_id(),
            disposition: err.disposition(),
        }
    }

    pub(crate) fn from_chunk(err: ChunkError) -> Self {
        Self {
            error_id: err.error_id(),
            disposition: None,
        }
    }

    pub(crate) fn from_dirty(err: DirtyError) -> Self {
        Self {
            error_id: err.error_id(),
            disposition: None,
        }
    }
}

impl std::fmt::Display for MutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for MutationError {}

pub struct MutationPreconditions;

impl MutationPreconditions {
    /// World/generation, expected revision, four-state presence, replay conflict.
    /// Read-only: does not reserve, abort, publish, or clear dirty.
    pub fn check(
        request: &MutationRequest,
        base: &PublishedReadView,
        ledger: &ReceiptLedger,
    ) -> Result<(), MutationError> {
        if request.txn_id.is_empty() || request.world_id.is_empty() {
            return Err(MutationError::invalid_handle());
        }
        let stamp = base.stamp();
        if request.world_id != stamp.world_id {
            return Err(MutationError::session_mismatch());
        }
        if request.generation != stamp.generation {
            return Err(MutationError::stale_epoch());
        }

        let plan = MutationPlanner::build(request)?;
        if plan.expected_world_revision() != stamp.world_revision {
            return Err(MutationError::revision_conflict());
        }

        for chunk_id in plan.chunk_ids() {
            match base.directory().lookup(chunk_id) {
                Ok(Some(slot)) => match slot.presence() {
                    "Ready" => {}
                    "NotLoaded" | "Pending" | "Unavailable" => {
                        debug_assert!(CHUNK_PRESENCE.contains(&slot.presence()));
                        return Err(MutationError::chunk_unavailable());
                    }
                    other => {
                        let _ = CHUNK_PRESENCE.contains(&other);
                        return Err(MutationError::invalid_handle());
                    }
                },
                Ok(None) => return Err(MutationError::chunk_unavailable()),
                // Invalid occupancy keys fail in the private stage path, not here.
                Err(err) if err.error_id() == "CoordinateOutOfBounds" => continue,
                Err(err) => return Err(MutationError::from_chunk(err)),
            }
        }

        match ledger.lookup(request) {
            Ok(LookupOutcome::Vacant) => Ok(()),
            Ok(LookupOutcome::InFlight) | Ok(LookupOutcome::Duplicate { .. }) => {
                Err(MutationError::invalid_handle())
            }
            Err(err) => Err(MutationError::from_ledger(err)),
        }
    }
}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}
