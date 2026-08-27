//! Origin token wrapping generated voxelContext / revision / tick-phase names.

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use std::collections::BTreeMap;

/// Generated tick-phase names (tick-phase-contract.schema.json enum).
pub const APPLY_PHASES: &[&str] = &[
    "IngressCapture",
    "DecodeAndCanonicalize",
    "ApplyInputs",
    "ProcessorPlan",
    "CrossWorldPrepare",
    "NativeJobBarrier",
    "CommitDecision",
    "VoxelCommit",
    "EcsCommandBufferCommit",
    "GasAndEventFinalize",
    "ReplicationProjection",
    "SnapshotHashMetrics",
    "EgressPublish",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginToken {
    world_context_id: String,
    instance_generation: u64,
    request_id: String,
    input_world_revision: u64,
    input_chunk_revision_set: BTreeMap<String, u64>,
    apply_phase: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginError {
    error_id: &'static str,
}

impl OriginError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }
}

impl OriginToken {
    pub fn try_new(
        world_context_id: impl Into<String>,
        instance_generation: u64,
        request_id: impl Into<String>,
        input_world_revision: u64,
        input_chunk_revision_set: BTreeMap<String, u64>,
        apply_phase: &'static str,
    ) -> Result<Self, OriginError> {
        let world_context_id = world_context_id.into();
        let request_id = request_id.into();
        if world_context_id.is_empty() || request_id.is_empty() {
            return Err(OriginError {
                error_id: stable("InvalidHandle"),
            });
        }
        if !APPLY_PHASES.contains(&apply_phase) {
            return Err(OriginError {
                error_id: stable("InvalidHandle"),
            });
        }
        let _ = SCHEMA_IDS.contains(&"voxel-revision-stamp");
        Ok(Self {
            world_context_id,
            instance_generation,
            request_id,
            input_world_revision,
            input_chunk_revision_set,
            apply_phase,
        })
    }

    pub fn world_context_id(&self) -> &str {
        &self.world_context_id
    }
    pub fn instance_generation(&self) -> u64 {
        self.instance_generation
    }
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
    pub fn input_world_revision(&self) -> u64 {
        self.input_world_revision
    }
    pub fn input_chunk_revision_set(&self) -> &BTreeMap<String, u64> {
        &self.input_chunk_revision_set
    }
    pub fn apply_phase(&self) -> &'static str {
        self.apply_phase
    }
}

fn stable(id: &'static str) -> &'static str {
    debug_assert!(STABLE_ERROR_IDS.contains(&id));
    id
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OriginEnvelope<T> {
    pub origin: OriginToken,
    pub config_hash: String,
    pub payload: T,
}
