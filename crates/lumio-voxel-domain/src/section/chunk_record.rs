//! Chunk records are identity-only containers; all data and revisions belong to Sections.

use super::SectionError;
use crate::key::ChunkId;
use lumio_voxel_contracts::voxel_world as vw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkRecord {
    id: ChunkId,
}

impl ChunkRecord {
    pub fn validate(
        chunk_key: &str,
        data: &[u8],
        chunk_revision: Option<u64>,
    ) -> Result<Self, SectionError> {
        if !data.is_empty() || chunk_revision.is_some() {
            return Err(SectionError::contract_violation(
                vw::CHUNK_CARRIES_DATA_ERROR,
            ));
        }
        Ok(Self {
            id: ChunkId::parse(chunk_key)?,
        })
    }

    pub const fn id(&self) -> &ChunkId {
        &self.id
    }
}
