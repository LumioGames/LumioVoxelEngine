//! Per-instance VoxelWorld lifecycle, generation, and typed command admission.

#![forbid(unsafe_code)]

mod admission;
mod barrier;
mod capture;
mod capture_admission;
mod command;
mod diagnostics;
mod durability_ack;
mod events;
mod fault;
mod instance;
mod residency;
mod restore;
mod routing;
mod shutdown;
mod state;
mod write_lane;

pub use admission::{AdmittedCommand, WorldCommand, WorldEndpoint};
pub use barrier::{BarrierScope, ForbiddenWork, reject_forbidden};
pub use capture::{CaptureEvidence, capture};
pub use capture_admission::RuntimeSnapshotCut;
pub use diagnostics::{DiagnosticsView, WorldDiagnostics};
pub use durability_ack::{AckEvidence, DurabilityReceipt, apply_durability_ack};
pub use events::{FailureBundleFragment, WorldEvent, WorldEventSink};
pub use fault::{FaultEvidence, WorldFaultPort};
pub use instance::{
    InstanceGenerationGuard, VoxelWorld, WorldConfigAdapter, WorldDescriptor, WorldStateView,
    intern_local_embedded_pair, intern_role,
};
pub use residency::{
    NoPinExemption, PinBudget, PinExemptionError, PinExemptionHook, PinHandle, PinId, PinReadiness,
    PinStatus, RegionPinError, RegionPinManager, RegionPinStatus, ResidencyPinError, UnloadReceipt,
    request_unload, section_keys_for_region, unload, unload_section,
};
pub use restore::{RestoreReceipt, restore};
pub use routing::WorldRouter;
pub use shutdown::WorldShutdown;
pub use write_lane::{WorldWriteLane, WriteLease};

use lumio_voxel_contracts::STABLE_ERROR_IDS;
use lumio_voxel_contracts::voxel_world as vw;

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

/// 收敛到单一 `'static` 实例。两个命名空间都认:体素公共语义走活契约的错误码,
/// 契约不定义的引擎通用失败仍走废弃镜像的 `STABLE_ERROR_IDS`。
pub(crate) fn intern_stable(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .or_else(|| vw::intern_error_code(id))
        .expect("mapped error id must be a contract error code or a frozen-mirror stable id")
}
