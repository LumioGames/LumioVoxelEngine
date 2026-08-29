//! GenerationBoundLeaseFamily reservation. Expire by generation, never wall clock.

use super::fingerprint::{MutationRequest, RequestFingerprint, canonical_fingerprint};
use crate::canonical::DuplicateMember;

/// Adapter-internal lease family selected by VOX-D-004. Not a Schema column.
pub const LEASE_FAMILY: &str = "GenerationBoundLeaseFamily";

pub struct GenerationBoundLeaseFamily;

impl GenerationBoundLeaseFamily {
    /// `now_generation` is the caller-supplied generation/tick analog.
    pub fn expired(reserved_generation: u64, now_generation: u64) -> bool {
        now_generation != reserved_generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationReservation {
    txn_id: String,
    world_id: String,
    generation: u64,
    fingerprint: RequestFingerprint,
}

impl MutationReservation {
    pub(super) fn from_request(request: &MutationRequest) -> Result<Self, DuplicateMember> {
        Ok(Self {
            txn_id: request.txn_id.clone(),
            world_id: request.world_id.clone(),
            generation: request.generation,
            fingerprint: canonical_fingerprint(request)?,
        })
    }

    pub fn txn_id(&self) -> &str {
        &self.txn_id
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn fingerprint(&self) -> RequestFingerprint {
        self.fingerprint
    }

    pub fn is_expired(&self, generation: u64) -> bool {
        GenerationBoundLeaseFamily::expired(self.generation, generation)
    }
}
