//! Immutable directory root and unpublished copy-on-write builder.

use super::slot::ChunkSlot;
use super::{ChunkError, ChunkId, ChunkPayload};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkDirectoryRoot {
    entries: Arc<BTreeMap<ChunkId, ChunkSlot>>,
}

impl ChunkDirectoryRoot {
    pub fn lookup(&self, chunk_id: &str) -> Result<Option<&ChunkSlot>, ChunkError> {
        let id = ChunkId::parse(chunk_id)?;
        Ok(self.entries.get(&id))
    }
}

#[derive(Clone, Debug)]
pub struct ChunkDirectoryBuilder {
    entries: Arc<BTreeMap<ChunkId, ChunkSlot>>,
}

impl Default for ChunkDirectoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkDirectoryBuilder {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(BTreeMap::new()),
        }
    }

    pub fn insert(&mut self, chunk_id: &str, slot: ChunkSlot) -> Result<(), ChunkError> {
        let id = ChunkId::parse(chunk_id)?;
        Arc::make_mut(&mut self.entries).insert(id, slot);
        Ok(())
    }

    /// Validate the transition, then write. Failure leaves the map unchanged.
    pub fn convert(
        &mut self,
        chunk_id: &str,
        presence: &str,
        payload: Option<ChunkPayload>,
    ) -> Result<(), ChunkError> {
        let id = ChunkId::parse(chunk_id)?;
        let next = {
            let current = self
                .entries
                .get(&id)
                .ok_or_else(ChunkError::invalid_handle)?;
            current.try_convert(presence, payload)?
        };
        Arc::make_mut(&mut self.entries).insert(id, next);
        Ok(())
    }

    /// Snapshot the current map. Later builder writes clone via `Arc::make_mut`.
    pub fn freeze(&self) -> ChunkDirectoryRoot {
        ChunkDirectoryRoot {
            entries: Arc::clone(&self.entries),
        }
    }
}
