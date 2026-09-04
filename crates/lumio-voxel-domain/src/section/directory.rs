//! Immutable directory root and unpublished copy-on-write builder.

use super::slot::SectionSlot;
use super::{SectionError, SectionId, SectionPayload};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionDirectoryRoot {
    entries: Arc<BTreeMap<SectionId, SectionSlot>>,
}

impl SectionDirectoryRoot {
    pub fn lookup(&self, section_id: &str) -> Result<Option<&SectionSlot>, SectionError> {
        let id = SectionId::parse(section_id)?;
        Ok(self.entries.get(&id))
    }
}

#[derive(Clone, Debug)]
pub struct SectionDirectoryBuilder {
    entries: Arc<BTreeMap<SectionId, SectionSlot>>,
}

impl Default for SectionDirectoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SectionDirectoryBuilder {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(BTreeMap::new()),
        }
    }

    pub fn insert(&mut self, section_id: &str, slot: SectionSlot) -> Result<(), SectionError> {
        let id = SectionId::parse(section_id)?;
        Arc::make_mut(&mut self.entries).insert(id, slot);
        Ok(())
    }

    /// Validate the transition, then write. Failure leaves the map unchanged.
    pub fn convert(
        &mut self,
        section_id: &str,
        presence: &str,
        payload: Option<SectionPayload>,
    ) -> Result<(), SectionError> {
        let id = SectionId::parse(section_id)?;
        let next = {
            let current = self
                .entries
                .get(&id)
                .ok_or_else(SectionError::invalid_handle)?;
            current.try_convert(presence, payload)?
        };
        Arc::make_mut(&mut self.entries).insert(id, next);
        Ok(())
    }

    /// Snapshot the current map. Later builder writes clone via `Arc::make_mut`.
    pub fn freeze(&self) -> SectionDirectoryRoot {
        SectionDirectoryRoot {
            entries: Arc::clone(&self.entries),
        }
    }
}
