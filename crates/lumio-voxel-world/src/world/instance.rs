//! Per-instance VoxelWorld: generation factory, owned publication stack, no Host slot.

#![forbid(unsafe_code)]

use super::WorldError;
use super::admission::WorldEndpoint;
use super::state::{WorldState, simulation_session_machine};
use lumio_voxel_contracts::{SCHEMA_IDS, VOXEL_WORLD_ROLES};
use lumio_voxel_domain::config_snapshot::{
    CapabilityView, GeneratedHostCapability, VoxelConfigSnapshot,
};
use lumio_voxel_domain::publication::{PublicationAuthority, PublishedStateRoot};
use lumio_voxel_domain::revision::{GeneratedRevisionStamp, PinRegistry, REVISION_STAMP_SCHEMA};
use lumio_voxel_domain::section::{DirtyFrontier, SectionDirectoryBuilder};
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::ReceiptLedger;
use lumio_voxel_ops::query::{QueryExecutor, QueryPlanner};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PIN_CAPACITY: usize = 16;
const LEDGER_CAPACITY: usize = 16;
const QUERY_SECTION_CAPACITY: usize = 16;

static NEXT_INSTANCE_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Adapter fields wrapping generated Role / Context / Capability / worldId.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldDescriptor {
    pub role: String,
    pub world_context_id: String,
    pub capabilities: Vec<String>,
    pub config: WorldConfigAdapter,
}

/// Generated `worldId` plus create-time adapter config. Not a new Schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldConfigAdapter {
    pub world_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldStateView {
    lifecycle: &'static str,
    machine: &'static str,
    role: &'static str,
    world_context_id: String,
    world_id: String,
    instance_generation: u64,
}

impl WorldStateView {
    pub fn lifecycle(&self) -> &'static str {
        self.lifecycle
    }

    pub fn lifecycle_machine(&self) -> &'static str {
        self.machine
    }

    pub fn role(&self) -> &'static str {
        self.role
    }

    pub fn world_context_id(&self) -> &str {
        &self.world_context_id
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn instance_generation(&self) -> u64 {
        self.instance_generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceGenerationGuard {
    world_context_id: String,
    generation: u64,
}

impl InstanceGenerationGuard {
    pub fn world_context_id(&self) -> &str {
        &self.world_context_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn check_origin(&self, origin: &OriginToken) -> Result<(), WorldError> {
        if origin.world_context_id().is_empty() || origin.request_id().is_empty() {
            return Err(WorldError::invalid_handle());
        }
        if origin.world_context_id() != self.world_context_id {
            return Err(WorldError::session_mismatch());
        }
        if origin.instance_generation() != self.generation {
            return Err(WorldError::stale_epoch());
        }
        Ok(())
    }
}

pub(crate) struct WorldInstance {
    pub(crate) role: &'static str,
    pub(crate) world_id: String,
    pub(crate) world_context_id: String,
    pub(crate) generation: u64,
    pub(crate) snapshot: Arc<VoxelConfigSnapshot>,
    pub(crate) state: WorldState,
    pub(crate) authority: PublicationAuthority,
    pub(crate) ledger: ReceiptLedger,
    pub(crate) query_planner: QueryPlanner,
    pub(crate) write_occupied: bool,
}

impl WorldInstance {
    pub(crate) fn generation_guard(&self) -> InstanceGenerationGuard {
        InstanceGenerationGuard {
            world_context_id: self.world_context_id.clone(),
            generation: self.generation,
        }
    }

    pub(crate) fn state_view(&self) -> WorldStateView {
        WorldStateView {
            lifecycle: self.state.current(),
            machine: simulation_session_machine(),
            role: self.role,
            world_context_id: self.world_context_id.clone(),
            world_id: self.world_id.clone(),
            instance_generation: self.generation,
        }
    }
}

/// One Authority or Replica tree. Host Session / WorldSlot are not owned here.
pub struct VoxelWorld {
    pub(crate) instance: WorldInstance,
}

impl std::fmt::Debug for VoxelWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelWorld")
            .field("role", &self.instance.role)
            .field("world_id", &self.instance.world_id)
            .field("world_context_id", &self.instance.world_context_id)
            .field("generation", &self.instance.generation)
            .field("lifecycle", &self.instance.state.current())
            .finish()
    }
}

impl VoxelWorld {
    pub fn create(
        descriptor: WorldDescriptor,
        snapshot: Arc<VoxelConfigSnapshot>,
    ) -> Result<Self, WorldError> {
        let _ = world_port_schema();
        let role = intern_role(&descriptor.role)?;
        if descriptor.world_context_id.is_empty() || descriptor.config.world_id.is_empty() {
            return Err(WorldError::invalid_handle());
        }
        validate_capabilities(&descriptor.capabilities, snapshot.as_ref())?;

        let generation = allocate_generation()?;
        let world_id = descriptor.config.world_id;
        let world_context_id = descriptor.world_context_id;
        let pins = PinRegistry::from_approved_snapshot(
            Arc::clone(&snapshot),
            PIN_CAPACITY,
            world_context_id.as_str(),
            generation,
        );
        let root = initial_root(&world_id, &world_context_id, generation)?;
        let authority = PublicationAuthority::new(
            world_id.clone(),
            world_context_id.clone(),
            generation,
            pins,
            root,
        )
        .map_err(|err| WorldError::mapped(err.error_id()))?;
        let ledger = ReceiptLedger::from_approved_snapshot(Arc::clone(&snapshot), LEDGER_CAPACITY)
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let query_planner = bind_query_planner(Arc::clone(&snapshot))?;

        Ok(Self {
            instance: WorldInstance {
                role,
                world_id,
                world_context_id,
                generation,
                snapshot,
                state: WorldState::created(),
                authority,
                ledger,
                query_planner,
                write_occupied: false,
            },
        })
    }

    pub fn endpoint(&mut self) -> WorldEndpoint<'_> {
        WorldEndpoint { world: self }
    }

    pub fn state_view(&self) -> WorldStateView {
        self.instance.state_view()
    }

    pub fn generation_guard(&self) -> InstanceGenerationGuard {
        self.instance.generation_guard()
    }

    pub fn publication_authority(&self) -> &PublicationAuthority {
        &self.instance.authority
    }

    pub(crate) fn ledger_mut(&mut self) -> &mut ReceiptLedger {
        &mut self.instance.ledger
    }
}

