//! Target-World invariant / worker fault. Does not invent a `Faulted` session state.

#![forbid(unsafe_code)]

use super::WorldError;
use super::events::{FailureBundleFragment, WorldEvent, WorldEventSink};
use super::instance::VoxelWorld;
use super::shutdown::WorldShutdown;
use super::state::{has_session_edge, intern_session_event, intern_session_state};
use lumio_voxel_contracts::STABLE_ERROR_IDS;

/// Short diagnostic label. Never a key or a raw payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultEvidence {
    diagnostic_name: &'static str,
}

impl FaultEvidence {
    pub fn new(diagnostic_name: &'static str) -> Self {
        Self { diagnostic_name }
    }

    pub fn diagnostic_name(&self) -> &'static str {
        self.diagnostic_name
    }
}

pub struct WorldFaultPort;

impl WorldFaultPort {
    /// Confine a severe fault to `world` and run Drain → FinalSnapshotTaken → Dispose.
    pub fn trip(
        world: &mut VoxelWorld,
        sink: &mut WorldEventSink,
        cause: &'static str,
        evidence: &FaultEvidence,
    ) -> Result<(), WorldError> {
        let cause = intern_cause(cause)?;
        if evidence.diagnostic_name.is_empty() {
            return Err(WorldError::invalid_handle());
        }
        if !shutdown_admissible(world) {
            return Err(WorldError::invalid_handle());
        }
        if !already_disposed(world) {
            sink.emit(WorldEvent::Failure(bundle(world, cause, evidence)));
        }
        WorldShutdown::begin(world, sink)?;
        WorldShutdown::drain(world, sink)?;
        WorldShutdown::finalize(world, sink)?;
        Ok(())
    }
}

fn intern_cause(cause: &'static str) -> Result<&'static str, WorldError> {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == cause)
        .ok_or_else(WorldError::invalid_handle)
}

fn shutdown_admissible(world: &VoxelWorld) -> bool {
    already_in_sequence(world) || can_drain(world)
}

fn already_in_sequence(world: &VoxelWorld) -> bool {
    let current = world.instance.state.current();
    current == intern_or("Draining")
        || current == intern_or("Snapshotted")
        || current == intern_or("Disposed")
}

fn already_disposed(world: &VoxelWorld) -> bool {
    world.instance.state.current() == intern_or("Disposed")
}

fn can_drain(world: &VoxelWorld) -> bool {
    let Some(event) = intern_session_event("Drain") else {
        return false;
    };
    let Some(to) = intern_session_state("Draining") else {
        return false;
    };
    has_session_edge(world.instance.state.current(), event, to)
}

fn intern_or(name: &'static str) -> &'static str {
    intern_session_state(name)
        .unwrap_or_else(|| panic!("{name} must exist as a generated SimulationSession state"))
}

fn bundle(
    world: &VoxelWorld,
    cause: &'static str,
    evidence: &FaultEvidence,
) -> FailureBundleFragment {
    let capture = world.instance.authority.capture();
    FailureBundleFragment::simulation(
        cause,
        world.instance.world_id.clone(),
        world.instance.world_context_id.clone(),
        world.instance.generation,
        world.instance.state.current(),
        capture.stamp().world_revision,
        evidence.diagnostic_name,
    )
}
