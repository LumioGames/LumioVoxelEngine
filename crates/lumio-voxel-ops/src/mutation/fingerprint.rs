//! Canonical request fingerprint via generated `canonical_object_pairs`.

use lumio_voxel_contracts::{Hash256, SCHEMA_IDS, canonical_object_pairs, sha256};
use std::collections::BTreeMap;

/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const MUTATION_RECEIPT_SCHEMA: &str = "voxel-mutation-receipt";

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

/// Fingerprint covers every contract field. Keys are sorted by the generated canonicalizer.
pub fn canonical_fingerprint(request: &MutationRequest) -> RequestFingerprint {
    debug_assert!(SCHEMA_IDS.contains(&MUTATION_RECEIPT_SCHEMA));
    let mut pairs: Vec<(String, String)> = Vec::with_capacity(3 + request.fields.len());
    for (k, v) in &request.fields {
        pairs.push((k.clone(), v.clone()));
    }
    pairs.push(("txn_id".to_string(), quote(&request.txn_id)));
    pairs.push(("world_id".to_string(), quote(&request.world_id)));
    pairs.push(("generation".to_string(), request.generation.to_string()));
    let canonical = canonical_object_pairs(&mut pairs);
    RequestFingerprint {
        hash: Hash256(sha256(canonical.as_bytes())),
    }
}

fn quote(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    out.push_str(raw);
    out.push('"');
    out
}
