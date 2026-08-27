//! Total adapter routing generated voxel-world-port requests to WorldEndpoint/Barrier.

#![forbid(unsafe_code)]

use super::error_mapping::PortError;
use crate::world::{
    AckEvidence, AdmittedCommand, CaptureEvidence, DurabilityReceipt, RestoreReceipt,
    RuntimeSnapshotCut, VoxelWorld, WorldCommand, WorldEventSink, WorldRouter, WorldShutdown,
};
use lumio_voxel_contracts::{BINDINGS, SCHEMA_IDS};
use lumio_voxel_ops::async_support::OriginEnvelope;
use lumio_voxel_ops::mutation::{GeneratedMutationReceipt, MutationRequest, PreparedMutation};
use lumio_voxel_ops::query::{GeneratedVoxelQueryOutcome, GeneratedVoxelQueryRequest};
use lumio_voxel_ops::snapshot::{SealedRestoreCandidate, VoxelCaptureRef};

const PORT_SCHEMA: &str = "voxel-world-port";
const PORT_RUST_TYPE: &str = "VoxelWorldPort";

/// Interned schema id plus generated rust binding name for `voxel-world-port`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortEvidence {
    pub schema_id: &'static str,
    pub binding_rust_type: &'static str,
}

/// Total adapter over one `VoxelWorld`. No extra interior mutability and no callbacks.
pub struct GeneratedVoxelWorldPortAdapter<'a> {
    world: &'a mut VoxelWorld,
}

impl<'a> GeneratedVoxelWorldPortAdapter<'a> {
    pub fn new(world: &'a mut VoxelWorld) -> Self {
        let _ = intern_schema();
        let _ = intern_binding();
        Self { world }
    }

    pub fn schema_id(&self) -> &'static str {
        intern_schema()
    }

    pub fn evidence(&self) -> PortEvidence {
        PortEvidence {
            schema_id: intern_schema(),
            binding_rust_type: intern_binding(),
        }
    }

    pub fn admit(&mut self, command: WorldCommand) -> Result<AdmittedCommand, PortError> {
        self.world
            .endpoint()
            .admit(command)
            .map_err(PortError::from)
    }

    pub fn query(
        &mut self,
        envelope: OriginEnvelope<GeneratedVoxelQueryRequest>,
    ) -> Result<OriginEnvelope<GeneratedVoxelQueryOutcome>, PortError> {
        WorldRouter::query(self.world, envelope).map_err(PortError::from)
    }

    pub fn prepare_mutation(
        &mut self,
        envelope: OriginEnvelope<MutationRequest>,
    ) -> Result<OriginEnvelope<PreparedMutation>, PortError> {
        WorldRouter::prepare(self.world, envelope).map_err(PortError::from)
    }

    pub fn commit(
        &mut self,
        envelope: OriginEnvelope<PreparedMutation>,
    ) -> Result<OriginEnvelope<GeneratedMutationReceipt>, PortError> {
        WorldRouter::commit(self.world, envelope).map_err(PortError::from)
    }

    pub fn abort(
        &mut self,
        envelope: OriginEnvelope<MutationRequest>,
    ) -> Result<OriginEnvelope<()>, PortError> {
        WorldRouter::abort(self.world, envelope).map_err(PortError::from)
    }

    pub fn capture(
        &mut self,
        cut: &RuntimeSnapshotCut,
    ) -> Result<(VoxelCaptureRef, CaptureEvidence), PortError> {
        crate::world::capture(self.world, cut).map_err(PortError::from)
    }

    pub fn restore(
        &mut self,
        candidate: SealedRestoreCandidate,
    ) -> Result<RestoreReceipt, PortError> {
        crate::world::restore(self.world, candidate).map_err(PortError::from)
    }

    pub fn apply_durability_ack(
        &mut self,
        ack: AckEvidence,
    ) -> Result<DurabilityReceipt, PortError> {
        crate::world::apply_durability_ack(self.world, ack).map_err(PortError::from)
    }

    pub fn shutdown(&mut self, sink: &mut WorldEventSink) -> Result<(), PortError> {
        WorldShutdown::begin(self.world, sink).map_err(PortError::from)?;
        WorldShutdown::drain(self.world, sink).map_err(PortError::from)?;
        WorldShutdown::finalize(self.world, sink).map_err(PortError::from)?;
        Ok(())
    }
}

fn intern_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == PORT_SCHEMA)
        .expect("voxel-world-port must exist in generated SCHEMA_IDS")
}

fn intern_binding() -> &'static str {
    BINDINGS
        .iter()
        .find(|binding| binding.schema_id == PORT_SCHEMA && binding.rust_type == PORT_RUST_TYPE)
        .map(|binding| binding.rust_type)
        .expect("generated BINDINGS must intern rust_type VoxelWorldPort")
}
