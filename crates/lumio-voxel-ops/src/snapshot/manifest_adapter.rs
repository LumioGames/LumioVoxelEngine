//! Generated Manifest pair builder. Does not invent a second serializer.

#![forbid(unsafe_code)]

use super::capture_ref::VoxelCaptureRef;
use super::{hex32, quote};
use lumio_voxel_contracts::{SCHEMA_EPOCH, SCHEMA_IDS, SNAPSHOT_CHECKSUM_OMIT, SNAPSHOT_MAGIC};

/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const SNAPSHOT_HEADER_SCHEMA: &str = "snapshot-header";
/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const SNAPSHOT_PAYLOAD_SCHEMA: &str = "voxel-snapshot-payload";

pub(super) fn header_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == SNAPSHOT_HEADER_SCHEMA)
        .expect("snapshot-header must exist in generated SCHEMA_IDS")
}

pub(super) fn payload_schema() -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|id| *id == SNAPSHOT_PAYLOAD_SCHEMA)
        .expect("voxel-snapshot-payload must exist in generated SCHEMA_IDS")
}

/// Adapter-internal canonical field pairs for one captured cut.
pub struct ManifestAdapter;

impl ManifestAdapter {
    pub fn pairs(capture: &VoxelCaptureRef) -> Vec<(String, String)> {
        let header = header_schema();
        let payload = payload_schema();
        let stamp = capture.stamp();
        let mut pairs = vec![
            ("schemaId".to_string(), quote(payload)),
            ("headerSchemaId".to_string(), quote(header)),
            ("magic".to_string(), quote(SNAPSHOT_MAGIC)),
            ("schemaEpoch".to_string(), SCHEMA_EPOCH.to_string()),
            ("worldId".to_string(), quote(&stamp.world_id)),
            ("contextId".to_string(), quote(&stamp.context_id)),
            ("generation".to_string(), stamp.generation.to_string()),
            (
                "worldRevision".to_string(),
                stamp.world_revision.to_string(),
            ),
        ];
        for (id, rev) in &stamp.chunk_revision_set {
            pairs.push((format!("chunkRevision.{id}"), rev.to_string()));
        }
        pairs.push(("configHash".to_string(), quote(capture.config_hash())));
        pairs.push((
            "rootIdentity".to_string(),
            quote(&hex32(&capture.root_identity())),
        ));
        debug_assert!(
            SNAPSHOT_CHECKSUM_OMIT
                .iter()
                .all(|omit| !pairs.iter().any(|(key, _)| key == omit)),
            "generated snapshot-header checksum omits checksum/hash"
        );
        pairs
    }
}
