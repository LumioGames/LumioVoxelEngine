#!/usr/bin/env python3
"""Drive the shipped DAG/generated-clean tools against fixtures and live cargo metadata."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PY = sys.executable
DAG = ROOT / "tools" / "architecture" / "check_crate_dag.py"
CLEAN = ROOT / "tools" / "architecture" / "check_generated_clean.py"
FIX = ROOT / "tools" / "architecture" / "fixtures"


def run(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=ROOT, text=True, capture_output=True)


def expect_fail(args: list[str], needle: str) -> None:
    proc = run(args)
    text = proc.stdout + proc.stderr
    if proc.returncode == 0 or needle not in text:
        raise SystemExit(f"expected fail containing {needle!r}: rc={proc.returncode}\n{text}")
    print("PASS fail-case", needle, args[-1] if args else "")


def expect_ok(args: list[str]) -> None:
    proc = run(args)
    if proc.returncode != 0:
        raise SystemExit(f"expected ok: {' '.join(args)}\n{proc.stdout}{proc.stderr}")
    print("PASS ok-case", " ".join(args[-2:]))


def main() -> int:
    expect_fail(
        [PY, str(DAG), str(FIX / "dag-forbidden-world-dep.json")],
        "lumio-voxel-world",
    )
    expect_fail(
        [PY, str(DAG), str(FIX / "dag-forbidden-test-support.json")],
        "test-support",
    )
    expect_fail(
        [PY, str(DAG), str(FIX / "dag-forbidden-persistence.json")],
        "persistence",
    )
    expect_ok([PY, str(DAG), str(FIX / "dag-legal.json")])
    expect_ok([PY, str(DAG)])
    expect_ok([PY, str(CLEAN)])

    generated = ROOT / "crates" / "lumio-voxel-contracts" / "generated"
    rogue = generated / "handwritten-guard.rs"
    rogue.write_text("pub struct FakeDto;\n", encoding="utf-8")
    try:
        expect_fail([PY, str(CLEAN)], "未锁定文件")
    finally:
        rogue.unlink(missing_ok=True)
    expect_ok([PY, str(CLEAN)])

    meta = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=ROOT,
            text=True,
        )
    )
    id_to_name = {p["id"]: p["name"] for p in meta["packages"]}
    members = [id_to_name[mid] for mid in meta["workspace_members"]]
    if len(members) != 7:
        raise SystemExit(f"expected 7 members, got {members}")
    for name in (
        "lumio-voxel-contracts",
        "lumio-voxel-domain",
        "lumio-voxel-ops",
        "lumio-voxel-world",
        "lumio-voxel-project",
        "lumio-voxel-migration",
        "lumio-voxel-test-support",
    ):
        if name not in members:
            raise SystemExit(f"missing {name} in {members}")
    joined = " ".join(members)
    if any(tok in joined for tok in ("persistence", "runtime", "ffi", "common")):
        raise SystemExit(f"forbidden crate in members: {members}")
    print("PASS cargo metadata seven members", members)
    print("ALL_PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
