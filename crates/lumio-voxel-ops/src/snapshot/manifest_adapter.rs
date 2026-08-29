//! Generated Manifest object builder. Does not invent a second serializer.

#![forbid(unsafe_code)]

use super::capture_ref::VoxelCaptureRef;
use super::hex32;
use super::restore_preflight::RestoreError;
use crate::canonical::{CanonicalObject, CanonicalValue};
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

/// Adapter-internal canonical object for one captured cut.
pub struct ManifestAdapter;

impl ManifestAdapter {
    pub fn object(capture: &VoxelCaptureRef) -> Result<CanonicalObject, RestoreError> {
        let header = header_schema();
        let payload = payload_schema();
        let stamp = capture.stamp();
        let mut object = CanonicalObject::new();
        let mut members: Vec<(String, CanonicalValue)> = vec![
            ("schemaId".into(), CanonicalValue::text(payload)),
            ("headerSchemaId".into(), CanonicalValue::text(header)),
            ("magic".into(), CanonicalValue::text(SNAPSHOT_MAGIC)),
            ("schemaEpoch".into(), CanonicalValue::Uint(SCHEMA_EPOCH)),
            ("worldId".into(), CanonicalValue::text(&stamp.world_id)),
            ("contextId".into(), CanonicalValue::text(&stamp.context_id)),
            ("generation".into(), CanonicalValue::Uint(stamp.generation)),
            (
                "worldRevision".into(),
                CanonicalValue::Uint(stamp.world_revision),
            ),
        ];
        for (id, rev) in &stamp.chunk_revision_set {
            members.push((format!("chunkRevision.{id}"), CanonicalValue::Uint(*rev)));
        }
        members.push((
            "configHash".into(),
            CanonicalValue::text(capture.config_hash()),
        ));
        members.push((
            "rootIdentity".into(),
            CanonicalValue::text(hex32(&capture.root_identity())),
        ));
        for (key, value) in members {
            object
                .insert(key, value)
                .map_err(|_| RestoreError::invalid_handle())?;
        }
        debug_assert!(
            SNAPSHOT_CHECKSUM_OMIT
                .iter()
                .all(|omit| !object.contains_key(omit)),
            "generated snapshot-header checksum omits checksum/hash"
        );
        Ok(object)
    }
}
