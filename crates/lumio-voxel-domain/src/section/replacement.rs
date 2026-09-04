//! Immutable replacement set. Digest is independent of insert order.

#![forbid(unsafe_code)]

use super::slot::SectionSlot;
use super::{SectionError, SectionId};
use lumio_voxel_contracts::sha256;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplacementSet {
    entries: BTreeMap<SectionId, SectionSlot>,
}

impl ReplacementSet {
    pub(super) fn from_entries(entries: BTreeMap<SectionId, SectionSlot>) -> Self {
        Self { entries }
    }

    pub fn get(&self, section_id: &str) -> Result<Option<&SectionSlot>, SectionError> {
        let id = SectionId::parse(section_id)?;
        Ok(self.entries.get(&id))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionReplacement {
    set: ReplacementSet,
    digest: [u8; 32],
}

impl SectionReplacement {
    pub(super) fn from_slots(set: ReplacementSet) -> Self {
        let digest = digest_slots(&set.entries);
        Self { set, digest }
    }

    pub fn set(&self) -> &ReplacementSet {
        &self.set
    }

    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

fn digest_slots(entries: &BTreeMap<SectionId, SectionSlot>) -> [u8; 32] {
    let mut buf = Vec::new();
    for (id, slot) in entries {
        buf.extend_from_slice(id.key().as_bytes());
        buf.push(0);
        buf.extend_from_slice(slot.presence().as_bytes());
        buf.push(0);
        if let Some(payload) = slot.payload() {
            buf.extend_from_slice(payload.schema_id().as_bytes());
            buf.push(0);
            // Sealed page digest is not a public payload accessor (R-00073 exclusive files).
            buf.extend_from_slice(&sha256(format!("{payload:?}").as_bytes()));
        } else {
            buf.extend_from_slice(&[0u8; 32]);
        }
    }
    sha256(&buf)
}
