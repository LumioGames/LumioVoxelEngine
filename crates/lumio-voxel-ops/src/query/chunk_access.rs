//! Directory lookup mapped onto generated `CHUNK_PRESENCE`. No payload leak.

#![forbid(unsafe_code)]

use super::QueryError;
use lumio_voxel_contracts::CHUNK_PRESENCE;
use lumio_voxel_domain::chunk::ChunkError;
use lumio_voxel_domain::publication::PublishedReadView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkAccessResult {
    chunk_id: String,
    presence: &'static str,
    schema_id: Option<&'static str>,
}

impl ChunkAccessResult {
    pub fn chunk_id(&self) -> &str {
        &self.chunk_id
    }

    pub fn presence(&self) -> &'static str {
        self.presence
    }

    pub fn schema_id(&self) -> Option<&'static str> {
        self.schema_id
    }
}

pub(super) fn intern_presence(name: &str) -> Result<&'static str, QueryError> {
    CHUNK_PRESENCE
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(QueryError::invalid_handle)
}

pub(super) fn access(
    view: &PublishedReadView,
    chunk_id: &str,
) -> Result<ChunkAccessResult, QueryError> {
    match view.directory().lookup(chunk_id) {
        Ok(Some(slot)) => {
            let presence = intern_presence(slot.presence())?;
            let schema_id = if presence == "Ready" {
                Some(
                    slot.payload()
                        .map(|payload| payload.schema_id())
                        .ok_or_else(QueryError::invalid_handle)?,
                )
            } else {
                None
            };
            Ok(ChunkAccessResult {
                chunk_id: chunk_id.to_string(),
                presence,
                schema_id,
            })
        }
        Ok(None) => Ok(ChunkAccessResult {
            chunk_id: chunk_id.to_string(),
            presence: intern_presence("NotLoaded")?,
            schema_id: None,
        }),
        Err(err) => Err(map_chunk(err)),
    }
}

fn map_chunk(err: ChunkError) -> QueryError {
    if err.error_id() == "CoordinateOutOfBounds" {
        QueryError::coordinate_out_of_bounds()
    } else {
        QueryError::invalid_handle()
    }
}
