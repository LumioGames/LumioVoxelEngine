# VOX-D-007 Spatial/Collision kernel adapter and cache key

- Card: R-00063 / GATE-007
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/spatial_collision.rs`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam. It does not freeze numeric defaults, pick a default kernel, edit Schema/ID/default config, or implement production spatial/mesh/collision code.

Produces: `DecisionEvidenceVOXD007`; `SpatialKernelProposal{kernel,version,license,cacheKey,invalidation,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `1175b08808a3fc865f70ebfbfa66c576562864e2` (detached, includes R-00034 `8c49fba` and R-00041) |
| Architecture HEAD | `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550` (`main`, matches card lock) |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture ADR-014 SHA-256 | `f4a7c142675dc8d7a011d24ff92067606a935989f76aa6d83abb78d4aff80a2c` |
| Prerequisite R-00034 | Consumable. Workflow status `in_review` with evidence; worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline`. |
| Prerequisite R-00047 | **Unmet.** Live card is `backlog` / unimplemented. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `d0467f529132ef0b91227af1f8df26a5729e871873a1590b706f7fbbda32069d` exposes only crate-DAG / generated-clean guards. No `VoxelPortHarness` and no Reference/Native differential runner. No substitute harness was invented. |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen):**

- Spatial/mesh/collision results are Voxel **sources**, not Gameplay/AOI/permission verdicts.
- Cache entries, if any, must include World Context / instance generation / Revision (仓内 ADR 0005 Origin Token; module READMEs). Coordinate-only keys are forbidden.
- Generic algorithms belong in NativeCore; this repo only adapts.
- No cross-repo Spatial/Mesh/Collision Schema exists; output is not a public wire type.
- Query four-state missing-chunk mapping; projection must not trigger Streaming.

**Open on this gate (VOX-D-007):**

- Which Native Kernel adapter (and its version/license/source hash) is approved.
- Remaining cache-key fields beyond the frozen World/generation/Revision identity (config hash, kernel id, material/semantic hash).
- Invalidation policy (event set, late-completion fencing).
- Precision / tolerance / LOD activation for P2.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors. Must not add unaudited dependencies.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion.

No third-party physics/mesh crate is named as a landable candidate: license, version, and source hash are unknown until an architecture-owner audit. A generic placeholder is listed only to be **held out** of production.

| id | kernel | version | license | cacheKey | invalidation | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `reference-voxel-kernel` | in-tree Reference Voxel kernel (shared operation corpus with Native) | unversioned (no harness yet) | Apache-2.0 (in-tree) | `worldContext + generation + worldRevision + chunkRevisionSet + configHash + kernelId` | ChunkChanged + AvailabilityChanged + generation bump; late completion never reinserts | ADR-014 `f4a7c142…aff80a2c` | Stop if results omit Revision, cache Gameplay verdicts, or miss across World. Not excluded by measurement (none ran). Required as the differential baseline once R-00047 exists. |
| `nativecore-spatial-adapter` | NativeCore spatial/kernel-context adapter (generic algorithms stay in NativeCore) | pending published NativeCore artifact | pending (architecture-generated artifact license) | same frozen identity fields plus kernel/capability hash | same events; cancel/exception never hits cache | v1.4 architecture `f1d36acf…c1afebd0` | Stop if Native types leak into public Schema, or if artifact hash cannot be verified. Not excluded: no measurements and no published kernel artifact (R-00037 recorded generator absent). |
| `unaudited-oss-kernel` | any extra OSS collision/mesh kernel behind the same adapter | unknown | unknown / unaudited | n/a | n/a | none | **Held out** without measurement: card forbids unaudited dependencies. Not a selectable default. Re-open only after license, version, source hash, and architecture-owner audit. |

## 4. Measurement plan (not executed)

Fixed once R-00047 is consumable: machine, toolchain, seed, corpus, schedule. Three runs per input; SHA-256 of raw traces. Statistics: accuracy vs Reference, throughput, cache hit rate, peak memory. Precision/tolerance rules are architecture-owner fields, not chosen here.

**Benchmark matrix** (card):

| axis | observe |
| --- | --- |
| candidate projection | count, order stability, missing-chunk four-state |
| occlusion | boundary cases; no empty-world disguise |
| mesh | canonical vertices/indices/hash vs Reference |
| collision | canonical shapes/hash vs Reference |
| cache | hit/miss under Revision/World/config/kernel changes; memory |

**Fault matrix** (card):

| fault | required observable |
| --- | --- |
| cross World | no cache hit; no shared mutable entry |
| cross Revision | miss; stale source not inserted |
| cancel | no cache insert; stable outcome |
| exceptional kernel result | stable generated outcome; never a Gameplay verdict |

**Replay commands (after R-00047):**

```text
cargo test -p lumio-voxel-test-support --all-features
# Reference vs Native differential on a frozen corpus; three-run hash compare
```

## 5. Measurements

**未执行** because R-00047 is unmet. Correctness, determinism, and fault matrices have no raw results. No kernel is selected. `unaudited-oss-kernel` is held out on license/process grounds, not on a fabricated bench score.

## 6. Proposal (not approved)

```text
SpatialKernelProposal {
  kernel: pending-architecture-owner,
  version: pending-architecture-owner,
  license: pending-architecture-owner,
  cacheKey: pending-architecture-owner,       // identity fields in §2 are frozen; extras are not
  invalidation: pending-architecture-owner,
  approvalStatus: blocked
}
```

Approved public configuration / Capability bits must be generated by the architecture repository.

## 7. Architecture owner approval

- Record: **none**
- `approvalStatus`: **blocked**
- Who must decide: architecture owner (kernel vendor/version/license, cache-key extras, invalidation, precision/LOD).
- NativeCore artifact hash must be verifiable before `nativecore-spatial-adapter` can leave pending.

**Blocked downstream (later cards whose live 执行前置 lists this gate):**

- R-00163 `[程序·Spatial] 实现 Revision-scoped Voxel Candidate 与 Occlusion Source`
- R-00193 `[程序·Mesh] 实现 Revision-scoped Voxel Mesh Source 构建`
- R-00194 `[程序·Collision] 实现 Revision-scoped Voxel Collision Source 构建`

Transitively: R-00166 (lists R-00163, not this gate).

**Continuable without this approval:** this evidence file and the measurement seam; typed Source ports that refuse to cache and refuse to select an unapproved kernel.

## 8. Commands actually run

Full transcript: `tests-R-00063.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/*.rs` | 0 | after one rustfmt apply |
| `rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test benchmarks/decision_gates/spatial_collision.rs` | 0 | `tests::gate_remains_blocked` ok (`approval_status() == "blocked"`) |
| `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` (local junctions for `.claude/*` placeholders; not committed) |
| `node --import windows-symlink-junction.mjs --test .spec/tools/spec-lint.test.mjs` | 0 | 13/13 pass |
| `cargo fmt --all -- --check` | 0 | workspace members only; seams not in Cargo.toml |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | msvc check (no link) |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 (msvc: no `link.exe`; gnu: pre-existing live DAG metadata false-positive, not this card) | no `VoxelPortHarness`; measurements 未执行 |

Host `rust-toolchain.toml` stays `1.98.0` msvc. GNU rustc was used only to link seam tests; toolchain file was not modified.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added.
