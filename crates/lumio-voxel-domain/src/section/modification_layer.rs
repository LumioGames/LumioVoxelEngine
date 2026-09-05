//! The room's single sparse modification layer and its dispatch decision.

use super::{
    DeltaEntry, SectionDeliveryState, SectionDispatch, SectionError, SectionId,
    SectionPayloadEnvelope, SectionStorage,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct SectionModification {
    section_revision: u64,
    storage: SectionStorage,
    latest_delta: Option<SectionPayloadEnvelope>,
}

#[derive(Clone, Debug, Default)]
pub struct RoomModificationLayer {
    sections: BTreeMap<SectionId, SectionModification>,
}

impl RoomModificationLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn set_ready(
        &mut self,
        section_id: SectionId,
        section_revision: u64,
        storage: SectionStorage,
    ) {
        self.sections.insert(
            section_id,
            SectionModification {
                section_revision,
                storage,
                latest_delta: None,
            },
        );
    }

    pub fn record_delta(
        &mut self,
        section_id: SectionId,
        section_revision: u64,
        entries: &[DeltaEntry],
    ) -> Result<(), SectionError> {
        let modification = self
            .sections
            .get_mut(&section_id)
            .ok_or_else(SectionError::section_unavailable)?;

        // Canonicalize by offset while preserving equal-offset submission
        // order. Delta decoding applies entries sequentially, so the final
        // submitted value remains authoritative for duplicate cells.
        let mut ordered_entries = entries.to_vec();
        ordered_entries.sort_by_key(|entry| entry.offset().raw());
        let envelope = SectionPayloadEnvelope::encode_delta(
            section_id,
            section_revision,
            modification.section_revision,
            &ordered_entries,
        );
        let decoded =
            envelope.decode(Some((&modification.storage, modification.section_revision)))?;
        modification.storage = decoded.storage().clone();
        modification.section_revision = decoded.section_revision();
        modification.latest_delta = Some(envelope);
        Ok(())
    }

    pub fn mark_unchanged(&mut self, section_id: SectionId) {
        self.sections.remove(&section_id);
    }

    pub fn dispatch_section(
        &self,
        section_id: SectionId,
        delivery: SectionDeliveryState,
    ) -> SectionDispatch {
        let receiver_revision = match delivery {
            SectionDeliveryState::Pending => return SectionDispatch::pending(section_id),
            SectionDeliveryState::Unavailable => return SectionDispatch::unavailable(section_id),
            SectionDeliveryState::Ready(revision) => Some(revision),
            SectionDeliveryState::FirstDelivery | SectionDeliveryState::Resync => None,
        };
        let Some(modification) = self.sections.get(&section_id) else {
            return SectionDispatch::unchanged(section_id);
        };

        let delta = receiver_revision.and_then(|revision| {
            modification
                .latest_delta
                .as_ref()
                .filter(|delta| delta.base_section_revision() == Some(revision))
                .cloned()
        });
        let envelope = delta.unwrap_or_else(|| {
            SectionPayloadEnvelope::encode_full(
                section_id,
                modification.section_revision,
                &modification.storage,
            )
        });
        SectionDispatch::ready(section_id, envelope)
    }
}
