// ORCHESTRATOR MERGE world/mod.rs:
//   mod durability_ack;
//   pub use durability_ack::{AckEvidence, DurabilityReceipt, apply_durability_ack};

//! Coverage-checked Dirty clear: exclusive DurabilityAck barrier, one `publish_once`.
#![forbid(unsafe_code)]

use super::WorldError;
use super::barrier::{BarrierScope, admit_scope};
use super::instance::VoxelWorld;
use lumio_voxel_contracts::SCHEMA_IDS;
use lumio_voxel_domain::publication::PublishedStateRoot;
use lumio_voxel_domain::revision::{RevisionAllocator, WorldRevision};
use lumio_voxel_domain::section::{DurabilityAckEvidence, SectionDeltaBuilder};

const ACK_KIND: &str = "DurabilityAck";
const ACK_SCHEMA: &str = "voxel-durability-ack";

/// Host `voxel-durability-ack` evidence applied under the World barrier.
pub type AckEvidence = DurabilityAckEvidence;

/// Evidence of one atomic Dirty-clear publication. Empty coverage does not publish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurabilityReceipt {
    coverage_len: usize,
    old_root: [u8; 32],
    new_root: [u8; 32],
}

impl DurabilityReceipt {
    pub fn coverage_len(&self) -> usize {
        self.coverage_len
    }

    pub fn old_root(&self) -> [u8; 32] {
        self.old_root
    }

    pub fn new_root(&self) -> [u8; 32] {
        self.new_root
    }
}

/// Validate Host evidence, then clear only covered Dirty under DurabilityAck occupancy.
pub fn apply_durability_ack(
    world: &mut VoxelWorld,
    ack: AckEvidence,
) -> Result<DurabilityReceipt, WorldError> {
    let _ = ack_schema_id();
    validate_before_occupancy(world, &ack)?;
    let mut barrier = DurabilityAckBarrier::acquire(world)?;
    barrier.enter()?;
    barrier.apply(ack)
}

/// Shares `WorldWriteLane` occupancy (`write_occupied`) for the DurabilityAck scope.
/// WriteLease keeps `&mut VoxelWorld` private, so the publish path uses this local barrier.
struct DurabilityAckBarrier<'a> {
    world: &'a mut VoxelWorld,
}

impl<'a> DurabilityAckBarrier<'a> {
    fn acquire(world: &'a mut VoxelWorld) -> Result<Self, WorldError> {
        if world.instance.write_occupied {
            return Err(WorldError::mapped("HandleDoubleRelease"));
        }
        world.instance.write_occupied = true;
        Ok(Self { world })
    }

    fn enter(&self) -> Result<(), WorldError> {
        admit_scope(self.world, BarrierScope::DurabilityAck)
    }

    fn apply(&mut self, ack: AckEvidence) -> Result<DurabilityReceipt, WorldError> {
        validate_before_occupancy(self.world, &ack)?;
        let view = self.world.instance.authority.capture();
        let frontier = view.dirty_frontier();
        let coverage = frontier
            .covered_by(&ack)
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let old_root = view.root().identity();
        let new_dirty = frontier.except_covered(&coverage);
        if coverage.is_empty() && new_dirty == *frontier {
            return Ok(DurabilityReceipt {
                coverage_len: 0,
                old_root,
                new_root: old_root,
            });
        }

        let stamp = view.stamp().clone();
        let directory = view.directory().clone();
        let replacement = SectionDeltaBuilder::new(view.directory())
            .freeze()
            .map_err(|err| WorldError::mapped(err.error_id()))?;
        let world_revision = world_revision_matching(stamp.world_revision)?;
        let new_root = PublishedStateRoot::new(stamp, directory, new_dirty);
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
        Ok(DurabilityReceipt {
            coverage_len: coverage.len(),
            old_root,
            new_root: published.root().identity(),
        })
    }
}

impl Drop for DurabilityAckBarrier<'_> {
    fn drop(&mut self) {
        self.world.instance.write_occupied = false;
    }
}

fn validate_before_occupancy(world: &VoxelWorld, ack: &AckEvidence) -> Result<(), WorldError> {
    if ack.kind != ACK_KIND {
        return Err(WorldError::invalid_handle());
    }
    if ack.world_id != world.instance.world_id {
        return Err(WorldError::session_mismatch());
    }
    if ack.context.context_id != world.instance.world_context_id {
        return Err(WorldError::session_mismatch());
    }
    if ack.context.generation != world.instance.generation {
        return Err(WorldError::stale_epoch());
    }
    let stamp_rev = world
        .publication_authority()
        .capture()
        .stamp()
        .world_revision;
    // Older covered_world_revision is a prior cut; coverage decides what to clear.
    if ack.covered_world_revision > stamp_rev {
        return Err(WorldError::mapped("EvidenceDigestMismatch"));
    }
    Ok(())
}

fn world_revision_matching(n: u64) -> Result<WorldRevision, WorldError> {
    let mut alloc = RevisionAllocator::new();
    for _ in 0..n {
        alloc
            .reserve_world()
            .map_err(|_| WorldError::invalid_handle())?
            .abandon();
    }
    alloc
        .reserve_world()
        .map_err(|_| WorldError::invalid_handle())?
        .finalize()
        .map_err(|_| WorldError::invalid_handle())
}

fn ack_schema_id() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == ACK_SCHEMA)
        .expect("voxel-durability-ack must exist in generated SCHEMA_IDS")
}
