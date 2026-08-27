//! CLI: `cargo check-generated-clean`

use lumio_voxel_test_support::generated_clean;
use lumio_voxel_test_support::workspace_root_from_manifest;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"));
    let lock_path = root.join(generated_clean::LOCK_PATH);
    let json = match std::fs::read_to_string(&lock_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cannot read {}: {e}", lock_path.display());
            return ExitCode::from(2);
        }
    };
    let locked = generated_clean::lock_from_json(&json);
    let generated = generated_clean::workspace_generated_dir(&root);
    let violations = generated_clean::violations(&generated, &locked);
    if violations.is_empty() {
        println!("check-generated-clean OK");
        ExitCode::SUCCESS
    } else {
        for v in &violations {
            eprintln!("FAIL {v}");
        }
        ExitCode::FAILURE
    }
}