pub fn intern_role(role: &str) -> Result<&'static str, WorldError> {
    if role.is_empty() {
        return Err(WorldError::invalid_handle());
    }
    VOXEL_WORLD_ROLES
        .iter()
        .copied()
        .find(|item| *item == role)
        .ok_or_else(WorldError::role_mismatch)
}

pub fn intern_local_embedded_pair(
    first: &str,
    second: &str,
) -> Result<(&'static str, &'static str), WorldError> {
    let first = intern_role(first)?;
    let second = intern_role(second)?;
    if first == second {
        return Err(WorldError::role_mismatch());
    }
    Ok((first, second))
}

fn allocate_generation() -> Result<u64, WorldError> {
    let generation = NEXT_INSTANCE_GENERATION.fetch_add(1, Ordering::Relaxed);
    if generation == 0 {
        Err(WorldError::invalid_handle())
    } else {
        Ok(generation)
    }
}

fn world_port_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == "voxel-world-port")
        .expect("voxel-world-port must exist in generated SCHEMA_IDS")
}

fn validate_capabilities(
    requested: &[String],
    snapshot: &VoxelConfigSnapshot,
) -> Result<(), WorldError> {
    if requested.is_empty() {
        return Err(WorldError::claim_not_granted());
    }
    let generated = GeneratedHostCapability::from_names(requested.iter().cloned());
    let _view = CapabilityView::derive(&generated, snapshot)
        .map_err(|err| WorldError::mapped(err.error_id()))?;
    Ok(())
}

fn bind_query_planner(snapshot: Arc<VoxelConfigSnapshot>) -> Result<QueryPlanner, WorldError> {
    let planner = QueryPlanner::from_approved_snapshot(snapshot, QUERY_SECTION_CAPACITY)
        .map_err(|err| WorldError::mapped(err.error_id()))?;
    let _: QueryExecutor = QueryExecutor;
    Ok(planner)
}

fn initial_root(
    world_id: &str,
    context_id: &str,
    generation: u64,
) -> Result<PublishedStateRoot, WorldError> {
    let stamp = GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.to_string(),
        context_id: context_id.to_string(),
        generation,
        world_revision: 0,
        section_revision_set: BTreeMap::new(),
    };
    let directory = SectionDirectoryBuilder::new().freeze();
    let dirty = DirtyFrontier::new(world_id, generation)
        .map_err(|err| WorldError::mapped(err.error_id()))?;
    Ok(PublishedStateRoot::new(stamp, directory, dirty))
}
