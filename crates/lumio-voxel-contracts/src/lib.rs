//! L0 contract bindings.
//!
//! Two sources live here and they are not equals:
//!
//! * [`voxel_world`] is the live one. It mirrors `wire/voxel-world-v1.json`
//!   (`lumio.voxel-world.v1`), the frozen public contract for voxel world data —
//!   Block / Section / Chunk layering, canonical keys, limits, presence, error codes.
//!   `tests/voxel_world_conformance.rs` proves every value equals that JSON.
//! * The `generated/` tree is a read-only mirror of baseline `LGE-V1.4-2026-08-27`.
//!   **It is deprecated**: its publisher repo `LumioGameEngineArchitecture` no longer
//!   exists, so it can never be regenerated, and it names the 16×16×16 data unit
//!   `Chunk`, which the live contract forbids. Live code must not take voxel layering
//!   semantics from it. What it still supplies is baseline-neutral plumbing (SHA-256,
//!   bounded buffers, and schema / binding / error tables that are not voxel-layering
//!   names).
//!
//! This crate must not define a second set of Schema fields, IDs, or serializers.

#![forbid(unsafe_code)]

pub mod legacy_baseline;
pub mod voxel_world;

#[path = "../generated/rust/lumio-gen-canonical-serializer/src/lib.rs"]
#[rustfmt::skip]
#[allow(dead_code)]
mod lumio_gen_canonical_serializer;
// `canonical_object_pairs` lives in this generated module and is deliberately NOT
// re-exported: it concatenates unescaped keys and unquoted values and accepts a
// repeated key, so a caller can forge one member out of another's value. Voxel
// encodes through `lumio_voxel_ops::canonical` instead. Re-exporting it would keep
// the defect reachable from every consumer of this crate, which is how it spread in
// the first place; the generated file itself must not be hand-edited, so the helper
// is quarantined here and `dead_code` allowed at the seam.
#[path = "../generated/rust/lumio-gen-contract-runtime/src/lib.rs"]
#[rustfmt::skip]
#[allow(dead_code)]
mod lumio_gen_contract_runtime;
// The vendored generated tree is published upstream as independent library crates, where
// every item is crate-public and reachable. Here it is `#[path]`-included into private
// modules, so anything this crate does not re-export becomes unreachable and trips
// `dead_code` — notably the ADR-040 Root ABI surface, which Voxel does not consume.
// Allow it at the seam rather than re-exporting items nothing here uses (that would grow
// the public API to silence a lint) or patching the generated files (forbidden).
#[path = "../generated/rust/lumio-gen-contract-types/src/lib.rs"]
#[rustfmt::skip]
#[allow(dead_code, clippy::upper_case_acronyms)]
mod lumio_gen_contract_types;
#[path = "../generated/rust/lumio-gen-language-binding/src/lib.rs"]
#[rustfmt::skip]
#[allow(dead_code)]
mod lumio_gen_language_binding;
#[path = "../generated/rust/lumio-gen-mapping-table/src/lib.rs"]
#[rustfmt::skip]
#[allow(dead_code)]
mod lumio_gen_mapping_table;
#[path = "../generated/rust/lumio-gen-protocol-permission-validator/src/lib.rs"]
#[rustfmt::skip]
#[allow(dead_code)]
mod lumio_gen_protocol_permission_validator;

use lumio_gen_contract_runtime::sha256_hex;
use std::fs;
use std::path::{Path, PathBuf};

pub use lumio_gen_canonical_serializer::{
    SNAPSHOT_CHECKSUM_OMIT, SNAPSHOT_MAGIC, checksum_domain_doc,
};
pub use lumio_gen_contract_runtime::{
    BoundedBuffer, BufferFull, ChainBreak, Hash256, hash_chain_append, hash_chain_verify, sha256,
};
pub use lumio_gen_contract_types::{
    BASELINE_ID, MACHINE_IDS, STABLE_ERROR_IDS, Transition, VOXEL_WORLD_ROLES, machine_ids,
    state_transition_table,
};
pub use lumio_gen_language_binding::Binding;

// Re-exported as `static`, not `pub use` of the generated `const`. A `const` is inlined
// at every use site, so each consuming crate materializes its own copy and there is no
// canonical address for an interned identifier — `std::ptr::eq` against the table then
// compares two distinct per-crate allocations. These tables are the ones the intern seams
// hand back by reference, so they must have exactly one materialization.
//
// The mirror's `CHUNK_PRESENCE` is deliberately NOT re-exported. Its four names belong to
// the 16×16×16 data unit, which the live contract calls a Section, and its second state is
// `NotLoaded` where the contract says `Unchanged`. Presence now comes from
// `voxel_world::SECTION_PRESENCE`; re-exporting the mirror's table would keep the wrong
// layering name reachable from every consumer.
pub static SCHEMA_IDS: &[&str] = lumio_gen_contract_types::SCHEMA_IDS;
pub static BINDINGS: &[Binding] = lumio_gen_language_binding::BINDINGS;
pub use lumio_gen_mapping_table::{MAPPING_REQUIRED, MAPPING_ROLES};
pub use lumio_gen_protocol_permission_validator::{ACTIVE_PERMISSION_FIELDS, is_active_field};

/// Is `id` a stable error identifier this workspace may report?
///
/// Two namespaces coexist by design. Voxel world-data semantics report the live contract's
/// snake_case codes (`voxel_world::VOXEL_WORLD_ERROR_CODES`); engine-generic failures that
/// the voxel contract does not define — handles, sessions, budgets, queues — still report
/// the frozen mirror's PascalCase ids. A single predicate keeps callers from having to know
/// which side a given error came from.
pub fn is_stable_error_id(id: &str) -> bool {
    STABLE_ERROR_IDS.contains(&id) || voxel_world::is_error_code(id)
}

