//! Workspace crate DAG guard (ADR-0006 / R-00041).
//!
//! `violations` is the shipped checker: tests and the `check-crate-dag`
//! example both call it. It does not re-implement Cargo; callers pass a
//! resolved (crate → normal deps) map.

use std::collections::{BTreeMap, BTreeSet};

pub const SEVEN_CRATES: [&str; 7] = [
    "lumio-voxel-contracts",
    "lumio-voxel-domain",
    "lumio-voxel-ops",
    "lumio-voxel-world",
    "lumio-voxel-project",
    "lumio-voxel-migration",
    "lumio-voxel-test-support",
];

const FORBIDDEN_EXTRA_TOKENS: [&str; 4] = ["persistence", "runtime", "ffi", "common"];

fn allowed_deps() -> BTreeMap<&'static str, BTreeSet<&'static str>> {
    BTreeMap::from([
        ("lumio-voxel-contracts", BTreeSet::from([])),
        (
            "lumio-voxel-domain",
            BTreeSet::from(["lumio-voxel-contracts"]),
        ),
        (
            "lumio-voxel-ops",
            BTreeSet::from(["lumio-voxel-contracts", "lumio-voxel-domain"]),
        ),
        (
            "lumio-voxel-project",
            BTreeSet::from([
                "lumio-voxel-contracts",
                "lumio-voxel-domain",
                "lumio-voxel-ops",
            ]),
        ),
        (
            "lumio-voxel-world",
            BTreeSet::from([
                "lumio-voxel-contracts",
                "lumio-voxel-domain",
                "lumio-voxel-ops",
                "lumio-voxel-project",
            ]),
        ),
        (
            "lumio-voxel-migration",
            BTreeSet::from([
                "lumio-voxel-contracts",
                "lumio-voxel-domain",
                "lumio-voxel-ops",
            ]),
        ),
        (
            "lumio-voxel-test-support",
            BTreeSet::from([
                "lumio-voxel-contracts",
                "lumio-voxel-domain",
                "lumio-voxel-ops",
                "lumio-voxel-world",
                "lumio-voxel-project",
                "lumio-voxel-migration",
            ]),
        ),
    ])
}

/// Return human-readable violations for a workspace normal-dependency graph.
pub fn violations(graph: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let allowed = allowed_deps();
    let seven: BTreeSet<&str> = SEVEN_CRATES.into_iter().collect();
    let mut out = Vec::new();

    let names: BTreeSet<&str> = graph.keys().map(String::as_str).collect();
    for extra in names.difference(&seven) {
        out.push(format!("未登记的 workspace crate: {extra}"));
        if FORBIDDEN_EXTRA_TOKENS.iter().any(|tok| extra.contains(tok)) {
            out.push(format!("禁止的额外 crate 名: {extra}"));
        }
    }
    for missing in seven.difference(&names) {
        out.push(format!("缺少冻结 crate: {missing}"));
    }

    for (krate, deps) in graph {
        if krate.contains("core-engine") || krate.contains("coreengine") {
            out.push(format!("禁止的 CoreEngine 源依赖 crate: {krate}"));
        }
        let Some(allow) = allowed.get(krate.as_str()) else {
            continue;
        };
        for dep in deps {
            if dep.contains("core-engine")
                || dep.contains("coreengine")
                || dep.contains("lumio-core")
            {
                out.push(format!("禁止的 CoreEngine 源依赖: {krate} -> {dep}"));
                continue;
            }
            if dep == "lumio-voxel-world"
                && krate != "lumio-voxel-world"
                && krate != "lumio-voxel-test-support"
            {
                out.push(format!("L0–L4/Tool 不得依赖 world: {krate} -> {dep}"));
            }
            if dep == "lumio-voxel-test-support" && krate != "lumio-voxel-test-support" {
                out.push(format!(
                    "生产 crate 不得依赖 test-support: {krate} -> {dep}"
                ));
            }
            if seven.contains(dep.as_str()) && !allow.contains(dep.as_str()) {
                out.push(format!("禁止的依赖方向: {krate} -> {dep}"));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Parse a compact fixture: `{ "crate": ["dep", ...] }`.
pub fn parse_fixture_graph(json: &str) -> BTreeMap<String, Vec<String>> {
    let mut graph = BTreeMap::new();
    let trimmed = json.trim();
    assert!(
        trimmed.starts_with('{') && trimmed.ends_with('}'),
        "fixture must be a JSON object"
    );
    let body = &trimmed[1..trimmed.len() - 1];
    if body.trim().is_empty() {
        return graph;
    }
    for entry in split_top_level(body, ',') {
        let (key, value) = entry
            .split_once(':')
            .expect("fixture entry needs key:value");
        let name = unquote(key);
        let mut deps = Vec::new();
        let value = value.trim();
        assert!(
            value.starts_with('[') && value.ends_with(']'),
            "deps must be array"
        );
        let inner = &value[1..value.len() - 1];
        if !inner.trim().is_empty() {
            for dep in split_top_level(inner, ',') {
                deps.push(unquote(dep));
            }
        }
        graph.insert(name, deps);
    }
    graph
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    assert!(
        s.starts_with('"') && s.ends_with('"'),
        "expected JSON string, got {s}"
    );
    s[1..s.len() - 1].to_string()
}

/// Build the live workspace graph via `cargo tree` (same method as NativeCore).
///
/// Each crate is queried on its own: `cargo tree` must not be passed
/// `--workspace` here, because that overrides `-p` and prints every member's
/// tree, which would give every crate the union of all depth-1 edges.
pub fn live_graph(
    workspace_root: &std::path::Path,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut graph = BTreeMap::new();
    for krate in SEVEN_CRATES {
        graph.insert(krate.to_string(), direct_deps(workspace_root, krate)?);
    }
    Ok(graph)
}

fn direct_deps(workspace_root: &std::path::Path, krate: &str) -> Result<Vec<String>, String> {
    let out = std::process::Command::new("cargo")
        .args([
            "tree", "-p", krate, "-e", "normal", "--depth", "1", "--prefix", "depth",
        ])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| format!("cargo tree 启动失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo tree -p {krate} 失败: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut deps = Vec::new();
    for line in stdout.lines() {
        if let Some(name) = line
            .strip_prefix('1')
            .and_then(|rest| rest.split_whitespace().next())
        {
            deps.push(name.to_string());
        }
    }
    Ok(deps)
}

fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    for (i, ch) in chars.iter().copied() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            c if c == sep && depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}
