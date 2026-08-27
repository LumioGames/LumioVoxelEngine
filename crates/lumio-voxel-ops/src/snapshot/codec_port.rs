//! Canonical codec port: bounded, cancelable, SDK-neutral writer.

#![forbid(unsafe_code)]

use super::capture_ref::VoxelCaptureRef;
use super::manifest_adapter::ManifestAdapter;
use super::stable;
use lumio_voxel_contracts::{BoundedBuffer, BufferFull, Hash256, canonical_object_pairs, sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotError {
    error_id: &'static str,
}

impl SnapshotError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub fn invalid_handle() -> Self {
        Self {
            error_id: stable("InvalidHandle"),
        }
    }

    pub fn budget_exceeded() -> Self {
        Self {
            error_id: stable("BudgetExceeded"),
        }
    }

    pub fn loader_cancelled() -> Self {
        Self {
            error_id: stable("LoaderCancelled"),
        }
    }
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for SnapshotError {}

/// SDK-neutral sink. Implementations must not take a World lease.
pub trait CaptureWriter {
    fn is_cancelled(&self) -> bool;
    fn write(&mut self, bytes: &[u8]) -> Result<(), SnapshotError>;
}

/// In-memory bounded writer. Tests and Host adapters share this surface.
pub struct MemoryCaptureWriter {
    buf: BoundedBuffer,
    cap: usize,
    cancelled: bool,
}

impl MemoryCaptureWriter {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: BoundedBuffer::new(cap),
            cap,
            cancelled: false,
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn as_slice(&self) -> &[u8] {
        self.buf.as_slice()
    }
}

impl CaptureWriter for MemoryCaptureWriter {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), SnapshotError> {
        if self.cancelled {
            return Err(SnapshotError::loader_cancelled());
        }
        let used = self.buf.as_slice().len();
        if used.saturating_add(bytes.len()) > self.cap {
            return Err(SnapshotError::budget_exceeded());
        }
        for &byte in bytes {
            match self.buf.push(byte) {
                Ok(()) => {}
                Err(BufferFull) => return Err(SnapshotError::budget_exceeded()),
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCaptureMetadata {
    payload_hash: Hash256,
    root_identity: [u8; 32],
    world_revision: u64,
    generation: u64,
    config_hash: String,
    byte_len: usize,
}

impl GeneratedCaptureMetadata {
    pub fn payload_hash(&self) -> Hash256 {
        self.payload_hash
    }

    pub fn root_identity(&self) -> [u8; 32] {
        self.root_identity
    }

    pub fn world_revision(&self) -> u64 {
        self.world_revision
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn byte_len(&self) -> usize {
        self.byte_len
    }
}

/// Encode without a World lease. Failure does not unpin still-live captures.
pub fn encode_capture(
    capture: &VoxelCaptureRef,
    writer: &mut impl CaptureWriter,
) -> Result<GeneratedCaptureMetadata, SnapshotError> {
    if writer.is_cancelled() {
        return Err(SnapshotError::loader_cancelled());
    }
    if capture.config_hash().is_empty() || capture.world_id().is_empty() {
        return Err(SnapshotError::invalid_handle());
    }
    let mut pairs = ManifestAdapter::pairs(capture);
    if pairs.is_empty() {
        return Err(SnapshotError::invalid_handle());
    }
    let canonical = canonical_object_pairs(&mut pairs);
    let bytes = canonical.into_bytes();
    if bytes.is_empty() {
        return Err(SnapshotError::invalid_handle());
    }
    writer.write(&bytes)?;
    Ok(GeneratedCaptureMetadata {
        payload_hash: Hash256(sha256(&bytes)),
        root_identity: capture.root_identity(),
        world_revision: capture.stamp().world_revision,
        generation: capture.instance_generation(),
        config_hash: capture.config_hash().to_string(),
        byte_len: bytes.len(),
    })
}
