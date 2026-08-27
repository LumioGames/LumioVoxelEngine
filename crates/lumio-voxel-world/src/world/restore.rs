// ORCHESTRATOR MERGE world/mod.rs:
//   mod restore;
//   pub use restore::{restore, RestoreReceipt};

//! Atomic restore: exclusive Restore barrier, recheck, one `publish_once`.
#![forbid(unsafe_code)]

use super::WorldError;
use super::barrier::{BarrierScope, admit_scope};
use super::instance::VoxelWorld;
use lumio_voxel_ops::snapshot::SealedRestoreCandidate;

/// Evidence of one atomic restore publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreReceipt {
    old_root: [u8; 32],
    new_root: [u8; 32],
}

impl RestoreReceipt {
    pub fn old_root(&self) -> [u8; 32] {
        self.old_root
    }

    pub fn new_root(&self) -> [u8; 32] {
        self.new_root
    }
}

/// Recheck then publish the sealed shadow root. Failure keeps the old World.
pub fn restore(
    world: &mut VoxelWorld,
    candidate: SealedRestoreCandidate,
) -> Result<RestoreReceipt, WorldError> {
    let mut barrier = RestoreBarrier::acquire(world)?;
    barrier.enter()?;
    barrier.publish(candidate)
}

/// Shares `WorldWriteLane` occupancy (`write_occupied`) for the Restore scope.
struct RestoreBarrier<'a> {
    world: &'a mut VoxelWorld,
}

impl<'a> RestoreBarrier<'a> {
    fn acquire(world: &'a mut VoxelWorld) -> Result<Self, WorldError> {
        if world.instance.write_occupied {
            return Err(WorldError::mapped("HandleDoubleRelease"));
        }
        world.instance.write_occupied = true;
        Ok(Self { world })
    }

    fn enter(&self) -> Result<(), WorldError> {
        admit_scope(self.world, BarrierScope::Restore)
    }

    fn publish(&mut self, candidate: SealedRestoreCandidate) -> Result<RestoreReceipt, WorldError> {
        recheck(self.world, &candidate)?;
        let old_root = self.world.instance.authority.capture().root().identity();
        let (new_root, replacement, world_revision) = candidate.into_publication();
        let mut prepared = self
            .world
            .instance
            .authority
            .prepare(world_revision, new_root, replacement)
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let token = prepared
            .seal()
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let published = self
            .world
            .instance
            .authority
            .publish_once(token)
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        self.world.instance.ledger.abort_in_flight();
        Ok(RestoreReceipt {
            old_root,
            new_root: published.root().identity(),
        })
    }
}

impl Drop for RestoreBarrier<'_> {
    fn drop(&mut self) {
        self.world.instance.write_occupied = false;
    }
}

fn recheck(world: &VoxelWorld, candidate: &SealedRestoreCandidate) -> Result<(), WorldError> {
    if candidate.world_id() != world.instance.world_id {
        return Err(WorldError::session_mismatch());
    }
    if candidate.generation() != world.instance.generation {
        return Err(WorldError::stale_epoch());
    }
    if candidate.context_id() != world.instance.world_context_id {
        return Err(WorldError::session_mismatch());
    }
    if candidate.config_hash() != world.instance.snapshot.config_hash() {
        return Err(WorldError::mapped("EvidenceDigestMismatch"));
    }
    if !candidate.hash_matches() {
        return Err(WorldError::mapped("ArtifactDigestMismatch"));
    }
    Ok(())
}
