//! Capture one published Voxel root under a short CaptureCut barrier.
//! Encoding, I/O, sleeps, worker waits, and ingress close stay outside the barrier.

#![forbid(unsafe_code)]

use super::WorldError;
use super::barrier::BarrierScope;
use super::capture_admission::RuntimeSnapshotCut;
use super::instance::VoxelWorld;
use super::write_lane::WorldWriteLane;
use lumio_voxel_domain::revision::GeneratedRevisionStamp;
use lumio_voxel_ops::snapshot::{CutEvidence, PinOrLease, VoxelCaptureRef};

/// Evidence that a CaptureCut barrier captured one published root and released occupancy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureEvidence {
    pub cut_id: String,
    pub voxel_stamp: GeneratedRevisionStamp,
    pub root_hash: [u8; 32],
    pub barrier_released: bool,
}

/// Validate a Runtime cut, pin one published root, then release CaptureCut occupancy.
pub fn capture(
    world: &mut VoxelWorld,
    cut: &RuntimeSnapshotCut,
) -> Result<(VoxelCaptureRef, CaptureEvidence), WorldError> {
    super::capture_admission::admit(world, cut)?;
    // WriteLease keeps `&mut VoxelWorld` private, so the published view is cloned
    // immediately before occupancy. Pin + VoxelCaptureRef are built while held.
    let view = world.publication_authority().capture();

    let mut lease = WorldWriteLane::try_acquire(world)?;
    lease.enter(BarrierScope::CaptureCut)?;
    super::capture_admission::validate_against_view(cut, &view)?;

    let pin = PinOrLease::Lease(view.lease().clone());
    let cut_evidence = CutEvidence {
        world_id: cut.world_id.clone(),
        context_id: cut.context_id.clone(),
        generation: cut.generation,
        world_revision: cut.world_revision,
        config_hash: cut.config_hash.clone(),
        artifact_hash: cut.artifact_hash,
    };
    let capture_ref = VoxelCaptureRef::new(&view, pin, cut_evidence)
        .map_err(|err| WorldError::mapped(err.error_id()))?;
    let cut_id = cut.cut_id.clone();
    let voxel_stamp = capture_ref.stamp().clone();
    let root_hash = capture_ref.root_identity();
    drop(lease);

    Ok((
        capture_ref,
        CaptureEvidence {
            cut_id,
            voxel_stamp,
            root_hash,
            barrier_released: true,
        },
    ))
}
