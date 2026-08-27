#!/usr/bin/env python3
"""Workspace crate DAG guard. Reads cargo metadata or a compact JSON fixture."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SEVEN = [
    "lumio-voxel-contracts",
    "lumio-voxel-domain",
    "lumio-voxel-ops",
    "lumio-voxel-world",
    "lumio-voxel-project",
    "lumio-voxel-migration",
    "lumio-voxel-test-support",
]
ALLOWED = {
    "lumio-voxel-contracts": set(),
    "lumio-voxel-domain": {"lumio-voxel-contracts"},
    "lumio-voxel-ops": {"lumio-voxel-contracts", "lumio-voxel-domain"},
    "lumio-voxel-project": {
        "lumio-voxel-contracts",
        "lumio-voxel-domain",
        "lumio-voxel-ops",
    },
    "lumio-voxel-world": {
        "lumio-voxel-contracts",
        "lumio-voxel-domain",
        "lumio-voxel-ops",
        "lumio-voxel-project",
    },
    "lumio-voxel-migration": {
        "lumio-voxel-contracts",
        "lumio-voxel-domain",
        "lumio-voxel-ops",
    },
    "lumio-voxel-test-support": {
        "lumio-voxel-contracts",
        "lumio-voxel-domain",
        "lumio-voxel-ops",
        "lumio-voxel-world",
        "lumio-voxel-project",
        "lumio-voxel-migration",
    },
}
FORBIDDEN_TOKENS = ("persistence", "runtime", "ffi", "common")


def violations(graph: dict[str, list[str]]) -> list[str]:
    seven = set(SEVEN)
    names = set(graph)
    out: list[str] = []
    for extra in sorted(names - seven):
        out.append(f"未登记的 workspace crate: {extra}")
        if any(tok in extra for tok in FORBIDDEN_TOKENS):
            out.append(f"禁止的额外 crate 名: {extra}")
    for missing in sorted(seven - names):
        out.append(f"缺少冻结 crate: {missing}")
    for krate, deps in graph.items():
        allow = ALLOWED.get(krate)
        if krate.find("core-engine") >= 0 or krate.find("coreengine") >= 0:
            out.append(f"禁止的 CoreEngine 源依赖 crate: {krate}")
        if allow is None:
            continue
        for dep in deps:
            if "core-engine" in dep or "coreengine" in dep or "lumio-core" in dep:
                out.append(f"禁止的 CoreEngine 源依赖: {krate} -> {dep}")
                continue
            if dep == "lumio-voxel-world" and krate not in (
                "lumio-voxel-world",
                "lumio-voxel-test-support",
            ):
                out.append(f"L0–L4/Tool 不得依赖 world: {krate} -> {dep}")
            if dep == "lumio-voxel-test-support" and krate != "lumio-voxel-test-support":
                out.append(f"生产 crate 不得依赖 test-support: {krate} -> {dep}")
            if dep in seven and dep not in allow:
                out.append(f"禁止的依赖方向: {krate} -> {dep}")
    return sorted(set(out))


def graph_from_metadata(meta: dict) -> dict[str, list[str]]:
    packages = {p["id"]: p for p in meta.get("packages", [])}
    members = []
    for mid in meta.get("workspace_members", []):
        pkg = packages.get(mid)
        if pkg:
            members.append(pkg["name"])
    member_set = set(members)
    graph: dict[str, list[str]] = {name: [] for name in members}
    by_name = {p["name"]: p for p in meta.get("packages", [])}
    for name in members:
        pkg = by_name.get(name)
        if not pkg:
            continue
        deps = []
        for dep in pkg.get("dependencies", []):
            if dep.get("kind") not in (None, "normal"):
                continue
            dep_name = dep["name"]
            if dep_name in member_set:
                deps.append(dep_name)
        graph[name] = sorted(set(deps))
    return graph


def load_live() -> dict[str, list[str]]:
    out = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        text=True,
    )
    return graph_from_metadata(json.loads(out))


def main(argv: list[str]) -> int:
    if argv:
        graph = json.loads(Path(argv[0]).read_text(encoding="utf-8"))
    else:
        graph = load_live()
    bad = violations(graph)
    if bad:
        for line in bad:
            print("FAIL", line, file=sys.stderr)
        return 1
    print(f"check-crate-dag OK: {len(graph)} crates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
