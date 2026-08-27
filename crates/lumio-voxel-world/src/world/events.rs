//! Bounded external event port. Overflow drops events and never vetoes domain state.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::SCHEMA_IDS;
use std::collections::VecDeque;

pub(crate) const FAILURE_BUNDLE_SCHEMA: &str = "failure-bundle";
pub(crate) const LOGGING_EVENT_SCHEMA: &str = "logging-event";
pub(crate) const INCIDENT_SIMULATION: &str = "Simulation";

/// Generated `failure-bundle` fragment. No keys and no raw payloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureBundleFragment {
    schema_id: &'static str,
    incident_kind: &'static str,
    error_id: &'static str,
    world_id: String,
    world_context_id: String,
    generation: u64,
    lifecycle: &'static str,
    last_world_revision: u64,
    diagnostic_name: &'static str,
}

impl FailureBundleFragment {
    pub fn schema_id(&self) -> &'static str {
        self.schema_id
    }

    pub fn incident_kind(&self) -> &'static str {
        self.incident_kind
    }

    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn world_context_id(&self) -> &str {
        &self.world_context_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn lifecycle(&self) -> &'static str {
        self.lifecycle
    }

    pub fn last_world_revision(&self) -> u64 {
        self.last_world_revision
    }

    pub fn diagnostic_name(&self) -> &'static str {
        self.diagnostic_name
    }

    pub(crate) fn simulation(
        error_id: &'static str,
        world_id: String,
        world_context_id: String,
        generation: u64,
        lifecycle: &'static str,
        last_world_revision: u64,
        diagnostic_name: &'static str,
    ) -> Self {
        Self {
            schema_id: intern_schema(FAILURE_BUNDLE_SCHEMA),
            incident_kind: INCIDENT_SIMULATION,
            error_id,
            world_id,
            world_context_id,
            generation,
            lifecycle,
            last_world_revision,
            diagnostic_name,
        }
    }
}

/// Generated `logging-event` fragment. Identifiers only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldEvent {
    Logging {
        schema_id: &'static str,
        event: &'static str,
        lifecycle: &'static str,
        generation: u64,
    },
    Failure(FailureBundleFragment),
}

/// External observation port. Emit failure cannot change commit or lifecycle.
pub struct WorldEventSink {
    capacity: usize,
    events: VecDeque<WorldEvent>,
    dropped: usize,
}

impl WorldEventSink {
    pub fn bounded(capacity: usize) -> Self {
        Self {
            capacity,
            events: VecDeque::new(),
            dropped: 0,
        }
    }

    /// Push one event. A full buffer drops this event and leaves domain state alone.
    pub fn emit(&mut self, event: WorldEvent) {
        if self.capacity == 0 || self.events.len() >= self.capacity {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.events.push_back(event);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn events(&self) -> impl Iterator<Item = &WorldEvent> {
        self.events.iter()
    }
}

pub(crate) fn intern_schema(id: &'static str) -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped schema id must exist in generated SCHEMA_IDS")
}

pub(crate) fn logging_event(
    event: &'static str,
    lifecycle: &'static str,
    generation: u64,
) -> WorldEvent {
    WorldEvent::Logging {
        schema_id: intern_schema(LOGGING_EVENT_SCHEMA),
        event,
        lifecycle,
        generation,
    }
}
