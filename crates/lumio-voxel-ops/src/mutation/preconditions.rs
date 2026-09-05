//! Read-only mutation preconditions. Never reserve, publish, or finalize.

#![forbid(unsafe_code)]

use super::fingerprint::MutationRequest;
use super::plan::MutationPlanner;
use super::receipt_ledger::{LedgerError, LookupOutcome, ReceiptLedger, ReplayDisposition};
use lumio_voxel_contracts::STABLE_ERROR_IDS;
use lumio_voxel_contracts::voxel_world::{self as vw, SECTION_PRESENCE};
use lumio_voxel_domain::publication::{PublishError, PublishedReadView};
use lumio_voxel_domain::section::{DirtyError, SectionError};

const WRITE_BATCH_TOO_LARGE: &str = "write_batch_too_large";
const UNSTRUCTURED_MUTATION_ENTRY: &str = "unstructured_mutation_entry";

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

    pub(crate) fn stale_section_revision() -> Self {
        Self {
            error_id: vw::intern_error_code(vw::STALE_SECTION_REVISION)
                .expect("stale_section_revision is a contract error code"),
            disposition: None,
        }
    }

    pub(crate) fn write_batch_too_large() -> Self {
        Self {
            error_id: vw::intern_error_code(WRITE_BATCH_TOO_LARGE)
                .expect("write_batch_too_large is a contract error code"),
            disposition: None,
        }
    }

    pub(crate) fn unstructured_mutation_entry() -> Self {
        Self {
            error_id: vw::intern_error_code(UNSTRUCTURED_MUTATION_ENTRY)
                .expect("unstructured_mutation_entry is a contract error code"),
            disposition: None,
        }
    }

    /// 契约 `presence.missing-is-not-air`:缺块永不等于空气。
    pub(crate) fn section_unavailable() -> Self {
        Self {
            error_id: vw::intern_error_code(vw::SECTION_UNAVAILABLE)
                .expect("section_unavailable is a contract error code"),
            disposition: None,
        }
    }

    pub(crate) fn from_ledger(err: LedgerError) -> Self {
        Self {
            error_id: err.error_id(),
            disposition: err.disposition(),
        }
    }

    pub(crate) fn from_section(err: SectionError) -> Self {
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

    pub(crate) fn snapshot_base_mismatch() -> Self {
        Self {
            error_id: stable("SnapshotBaseMismatch"),
            disposition: None,
        }
    }

    pub(crate) fn from_publish(err: PublishError) -> Self {
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

        // Same-fingerprint Duplicate is idempotent replay: skip the expected-revision
        // check (the published stamp already advanced). InFlight stays rejected.
        match ledger.lookup(request) {
            Ok(LookupOutcome::Duplicate { .. }) => return Ok(()),
            Ok(LookupOutcome::InFlight) => return Err(MutationError::invalid_handle()),
            Ok(LookupOutcome::Vacant) => {}
            Err(err) => return Err(MutationError::from_ledger(err)),
        }

        let plan = MutationPlanner::build(request)?;

        for section_id in plan.section_ids() {
            let expected = plan
                .section_edits()
                .get(section_id)
                .and_then(|edits| edits.entries().first())
                .map(|entry| entry.expected_section_revision)
                .ok_or_else(MutationError::unstructured_mutation_entry)?;
            let current = stamp
                .section_revision_set
                .get(section_id)
                .copied()
                .unwrap_or(stamp.world_revision);
            if expected != current
                || plan
                    .section_edits()
                    .get(section_id)
                    .into_iter()
                    .flat_map(|edits| edits.entries())
                    .any(|entry| entry.expected_section_revision != current)
            {
                return Err(MutationError::stale_section_revision());
            }
        }

        for section_id in plan.section_ids() {
            match base.directory().lookup(section_id) {
                Ok(Some(slot)) => match slot.presence() {
                    "Ready" => {}
                    "Unchanged" | "Pending" | "Unavailable" => {
                        debug_assert!(SECTION_PRESENCE.contains(&slot.presence()));
                        return Err(MutationError::section_unavailable());
                    }
                    other => {
                        let _ = SECTION_PRESENCE.contains(&other);
                        return Err(MutationError::invalid_handle());
                    }
                },
                Ok(None) => return Err(MutationError::section_unavailable()),
                // Invalid occupancy keys fail in the private stage path, not here.
                // 占位/单元格 key 不是 Section 键,跳过;它们在私有 stage 路径上失败。
                Err(SectionError::Key(_)) => continue,
                Err(err) => return Err(MutationError::from_section(err)),
            }
        }

        Ok(())
    }
}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}
