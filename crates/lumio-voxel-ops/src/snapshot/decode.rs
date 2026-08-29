//! Decode Canonical capture bytes produced by `encode_capture`.
//!
//! Inverse of `CanonicalObject::encode` over `ManifestAdapter::object`. Does not
//! invent a second serializer and does not touch a World.

#![forbid(unsafe_code)]

use super::restore_preflight::RestoreError;
use crate::canonical::{self, CanonicalObject, DecodeError};

/// Parse Canonical object bytes back into typed members.
///
/// Truncation, regrouping, a member out of order, a duplicate member name and a
/// non-minimal escape are all rejected: `canonical::decode` re-encodes what it
/// parsed and requires the bytes back.
pub fn decode_canonical_object(bytes: &[u8]) -> Result<CanonicalObject, RestoreError> {
    if bytes.is_empty() {
        return Err(RestoreError::invalid_handle());
    }
    canonical::decode(bytes).map_err(|err| match err {
        // Bytes that parse but are not the canonical spelling of what they parse to
        // are a digest-domain problem, not a handle problem.
        DecodeError::NotCanonical => RestoreError::artifact_digest_mismatch(),
        DecodeError::Malformed | DecodeError::DuplicateMember => RestoreError::invalid_handle(),
    })
}
