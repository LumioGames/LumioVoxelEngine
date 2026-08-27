//! CLI: `cargo check-crate-dag` — live workspace DAG or a JSON fixture.

use lumio_voxel_test_support::crate_dag;
use lumio_voxel_test_support::workspace_root_from_manifest;
use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let graph = if let Some(path) = args.first() {
        let json = match fs::read_to_string(path) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        };
        crate_dag::parse_fixture_graph(&json)
    } else {
        let root = workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"));
        match crate_dag::live_graph(&root) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        }
    };
    let violations = crate_dag::violations(&graph);
    if violations.is_empty() {
        println!("check-crate-dag OK: {} crates", graph.len());
        ExitCode::SUCCESS
    } else {
        for v in &violations {
            eprintln!("FAIL {v}");
        }
        ExitCode::FAILURE
    }
}
