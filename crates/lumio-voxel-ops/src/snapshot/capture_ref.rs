//! Immutable capture of one published Voxel root plus its Pin/lease.

#![forbid(unsafe_code)]

use super::stable;
use lumio_voxel_domain::publication::{PublishedReadView, PublishedStateRoot};
use lumio_voxel_domain::revision::{GeneratedRevisionStamp, ReadViewLease, RevisionPin};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureError {
    error_id: &'static str,
}

impl CaptureError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub(super) fn invalid_handle() -> Self {
        Self {
            error_id: stable("InvalidHandle"),
        }
    }
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for CaptureError {}

/// Runtime Cut description consumed by Voxel. Voxel does not own SnapshotCut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutEvidence {
    pub world_id: String,
    pub context_id: String,
    pub generation: u64,
    pub world_revision: u64,
    pub config_hash: String,
    pub artifact_hash: [u8; 32],
}

/// Pin handle owned by the capture. Clone/drop only change the pin refcount.
#[derive(Clone, Debug)]
pub enum PinOrLease {
    Pin(RevisionPin),
    Lease(ReadViewLease),
}

impl PinOrLease {
    fn stamp(&self) -> &GeneratedRevisionStamp {
        match self {
            Self::Pin(pin) => pin.stamp(),
            Self::Lease(lease) => lease.stamp(),
        }
    }
}

impl From<RevisionPin> for PinOrLease {
    fn from(pin: RevisionPin) -> Self {
        Self::Pin(pin)
    }
}

impl From<ReadViewLease> for PinOrLease {
    fn from(lease: ReadViewLease) -> Self {
        Self::Lease(lease)
    }
}

/// Background encode reads a capture only through this port.
pub trait CaptureReadPort {
    fn stamp(&self) -> &GeneratedRevisionStamp;
    fn root_identity(&self) -> [u8; 32];
    fn world_id(&self) -> &str;
    fn context_id(&self) -> &str;
    fn instance_generation(&self) -> u64;
    fn section_revision_set(&self) -> &BTreeMap<String, u64>;
    fn config_hash(&self) -> &str;
}

/// Holds one immutable published root `Arc` and the Pin that keeps it live.
#[derive(Clone, Debug)]
pub struct VoxelCaptureRef {
    root: Arc<PublishedStateRoot>,
    pin: PinOrLease,
    config_hash: String,
}

impl VoxelCaptureRef {
    pub fn new(
        view: &PublishedReadView,
        pin_or_lease: PinOrLease,
        cut_evidence: CutEvidence,
    ) -> Result<Self, CaptureError> {
        let _ = super::manifest_adapter::header_schema();
        let _ = super::manifest_adapter::payload_schema();
        validate_evidence(view, &pin_or_lease, &cut_evidence)?;
        Ok(Self {
            root: view.root_arc(),
            pin: pin_or_lease,
            config_hash: cut_evidence.config_hash,
        })
    }

    pub fn stamp(&self) -> &GeneratedRevisionStamp {
        self.root.stamp()
    }

    pub fn root_identity(&self) -> [u8; 32] {
        self.root.identity()
    }

    pub fn world_id(&self) -> &str {
        &self.root.stamp().world_id
    }

    pub fn context_id(&self) -> &str {
        &self.root.stamp().context_id
    }

    pub fn instance_generation(&self) -> u64 {
        self.root.stamp().generation
    }

    pub fn section_revision_set(&self) -> &BTreeMap<String, u64> {
        &self.root.stamp().section_revision_set
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn root(&self) -> &PublishedStateRoot {
        &self.root
    }

    pub fn pin_stamp(&self) -> &GeneratedRevisionStamp {
        self.pin.stamp()
    }
}

impl CaptureReadPort for VoxelCaptureRef {
    fn stamp(&self) -> &GeneratedRevisionStamp {
        VoxelCaptureRef::stamp(self)
    }

    fn root_identity(&self) -> [u8; 32] {
        VoxelCaptureRef::root_identity(self)
    }

    fn world_id(&self) -> &str {
        VoxelCaptureRef::world_id(self)
    }

    fn context_id(&self) -> &str {
        VoxelCaptureRef::context_id(self)
    }

    fn instance_generation(&self) -> u64 {
        VoxelCaptureRef::instance_generation(self)
    }

    fn section_revision_set(&self) -> &BTreeMap<String, u64> {
        VoxelCaptureRef::section_revision_set(self)
    }

    fn config_hash(&self) -> &str {
        VoxelCaptureRef::config_hash(self)
    }
}

fn validate_evidence(
    view: &PublishedReadView,
    pin_or_lease: &PinOrLease,
    evidence: &CutEvidence,
) -> Result<(), CaptureError> {
    if evidence.world_id.is_empty() || evidence.context_id.is_empty() {
        return Err(CaptureError::invalid_handle());
    }
    if !super::is_hash256(&evidence.config_hash) {
        return Err(CaptureError::invalid_handle());
    }
    let stamp = view.stamp();
    if evidence.world_id != stamp.world_id
        || evidence.context_id != stamp.context_id
        || evidence.generation != stamp.generation
        || evidence.world_revision != stamp.world_revision
    {
        return Err(CaptureError::invalid_handle());
    }
    if evidence.artifact_hash != view.root().identity() {
        return Err(CaptureError::invalid_handle());
    }
    if pin_or_lease.stamp() != stamp || view.lease().stamp() != stamp {
        return Err(CaptureError::invalid_handle());
    }
    Ok(())
}