pub const CRATE_NAME: &str = "lumio-voxel-contracts";
pub const SCHEMA_EPOCH: u64 = 1;

/// Hash / baseline / epoch mismatch when loading published artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractLoadError {
    Missing { path: String },
    BaselineMismatch { artifact_id: String, found: String },
    SchemaEpochMismatch { artifact_id: String, found: u64 },
    HashMismatch { artifact_id: String },
    ImplementationDependency { artifact_id: String },
    Io(String),
}

impl std::fmt::Display for ContractLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { path } => write!(f, "missing generated artifact path {path}"),
            Self::BaselineMismatch { artifact_id, found } => {
                write!(f, "{artifact_id} baseline {found} != {BASELINE_ID}")
            }
            Self::SchemaEpochMismatch { artifact_id, found } => {
                write!(f, "{artifact_id} schemaEpoch {found} != {SCHEMA_EPOCH}")
            }
            Self::HashMismatch { artifact_id } => {
                write!(f, "{artifact_id} outputHash does not match package bytes")
            }
            Self::ImplementationDependency { artifact_id } => {
                write!(f, "{artifact_id} implementationDependencies must be empty")
            }
            Self::Io(msg) => write!(f, "artifact io: {msg}"),
        }
    }
}

impl std::error::Error for ContractLoadError {}

/// Default consume root: `crates/lumio-voxel-contracts/generated`.
pub fn generated_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generated")
}

/// Verify locked V1.4 artifacts at the crate's generated/ tree.
pub fn verify_artifact_hashes() -> Result<(), ContractLoadError> {
    verify_artifact_hashes_at(&generated_root())
}

/// Verify every published package under `root` (rust/ + csharp/).
pub fn verify_artifact_hashes_at(root: &Path) -> Result<(), ContractLoadError> {
    if !root.is_dir() {
        return Err(ContractLoadError::Missing {
            path: root.display().to_string(),
        });
    }
    let mut packages = Vec::new();
    for group in ["rust", "csharp"] {
        let group_dir = root.join(group);
        if !group_dir.is_dir() {
            return Err(ContractLoadError::Missing {
                path: group_dir.display().to_string(),
            });
        }
        let mut entries: Vec<_> = fs::read_dir(&group_dir)
            .map_err(|e| ContractLoadError::Io(e.to_string()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        for pkg in entries {
            let desc_path = pkg.join("artifact.descriptor.json");
            if desc_path.is_file() {
                packages.push(pkg);
            }
        }
    }
    if packages.len() != 12 {
        return Err(ContractLoadError::Missing {
            path: format!(
                "expected 12 kind×language packages, found {} under {}",
                packages.len(),
                root.display()
            ),
        });
    }
    for pkg in &packages {
        verify_one(pkg)?;
    }
    Ok(())
}

fn verify_one(pkg: &Path) -> Result<(), ContractLoadError> {
    let desc_path = pkg.join("artifact.descriptor.json");
    let text = fs::read_to_string(&desc_path).map_err(|e| ContractLoadError::Io(e.to_string()))?;
    let artifact_id = json_string(&text, "artifactId").unwrap_or_else(|| pkg.display().to_string());
    let baseline = json_string(&text, "baselineId").unwrap_or_default();
    if baseline != BASELINE_ID {
        return Err(ContractLoadError::BaselineMismatch {
            artifact_id,
            found: baseline,
        });
    }
    let epoch = json_u64(&text, "schemaEpoch").unwrap_or(u64::MAX);
    if epoch != SCHEMA_EPOCH {
        return Err(ContractLoadError::SchemaEpochMismatch {
            artifact_id,
            found: epoch,
        });
    }
    if !text.contains("\"implementationDependencies\":[]") {
        return Err(ContractLoadError::ImplementationDependency { artifact_id });
    }
    let declared = json_string(&text, "outputHash").unwrap_or_default();
    let computed = dir_output_hash(pkg)?;
    if declared != computed {
        return Err(ContractLoadError::HashMismatch { artifact_id });
    }
    Ok(())
}

fn dir_output_hash(directory: &Path) -> Result<String, ContractLoadError> {
    let mut files = collect_files(directory)?;
    files.retain(|(rel, _)| !rel.ends_with(".descriptor.json"));
    let mut lines = Vec::with_capacity(files.len());
    for (rel, bytes) in files {
        lines.push(format!("{rel}={}", sha256_hex(&bytes)));
    }
    Ok(sha256_hex(lines.join("\n").as_bytes()))
}

fn collect_files(root: &Path) -> Result<Vec<(String, Vec<u8>)>, ContractLoadError> {
    let mut out = Vec::new();
    collect_files_rec(root, root, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn collect_files_rec(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), ContractLoadError> {
    let entries = fs::read_dir(dir).map_err(|e| ContractLoadError::Io(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| ContractLoadError::Io(e.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_rec(root, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path).map_err(|e| ContractLoadError::Io(e.to_string()))?;
        out.push((rel, bytes));
    }
    Ok(())
}

fn json_string(obj: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let i = obj.find(&pat)?;
    let rest = &obj[i + pat.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn json_u64(obj: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{key}\":");
    let i = obj.find(&pat)?;
    let rest = obj[i + pat.len()..].trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}
