//! Test-support crate: workspace guards plus later harnesses (R-00047).
//!
//! Production crates must not depend on this package (crate DAG guard).

#![forbid(unsafe_code)]

pub mod b0_harness;
pub mod b2_harness;
pub mod crate_dag;
pub mod deterministic_executor;
pub mod fault_injection;
pub mod fixture_runner;
pub mod generated_clean;
pub mod mvp_harness;
pub mod reference_harness;

pub const CRATE_NAME: &str = "lumio-voxel-test-support";

pub fn workspace_root_from_manifest(manifest_dir: &str) -> std::path::PathBuf {
    let manifest = std::path::Path::new(manifest_dir);
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| manifest.to_path_buf())
}
