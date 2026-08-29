//! Canonical request fingerprint over the Voxel-local canonical object encoding.

use crate::canonical::{CanonicalObject, DuplicateMember};
use lumio_voxel_contracts::{Hash256, SCHEMA_IDS, sha256};
use std::collections::BTreeMap;

/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const MUTATION_RECEIPT_SCHEMA: &str = "voxel-mutation-receipt";

/// Member naming the encoding the fingerprint was taken over.
///
/// Without it the bytes carry no statement of which form produced them, so a later
/// format change would show up only as digests that quietly stopped matching. This
/// is deliberately not the Architecture form id: that form's member-name grammar
/// excludes `txn_id` and `c:0:0:0`, so claiming it here would be a false mark.
pub const CANONICAL_FORM_FIELD: &str = "canonicalForm";
pub const CANONICAL_FORM_ID: &str = "VoxelCanonicalObjectV1";

/// Generated request identity plus additional contract fields.
/// Field names wrap the generated contract (`txn_id`, `world_id`, `generation`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationRequest {
    pub txn_id: String,
    pub world_id: String,
    pub generation: u64,
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestFingerprint {
    hash: Hash256,
}

impl RequestFingerprint {
    pub fn hash(self) -> Hash256 {
        self.hash
    }
}

/// Fingerprint covers every contract field plus the caller-supplied ones.
///
/// Caller field values are strings by declared type, so they are encoded as strings:
/// no key is special-cased into an integer. Reading a value's type from its name is
/// the same "the caller passed the right thing" assumption this encoding removes.
///
/// A caller field named like a contract member is rejected rather than resolved, so
/// two members can never share a name.
pub fn canonical_fingerprint(
    request: &MutationRequest,
) -> Result<RequestFingerprint, DuplicateMember> {
    debug_assert!(SCHEMA_IDS.contains(&MUTATION_RECEIPT_SCHEMA));
    let mut object = CanonicalObject::new();
    for (key, value) in &request.fields {
        object.insert_text(key.clone(), value.clone())?;
    }
    object.insert_text(CANONICAL_FORM_FIELD, CANONICAL_FORM_ID)?;
    object.insert_text("txn_id", request.txn_id.clone())?;
    object.insert_text("world_id", request.world_id.clone())?;
    object.insert_uint("generation", request.generation)?;
    Ok(RequestFingerprint {
        hash: Hash256(sha256(object.encode().as_bytes())),
    })
}
