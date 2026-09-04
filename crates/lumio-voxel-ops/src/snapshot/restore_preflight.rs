//! Validate encoded capture bytes off the live World.

#![forbid(unsafe_code)]

use super::decode::decode_canonical_object;
use super::manifest_adapter::{SNAPSHOT_HEADER_SCHEMA, SNAPSHOT_PAYLOAD_SCHEMA};
use super::{is_hash256, stable};
use crate::canonical::CanonicalObject;
use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SNAPSHOT_MAGIC};
use lumio_voxel_domain::config_snapshot::VoxelConfigSnapshot;
use std::collections::BTreeMap;

const SECTION_REVISION_PREFIX: &str = "sectionRevision.";

/// Stable restore/preflight error. `error_id` is interned from generated ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreError {
    error_id: &'static str,
}

impl RestoreError {
    pub fn error_id(&self) -> &'static str {
        self.error_id
    }

    pub(super) fn invalid_handle() -> Self {
        Self {
            error_id: stable("InvalidHandle"),
        }
    }

    pub(super) fn artifact_digest_mismatch() -> Self {
        Self {
            error_id: stable("ArtifactDigestMismatch"),
        }
    }

    pub(super) fn evidence_digest_mismatch() -> Self {
        Self {
            error_id: stable("EvidenceDigestMismatch"),
        }
    }

    pub(super) fn manifest_unsupported_version() -> Self {
        Self {
            error_id: stable("ManifestUnsupportedVersion"),
        }
    }

    pub(super) fn session_mismatch() -> Self {
        Self {
            error_id: stable("SessionMismatch"),
        }
    }

    pub(super) fn stale_epoch() -> Self {
        Self {
            error_id: stable("StaleEpoch"),
        }
    }

    pub(super) fn mapped(id: &'static str) -> Self {
        Self {
            error_id: stable(id),
        }
    }
}

impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id)
    }
}

impl std::error::Error for RestoreError {}

/// Typed capture fields after Canonical decode and preflight checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedRestore {
    world_id: String,
    context_id: String,
    generation: u64,
    world_revision: u64,
    section_revision_set: BTreeMap<String, u64>,
    config_hash: String,
    root_identity: [u8; 32],
}

impl DecodedRestore {
    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn context_id(&self) -> &str {
        &self.context_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn world_revision(&self) -> u64 {
        self.world_revision
    }

    pub fn section_revision_set(&self) -> &BTreeMap<String, u64> {
        &self.section_revision_set
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn root_identity(&self) -> [u8; 32] {
        self.root_identity
    }
}

/// Preflight a complete restore candidate. Does not take a World write lease.
pub struct RestorePreflight;

impl RestorePreflight {
    pub fn validate(
        bytes: &[u8],
        expected_world_id: &str,
        expected_generation: u64,
        snapshot: &VoxelConfigSnapshot,
    ) -> Result<DecodedRestore, RestoreError> {
        let _ = super::manifest_adapter::header_schema();
        let _ = super::manifest_adapter::payload_schema();
        if expected_world_id.is_empty() {
            return Err(RestoreError::invalid_handle());
        }
        if snapshot.baseline_id() != BASELINE_ID || snapshot.schema_epoch() != SCHEMA_EPOCH {
            return Err(RestoreError::manifest_unsupported_version());
        }

        // Decoded members are already unique: a canonical object is keyed by name and
        // `decode_canonical_object` rejects a repeat rather than keeping one of them.
        let fields = decode_canonical_object(bytes)?;

        let schema_id = require_text(&fields, "schemaId")?;
        let header_schema = require_text(&fields, "headerSchemaId")?;
        let magic = require_text(&fields, "magic")?;
        let schema_epoch = require_uint(&fields, "schemaEpoch")?;
        if schema_id != SNAPSHOT_PAYLOAD_SCHEMA
            || header_schema != SNAPSHOT_HEADER_SCHEMA
            || magic != SNAPSHOT_MAGIC
            || schema_epoch != SCHEMA_EPOCH
        {
            return Err(RestoreError::manifest_unsupported_version());
        }

        let world_id = require_text(&fields, "worldId")?;
        let context_id = require_text(&fields, "contextId")?;
        if world_id.is_empty() || context_id.is_empty() {
            return Err(RestoreError::invalid_handle());
        }
        if world_id != expected_world_id {
            return Err(RestoreError::session_mismatch());
        }

        let generation = require_uint(&fields, "generation")?;
        if generation != expected_generation {
            return Err(RestoreError::stale_epoch());
        }
        let world_revision = require_uint(&fields, "worldRevision")?;

        let config_hash = require_text(&fields, "configHash")?;
        if !is_hash256(&config_hash) || config_hash != snapshot.config_hash() {
            return Err(RestoreError::evidence_digest_mismatch());
        }

        let root_hex = require_text(&fields, "rootIdentity")?;
        let root_identity = parse_hex32(&root_hex)?;

        let section_revision_set = collect_section_revisions(&fields)?;

        Ok(DecodedRestore {
            world_id,
            context_id,
            generation,
            world_revision,
            section_revision_set,
            config_hash,
            root_identity,
        })
    }
}

/// A string member. A member of any other type is a rejection, not a coercion.
fn require_text(fields: &CanonicalObject, key: &str) -> Result<String, RestoreError> {
    fields
        .get(key)
        .and_then(|value| value.as_text())
        .map(str::to_string)
        .ok_or_else(RestoreError::invalid_handle)
}

/// An integer member. A quoted digit string is a different value and stays rejected.
fn require_uint(fields: &CanonicalObject, key: &str) -> Result<u64, RestoreError> {
    fields
        .get(key)
        .and_then(|value| value.as_uint())
        .ok_or_else(RestoreError::invalid_handle)
}

fn collect_section_revisions(
    fields: &CanonicalObject,
) -> Result<BTreeMap<String, u64>, RestoreError> {
    let mut sections = BTreeMap::new();
    for (key, value) in fields.members() {
        if !key.starts_with(SECTION_REVISION_PREFIX) {
            if !is_known_field(key) {
                return Err(RestoreError::manifest_unsupported_version());
            }
            continue;
        }
        let section_id = &key[SECTION_REVISION_PREFIX.len()..];
        if section_id.is_empty() {
            return Err(RestoreError::invalid_handle());
        }
        let revision = value.as_uint().ok_or_else(RestoreError::invalid_handle)?;
        if sections.insert(section_id.to_string(), revision).is_some() {
            return Err(RestoreError::invalid_handle());
        }
    }
    Ok(sections)
}

fn is_known_field(key: &str) -> bool {
    matches!(
        key,
        "schemaId"
            | "headerSchemaId"
            | "magic"
            | "schemaEpoch"
            | "worldId"
            | "contextId"
            | "generation"
            | "worldRevision"
            | "configHash"
            | "rootIdentity"
    )
}

fn parse_hex32(value: &str) -> Result<[u8; 32], RestoreError> {
    if !is_hash256(value) {
        return Err(RestoreError::artifact_digest_mismatch());
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let start = i * 2;
        *slot = u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| RestoreError::artifact_digest_mismatch())?;
    }
    Ok(out)
}
