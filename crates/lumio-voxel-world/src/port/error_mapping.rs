//! Map World / Query / Mutation error ids onto interned `STABLE_ERROR_IDS`.

#![forbid(unsafe_code)]

use crate::world::{WorldError, intern_stable};
use lumio_voxel_contracts::STABLE_ERROR_IDS;
use lumio_voxel_ops::mutation::MutationError;
use lumio_voxel_ops::query::QueryError;
use std::borrow::Cow;

/// Port-facing error. Known ids borrow the generated table; an id introduced by
/// an upstream producer is retained as an owned value instead of being disguised
/// as an unrelated known error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortError {
    error_id: Cow<'static, str>,
}

impl PortError {
    pub fn error_id(&self) -> &str {
        self.error_id.as_ref()
    }

    pub fn is_registered(&self) -> bool {
        STABLE_ERROR_IDS.contains(&self.error_id())
    }
}

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for PortError {}

impl From<WorldError> for PortError {
    fn from(err: WorldError) -> Self {
        map_world_error(err)
    }
}

/// Exhaustive mapping of known generated ids. Unknown ids remain observable at
/// the Port boundary so callers cannot confuse a new producer error with
/// `InvalidHandle`.
pub fn map_internal_error(error_id: &str) -> PortError {
    let mapped = match error_id {
        "RevisionConflict" => "RevisionConflict",
        "MaintenanceKick" => "MaintenanceKick",
        "ReleaseMismatch" => "ReleaseMismatch",
        "NativeAbiMismatch" => "NativeAbiMismatch",
        "StaleEpoch" => "StaleEpoch",
        "FencingTokenStale" => "FencingTokenStale",
        "ManifestMalformed" => "ManifestMalformed",
        "ManifestUnsupportedVersion" => "ManifestUnsupportedVersion",
        "ManifestDigestMismatch" => "ManifestDigestMismatch",
        "ArtifactMissing" => "ArtifactMissing",
        "ArtifactDigestMismatch" => "ArtifactDigestMismatch",
        "SignatureMissing" => "SignatureMissing",
        "SignatureInvalid" => "SignatureInvalid",
        "TrustRootUnknown" => "TrustRootUnknown",
        "TrustPolicyRejected" => "TrustPolicyRejected",
        "KeyRevoked" => "KeyRevoked",
        "EvidenceMissing" => "EvidenceMissing",
        "EvidenceDigestMismatch" => "EvidenceDigestMismatch",
        "TargetProfileMismatch" => "TargetProfileMismatch",
        "CapabilityMissing" => "CapabilityMissing",
        "SymbolMissing" => "SymbolMissing",
        "SymbolCollision" => "SymbolCollision",
        "PackageIdentityConflict" => "PackageIdentityConflict",
        "WorkerPoolDuplicate" => "WorkerPoolDuplicate",
        "LoaderTimeout" => "LoaderTimeout",
        "LoaderCancelled" => "LoaderCancelled",
        "LoaderOutOfMemory" => "LoaderOutOfMemory",
        "PartialLoadRolledBack" => "PartialLoadRolledBack",
        "InvalidHandle" => "InvalidHandle",
        "HandleDoubleRelease" => "HandleDoubleRelease",
        "MessagePermissionDenied" => "MessagePermissionDenied",
        "StaleConnectionGeneration" => "StaleConnectionGeneration",
        "ChunkUnavailable" => "ChunkUnavailable",
        "TargetRevisionUnavailable" => "TargetRevisionUnavailable",
        "BudgetExceeded" => "BudgetExceeded",
        "QueueFull" => "QueueFull",
        "CoordinateOutOfBounds" => "CoordinateOutOfBounds",
        "DirtyChunkNotDurable" => "DirtyChunkNotDurable",
        "SnapshotBaseMismatch" => "SnapshotBaseMismatch",
        "SessionMismatch" => "SessionMismatch",
        "RoleMismatch" => "RoleMismatch",
        "ClaimNotGranted" => "ClaimNotGranted",
        "SessionAntiReplay" => "SessionAntiReplay",
        "InvalidArgument" => "InvalidArgument",
        "WrongContext" => "WrongContext",
        "BufferTooSmall" => "BufferTooSmall",
        "CapacityExceeded" => "CapacityExceeded",
        "Cancelled" => "Cancelled",
        "TimedOut" => "TimedOut",
        "ContextClosing" => "ContextClosing",
        "ContextDestroyed" => "ContextDestroyed",
        "PanicBoundary" => "PanicBoundary",
        "InternalInvariant" => "InternalInvariant",
        _ => {
            return PortError {
                error_id: Cow::Owned(error_id.to_owned()),
            };
        }
    };
    PortError {
        error_id: Cow::Borrowed(intern_stable(mapped)),
    }
}

pub fn map_world_error(err: WorldError) -> PortError {
    map_internal_error(err.error_id())
}

pub fn map_query_error(err: QueryError) -> PortError {
    map_internal_error(err.error_id())
}

pub fn map_mutation_error(err: MutationError) -> PortError {
    map_internal_error(err.error_id())
}
