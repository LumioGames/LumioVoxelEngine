# VOX-D-008 Voxel Migration node granularity

- Card: R-00064 / GATE-008
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/migration_nodes.rs`; `benchmarks/decision_gates/data/vox-d-008/`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam driven by the shipped R-00047 harness. It does not freeze numeric defaults, pick a default node plan, freeze `toolVersion` as a production default, edit Schema/ID/default config, or implement production migration code.

Produces: `DecisionEvidenceVOXD008`; `MigrationGranularityProposal{nodePlan,checkpointRule,budget,measurements,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (`feat(R-00047): add deterministic harness, faults and fixture runner`) |
| Architecture planning lock (card) | `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550` |
| Architecture checkout HEAD (hashed sources) | `3d5e29db72b70c88fb61e392832afe2a762b25cb` |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture ADR-013 SHA-256 | `2fc97e229c7a8b325f319113c1fc285cc112f3b11cebf118dcb2a95df717a07a` |
| `migration-manifest.schema.json` SHA-256 | `ba5ceb5f047a3a2bb7d694f791fda61c894d3bca739365fedf44850874c48618` |
| Prerequisite R-00034 | Consumable (this HEAD is after `8c49fba`). |
| Prerequisite R-00047 | **Met** at `b2f0d8a`. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `7e7bf9b24a8e31e55130bcd5c6336f748b2d456bdd4c8b20b3643082099ed742` exports `deterministic_executor` / `reference_harness` / `fault_injection` / `fixture_runner`. |
| `deterministic_executor.rs` SHA-256 | `46ae8ad5d6d4d27aa263a5d90918d35d8d606db12ea28aebc1433cd4125eec1e` |
| `reference_harness.rs` SHA-256 | `fcbc9274fe18eb028021b44e78e8e94e2da435e46ac4fdac3dbda3e94737ef1f` |
| `fault_injection.rs` SHA-256 | `b39959ed9723619733c566bcd7b356073c6480671c4aba5b1f72c666a1fd3104` |
| Seam `migration_nodes.rs` SHA-256 | `a935381fe4bdbd1247a90a83756e21edc84487ee237ecd19045ffecb6810465d` |
| Seam measurements artifact SHA-256 | `fe3ffb70302c4e1846f61cb17cac204102aeea0d247f0d38249f3bb34c5edfc8` (`benchmarks/decision_gates/data/vox-d-008/measurements.txt`) |
| Machine | `LUMIO` / Microsoft Windows NT 10.0.26200.0 |
| Toolchain (host file) | `rustc 1.98.0 (88d9e12ae 2026-08-18)` `x86_64-pc-windows-msvc` |
| Seed | `0x000000000000d008` |

`lumio_voxel_contracts::SCHEMA_IDS` contains `migration-manifest`. Every seam op uses that leaked id. No second schema id was invented.

## 2. Frozen contract vs open fields

**Already frozen (do not reopen):**

- ADR-013: immutable source Snapshot, acyclic DAG, staging, atomic activation of a **new** pointer, Game vs Voxel typed nodes, Host owns DAG/staging/activation.
- Manifest public fields include node ids, dependencies, `inputHash` / `outputHash` / `toolVersion`, idempotency.
- Failure: abort staging, keep previous Active; rerun from immutable inputs or a verified node checkpoint.
- This crate exposes `describe_nodes` / `run_node` / `verify_node` only. No `validate_manifest` of the full graph, no `request_activation`, no live-World writes, no Tick callback.
- **Frozen on this re-measure:** architecture `migration-manifest` DAG shape (nodeId, dependsOn, inputHash, outputHash, toolVersion, idempotent).

**Open on this gate (VOX-D-008):**

- Node split (chunk vs region vs schema-epoch vs whole-snapshot).
- Checkpoint rule (which boundaries are confirmed).
- Memory / redo budget (numeric values unfrozen).
- Replay grain used for crash-at-node benches.
- Production `toolVersion` default (seam-local `0.0.0` / `0.0.1` are mismatch labels only).

Node payload Schema still must be registered in the architecture source before it is a public field.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors. Must not perform irreversible migration.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion.

| id | nodePlan | checkpointRule | budget | version | license | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `per-chunk-node` | one node per chunk transform | confirm after each node (Host-owned checkpoint record) | streaming; peak-memory cap unfrozen | unversioned | Apache-2.0 (in-tree) | ADR-013 `2fc97e22…f717a07a` | Stop if a crash mutates source Snapshot or Active. Not excluded: seam corpus does not measure peak memory. |
| `per-region-node` | one node per declared region/AOI set | confirm after each node; intra-node not confirmed | region buffer; cap unfrozen | unversioned | Apache-2.0 (in-tree) | ADR-013 | Stop if region split is encoded as a new Manifest field without an architecture ADR. Not excluded: no large-world numbers. |
| `whole-snapshot-node` | one Voxel node for the entire snapshot payload | confirm only when the whole node verifies | fully buffered; cap unfrozen | unversioned | Apache-2.0 (in-tree) | ADR-013 | Stop if peak memory is treated as "unlimited" or if partial output can activate. Not excluded: no peak-memory numbers. |

Host DAG orchestration, fsync, and Active-pointer swap are **not** candidates; they stay Host-owned in every row.

## 4. Measurement plan (seam corpus executed)

Fixed: machine `LUMIO`, toolchain `1.98.0`, seed `0xD008`, corpus below. Three runs per input; SHA-256 of traces and snapshots from `lumio_voxel_contracts::sha256`. No summary-only charts. Payload hashes are SHA-256 of the UTF-8 input bytes actually scheduled (not placeholder `aaaa…` from architecture JSON fixtures).

**Benchmark matrix** (card vs this re-measure):

| axis | observe | this re-measure |
| --- | --- | --- |
| small world | artifact size; verify time | linear + diamond DAG schedule replay (not a Voxel world) |
| large world | peak memory; redo volume | **unmeasured** (harness is not a world migrator) |
| version span | rejected unknown schema; accepted golden upgrades | toolVersion-mismatch reject; ops use generated `migration-manifest` only |
| node crash | leftover unconfirmed candidate only | FaultPoints after visible write; no Active pointer in this crate |
| restart / replay | same `outputHash` from confirmed checkpoint | three-run identical `snapshot_hash` / `trace_digest` |

**Fault matrix** (card vs this re-measure):

| fault | required observable | mapping / result |
| --- | --- | --- |
| node interrupt | old Active preserved; replay from last confirmed checkpoint | Host-owned Active is out of scope; seam uses unrecoverable FaultPoints after a visible write |
| wrong input Hash | reject; no output | corpus `hash-mismatch` → `ManifestDigestMismatch`; `executed=false` |
| wrong toolVersion | reject; no output | corpus `tool-version-mismatch` → `ManifestUnsupportedVersion`; `executed=false` |
| corrupt output | not validated; Active unchanged | `CorruptSnapshot` / `EvidenceDigestMismatch` (`recoverable=false`, write already published) |
| cycle (architecture invalid fixture) | reject DAG | `FaultPoint::PostPublication` / `PartialLoadRolledBack` |
| missing-node-hash | reject | `FaultPoint::LostResult` / `EvidenceMissing` |
| missing-tool-version | reject | `FaultPoint::CorruptSnapshot` / `EvidenceDigestMismatch` |

Architecture invalid fixtures (file SHA-256, not copied as node hashes):

| fixture | SHA-256 |
| --- | --- |
| `fixtures/valid/migration-manifest.json` | `3aa5c0faec437d41c1a72a87e968410f104700275f6ef17c4559befb6deefea1` |
| `fixtures/invalid/migration-cycle.json` | `4be7b282458f95698b809ef3420c594a7cd7f9b8aa6fb8c457e18d4e8bf9aa06` |
| `fixtures/invalid/migration-missing-node-hash.json` | `5b8d18899970faabca3a6647cfc2750aee322e1bf2e51b58b36eb7b777b0a595` |
| `fixtures/invalid/migration-missing-tool-version.json` | `66a1ae303591f4e038d0bc4192a69be586917417b8042218edc564bb4fa517b0` |

Those JSON node `inputHash`/`outputHash` fields are schema-shaped placeholders. The seam does **not** reuse them.

**Replay commands:**

```text
cargo test -p lumio-voxel-test-support --all-features
cargo build --lib
rustc --edition 2024 --crate-type rlib --crate-name vox_d_008_seam -L target/debug/deps --extern lumio_voxel_test_support=<rlib> --extern lumio_voxel_contracts=<rlib> benchmarks/decision_gates/migration_nodes.rs -o <seam-out>/vox-d-008.rlib
# optional hash capture (gnu host rustc; rust-toolchain.toml unchanged):
rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test --crate-name vox_d_008_seam ...
```

## 5. Measurements

Seam corpus **executed** against shipped `DeterministicExecutor` / `VoxelPortHarness` / `FaultPoint`. Three repeats per case; traces compared by `PartialEq` (byte identity). Raw values: `benchmarks/decision_gates/data/vox-d-008/measurements.txt`.

No candidate is excluded. No node-split, checkpoint rule, memory budget, or production `toolVersion` is written into production or this proposal.

### 5.1 Correctness / determinism (3 runs)

| case | runs | identical | snapshot_hash | trace_digest |
| --- | --- | --- | --- | --- |
| `linear-dag` (n0→n1→n2) | 3 | yes | `0f0bb0c2444cc735a6404cf610a376d6b3e17cc093b72b9a56aecfe520da3995` | `971370a8784d12b1f95ad2b66318b82b72d0dd7b32daba04c150a79372c78f95` |
| `diamond-dag` (n0; n1,n2←n0; n3←n1,n2) | 3 | yes | `29e014ac1fa725d33124fcc924ffd92a00f18bea7c4a95b7c7c6df56d996dea6` | `f285647ebf2d3e735ba9bc343d527417ee693275a6361cae984ca35556bb1cfc` |
| `hash-mismatch` | 3 | yes (reject) | n/a (no execute) | n/a |
| `tool-version-mismatch` | 3 | yes (reject) | n/a (no execute) | n/a |

Rejects use generated error ids only: `ManifestDigestMismatch`, `ManifestUnsupportedVersion`. `executed=false` (no `VoxelPortHarness::execute`).

### 5.2 Faults (unrecoverable after visible write)

| case | FaultPoint | error id | recoverable | wrote | snapshot_hash (3-run identical) |
| --- | --- | --- | --- | --- | --- |
| `cycle` | `PostPublication` | `PartialLoadRolledBack` | false | yes | `c802aad98127acf2f3405020e30e03e9b5b0d3f980db240452e9ebf6ef9f277c` |
| `missing-node-hash` | `LostResult` | `EvidenceMissing` | false | yes | `e5eebbbdc76070f03b0cd744a6a6363c7d834b3d71dd99616b4baa7a8828160f` |
| `missing-tool-version` | `CorruptSnapshot` | `EvidenceDigestMismatch` | false | yes | `a84429663e3fd4f968bd27fe8419f2f3b579b3aeaa7722f195ce4339f8ccead0` |

`PrePublication` / `StaleCompletion` were not used: they are recoverable and fire before a visible write.

### 5.3 Unmeasured (stop thresholds for node-split still open)

Peak memory, redo volume, artifact size of a real world, and verify-time of production transformers were **not** collected. The shipped harness replays `GeneratedVoxelOperation` schedules; it does not load Voxel Snapshots. Those axes remain open for the architecture owner.

## 6. Proposal (not approved)

```text
MigrationGranularityProposal {
  nodePlan: pending-architecture-owner,
  checkpointRule: pending-architecture-owner,
  budget: pending-architecture-owner,
  measurements: seam-corpus-executed,
  approvalStatus: blocked
}
```

Approved public configuration must be generated by the architecture repository. Manifest node payload Schema, if needed, is registered there first. Seam-local tool version strings are not a production default.

## 7. Architecture owner approval

- Record: **none**
- `approvalStatus`: **blocked**
- Who must decide: architecture owner (node split, checkpoint grain, budgets, production toolVersion).
- What must not be decided here: Host Active-pointer policy, fsync, full DAG.
- What this re-measure freezes: nothing beyond the already-published `migration-manifest` DAG shape.

**Blocked downstream (later cards whose live 执行前置 lists this gate):**

- R-00169 `[程序·Migration] 实现生成 Manifest 预检与纯节点转换器`

Transitively: R-00170 (lists R-00169, not this gate). R-00170 consumes `ApprovedMigrationGranularity` and must not invent node grain.

**Continuable without this approval:** this evidence file, the measurement seam, and Manifest adapter work that validates generated fields without choosing split/budget.

## 8. Commands actually run

Full transcript: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00064.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/migration_nodes.rs` | 0 | formatted then clean |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 | `error: linker link.exe not found` (msvc). Libs compiled; test binary not linked. |
| `cargo build --lib` | 0 | `Finished dev profile`; rlibs in `target/debug/deps` (no `link.exe`) |
| `rustc --edition 2024 --crate-type rlib --crate-name vox_d_008_seam -L target/debug/deps --extern lumio_voxel_test_support=liblumio_voxel_test_support-6da53ff03bc58986.rlib --extern lumio_voxel_contracts=liblumio_voxel_contracts-a62c2cca441f7fde.rlib benchmarks/decision_gates/migration_nodes.rs -o .../seam-out/vox-d-008.rlib` | 0 | rlib SHA-256 `0caab3f37b5e9f385be935f95883d557c49d73c85a93cccf5681f699d9e7e7b5` |
| `rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test ... migration_nodes.rs` then `vox-d-008-tests.exe --nocapture` | 0 | 8 passed; hashes in §5. GNU host used only to link tests; `rust-toolchain.toml` not modified. |

Host `rust-toolchain.toml` stays `1.98.0` msvc.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added. No commit.
