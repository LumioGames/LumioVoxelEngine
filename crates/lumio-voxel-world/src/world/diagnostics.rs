//! Per-instance diagnostics snapshot. Not a recovery record.

#![forbid(unsafe_code)]

use super::instance::VoxelWorld;
use super::state::simulation_session_machine;

/// Frozen view of one World's health. Does not include keys or payloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticsView {
    lifecycle: &'static str,
    lifecycle_machine: &'static str,
    generation: u64,
    world_id: String,
    world_context_id: String,
    last_world_revision: u64,
    published_root: [u8; 32],
    in_flight_reservations: usize,
    write_occupied: bool,
}

impl DiagnosticsView {
    pub fn lifecycle(&self) -> &'static str {
        self.lifecycle
    }

    pub fn lifecycle_machine(&self) -> &'static str {
        self.lifecycle_machine
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn world_context_id(&self) -> &str {
        &self.world_context_id
    }

    pub fn last_world_revision(&self) -> u64 {
        self.last_world_revision
    }

    pub fn published_root(&self) -> [u8; 32] {
        self.published_root
    }

    pub fn in_flight_reservations(&self) -> usize {
        self.in_flight_reservations
    }

    pub fn write_occupied(&self) -> bool {
        self.write_occupied
    }
}

pub struct WorldDiagnostics;

impl WorldDiagnostics {
    pub fn snapshot(world: &VoxelWorld) -> DiagnosticsView {
        let capture = world.instance.authority.capture();
        DiagnosticsView {
            lifecycle: world.instance.state.current(),
            lifecycle_machine: simulation_session_machine(),
            generation: world.instance.generation,
            world_id: world.instance.world_id.clone(),
            world_context_id: world.instance.world_context_id.clone(),
            last_world_revision: capture.stamp().world_revision,
            published_root: capture.root().identity(),
            in_flight_reservations: world.instance.ledger.in_flight_count(),
            write_occupied: world.instance.write_occupied,
        }
    }
}
