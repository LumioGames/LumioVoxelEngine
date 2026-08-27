//! Map internal revisions onto generated VoxelRevisionStamp field names.

use super::allocator::{ChunkRevision, WorldRevision};
use lumio_voxel_contracts::SCHEMA_IDS;
use std::collections::BTreeMap;

/// Generated schema this mapping wraps. Must stay in `SCHEMA_IDS`.
pub const REVISION_STAMP_SCHEMA: &str = "voxel-revision-stamp";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedRevisionStamp {
    pub schema_id: &'static str,
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub world_revision: u64,
    pub chunk_revision_set: BTreeMap<String, u64>,
}

pub fn to_generated_stamp(
    world_id: impl Into<String>,
    context_id: impl Into<String>,
    generation: u64,
    world: WorldRevision,
    chunks: &[(String, ChunkRevision)],
) -> GeneratedRevisionStamp {
    debug_assert!(SCHEMA_IDS.contains(&REVISION_STAMP_SCHEMA));
    let mut chunk_revision_set = BTreeMap::new();
    for (id, rev) in chunks {
        chunk_revision_set.insert(id.clone(), rev.value());
    }
    GeneratedRevisionStamp {
        schema_id: REVISION_STAMP_SCHEMA,
        world_id: world_id.into(),
        context_id: context_id.into(),
        generation,
        world_revision: world.value(),
        chunk_revision_set,
    }
}
