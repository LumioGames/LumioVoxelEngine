//! Immutable Voxel config snapshot and Capability view (R-00066).
//!
//! Numeric VOX-D defaults are not materialized here. Blocked P0 gates refuse
//! to start affected capabilities and report the blocked gate list.

use lumio_voxel_contracts::{BASELINE_ID, SCHEMA_EPOCH, SCHEMA_IDS, STABLE_ERROR_IDS};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

pub const P0_DECISION_GATES: &[&str] = &["VOX-D-001", "VOX-D-002", "VOX-D-003", "VOX-D-004"];

const APPROVED: &str = "approved";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateSourceHashes {
    pub architecture_baseline_id: String,
    pub voxel_head: String,
    pub architecture_mirror_sha256: String,
    pub v13_decision_gates_sha256: String,
    pub blueprint_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionEvidence {
    pub gate_id: String,
    pub approval_status: String,
    pub source_hashes: GateSourceHashes,
    pub evidence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedHostCapability {
    capabilities: Vec<String>,
}

impl GeneratedHostCapability {
    pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut capabilities: Vec<String> = names.into_iter().map(Into::into).collect();
        capabilities.sort();
        capabilities.dedup();
        Self { capabilities }
    }

    pub fn names(&self) -> &[String] {
        &self.capabilities
    }

    fn contains(&self, name: &str) -> bool {
        self.capabilities.iter().any(|n| n == name)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GeneratedVoxelConfig {
    pub schema_id: &'static str,
    pub host_capability_schema_id: &'static str,
    pub schema_epoch: u64,
    pub config_hash: String,
    pub gate_source_hashes: BTreeMap<String, String>,
    pub host_capability: GeneratedHostCapability,
    pub start_capabilities: Vec<String>,
    pub key_material: Option<String>,
}

impl fmt::Debug for GeneratedVoxelConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeneratedVoxelConfig")
            .field("schema_id", &self.schema_id)
            .field("host_capability_schema_id", &self.host_capability_schema_id)
            .field("schema_epoch", &self.schema_epoch)
            .field("config_hash", &self.config_hash)
            .field("gate_source_hashes", &self.gate_source_hashes)
            .field("host_capability", &self.host_capability)
            .field("start_capabilities", &self.start_capabilities)
            .finish()
    }
}

#[derive(PartialEq, Eq)]
pub struct VoxelConfigSnapshot {
    baseline_id: &'static str,
    schema_epoch: u64,
    config_hash: String,
    gate_source_hashes: BTreeMap<String, String>,
    allow_list: Vec<String>,
}

impl fmt::Debug for VoxelConfigSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoxelConfigSnapshot")
            .field("baseline_id", &self.baseline_id)
            .field("schema_epoch", &self.schema_epoch)
            .field("config_hash", &self.config_hash)
            .field("gate_source_hashes", &self.gate_source_hashes)
            .field("capabilities", &self.allow_list)
            .finish()
    }
}

impl VoxelConfigSnapshot {
    pub fn from_generated(
        config: &GeneratedVoxelConfig,
        gate_evidence: &[DecisionEvidence],
    ) -> Result<Arc<Self>, ConfigError> {
        require_generated_schema(config.schema_id, "config-table")?;
        require_generated_schema(config.host_capability_schema_id, "host-capability")?;
        if config.schema_epoch != SCHEMA_EPOCH {
            return Err(ConfigError::SchemaEpochMismatch {
                error_id: stable_error("StaleEpoch"),
                found: config.schema_epoch,
            });
        }
        if !is_hash256(&config.config_hash) {
            return Err(ConfigError::HashMismatch {
                error_id: stable_error("EvidenceDigestMismatch"),
                gate: "configHash".to_string(),
            });
        }

        let mut by_gate: BTreeMap<&str, &DecisionEvidence> = BTreeMap::new();
        for ev in gate_evidence {
            if by_gate.insert(ev.gate_id.as_str(), ev).is_some() {
                return Err(ConfigError::HashMismatch {
                    error_id: stable_error("EvidenceDigestMismatch"),
                    gate: ev.gate_id.clone(),
                });
            }
        }

        let mut missing = Vec::new();
        let mut blocked = Vec::new();
        let mut verified_hashes = BTreeMap::new();
        let mut identity: Option<&GateSourceHashes> = None;
        for gate in P0_DECISION_GATES {
            let Some(ev) = by_gate.get(gate).copied() else {
                missing.push((*gate).to_string());
                continue;
            };
            if ev.source_hashes.architecture_baseline_id != BASELINE_ID {
                return Err(ConfigError::HashMismatch {
                    error_id: stable_error("EvidenceDigestMismatch"),
                    gate: (*gate).to_string(),
                });
            }
            if let Some(prev) = identity {
                if prev != &ev.source_hashes {
                    return Err(ConfigError::HashMismatch {
                        error_id: stable_error("EvidenceDigestMismatch"),
                        gate: (*gate).to_string(),
                    });
                }
            } else {
                identity = Some(&ev.source_hashes);
            }
            let expected = config.gate_source_hashes.get(*gate);
            if expected.map(String::as_str) != Some(ev.evidence_digest.as_str())
                || !is_hash256(&ev.evidence_digest)
            {
                return Err(ConfigError::HashMismatch {
                    error_id: stable_error("EvidenceDigestMismatch"),
                    gate: (*gate).to_string(),
                });
            }
            verified_hashes.insert((*gate).to_string(), ev.evidence_digest.clone());
            if ev.approval_status != APPROVED {
                blocked.push((*gate).to_string());
            }
        }
        if !missing.is_empty() {
            return Err(ConfigError::EvidenceMissing {
                error_id: stable_error("EvidenceMissing"),
                gates: missing,
            });
        }

        for name in &config.start_capabilities {
            if !config.host_capability.contains(name) {
                return Err(ConfigError::UnknownCapability {
                    error_id: stable_error("CapabilityMissing"),
                    name: name.clone(),
                });
            }
        }

        if !blocked.is_empty() {
            return Err(ConfigError::Blocked {
                error_id: stable_error("TrustPolicyRejected"),
                gates: blocked,
            });
        }

        Ok(Arc::new(Self {
            baseline_id: BASELINE_ID,
            schema_epoch: SCHEMA_EPOCH,
            config_hash: config.config_hash.clone(),
            gate_source_hashes: verified_hashes,
            allow_list: config.host_capability.names().to_vec(),
        }))
    }

