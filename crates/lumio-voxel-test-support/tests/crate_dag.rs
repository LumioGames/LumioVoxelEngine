use lumio_voxel_test_support::crate_dag;
use lumio_voxel_test_support::workspace_root_from_manifest;
use std::collections::BTreeMap;
use std::process::Command;

fn legal_graph() -> BTreeMap<String, Vec<String>> {
    crate_dag::parse_fixture_graph(include_str!(
        "../../../tools/architecture/fixtures/dag-legal.json"
    ))
}

#[test]
fn reverse_world_edge_is_rejected() {
    let json = include_str!("../../../tools/architecture/fixtures/dag-forbidden-world-dep.json");
    let graph = crate_dag::parse_fixture_graph(json);
    let v = crate_dag::violations(&graph);
    assert!(
        v.iter()
            .any(|s| s.contains("lumio-voxel-domain") && s.contains("lumio-voxel-world")),
        "expected reverse world edge to fail, got {v:?}"
    );
}

#[test]
fn production_cannot_depend_on_test_support() {
    let json = include_str!("../../../tools/architecture/fixtures/dag-forbidden-test-support.json");
    let graph = crate_dag::parse_fixture_graph(json);
    let v = crate_dag::violations(&graph);
    assert!(
        v.iter().any(|s| s.contains("test-support")),
        "expected test-support reverse edge to fail, got {v:?}"
    );
}

#[test]
fn extra_persistence_crate_is_rejected() {
    let json = include_str!("../../../tools/architecture/fixtures/dag-forbidden-persistence.json");
    let graph = crate_dag::parse_fixture_graph(json);
    let v = crate_dag::violations(&graph);
    assert!(
        v.iter().any(|s| s.contains("persistence")),
        "expected extra persistence crate to fail, got {v:?}"
    );
}

#[test]
fn legal_seven_crate_graph_has_no_violations() {
    let v = crate_dag::violations(&legal_graph());
    assert!(v.is_empty(), "legal fixture must pass, got {v:?}");
}

#[test]
fn cargo_metadata_lists_exactly_seven_members() {
    let root = workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(&root)
        .output()
        .expect("cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = String::from_utf8_lossy(&out.stdout);
    let idx = json
        .find("\"workspace_members\"")
        .expect("workspace_members");
    let slice = &json[idx..];
    let start = slice.find('[').unwrap();
    let end = slice.find(']').unwrap();
    let members = &slice[start..=end];
    let count = members.matches("lumio-voxel-").count();
    assert_eq!(count, 7, "workspace_members={members}");
    for name in crate_dag::SEVEN_CRATES {
        assert!(members.contains(name), "missing member {name} in {members}");
    }
    assert!(!members.contains("persistence"));
    assert!(!members.contains("runtime"));
    assert!(!json.contains("lumio-voxel-ffi"));
    assert!(!json.contains("lumio-voxel-common"));
}

#[test]
fn live_workspace_graph_has_no_violations() {
    let root = workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"));
    let graph = crate_dag::live_graph(&root).expect("live graph");
    let v = crate_dag::violations(&graph);
    assert!(v.is_empty(), "live DAG violations: {v:?}");
}
