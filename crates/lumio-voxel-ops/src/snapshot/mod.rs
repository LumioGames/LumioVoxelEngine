//! Immutable VoxelCaptureRef and generated Canonical codec port (R-00134).
//!
//! Production code must not depend on `lumio-voxel-test-support`.
//! Encoding does not take a World lease, fsync, restore, or touch the filesystem.

#![forbid(unsafe_code)]

mod capture_ref;
mod codec_port;
mod decode;
mod manifest_adapter;
mod restore_preflight;
mod restore_shadow;

pub use capture_ref::{CaptureError, CaptureReadPort, CutEvidence, PinOrLease, VoxelCaptureRef};
pub use codec_port::{
    CaptureWriter, GeneratedCaptureMetadata, MemoryCaptureWriter, SnapshotError, encode_capture,
};
pub use decode::decode_canonical_object;
pub use manifest_adapter::{ManifestAdapter, SNAPSHOT_HEADER_SCHEMA, SNAPSHOT_PAYLOAD_SCHEMA};
pub use restore_preflight::{DecodedRestore, RestoreError, RestorePreflight};
pub use restore_shadow::{RestoreShadowBuilder, SealedRestoreCandidate};

use lumio_voxel_contracts::STABLE_ERROR_IDS;

pub(super) fn stable(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in generated STABLE_ERROR_IDS")
}

pub(super) fn hex32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

pub(super) fn is_hash256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
