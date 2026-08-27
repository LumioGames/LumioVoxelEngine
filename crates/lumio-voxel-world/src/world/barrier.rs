//! Typed short Barrier scopes. Work is bounded and in-memory only.

#![forbid(unsafe_code)]

use super::WorldError;
use super::instance::VoxelWorld;
use super::state::{query_admissible, write_admissible};

/// Work category admitted into a short in-memory barrier. Not a duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarrierScope {
    Mutation,
    CaptureCut,
    DurabilityAck,
    Restore,
    StreamingApply,
}

/// Probe kinds for forbidden barrier work. Mapped onto generated error ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForbiddenWork {
    Io,
    Sleep,
    UnboundedLoop,
    Callback,
}

/// Rejects I/O, waits, unbounded loops, and C# callbacks. Tests use this as a probe.
pub fn reject_forbidden(work: ForbiddenWork) -> WorldError {
    match work {
        ForbiddenWork::Io | ForbiddenWork::Sleep => WorldError::mapped("LoaderTimeout"),
        ForbiddenWork::UnboundedLoop => WorldError::mapped("BudgetExceeded"),
        ForbiddenWork::Callback => WorldError::mapped("InvalidHandle"),
    }
}

pub(crate) fn admit_scope(world: &VoxelWorld, scope: BarrierScope) -> Result<(), WorldError> {
    let state = world.instance.state.current();
    let admitted = match scope {
        BarrierScope::CaptureCut => query_admissible(state),
        BarrierScope::Mutation
        | BarrierScope::DurabilityAck
        | BarrierScope::Restore
        | BarrierScope::StreamingApply => write_admissible(state),
    };
    if admitted {
        Ok(())
    } else {
        Err(WorldError::claim_not_granted())
    }
}
