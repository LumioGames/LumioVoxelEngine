//! Ordered SimulationSession close. Drain → FinalSnapshotTaken → Dispose.

#![forbid(unsafe_code)]

use super::WorldError;
use super::diagnostics::WorldDiagnostics;
use super::events::{WorldEventSink, logging_event};
use super::instance::VoxelWorld;
use super::state::{has_session_edge, intern_session_event, intern_session_state};

pub struct WorldShutdown;

impl WorldShutdown {
    /// Close ingress and reject new writes via generated `Drain`.
    pub fn begin(world: &mut VoxelWorld, sink: &mut WorldEventSink) -> Result<(), WorldError> {
        if sequence_reached(world, "Draining") {
            return Ok(());
        }
        apply_generated(world, "Drain", "Draining")?;
        emit_phase(world, sink, "Drain");
        Ok(())
    }

    /// Abort in-flight reservations, drop jobs, export diagnostics, take the final snapshot.
    pub fn drain(world: &mut VoxelWorld, sink: &mut WorldEventSink) -> Result<(), WorldError> {
        if sequence_reached(world, "Snapshotted") {
            return Ok(());
        }
        if !can_apply(world, "FinalSnapshotTaken", "Snapshotted") {
            return Err(WorldError::invalid_handle());
        }
        world.instance.ledger.abort_in_flight();
        let _exported = WorldDiagnostics::snapshot(world);
        apply_generated(world, "FinalSnapshotTaken", "Snapshotted")?;
        emit_phase(world, sink, "FinalSnapshotTaken");
        Ok(())
    }

    /// Release occupancy, dispose the session, and freeze the old generation.
    pub fn finalize(world: &mut VoxelWorld, sink: &mut WorldEventSink) -> Result<(), WorldError> {
        if sequence_reached(world, "Disposed") {
            return Ok(());
        }
        if !can_apply(world, "Dispose", "Disposed") {
            return Err(WorldError::invalid_handle());
        }
        let old_generation = world.instance.generation;
        apply_generated(world, "Dispose", "Disposed")?;
        world.instance.write_occupied = false;
        invalidate_generation(world);
        emit_logging(
            sink,
            "Dispose",
            world.instance.state.current(),
            old_generation,
        );
        Ok(())
    }
}

fn emit_phase(world: &VoxelWorld, sink: &mut WorldEventSink, event: &'static str) {
    emit_logging(
        sink,
        event,
        world.instance.state.current(),
        world.instance.generation,
    );
}

fn emit_logging(
    sink: &mut WorldEventSink,
    event: &'static str,
    lifecycle: &'static str,
    generation: u64,
) {
    let event = intern_session_event(event).expect("shutdown event is generated");
    sink.emit(logging_event(event, lifecycle, generation));
}

fn apply_generated(
    world: &mut VoxelWorld,
    event: &'static str,
    to: &'static str,
) -> Result<(), WorldError> {
    let event = intern_session_event(event).ok_or_else(WorldError::invalid_handle)?;
    let to = intern_session_state(to).ok_or_else(WorldError::invalid_handle)?;
    world.instance.state.apply(event, to)
}

fn can_apply(world: &VoxelWorld, event: &'static str, to: &'static str) -> bool {
    let Some(event) = intern_session_event(event) else {
        return false;
    };
    let Some(to) = intern_session_state(to) else {
        return false;
    };
    has_session_edge(world.instance.state.current(), event, to)
}

fn sequence_reached(world: &VoxelWorld, target: &'static str) -> bool {
    let Some(target) = intern_session_state(target) else {
        return false;
    };
    let current = world.instance.state.current();
    if current == target {
        return true;
    }
    match target {
        name if name == intern_or("Draining") => {
            current == intern_or("Snapshotted") || current == intern_or("Disposed")
        }
        name if name == intern_or("Snapshotted") => current == intern_or("Disposed"),
        _ => false,
    }
}

fn intern_or(name: &'static str) -> &'static str {
    intern_session_state(name)
        .unwrap_or_else(|| panic!("{name} must exist as a generated SimulationSession state"))
}

fn invalidate_generation(world: &mut VoxelWorld) {
    let next = world.instance.generation.wrapping_add(1);
    world.instance.generation = if next == 0 { 1 } else { next };
}
