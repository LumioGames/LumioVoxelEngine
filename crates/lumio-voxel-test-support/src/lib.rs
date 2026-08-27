//! Test-support crate: workspace guards plus later harnesses (R-00047).
//!
//! Production crates must not depend on this package (crate DAG guard).

#![forbid(unsafe_code)]

pub mod crate_dag;
pub mod generated_clean;

pub const CRATE_NAME: &str = "lumio-voxel-test-support";

pub fn workspace_root_from_manifest(manifest_dir: &str) -> std::path::PathBuf {
    let manifest = std::path::Path::new(manifest_dir);
    manifest
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| manifest.to_path_buf())
}
