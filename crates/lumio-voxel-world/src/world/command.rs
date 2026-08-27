//! Typed routed commands wrapping `AdmittedCommand` and `OriginEnvelope`.

#![forbid(unsafe_code)]

use super::WorldError;
use super::admission::AdmittedCommand;
use lumio_voxel_ops::async_support::OriginEnvelope;
use lumio_voxel_ops::mutation::{MutationRequest, PreparedMutation};
use lumio_voxel_ops::query::GeneratedVoxelQueryRequest;

pub(crate) struct RoutedCommand<T> {
    admitted: AdmittedCommand,
    envelope: OriginEnvelope<T>,
}

impl<T> RoutedCommand<T> {
    pub(crate) fn admitted(&self) -> &AdmittedCommand {
        &self.admitted
    }

    pub(crate) fn envelope(&self) -> &OriginEnvelope<T> {
        &self.envelope
    }

    pub(crate) fn into_envelope(self) -> OriginEnvelope<T> {
        self.envelope
    }
}

impl RoutedCommand<GeneratedVoxelQueryRequest> {
    pub(crate) fn query(
        admitted: AdmittedCommand,
        envelope: OriginEnvelope<GeneratedVoxelQueryRequest>,
    ) -> Result<Self, WorldError> {
        match admitted {
            AdmittedCommand::Query => Ok(Self { admitted, envelope }),
            _ => Err(WorldError::invalid_handle()),
        }
    }
}

impl RoutedCommand<MutationRequest> {
    pub(crate) fn mutation(
        admitted: AdmittedCommand,
        envelope: OriginEnvelope<MutationRequest>,
    ) -> Result<Self, WorldError> {
        match admitted {
            AdmittedCommand::Mutation => Ok(Self { admitted, envelope }),
            _ => Err(WorldError::invalid_handle()),
        }
    }
}

impl RoutedCommand<PreparedMutation> {
    pub(crate) fn commit(
        admitted: AdmittedCommand,
        envelope: OriginEnvelope<PreparedMutation>,
    ) -> Result<Self, WorldError> {
        match admitted {
            AdmittedCommand::Mutation => Ok(Self { admitted, envelope }),
            _ => Err(WorldError::invalid_handle()),
        }
    }
}
