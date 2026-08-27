//! Move-only prepared mutation. Commit (R-00104) publishes; this type does not.

#![forbid(unsafe_code)]

use super::fingerprint::{MutationRequest, RequestFingerprint};
use super::reservation::MutationReservation;
use lumio_voxel_domain::chunk::{ChunkReplacement, DirtyFrontier};

/// Sealed prepare token. Not `Clone`; commit consumes it by value.
pub struct PreparedMutation {
    txn_id: String,
    fingerprint: RequestFingerprint,
    base_identity: [u8; 32],
    generation: u64,
    config_hash: String,
    reservation: MutationReservation,
    #[allow(dead_code)]
    request: MutationRequest,
    #[allow(dead_code)]
    replacement: ChunkReplacement,
    #[allow(dead_code)]
    dirty: DirtyFrontier,
    #[allow(dead_code)]
    target_world_revision: u64,
}

impl PreparedMutation {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bind(
        request: MutationRequest,
        fingerprint: RequestFingerprint,
        base_identity: [u8; 32],
        generation: u64,
        config_hash: String,
        reservation: MutationReservation,
        replacement: ChunkReplacement,
        dirty: DirtyFrontier,
        target_world_revision: u64,
    ) -> Self {
        Self {
            txn_id: request.txn_id.clone(),
            fingerprint,
            base_identity,
            generation,
            config_hash,
            reservation,
            request,
            replacement,
            dirty,
            target_world_revision,
        }
    }

    pub fn txn_id(&self) -> &str {
        &self.txn_id
    }

    pub fn fingerprint(&self) -> RequestFingerprint {
        self.fingerprint
    }

    pub fn base_identity(&self) -> [u8; 32] {
        self.base_identity
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn reservation(&self) -> &MutationReservation {
        &self.reservation
    }

    #[allow(dead_code)]
    pub(crate) fn request(&self) -> &MutationRequest {
        &self.request
    }

    #[allow(dead_code)]
    pub(crate) fn replacement(&self) -> &ChunkReplacement {
        &self.replacement
    }

    #[allow(dead_code)]
    pub(crate) fn dirty(&self) -> &DirtyFrontier {
        &self.dirty
    }

    #[allow(dead_code)]
    pub(crate) fn target_world_revision(&self) -> u64 {
        self.target_world_revision
    }
}

impl std::fmt::Debug for PreparedMutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedMutation")
            .field("txn_id", &self.txn_id)
            .field("generation", &self.generation)
            .field("config_hash", &self.config_hash)
            .finish_non_exhaustive()
    }
}
