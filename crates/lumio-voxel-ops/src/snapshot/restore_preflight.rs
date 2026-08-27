//! Validate encoded capture bytes off the live World.

#![forbid(unsafe_code)]

use super::decode::{decode_canonical_pairs, parse_u64, unquote};
use super::manifest_adapter::{SNAPSHOT_HEADER_SCHEMA, SNAPSHOT_PAYLOAD_SCHEMA};
use super::{is_hash256, stable};
use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SNAPSHOT_MAGIC};
use lumio_voxel_domain::config_snapshot::VoxelConfigSnapshot;
use std::collections::BTreeMap;

const CHUNK_REVISION_PREFIX: &str = "chunkRevision.";

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
    chunk_revision_set: BTreeMap<String, u64>,
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

    pub fn chunk_revision_set(&self) -> &BTreeMap<String, u64> {
        &self.chunk_revision_set
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

        let pairs = decode_canonical_pairs(bytes)?;
        let fields = collect_fields(pairs)?;

        let schema_id = require_quoted(&fields, "schemaId")?;
        let header_schema = require_quoted(&fields, "headerSchemaId")?;
        let magic = require_quoted(&fields, "magic")?;
        let schema_epoch = require_u64(&fields, "schemaEpoch")?;
        if schema_id != SNAPSHOT_PAYLOAD_SCHEMA
            || header_schema != SNAPSHOT_HEADER_SCHEMA
            || magic != SNAPSHOT_MAGIC
            || schema_epoch != SCHEMA_EPOCH
        {
            return Err(RestoreError::manifest_unsupported_version());
        }

        let world_id = require_quoted(&fields, "worldId")?;
        let context_id = require_quoted(&fields, "contextId")?;
        if world_id.is_empty() || context_id.is_empty() {
            return Err(RestoreError::invalid_handle());
        }
        if world_id != expected_world_id {
            return Err(RestoreError::session_mismatch());
        }

        let generation = require_u64(&fields, "generation")?;
        if generation != expected_generation {
            return Err(RestoreError::stale_epoch());
        }
        let world_revision = require_u64(&fields, "worldRevision")?;

        let config_hash = require_quoted(&fields, "configHash")?;
        if !is_hash256(&config_hash) || config_hash != snapshot.config_hash() {
            return Err(RestoreError::evidence_digest_mismatch());
        }

        let root_hex = require_quoted(&fields, "rootIdentity")?;
        let root_identity = parse_hex32(&root_hex)?;

        let chunk_revision_set = collect_chunk_revisions(&fields)?;

        Ok(DecodedRestore {
            world_id,
            context_id,
            generation,
            world_revision,
            chunk_revision_set,
            config_hash,
            root_identity,
        })
    }
}

fn collect_fields(pairs: Vec<(String, String)>) -> Result<BTreeMap<String, String>, RestoreError> {
    let mut fields = BTreeMap::new();
    for (key, value) in pairs {
        if fields.insert(key, value).is_some() {
            return Err(RestoreError::invalid_handle());
        }
    }
    Ok(fields)
}

fn require_quoted(fields: &BTreeMap<String, String>, key: &str) -> Result<String, RestoreError> {
    let value = fields.get(key).ok_or_else(RestoreError::invalid_handle)?;
    Ok(unquote(value)?.to_string())
}

fn require_u64(fields: &BTreeMap<String, String>, key: &str) -> Result<u64, RestoreError> {
    let value = fields.get(key).ok_or_else(RestoreError::invalid_handle)?;
    parse_u64(value)
}

fn collect_chunk_revisions(
    fields: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, u64>, RestoreError> {
    let mut chunks = BTreeMap::new();
    for (key, value) in fields {
        if !key.starts_with(CHUNK_REVISION_PREFIX) {
            if !is_known_field(key) {
                return Err(RestoreError::manifest_unsupported_version());
            }
            continue;
        }
        let chunk_id = &key[CHUNK_REVISION_PREFIX.len()..];
        if chunk_id.is_empty() {
            return Err(RestoreError::invalid_handle());
        }
        let revision = parse_u64(value)?;
        if chunks.insert(chunk_id.to_string(), revision).is_some() {
            return Err(RestoreError::invalid_handle());
        }
    }
    Ok(chunks)
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
