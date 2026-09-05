//! Per-instance serial write occupancy. Not a process-global lock.

#![forbid(unsafe_code)]

use super::WorldError;
use super::admission::WorldCommand;
use super::barrier::{BarrierScope, admit_scope};
use super::instance::VoxelWorld;
use lumio_voxel_ops::async_support::OriginToken;
use lumio_voxel_ops::mutation::{
    GeneratedMutationReceipt, MutationError, MutationRequest, PreparedMutation, commit, prepare,
};
use lumio_voxel_ops::query::{
    GeneratedVoxelQueryOutcome, GeneratedVoxelQueryRequest, QueryError, QueryExecutor,
};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Per-World write occupancy factory. Two `VoxelWorld` values do not share a lane.
pub struct WorldWriteLane;

/// Exclusive write occupancy. Not `Clone`. `PhantomData<*mut ()>` makes the
/// lease `!Send`/`!Sync` so it cannot move to another thread even if
/// `VoxelWorld` auto-traits would otherwise allow it.
#[must_use = "the write lease occupies the world until dropped"]
pub struct WriteLease<'a> {
    world: &'a mut VoxelWorld,
    scope: Option<BarrierScope>,
    _not_send: PhantomData<*mut ()>,
}

impl WorldWriteLane {
    pub fn try_acquire(world: &mut VoxelWorld) -> Result<WriteLease<'_>, WorldError> {
        if world.instance.write_occupied {
            return Err(WorldError::mapped("HandleDoubleRelease"));
        }
        world.instance.write_occupied = true;
        Ok(WriteLease {
            world,
            scope: None,
            _not_send: PhantomData,
        })
    }
}

impl Drop for WriteLease<'_> {
    fn drop(&mut self) {
        self.scope = None;
        self.world.instance.write_occupied = false;
    }
}

impl std::fmt::Debug for WriteLease<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteLease")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl WriteLease<'_> {
    pub fn enter(&mut self, scope: BarrierScope) -> Result<(), WorldError> {
        if self.scope.is_some() {
            return Err(WorldError::invalid_handle());
        }
        admit_scope(self.world, scope)?;
        self.scope = Some(scope);
        Ok(())
    }

    pub fn prepare(&mut self, request: &MutationRequest) -> Result<PreparedMutation, WorldError> {
        self.require_scope(BarrierScope::Mutation)?;
        re_admit_mutation(self.world, request)?;
        let view = self.world.publication_authority().capture();
        prepare(request, &view, self.world.ledger_mut()).map_err(map_mutation)
    }

    pub fn commit(
        &mut self,
        prepared: PreparedMutation,
    ) -> Result<GeneratedMutationReceipt, WorldError> {
        self.require_scope(BarrierScope::Mutation)?;
        let request = MutationRequest {
            txn_id: prepared.txn_id().to_string(),
            world_id: self.world.instance.world_id.clone(),
            generation: self.world.instance.generation,
            entries: Vec::new(),
        };
        re_admit_mutation(self.world, &request)?;
        commit(
            prepared,
            &self.world.instance.authority,
            &mut self.world.instance.ledger,
        )
        .map_err(map_mutation)
    }

    pub fn abort(&mut self, request: &MutationRequest) -> Result<(), WorldError> {
        self.require_scope(BarrierScope::Mutation)?;
        re_admit_mutation(self.world, request)?;
        self.world
            .ledger_mut()
            .abort(request)
            .map_err(|err| WorldError::mapped(err.error_id()))
    }

    pub fn query(
        &mut self,
        request: &GeneratedVoxelQueryRequest,
    ) -> Result<GeneratedVoxelQueryOutcome, WorldError> {
        self.require_scope(BarrierScope::CaptureCut)?;
        re_admit_query(self.world, request)?;
        let snapshot = Arc::clone(&self.world.instance.snapshot);
        let view = self.world.instance.authority.capture();
        let plan = self
            .world
            .instance
            .query_planner
            .plan(request, &view, snapshot.as_ref())
            .map_err(map_query)?;
        let outcome = match self.world.instance.region_pins.as_ref() {
            Some(pins) => QueryExecutor::execute_with_presence_guard(&plan, &view, pins),
            None => QueryExecutor::execute(&plan, &view),
        };
        outcome.map_err(map_query)
    }

    fn require_scope(&self, expected: BarrierScope) -> Result<(), WorldError> {
        match self.scope {
            Some(scope) if scope == expected => Ok(()),
            _ => Err(WorldError::invalid_handle()),
        }
    }
}

fn re_admit_mutation(world: &mut VoxelWorld, request: &MutationRequest) -> Result<(), WorldError> {
    let origin = origin_for(world, &request.txn_id)?;
    world.endpoint().admit(WorldCommand::Mutation {
        origin,
        request: request.clone(),
    })?;
    Ok(())
}

fn re_admit_query(
    world: &mut VoxelWorld,
    request: &GeneratedVoxelQueryRequest,
) -> Result<(), WorldError> {
    let origin = origin_for(world, &request.query_id)?;
    world.endpoint().admit(WorldCommand::Query {
        origin,
        request: request.clone(),
    })?;
    Ok(())
}

fn origin_for(world: &VoxelWorld, request_id: &str) -> Result<OriginToken, WorldError> {
    OriginToken::try_new(
        world.instance.world_context_id.clone(),
        world.instance.generation,
        request_id,
        0,
        BTreeMap::new(),
        "VoxelCommit",
    )
    .map_err(|err| WorldError::mapped(err.error_id()))
}

fn map_mutation(err: MutationError) -> WorldError {
    WorldError::mapped(err.error_id())
}

fn map_query(err: QueryError) -> WorldError {
    WorldError::mapped(err.error_id())
}
