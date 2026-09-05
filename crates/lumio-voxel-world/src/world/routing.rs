//! Acquire a write lease, enter a typed Barrier, then release occupancy.

#![forbid(unsafe_code)]

use super::WorldError;
use super::admission::{AdmittedCommand, WorldCommand};
use super::barrier::BarrierScope;
use super::command::RoutedCommand;
use super::instance::VoxelWorld;
use super::write_lane::{WorldWriteLane, WriteLease};
use lumio_voxel_ops::async_support::{
    CompletionDisposition, OriginEnvelope, OriginToken, validate_completion,
};
use lumio_voxel_ops::mutation::{GeneratedMutationReceipt, MutationRequest, PreparedMutation};
use lumio_voxel_ops::query::{GeneratedVoxelQueryOutcome, GeneratedVoxelQueryRequest};
use std::collections::BTreeMap;

/// Routes admitted query / prepare / commit / abort through the serial write lane.
pub struct WorldRouter;

impl WorldRouter {
    pub fn query(
        world: &mut VoxelWorld,
        envelope: OriginEnvelope<GeneratedVoxelQueryRequest>,
    ) -> Result<OriginEnvelope<GeneratedVoxelQueryOutcome>, WorldError> {
        check_config_hash(world, &envelope.config_hash)?;
        let origin = envelope.origin.clone();
        let admitted = world.endpoint().admit(WorldCommand::Query {
            origin: origin.clone(),
            request: envelope.payload.clone(),
        })?;
        let routed = RoutedCommand::query(admitted, envelope)?;
        if !matches!(routed.admitted(), AdmittedCommand::Query) {
            return Err(WorldError::invalid_handle());
        }
        let payload = with_scope(world, BarrierScope::CaptureCut, |lease| {
            lease.query(&routed.envelope().payload)
        })?;
        Ok(complete(world, origin, payload))
    }

    pub fn prepare(
        world: &mut VoxelWorld,
        envelope: OriginEnvelope<MutationRequest>,
    ) -> Result<OriginEnvelope<PreparedMutation>, WorldError> {
        check_config_hash(world, &envelope.config_hash)?;
        let origin = envelope.origin.clone();
        let admitted = world.endpoint().admit(WorldCommand::Mutation {
            origin: origin.clone(),
            request: envelope.payload.clone(),
        })?;
        let routed = RoutedCommand::mutation(admitted, envelope)?;
        if !matches!(routed.admitted(), AdmittedCommand::Mutation) {
            return Err(WorldError::invalid_handle());
        }
        let payload = with_scope(world, BarrierScope::Mutation, |lease| {
            lease.prepare(&routed.envelope().payload)
        })?;
        Ok(complete(world, origin, payload))
    }

    pub fn commit(
        world: &mut VoxelWorld,
        envelope: OriginEnvelope<PreparedMutation>,
    ) -> Result<OriginEnvelope<GeneratedMutationReceipt>, WorldError> {
        check_config_hash(world, &envelope.config_hash)?;
        require_accept(validate_completion(
            &expected_origin(world, envelope.origin.apply_phase())?,
            &envelope.origin,
        ))?;
        let origin = envelope.origin.clone();
        let admit_req = MutationRequest {
            txn_id: envelope.payload.txn_id().to_string(),
            world_id: world.instance.world_id.clone(),
            generation: world.instance.generation,
            entries: Vec::new(),
        };
        let admitted = world.endpoint().admit(WorldCommand::Mutation {
            origin: origin.clone(),
            request: admit_req,
        })?;
        let routed = RoutedCommand::commit(admitted, envelope)?;
        if !matches!(routed.admitted(), AdmittedCommand::Mutation) {
            return Err(WorldError::invalid_handle());
        }
        let prepared = routed.into_envelope().payload;
        let payload = with_scope(world, BarrierScope::Mutation, |lease| {
            lease.commit(prepared)
        })?;
        Ok(complete(world, origin, payload))
    }

    pub fn abort(
        world: &mut VoxelWorld,
        envelope: OriginEnvelope<MutationRequest>,
    ) -> Result<OriginEnvelope<()>, WorldError> {
        check_config_hash(world, &envelope.config_hash)?;
        require_accept(validate_completion(
            &expected_origin(world, envelope.origin.apply_phase())?,
            &envelope.origin,
        ))?;
        let origin = envelope.origin.clone();
        let admitted = world.endpoint().admit(WorldCommand::Mutation {
            origin: origin.clone(),
            request: envelope.payload.clone(),
        })?;
        let routed = RoutedCommand::mutation(admitted, envelope)?;
        if !matches!(routed.admitted(), AdmittedCommand::Mutation) {
            return Err(WorldError::invalid_handle());
        }
        with_scope(world, BarrierScope::Mutation, |lease| {
            lease.abort(&routed.envelope().payload)
        })?;
        Ok(complete(world, origin, ()))
    }
}

fn with_scope<R>(
    world: &mut VoxelWorld,
    scope: BarrierScope,
    work: impl FnOnce(&mut WriteLease<'_>) -> Result<R, WorldError>,
) -> Result<R, WorldError> {
    let mut lease = WorldWriteLane::try_acquire(world)?;
    lease.enter(scope)?;
    work(&mut lease)
}

fn complete<T>(world: &VoxelWorld, origin: OriginToken, payload: T) -> OriginEnvelope<T> {
    OriginEnvelope {
        origin,
        config_hash: world.instance.snapshot.config_hash().to_string(),
        payload,
    }
}

fn check_config_hash(world: &VoxelWorld, hash: &str) -> Result<(), WorldError> {
    if hash != world.instance.snapshot.config_hash() {
        return Err(WorldError::session_mismatch());
    }
    Ok(())
}

fn expected_origin(world: &VoxelWorld, phase: &'static str) -> Result<OriginToken, WorldError> {
    OriginToken::try_new(
        world.instance.world_context_id.clone(),
        world.instance.generation,
        "write-lane-basis",
        0,
        BTreeMap::new(),
        phase,
    )
    .map_err(|err| WorldError::mapped(err.error_id()))
}

fn require_accept(disposition: CompletionDisposition) -> Result<(), WorldError> {
    match disposition {
        CompletionDisposition::Accept => Ok(()),
        CompletionDisposition::Stale | CompletionDisposition::Late => {
            Err(WorldError::stale_epoch())
        }
        CompletionDisposition::WrongWorld => Err(WorldError::session_mismatch()),
        CompletionDisposition::Duplicate | CompletionDisposition::Cancelled => {
            Err(WorldError::invalid_handle())
        }
    }
}
