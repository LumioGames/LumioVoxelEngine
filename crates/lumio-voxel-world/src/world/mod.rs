//! Per-instance VoxelWorld lifecycle, generation, and typed command admission.

#![forbid(unsafe_code)]

mod admission;
mod instance;
mod state;

pub use admission::{AdmittedCommand, WorldCommand, WorldEndpoint};
pub use instance::{
    InstanceGenerationGuard, VoxelWorld, WorldConfigAdapter, WorldDescriptor, WorldStateView,
    intern_local_embedded_pair, intern_role,
};

use lumio_voxel_contracts::STABLE_ERROR_IDS;

/// Stable world error. `error_id` is always interned from generated `STABLE_ERROR_IDS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldError {
    error_id: &'static str,
}

impl WorldError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub(crate) fn mapped(id: &'static str) -> Self {
        Self {
            error_id: intern_stable(id),
        }
    }

    pub(crate) fn invalid_handle() -> Self {
        Self::mapped("InvalidHandle")
    }

    pub(crate) fn stale_epoch() -> Self {
        Self::mapped("StaleEpoch")
    }

    pub(crate) fn session_mismatch() -> Self {
        Self::mapped("SessionMismatch")
    }

    pub(crate) fn role_mismatch() -> Self {
        Self::mapped("RoleMismatch")
    }

    pub(crate) fn claim_not_granted() -> Self {
        Self::mapped("ClaimNotGranted")
    }
}

impl std::fmt::Display for WorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for WorldError {}

pub(crate) fn intern_stable(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in generated STABLE_ERROR_IDS")
}
