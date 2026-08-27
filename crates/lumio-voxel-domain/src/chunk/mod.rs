//! Immutable chunk payload, four-state presence slot, and COW directory root.
//!
//! Presence maps 1:1 onto generated `CHUNK_PRESENCE`. That is not the
//! `VoxelChunkResidency` machine. IsolatedCubicExtentFamily is adapter-internal
//! (`LGE-V1.4-VOX-D-P0-2026-08-28`); no public `chunk_size` / `page_size`.

#![forbid(unsafe_code)]

mod delta;
mod directory;
mod dirty;
mod payload;
mod replacement;
mod slot;

pub use delta::{ChunkDeltaBuilder, StagedEdit};
pub use directory::{ChunkDirectoryBuilder, ChunkDirectoryRoot};
pub use dirty::{
    CoveredChunkAck, DirtyCoverage, DirtyError, DirtyFrontier, DurabilityAckContext,
    DurabilityAckEvidence,
};
pub use payload::{ChunkPage, ChunkPayload};
pub use replacement::{ChunkReplacement, ReplacementSet};
pub use slot::ChunkSlot;

use lumio_voxel_contracts::STABLE_ERROR_IDS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkError {
    CoordinateOutOfBounds { error_id: &'static str },
    EvidenceDigestMismatch { error_id: &'static str },
    InvalidHandle { error_id: &'static str },
    ChunkUnavailable { error_id: &'static str },
}

impl ChunkError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::CoordinateOutOfBounds { error_id }
            | Self::EvidenceDigestMismatch { error_id }
            | Self::InvalidHandle { error_id }
            | Self::ChunkUnavailable { error_id } => error_id,
        }
    }

    fn coordinate_out_of_bounds() -> Self {
        Self::CoordinateOutOfBounds {
            error_id: stable_error("CoordinateOutOfBounds"),
        }
    }

    fn evidence_digest_mismatch() -> Self {
        Self::EvidenceDigestMismatch {
            error_id: stable_error("EvidenceDigestMismatch"),
        }
    }

    fn invalid_handle() -> Self {
        Self::InvalidHandle {
            error_id: stable_error("InvalidHandle"),
        }
    }

    fn chunk_unavailable() -> Self {
        Self::ChunkUnavailable {
            error_id: stable_error("ChunkUnavailable"),
        }
    }
}

impl std::fmt::Display for ChunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for ChunkError {}

fn stable_error(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in generated STABLE_ERROR_IDS")
}

/// Canonical `voxelChunkId`: `c:x:y:z` with i32-range decimal components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ChunkId {
    x: i32,
    y: i32,
    z: i32,
}

impl ChunkId {
    fn parse(raw: &str) -> Result<Self, ChunkError> {
        let mut parts = raw.split(':');
        let prefix = parts
            .next()
            .ok_or_else(ChunkError::coordinate_out_of_bounds)?;
        if prefix != "c" {
            return Err(ChunkError::coordinate_out_of_bounds());
        }
        let x = parse_coord(
            parts
                .next()
                .ok_or_else(ChunkError::coordinate_out_of_bounds)?,
        )?;
        let y = parse_coord(
            parts
                .next()
                .ok_or_else(ChunkError::coordinate_out_of_bounds)?,
        )?;
        let z = parse_coord(
            parts
                .next()
                .ok_or_else(ChunkError::coordinate_out_of_bounds)?,
        )?;
        if parts.next().is_some() {
            return Err(ChunkError::coordinate_out_of_bounds());
        }
        Ok(Self { x, y, z })
    }
}

fn parse_coord(raw: &str) -> Result<i32, ChunkError> {
    if raw.is_empty() {
        return Err(ChunkError::coordinate_out_of_bounds());
    }
    let digits = match raw.strip_prefix('-') {
        Some(rest) => rest,
        None => raw,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ChunkError::coordinate_out_of_bounds());
    }
    if digits.len() > 1 && digits.as_bytes()[0] == b'0' {
        return Err(ChunkError::coordinate_out_of_bounds());
    }
    if raw.starts_with('-') && digits == "0" {
        return Err(ChunkError::coordinate_out_of_bounds());
    }
    raw.parse::<i32>()
        .map_err(|_| ChunkError::coordinate_out_of_bounds())
}
