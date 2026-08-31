//! Total adapter routing generated voxel-world-port requests to WorldEndpoint/Barrier.

#![forbid(unsafe_code)]

use super::error_mapping::PortError;
use crate::world::{
    AckEvidence, AdmittedCommand, CaptureEvidence, DurabilityReceipt, RestoreReceipt,
    RuntimeSnapshotCut, VoxelWorld, WorldCommand, WorldDescriptor, WorldEventSink, WorldRouter,
    WorldShutdown,
};
use lumio_voxel_contracts::{BINDINGS, SCHEMA_IDS};
use lumio_voxel_domain::config_snapshot::VoxelConfigSnapshot;
use lumio_voxel_ops::async_support::{OriginEnvelope, OriginToken};
use lumio_voxel_ops::mutation::{
    GeneratedMutationReceipt, MutationRequest, PreparedMutation, ReceiptStatus,
};
use lumio_voxel_ops::query::{GeneratedVoxelQueryOutcome, GeneratedVoxelQueryRequest};
use lumio_voxel_ops::snapshot::{SealedRestoreCandidate, VoxelCaptureRef};
use std::collections::BTreeMap;
use std::sync::Arc;

const PORT_SCHEMA: &str = "voxel-world-port";
const PORT_RUST_TYPE: &str = "VoxelWorldPort";

/// Method names frozen by the generated `voxel-world-port` contract.
///
/// This table is a conformance witness only; it does not define a second
/// schema or serializer. The source of truth remains the Architecture artifact.
pub const GENERATED_PORT_METHODS: &[&str; 11] = &[
    "createWorld",
    "query",
    "prepareMutation",
    "commit",
    "abort",
    "status",
    "capture",
    "applyDurabilityAck",
    "restore",
    "quiesce",
    "destroy",
];

/// Compatibility alias for callers that use the shorter Port terminology.
pub const PORT_METHODS: &[&str; 11] = GENERATED_PORT_METHODS;

/// Mutation status projection from the generated receipt contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationStatus {
    Unknown,
    Prepared,
    Applied,
    Aborted,
    ResultPruned,
}

impl MutationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Prepared => "Prepared",
            Self::Applied => "Applied",
            Self::Aborted => "Aborted",
            Self::ResultPruned => "ResultPruned",
        }
    }
}

impl From<ReceiptStatus> for MutationStatus {
    fn from(status: ReceiptStatus) -> Self {
        match status {
            ReceiptStatus::Unknown => Self::Unknown,
            ReceiptStatus::Prepared => Self::Prepared,
            ReceiptStatus::Applied => Self::Applied,
        }
    }
}

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
    /// Generated `createWorld` entry point. The returned world is owned by the
    /// caller and must subsequently be accessed through this adapter surface.
    pub fn create_world(
        descriptor: WorldDescriptor,
        snapshot: Arc<VoxelConfigSnapshot>,
    ) -> Result<VoxelWorld, PortError> {
        VoxelWorld::create(descriptor, snapshot).map_err(PortError::from)
    }

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

    /// Generated `status` entry point for a mutation transaction.
    pub fn status(&self, txn_id: &str) -> Result<MutationStatus, PortError> {
        if txn_id.is_empty() {
            return Err(super::error_mapping::map_internal_error("InvalidHandle"));
        }
        Ok(self.world.instance.ledger.status(txn_id).into())
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

    /// Quiesce ingress by applying the generated SimulationSession pause edge.
    /// The reason is an audit-side input and is intentionally not persisted in
    /// the frozen Port payload.
    pub fn quiesce(&mut self, reason: impl AsRef<str>) -> Result<(), PortError> {
        let reason = reason.as_ref();
        if reason.is_empty() {
            return Err(super::error_mapping::map_internal_error("InvalidArgument"));
        }
        if self.world.state_view().lifecycle() == "Paused" {
            return Ok(());
        }
        let guard = self.world.generation_guard();
        let origin = OriginToken::try_new(
            guard.world_context_id(),
            guard.generation(),
            format!("quiesce:{reason}"),
            0,
            BTreeMap::new(),
            "VoxelCommit",
        )
        .map_err(|err| super::error_mapping::map_internal_error(err.error_id()))?;
        self.world
            .endpoint()
            .admit(WorldCommand::Lifecycle {
                event: "Pause",
                to: "Paused",
                origin,
            })
            .map(|_| ())
            .map_err(PortError::from)
    }

    /// Generated `destroy` entry point. Destruction is the ordered shutdown
    /// sequence; a bounded sink keeps event delivery outside the caller's API.
    pub fn destroy(&mut self) -> Result<(), PortError> {
        let mut sink = WorldEventSink::bounded(16);
        self.shutdown(&mut sink)
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
