//! Interned `SimulationSession` mapping. Not `WorldSlotHost` or the section residency machine.

#![forbid(unsafe_code)]

use super::WorldError;
use lumio_voxel_contracts::{MACHINE_IDS, state_transition_table};

pub(crate) struct WorldState {
    current: &'static str,
}

impl WorldState {
    pub(crate) fn created() -> Self {
        Self {
            current: intern_session_state("Created")
                .expect("Created is a generated SimulationSession state"),
        }
    }

    pub(crate) fn current(&self) -> &'static str {
        self.current
    }

    pub(crate) fn apply(
        &mut self,
        event: &'static str,
        to: &'static str,
    ) -> Result<(), WorldError> {
        if !has_session_edge(self.current, event, to) {
            return Err(WorldError::invalid_handle());
        }
        self.current = to;
        Ok(())
    }
}

pub(crate) fn simulation_session_machine() -> &'static str {
    MACHINE_IDS
        .iter()
        .copied()
        .find(|id| *id == "SimulationSession")
        .expect("SimulationSession must exist in generated MACHINE_IDS")
}

pub(crate) fn intern_session_state(name: &str) -> Option<&'static str> {
    let machine = simulation_session_machine();
    state_transition_table().iter().find_map(|transition| {
        if transition.machine != machine {
            return None;
        }
        if transition.from == name {
            Some(transition.from)
        } else if transition.to == name {
            Some(transition.to)
        } else {
            None
        }
    })
}

pub(crate) fn intern_session_event(name: &str) -> Option<&'static str> {
    let machine = simulation_session_machine();
    state_transition_table().iter().find_map(|transition| {
        if transition.machine == machine && transition.event == name {
            Some(transition.event)
        } else {
            None
        }
    })
}

pub(crate) fn has_session_edge(from: &'static str, event: &'static str, to: &'static str) -> bool {
    let machine = simulation_session_machine();
    state_transition_table().iter().any(|transition| {
        transition.machine == machine
            && transition.from == from
            && transition.event == event
            && transition.to == to
    })
}

pub(crate) fn query_admissible(state: &'static str) -> bool {
    state == intern_or_bug("Ready")
        || state == intern_or_bug("Running")
        || state == intern_or_bug("Paused")
}

pub(crate) fn write_admissible(state: &'static str) -> bool {
    state == intern_or_bug("Running")
}

fn intern_or_bug(name: &'static str) -> &'static str {
    intern_session_state(name)
        .unwrap_or_else(|| panic!("{name} must exist as a generated SimulationSession state"))
}
