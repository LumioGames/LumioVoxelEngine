//! Decode Canonical capture bytes produced by `encode_capture`.
//!
//! Inverse of `canonical_object_pairs` over `ManifestAdapter::pairs`. Does not
//! invent a second serializer and does not touch a World.

#![forbid(unsafe_code)]

use super::restore_preflight::RestoreError;
use lumio_voxel_contracts::canonical_object_pairs;

/// Parse Canonical object pairs. Values keep their encoded form (quoted strings
/// or bare numbers) so a recanonicalize round-trip can detect truncation.
pub fn decode_canonical_pairs(bytes: &[u8]) -> Result<Vec<(String, String)>, RestoreError> {
    if bytes.is_empty() {
        return Err(RestoreError::invalid_handle());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| RestoreError::invalid_handle())?;
    let pairs = parse_pairs(text)?;
    let mut recanon = pairs.clone();
    let canonical = canonical_object_pairs(&mut recanon);
    if canonical.as_bytes() != bytes {
        return Err(RestoreError::artifact_digest_mismatch());
    }
    Ok(pairs)
}

pub(super) fn unquote(value: &str) -> Result<&str, RestoreError> {
    let inner = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'));
    inner.ok_or_else(RestoreError::invalid_handle)
}

pub(super) fn parse_u64(value: &str) -> Result<u64, RestoreError> {
    if value.starts_with('"') || value.is_empty() {
        return Err(RestoreError::invalid_handle());
    }
    if value.len() > 1 && value.as_bytes()[0] == b'0' {
        return Err(RestoreError::invalid_handle());
    }
    value.parse().map_err(|_| RestoreError::invalid_handle())
}

fn parse_pairs(text: &str) -> Result<Vec<(String, String)>, RestoreError> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'{') || bytes.last() != Some(&b'}') {
        return Err(RestoreError::invalid_handle());
    }
    if bytes.len() == 2 {
        return Ok(Vec::new());
    }
    let end = bytes.len() - 1;
    let mut i = 1;
    let mut pairs = Vec::new();
    while i < end {
        if bytes[i] != b'"' {
            return Err(RestoreError::invalid_handle());
        }
        i += 1;
        let key_start = i;
        while i < end && bytes[i] != b'"' {
            i += 1;
        }
        if i >= end {
            return Err(RestoreError::invalid_handle());
        }
        let key = std::str::from_utf8(&bytes[key_start..i])
            .map_err(|_| RestoreError::invalid_handle())?;
        i += 1;
        if i >= end || bytes[i] != b':' {
            return Err(RestoreError::invalid_handle());
        }
        i += 1;
        let value = if i < end && bytes[i] == b'"' {
            let val_start = i;
            i += 1;
            while i < end && bytes[i] != b'"' {
                i += 1;
            }
            if i >= end {
                return Err(RestoreError::invalid_handle());
            }
            i += 1;
            std::str::from_utf8(&bytes[val_start..i])
                .map_err(|_| RestoreError::invalid_handle())?
                .to_string()
        } else {
            let val_start = i;
            while i < end && bytes[i] != b',' {
                i += 1;
            }
            let raw = std::str::from_utf8(&bytes[val_start..i])
                .map_err(|_| RestoreError::invalid_handle())?;
            if raw.is_empty() {
                return Err(RestoreError::invalid_handle());
            }
            raw.to_string()
        };
        if key.is_empty() {
            return Err(RestoreError::invalid_handle());
        }
        pairs.push((key.to_string(), value));
        if i < end {
            if bytes[i] != b',' {
                return Err(RestoreError::invalid_handle());
            }
            i += 1;
            if i >= end {
                return Err(RestoreError::invalid_handle());
            }
        }
    }
    Ok(pairs)
}
