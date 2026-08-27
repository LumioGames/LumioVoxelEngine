//! Single atomic published-state root (R-00078).
//!
//! Readers clone one `Arc<PublishedStateRoot>` per capture and therefore see
//! every member of the same cut.

#![forbid(unsafe_code)]

mod authority;
mod prepared;
mod root;

pub use authority::{PublicationAuthority, PublishedReadView};
pub use prepared::{PreparedPublication, PublicationToken};
pub use root::{AuxiliaryIndexes, PublishedStateRoot};

use crate::revision::PinError;
use lumio_voxel_contracts::STABLE_ERROR_IDS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishError {
    InvalidHandle { error_id: &'static str },
    HandleDoubleRelease { error_id: &'static str },
    SnapshotBaseMismatch { error_id: &'static str },
    SessionMismatch { error_id: &'static str },
    StaleEpoch { error_id: &'static str },
    BudgetExceeded { error_id: &'static str },
}

impl PublishError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::InvalidHandle { error_id }
            | Self::HandleDoubleRelease { error_id }
            | Self::SnapshotBaseMismatch { error_id }
            | Self::SessionMismatch { error_id }
            | Self::StaleEpoch { error_id }
            | Self::BudgetExceeded { error_id } => error_id,
        }
    }

    pub(crate) fn invalid_handle() -> Self {
        Self::InvalidHandle {
            error_id: stable("InvalidHandle"),
        }
    }

    pub(crate) fn handle_double_release() -> Self {
        Self::HandleDoubleRelease {
            error_id: stable("HandleDoubleRelease"),
        }
    }

    pub(crate) fn snapshot_base_mismatch() -> Self {
        Self::SnapshotBaseMismatch {
            error_id: stable("SnapshotBaseMismatch"),
        }
    }

    pub(crate) fn session_mismatch() -> Self {
        Self::SessionMismatch {
            error_id: stable("SessionMismatch"),
        }
    }

    pub(crate) fn stale_epoch() -> Self {
        Self::StaleEpoch {
            error_id: stable("StaleEpoch"),
        }
    }
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for PublishError {}

pub(crate) fn stable(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in generated STABLE_ERROR_IDS")
}

pub(crate) fn map_pin(err: PinError) -> PublishError {
    match err {
        PinError::InvalidHandle { error_id } => PublishError::InvalidHandle { error_id },
        PinError::BudgetExceeded { error_id } => PublishError::BudgetExceeded { error_id },
    }
}
