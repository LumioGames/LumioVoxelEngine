#!/usr/bin/env python3
"""Reject handwritten files in the generated-contract directory."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATED = ROOT / "crates" / "lumio-voxel-contracts" / "generated"
LOCK = ROOT / "tools" / "architecture" / "generated-lock.json"


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def violations(generated: Path, locked: dict[str, str]) -> list[str]:
    out: list[str] = []
    if not generated.exists():
        if locked:
            out.append(f"生成目录缺失但 lock 非空: {generated}")
        return out
    found: dict[str, str] = {}
    for path in generated.rglob("*"):
        if path.is_dir():
            continue
        rel = path.relative_to(generated).as_posix()
        found[rel] = sha256_file(path)
    for rel, digest in found.items():
        want = locked.get(rel)
        if want is None:
            out.append(f"生成目录出现未锁定文件（疑似手改）: {rel}")
        elif want != digest:
            out.append(f"生成文件 hash 与 lock 不一致: {rel}")
    for rel in locked:
        if rel not in found:
            out.append(f"lock 中的生成文件缺失: {rel}")
    return sorted(out)


def load_lock(path: Path) -> dict[str, str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    return {item["rel"]: item["sha256"] for item in data.get("files", [])}


def main() -> int:
    bad = violations(GENERATED, load_lock(LOCK))
    if bad:
        for line in bad:
            print("FAIL", line, file=sys.stderr)
        return 1
    print("check-generated-clean OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
