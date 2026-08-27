//! Admit a Runtime-supplied SnapshotCut against the live VoxelWorld.

#![forbid(unsafe_code)]

use super::WorldError;
use super::instance::VoxelWorld;
use super::state::query_admissible;
use lumio_voxel_domain::publication::PublishedReadView;

/// Adapter wrapping Runtime SnapshotCut fields. Voxel does not own or modify a Cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSnapshotCut {
    pub cut_id: String,
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub world_revision: u64,
    pub config_hash: String,
    pub artifact_hash: [u8; 32],
}

impl RuntimeSnapshotCut {
    /// Copy live world identity into the adapter. Does not create a Runtime Cut object.
    pub fn from_live(world: &VoxelWorld, cut_id: impl Into<String>) -> Self {
        let view = world.publication_authority().capture();
        let stamp = view.stamp();
        Self {
            cut_id: cut_id.into(),
            world_id: stamp.world_id.clone(),
            context_id: stamp.context_id.clone(),
            generation: stamp.generation,
            world_revision: stamp.world_revision,
            config_hash: world.instance.snapshot.config_hash().to_string(),
            artifact_hash: view.root().identity(),
        }
    }
}

pub(crate) fn admit(world: &VoxelWorld, cut: &RuntimeSnapshotCut) -> Result<(), WorldError> {
    if cut.cut_id.is_empty() || cut.world_id.is_empty() || cut.context_id.is_empty() {
        return Err(WorldError::invalid_handle());
    }
    if !is_hash256(&cut.config_hash) {
        return Err(WorldError::invalid_handle());
    }
    if cut.world_id != world.instance.world_id {
        return Err(WorldError::session_mismatch());
    }
    if cut.context_id != world.instance.world_context_id {
        return Err(WorldError::session_mismatch());
    }
    if cut.generation != world.instance.generation {
        return Err(WorldError::stale_epoch());
    }
    if !query_admissible(world.instance.state.current()) {
        return Err(WorldError::claim_not_granted());
    }
    if cut.config_hash != world.instance.snapshot.config_hash() {
        return Err(WorldError::invalid_handle());
    }
    Ok(())
}

pub(crate) fn validate_against_view(
    cut: &RuntimeSnapshotCut,
    view: &PublishedReadView,
) -> Result<(), WorldError> {
    let stamp = view.stamp();
    if cut.world_id != stamp.world_id || cut.context_id != stamp.context_id {
        return Err(WorldError::session_mismatch());
    }
    if cut.generation != stamp.generation {
        return Err(WorldError::stale_epoch());
    }
    if cut.world_revision != stamp.world_revision {
        return Err(WorldError::invalid_handle());
    }
    if cut.artifact_hash != view.root().identity() {
        return Err(WorldError::invalid_handle());
    }
    if view.lease().stamp() != stamp {
        return Err(WorldError::invalid_handle());
    }
    Ok(())
}

fn is_hash256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
