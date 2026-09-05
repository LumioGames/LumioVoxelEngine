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

    pub fn iter(&self) -> impl Iterator<Item = (&SectionId, &SectionSlot)> {
        self.entries.iter()
    }

    pub(crate) fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("{self:?}").into_bytes();
        for (id, slot) in self.entries.iter() {
            if let Some(digest) = slot
                .payload()
                .and_then(SectionPayload::storage_identity_digest)
            {
                bytes.push(0);
                bytes.extend_from_slice(b"SectionStorageV1");
                bytes.push(0);
                bytes.extend_from_slice(id.key().as_bytes());
                bytes.push(0);
                bytes.extend_from_slice(&digest);
            }
        }
        bytes
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
