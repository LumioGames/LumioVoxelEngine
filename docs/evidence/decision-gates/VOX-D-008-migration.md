# VOX-D-008 Voxel Migration node granularity

- Card: R-00064 / GATE-008
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/migration_nodes.rs`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam. It does not freeze numeric defaults, pick a default node plan, edit Schema/ID/default config, or implement production migration code.

Produces: `DecisionEvidenceVOXD008`; `MigrationGranularityProposal{nodePlan,checkpointRule,budget,measurements,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `1175b08808a3fc865f70ebfbfa66c576562864e2` (detached, includes R-00034 `8c49fba` and R-00041) |
| Architecture HEAD | `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550` (`main`, matches card lock) |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture ADR-013 SHA-256 | `2fc97e229c7a8b325f319113c1fc285cc112f3b11cebf118dcb2a95df717a07a` |
| Prerequisite R-00034 | Consumable. Workflow status `in_review` with evidence; worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline`. |
| Prerequisite R-00047 | **Unmet.** Live card is `backlog` / unimplemented. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `d0467f529132ef0b91227af1f8df26a5729e871873a1590b706f7fbbda32069d` exposes only crate-DAG / generated-clean guards. No `VoxelPortHarness`. No substitute harness was invented. |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen):**

- ADR-013: immutable source Snapshot, acyclic DAG, staging, atomic activation of a **new** pointer, Game vs Voxel typed nodes, Host owns DAG/staging/activation.
- Manifest public fields include node ids, dependencies, `inputHash` / `outputHash` / `toolVersion`, idempotency.
- Failure: abort staging, keep previous Active; rerun from immutable inputs or a verified node checkpoint.
- This crate exposes `describe_nodes` / `run_node` / `verify_node` only. No `validate_manifest` of the full graph, no `request_activation`, no live-World writes, no Tick callback.

**Open on this gate (VOX-D-008):**

- Node split (chunk vs region vs schema-epoch vs whole-snapshot).
- Checkpoint rule (which boundaries are confirmed).
- Memory / redo budget (numeric values unfrozen).
- Replay grain used for crash-at-node benches.

Node payload Schema still must be registered in the architecture source before it is a public field.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors. Must not perform irreversible migration.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion.

| id | nodePlan | checkpointRule | budget | version | license | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `per-chunk-node` | one node per chunk transform | confirm after each node (Host-owned checkpoint record) | streaming; peak-memory cap unfrozen | unversioned | Apache-2.0 (in-tree) | ADR-013 `2fc97e22…f717a07a` | Stop if a crash mutates source Snapshot or Active. Not excluded: no measurements. |
| `per-region-node` | one node per declared region/AOI set | confirm after each node; intra-node not confirmed | region buffer; cap unfrozen | unversioned | Apache-2.0 (in-tree) | ADR-013 | Stop if region split is encoded as a new Manifest field without an architecture ADR. Not excluded: no measurements. |
| `whole-snapshot-node` | one Voxel node for the entire snapshot payload | confirm only when the whole node verifies | fully buffered; cap unfrozen | unversioned | Apache-2.0 (in-tree) | ADR-013 | Stop if peak memory is treated as "unlimited" or if partial output can activate. Not excluded: no measurements. |

Host DAG orchestration, fsync, and Active-pointer swap are **not** candidates; they stay Host-owned in every row.

## 4. Measurement plan (not executed)

Fixed once R-00047 is consumable: machine, toolchain, seed, corpus (small/large worlds, version spans), crash schedule. Three runs per input; SHA-256 of traces and output artifacts. Statistics: peak memory, redo volume, artifact size, verify time. No summary-only charts.

**Benchmark matrix** (card):

| axis | observe |
| --- | --- |
| small world | artifact size; verify time |
| large world | peak memory; redo volume |
| version span | rejected unknown schema; accepted golden upgrades |
| node crash | leftover unconfirmed candidate only |
| restart / replay | same `outputHash` from confirmed checkpoint |

**Fault matrix** (card):

| fault | required observable |
| --- | --- |
| node interrupt | old Active preserved; replay from last confirmed checkpoint |
| wrong input Hash | reject; no output |
| wrong toolVersion | reject; no output |
| corrupt output | not validated; Active unchanged |

**Replay commands (after R-00047):**

```text
cargo test -p lumio-voxel-test-support --all-features
# crash-at-boundary schedules; three-run outputHash compare; RestorePreflight on validated artifacts
```

## 5. Measurements

**未执行** because R-00047 is unmet. Correctness, determinism, and fault matrices have no raw results. No candidate is excluded. No node-split, checkpoint rule, or memory budget is written into production or this proposal.

## 6. Proposal (not approved)

```text
MigrationGranularityProposal {
  nodePlan: pending-architecture-owner,
  checkpointRule: pending-architecture-owner,
  budget: pending-architecture-owner,
  measurements: not-executed,
  approvalStatus: blocked
}
```

Approved public configuration must be generated by the architecture repository. Manifest node payload Schema, if needed, is registered there first.

## 7. Architecture owner approval

- Record: **none**
- `approvalStatus`: **blocked**
- Who must decide: architecture owner (node split, checkpoint grain, budgets).
- What must not be decided here: Host Active-pointer policy, fsync, full DAG.

**Blocked downstream (later cards whose live 执行前置 lists this gate):**

- R-00169 `[程序·Migration] 实现生成 Manifest 预检与纯节点转换器`

Transitively: R-00170 (lists R-00169, not this gate). R-00170 consumes `ApprovedMigrationGranularity` and must not invent node grain.

**Continuable without this approval:** this evidence file and the measurement seam; Manifest adapter work that validates generated fields without choosing split/budget.

## 8. Commands actually run

Full transcript: `tests-R-00064.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/*.rs` | 0 | after one rustfmt apply |
| `rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test benchmarks/decision_gates/migration_nodes.rs` | 0 | `tests::gate_remains_blocked` ok (`approval_status() == "blocked"`) |
| `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` (local junctions for `.claude/*` placeholders; not committed) |
| `node --import windows-symlink-junction.mjs --test .spec/tools/spec-lint.test.mjs` | 0 | 13/13 pass |
| `cargo fmt --all -- --check` | 0 | workspace members only; seams not in Cargo.toml |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | msvc check (no link) |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 (msvc: no `link.exe`; gnu: pre-existing live DAG metadata false-positive, not this card) | no `VoxelPortHarness`; measurements 未执行 |

Host `rust-toolchain.toml` stays `1.98.0` msvc. GNU rustc was used only to link seam tests; toolchain file was not modified.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added.
