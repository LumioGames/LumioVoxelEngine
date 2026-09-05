//! Per-Section dispatch responses for the contract presence states.

use super::{SectionError, SectionId, SectionPayloadEnvelope};
use lumio_voxel_contracts::voxel_world as vw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionDeliveryState {
    FirstDelivery,
    Ready(u64),
    Resync,
    Pending,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DispatchContent {
    Ready(SectionPayloadEnvelope),
    Unchanged,
    Pending,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionDispatch {
    section_id: SectionId,
    content: DispatchContent,
}

impl SectionDispatch {
    pub fn validate_unchanged_ticket(
        section_id: SectionId,
        payload_length: u32,
        payload: &[u8],
    ) -> Result<Self, SectionError> {
        if payload_length != vw::SHORT_TICKET_PAYLOAD_LENGTH
            || payload.len() != payload_length as usize
        {
            return Err(SectionError::contract_violation(
                vw::SECTION_ENCODING_MISMATCH,
            ));
        }
        Ok(Self::unchanged(section_id))
    }

    pub const fn section_id(&self) -> &SectionId {
        &self.section_id
    }

    pub fn presence(&self) -> &'static str {
        let presence = match self.content {
            DispatchContent::Ready(_) => "Ready",
            DispatchContent::Unchanged => "Unchanged",
            DispatchContent::Pending => "Pending",
            DispatchContent::Unavailable => "Unavailable",
        };
        vw::intern_presence(presence).expect("dispatch presence is declared by the contract")
    }

    pub fn payload_length(&self) -> Option<u32> {
        match &self.content {
            DispatchContent::Ready(envelope) => Some(envelope.payload_length()),
            DispatchContent::Unchanged => Some(vw::SHORT_TICKET_PAYLOAD_LENGTH),
            DispatchContent::Pending | DispatchContent::Unavailable => None,
        }
    }

    pub fn payload(&self) -> Option<&[u8]> {
        match &self.content {
            DispatchContent::Ready(envelope) => Some(envelope.payload()),
            DispatchContent::Unchanged => Some(&[]),
            DispatchContent::Pending | DispatchContent::Unavailable => None,
        }
    }

    pub fn envelope(&self) -> Option<&SectionPayloadEnvelope> {
        match &self.content {
            DispatchContent::Ready(envelope) => Some(envelope),
            DispatchContent::Unchanged
            | DispatchContent::Pending
            | DispatchContent::Unavailable => None,
        }
    }

    pub fn require_available(&self) -> Result<(), SectionError> {
        match self.content {
            DispatchContent::Ready(_) | DispatchContent::Unchanged => Ok(()),
            DispatchContent::Pending | DispatchContent::Unavailable => {
                Err(SectionError::section_unavailable())
            }
        }
    }

    pub(super) fn ready(section_id: SectionId, envelope: SectionPayloadEnvelope) -> Self {
        Self {
            section_id,
            content: DispatchContent::Ready(envelope),
        }
    }

    pub(super) const fn unchanged(section_id: SectionId) -> Self {
        Self {
            section_id,
            content: DispatchContent::Unchanged,
        }
    }

    pub(super) const fn pending(section_id: SectionId) -> Self {
        Self {
            section_id,
            content: DispatchContent::Pending,
        }
    }

    pub(super) const fn unavailable(section_id: SectionId) -> Self {
        Self {
            section_id,
            content: DispatchContent::Unavailable,
        }
    }
}
