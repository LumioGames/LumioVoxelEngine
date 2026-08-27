//! Typed command admission. Writes never enter a write path unless Running.

#![forbid(unsafe_code)]

use super::WorldError;
use super::instance::VoxelWorld;
use super::state::{
    intern_session_event, intern_session_state, query_admissible, write_admissible,
};
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::MutationRequest;
use lumio_voxel_ops::query::GeneratedVoxelQueryRequest;

#[derive(Debug)]
pub enum WorldCommand {
    Lifecycle {
        event: &'static str,
        to: &'static str,
        origin: OriginToken,
    },
    Query {
        origin: OriginToken,
        request: GeneratedVoxelQueryRequest,
    },
    Mutation {
        origin: OriginToken,
        request: MutationRequest,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmittedCommand {
    Lifecycle {
        from: &'static str,
        event: &'static str,
        to: &'static str,
    },
    Query,
    Mutation,
}

pub struct WorldEndpoint<'a> {
    pub(crate) world: &'a mut VoxelWorld,
}

impl WorldEndpoint<'_> {
    pub fn admit(&mut self, command: WorldCommand) -> Result<AdmittedCommand, WorldError> {
        let instance = &mut self.world.instance;
        match command {
            WorldCommand::Lifecycle { event, to, origin } => {
                instance.generation_guard().check_origin(&origin)?;
                let event = intern_session_event(event).ok_or_else(WorldError::invalid_handle)?;
                let to = intern_session_state(to).ok_or_else(WorldError::invalid_handle)?;
                let from = instance.state.current();
                instance.state.apply(event, to)?;
                Ok(AdmittedCommand::Lifecycle { from, event, to })
            }
            WorldCommand::Query { origin, request } => {
                instance.generation_guard().check_origin(&origin)?;
                if request.query_id.is_empty()
                    || request.world_id.is_empty()
                    || request.context.is_empty()
                {
                    return Err(WorldError::invalid_handle());
                }
                if request.world_id != instance.world_id {
                    return Err(WorldError::session_mismatch());
                }
                if request.context != instance.world_context_id {
                    return Err(WorldError::session_mismatch());
                }
                if !query_admissible(instance.state.current()) {
                    return Err(WorldError::claim_not_granted());
                }
                if instance.query_planner.config_hash() != instance.snapshot.config_hash() {
                    return Err(WorldError::invalid_handle());
                }
                Ok(AdmittedCommand::Query)
            }
            WorldCommand::Mutation { origin, request } => {
                instance.generation_guard().check_origin(&origin)?;
                if request.txn_id.is_empty() || request.world_id.is_empty() {
                    return Err(WorldError::invalid_handle());
                }
                if request.world_id != instance.world_id {
                    return Err(WorldError::session_mismatch());
                }
                if request.generation != instance.generation {
                    return Err(WorldError::stale_epoch());
                }
                if !write_admissible(instance.state.current()) {
                    return Err(WorldError::claim_not_granted());
                }
                if instance.ledger.config_hash() != instance.snapshot.config_hash() {
                    return Err(WorldError::invalid_handle());
                }
                Ok(AdmittedCommand::Mutation)
            }
        }
    }
}