    pub fn baseline_id(&self) -> &'static str {
        self.baseline_id
    }

    pub fn schema_epoch(&self) -> u64 {
        self.schema_epoch
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }

    pub fn gate_source_hashes(&self) -> &BTreeMap<String, String> {
        &self.gate_source_hashes
    }

    pub fn capabilities(&self) -> &[String] {
        &self.allow_list
    }

    pub fn audit_summary(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityView {
    enabled: BTreeSet<String>,
}

impl CapabilityView {
    pub fn derive(
        generated: &GeneratedHostCapability,
        snapshot: &VoxelConfigSnapshot,
    ) -> Result<Self, CapabilityError> {
        let allow: BTreeSet<&str> = snapshot.allow_list.iter().map(String::as_str).collect();
        let mut enabled = BTreeSet::new();
        for name in generated.names() {
            if !allow.contains(name.as_str()) {
                return Err(CapabilityError::Expansion {
                    error_id: stable_error("ClaimNotGranted"),
                    name: name.clone(),
                });
            }
            enabled.insert(name.clone());
        }
        Ok(Self { enabled })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.enabled.contains(name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Blocked {
        error_id: &'static str,
        gates: Vec<String>,
    },
    EvidenceMissing {
        error_id: &'static str,
        gates: Vec<String>,
    },
    HashMismatch {
        error_id: &'static str,
        gate: String,
    },
    UnknownCapability {
        error_id: &'static str,
        name: String,
    },
    SchemaEpochMismatch {
        error_id: &'static str,
        found: u64,
    },
}

impl ConfigError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::Blocked { error_id, .. }
            | Self::EvidenceMissing { error_id, .. }
            | Self::HashMismatch { error_id, .. }
            | Self::UnknownCapability { error_id, .. }
            | Self::SchemaEpochMismatch { error_id, .. } => error_id,
        }
    }

    pub fn blocked_gates(&self) -> &[String] {
        match self {
            Self::Blocked { gates, .. } | Self::EvidenceMissing { gates, .. } => gates,
            _ => &[],
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blocked { error_id, gates } => {
                write!(f, "{error_id}: blocked gates {}", gates.join(", "))
            }
            Self::EvidenceMissing { error_id, gates } => {
                write!(f, "{error_id}: {}", gates.join(", "))
            }
            Self::HashMismatch { error_id, gate } => write!(f, "{error_id}: {gate}"),
            Self::UnknownCapability { error_id, name } => write!(f, "{error_id}: {name}"),
            Self::SchemaEpochMismatch { error_id, found } => {
                write!(f, "{error_id}: schemaEpoch {found}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityError {
    Expansion {
        error_id: &'static str,
        name: String,
    },
}

impl CapabilityError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::Expansion { error_id, .. } => error_id,
        }
    }
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expansion { error_id, name } => write!(f, "{error_id}: {name}"),
        }
    }
}

impl std::error::Error for CapabilityError {}

fn stable_error(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in generated STABLE_ERROR_IDS")
}

fn require_generated_schema(found: &str, expected: &str) -> Result<(), ConfigError> {
    let resolved = SCHEMA_IDS.iter().copied().find(|id| *id == expected);
    if resolved == Some(found) {
        Ok(())
    } else {
        Err(ConfigError::UnknownCapability {
            error_id: stable_error("CapabilityMissing"),
            name: found.to_string(),
        })
    }
}

fn is_hash256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
