# LumioVoxelEngine Framework Design Delivery

This directory is a documentation-only implementation design overlay for `LGE-V1.3-2026-08-27`.

## Start here

1. [`FRAMEWORK_IMPLEMENTATION_DESIGN.md`](FRAMEWORK_IMPLEMENTATION_DESIGN.md)
2. [`00_SOURCE_INVENTORY.md`](00_SOURCE_INVENTORY.md)
3. [`DECISION_GATES.md`](DECISION_GATES.md)
4. [`TASK_CARDS.md`](TASK_CARDS.md)
5. [`VERIFICATION_MATRIX.md`](VERIFICATION_MATRIX.md)
6. Ten package designs under [`packages/`](packages/01-revision-publication.md)

## Validate

```bash
python3 validate_design_package.py
```

The package contains no `.rs`, `Cargo.toml`, `.cs`, or `.csproj` production artifacts. File paths in task cards are implementation targets, not files created by this delivery. Angle-bracket crate aliases must be replaced only by the exact names resolved in `00_SOURCE_INVENTORY.md`/W0 evidence.
