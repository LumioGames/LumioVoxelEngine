# VOX-D-007 Spatial/Collision kernel adapter and cache key

- Card: R-00063 / GATE-007
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28; re-measured 2026-08-29 on a linking host (see §4)
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/spatial_collision.rs`; optional corpus `benchmarks/decision_gates/data/vox-d-007/`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam over the shipped Reference harness. It does not freeze numeric defaults, pick a default kernel, edit Schema/ID/default config, invent a public spatial Schema, or freeze NativeCore kernel artifact hashes as production defaults.

Produces: `DecisionEvidenceVOXD007`; `SpatialKernelProposal{kernel,version,license,cacheKey,invalidation,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (`feat(R-00047): add deterministic harness, faults and fixture runner`) |
| Architecture HEAD | `3d5e29db72b70c88fb61e392832afe2a762b25cb` (`main`, measured this run). Card lock cited `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550`; that commit is an ancestor, not re-checked out. |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` (Voxel mirror and Architecture copy identical) |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture ADR-014 SHA-256 | `e941c0e92b1bea98df0da9ac6dab6268aca1cbc042e4e7fab02e7e2685906747` (recomputed this run; not copied from the previous blocked write-up) |
| Prerequisite R-00034 | Consumable. Worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline`. |
| Prerequisite R-00047 | **Met.** Delivered at `b2f0d8a`. `lumio-voxel-test-support` exports `DeterministicExecutor`, `VoxelPortHarness`, `FaultPoint`, `fixture_runner`. No substitute harness was invented. |

R-00047 source SHA-256 (this HEAD, `Get-FileHash`):

| Path | SHA-256 |
| --- | --- |
| `crates/lumio-voxel-test-support/src/lib.rs` | `7e7bf9b24a8e31e55130bcd5c6336f748b2d456bdd4c8b20b3643082099ed742` |
| `crates/lumio-voxel-test-support/src/deterministic_executor.rs` | `46ae8ad5d6d4d27aa263a5d90918d35d8d606db12ea28aebc1433cd4125eec1e` |
| `crates/lumio-voxel-test-support/src/reference_harness.rs` | `fcbc9274fe18eb028021b44e78e8e94e2da435e46ac4fdac3dbda3e94737ef1f` |
| `crates/lumio-voxel-test-support/src/fault_injection.rs` | `b39959ed9723619733c566bcd7b356073c6480671c4aba5b1f72c666a1fd3104` |
| `crates/lumio-voxel-test-support/src/fixture_runner.rs` | `a8c6038c45f53ba411c4d495bbc8cab5d106f014d7ee853ca767f2272a63fd4c` |

Seam / corpus SHA-256 (this worktree, after rewrite):

| Path | SHA-256 |
| --- | --- |
| `benchmarks/decision_gates/spatial_collision.rs` | `594396f6a747c2812a622cef2f201ae680dd14c603f158ab7d04a1bbfc0b018f` |
| `benchmarks/decision_gates/data/vox-d-007/candidate-set.json` | `cec797265640d66a9b33382ee228925a1715d534f316cd6dbc91a8e6a22bbfdd` |
| `benchmarks/decision_gates/data/vox-d-007/occlusion-miss.json` | `c4cbc47fa24455c4eeacb9a4970e49417372839af1acad046f83a09c2e0aefb3` |
| `benchmarks/decision_gates/data/vox-d-007/cache-key-with-world-revision.json` | `5357b232fbd33c887444decbb1a55d68b1b0ec95a5e5baeaaf734c45559b95d1` |
| `benchmarks/decision_gates/data/vox-d-007/cancel-before-complete.json` | `2aacab36de75c5b9adfe7856c69ba4a08a8a7c99467d8adc02b5d539578ea54e` |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen):**

- Spatial/mesh/collision results are Voxel **sources**, not Gameplay/AOI/permission verdicts.
- Cache entries, if any, must include World Context / instance generation / Revision (仓内 ADR 0005 Origin Token; module READMEs). Coordinate-only keys are forbidden.
- Generic algorithms belong in NativeCore; this repo only adapts.
- No cross-repo Spatial/Mesh/Collision Schema exists; output is not a public wire type. Ops on this seam use generated `schema_id` `voxel-query` only (spatial is a projection over query).
- Query four-state missing-chunk mapping; projection must not trigger Streaming.

**Open on this gate (VOX-D-007):**

- Which Native Kernel adapter (and its version/license/source hash) is approved.
- Remaining cache-key fields beyond the frozen World/generation/Revision identity (config hash, kernel id, material/semantic hash).
- Invalidation policy (event set, late-completion fencing).
- Precision / tolerance / LOD activation for P2.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors. Must not add unaudited dependencies. Must not freeze a NativeCore kernel artifact hash as a production default.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion. `nativecore_kernel_artifact_hash()` on the seam returns `None`.

No third-party physics/mesh crate is named as a landable candidate: license, version, and source hash are unknown until an architecture-owner audit. A generic placeholder is listed only to be **held out** of production.

| id | kernel | version | license | cacheKey | invalidation | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `reference-voxel-kernel` | in-tree Reference Voxel kernel (shared operation corpus with Native once an artifact exists) | unversioned Reference harness at R-00047 | Apache-2.0 (in-tree) | `worldContext + generation + worldRevision + chunkRevisionSet` observed in seam payloads; extra fields unfrozen | ChunkChanged + AvailabilityChanged + generation bump; late completion never reinserts; PrePublication cancel does not insert | R-00047 `reference_harness.rs` `fcbc9274…737ef1f` | Stop if results omit Revision, cache Gameplay verdicts, or hit across World. Not excluded. Required differential baseline. |
| `nativecore-spatial-adapter` | NativeCore spatial/kernel-context adapter (generic algorithms stay in NativeCore) | pending published NativeCore artifact | pending (architecture-generated artifact license) | same frozen identity fields plus kernel/capability hash (kernel hash **not** written here) | same events; cancel/exception never hits cache | **not frozen** — no NativeCore kernel artifact hash is recorded as a production default | Stop if Native types leak into public Schema, or if an unpublished hash is copied into config. Not excluded by this harness run: Native differential 未执行 (artifact unpublished). |
| `unaudited-oss-kernel` | any extra OSS collision/mesh kernel behind the same adapter | unknown | unknown / unaudited | n/a | n/a | none | **Held out** without measurement: card forbids unaudited dependencies. Not a selectable default. Re-open only after license, version, source hash, and architecture-owner audit. |

## 4. Measurement seam — executed on a linking host

**Status: executed.** The previous revision reached its numbers through a Windows GNU cross-toolchain, used only because the msvc host had no `link.exe`. This gate was re-run on a host whose default toolchain links, so no substitute toolchain is involved and `cargo check` is not accepted as evidence.

Run of 2026-08-29, at repository commit `13d515f358ffeb182e9659d5bde4fa119496f711` (`origin/main`):

| leg | host triple | rustc | seam result |
| --- | --- | --- | --- |
| primary | `x86_64-apple-darwin` (Rosetta 2 on an Apple Silicon machine; rustup default host) | `1.98.0 (88d9e12ae 2026-08-18)`, pinned by `rust-toolchain.toml` | 9 passed / 0 failed |
| second | `aarch64-apple-darwin` (native) | `1.98.0 (88d9e12ae 2026-08-18)` | 9 passed / 0 failed; output byte-identical to the primary leg |

Generation commands:

```bash
benchmarks/decision_gates/run_seam_replay.sh spatial_collision
SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin SEAM_OUT_DIR=target/decision-gate-seams-aarch64 \
  benchmarks/decision_gates/run_seam_replay.sh spatial_collision
```

Fixed: seed `0x00000007`, corpus below, `DeterministicExecutor` schedule order. Three runs per corpus schedule; SHA-256 of `Trace.snapshot`. Statistics at this layer: byte-identity of traces, World/Revision isolation of snapshot hashes, mapped `FaultPoint` error ids. Precision/tolerance, mesh vertex hashes, collision-shape hashes, throughput, and peak memory of a Native kernel remain architecture-owner fields and were **not** invented. Seam source unchanged by this run — `spatial_collision.rs` still hashes to `594396f6a747c2812a622cef2f201ae680dd14c603f158ab7d04a1bbfc0b018f`, as recorded in §1.

**Executed corpus** (all ops `schema_id = "voxel-query"`):

| id | observe |
| --- | --- |
| `candidate-set` | candidate projection payloads; three-run snapshot identity |
| `occlusion-miss` | neighbor `Pending`; payload is not empty-world |
| `cache-key-with-world-revision` | same coordinates, different World or Revision → different snapshot |
| `cancel-before-complete` | cancel payload without completion; PrePublication does not insert |

**Executed fault mapping** (research names → shipped `FaultPoint`; no new fault enum):

| research fault | FaultPoint | stable error id | recoverable |
| --- | --- | --- | --- |
| `cross-world-cache-hit` | `CorruptSnapshot` | `EvidenceDigestMismatch` | false |
| `missing-neighbor-chunk` | `LostResult` | `EvidenceMissing` | false |
| `cancel-after-visible` | `PostPublication` | `PartialLoadRolledBack` | false |

**Not executed (gap, not a fabricated score):**

- Reference vs NativeCore spatial/mesh/collision differential (no published NativeCore kernel artifact).
- Throughput, cache hit-rate time series, peak memory of a real kernel.
- Numeric precision / tolerance / LOD.

## 5. Measurements

`approval_status() = "blocked"`. `measurements_executed() = true` for the harness layer. `nativecore_kernel_artifact_hash() = None`. `public_spatial_schema_id() = None`.

Raw traces: seam tests linked and executed by the host's own toolchain (no cross-toolchain, no `link.exe` workaround). 9 tests passed on each leg. Snapshot values below are `Trace.snapshot` hex from `DeterministicExecutor::run` (R-00047 `sha256` of committed seq + schema_id + payload). Three repeats matched, on both legs.

| corpus | run1 = run2 = run3 snapshot SHA-256 |
| --- | --- |
| `candidate-set` | `296b33f3adbcf028f004a5a94fb9961a6a8af0f73fb2b863788a2f0db3e2376a` |
| `occlusion-miss` | `86d55f683889d603a1bff2f68259354388da582931cd568915b36217ea999497` |
| `cache-key-with-world-revision` | `174fdc3bc79e0fd19968759d57de631ebe38ee9739eadc3cb4af1de26b689038` |
| `cancel-before-complete` | `8eb5310720d73f9469955ffa5dfeb0b378a3fcbe56a92c3141bda365998f3161` |

Cache-key identity isolation (single-op schedules, same `coord=0,0,0`):

| identity | snapshot SHA-256 |
| --- | --- |
| world `alpha`, revision `10` | `ea41902c01796f11f10059531c58ce2968d6c83ca1fe9357ae3c84c3ed3d4d41` |
| world `alpha`, revision `11` | `49948e154d040b6b5ea33a1fc2bc859350b3455a04b900183c1a3f31aa93c6fa` |
| world `beta`, revision `10` | `d5cec5edc074939cdf0375590a9a7b9ed0f5376f2d61deb3db3255546f605ae8` |

The three identity hashes differ. Coordinate-only keys are therefore insufficient on this seam — a cache key must carry World and Revision, which is the one substantive constraint this gate contributes to the open `cacheKey` field.

Every snapshot value in the two tables above **replaces** the corresponding value in the previous revision (`6c72b0bf…`, `3835381c…`, `b19b5f60…`, `7db85d9a…`, `1578f473…`, `1d321688…`, `2a607263…`). The seam source is unchanged; the digest function beneath it was corrected. VOX-D-006 §8.1 records the root cause and the reproduction that confirms the old values were genuine rather than invented, and the ordering conclusion above holds under both the old and the new digest.

Fault repeats (three runs, byte-identical outcomes):

| fault | point | error | recoverable |
| --- | --- | --- | --- |
| `cross-world-cache-hit` | `CorruptSnapshot` | `EvidenceDigestMismatch` | false |
| `missing-neighbor-chunk` | `LostResult` | `EvidenceMissing` | false |
| `cancel-after-visible` | `PostPublication` | `PartialLoadRolledBack` | false |

`cancel_before_complete_does_not_insert=true` (`FaultPoint::PrePublication` / `InvalidHandle`, recoverable, committed snapshot unchanged).

No kernel is selected. `unaudited-oss-kernel` remains held out on license/process grounds. NativeCore adapter is **not** selected and its artifact hash is **not** recorded as a default.

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
- `approvalStatus`: **blocked** — unchanged by this run. Executing the seam is not self-approval; `selected_default_candidate()` still returns `None`.
- Blocked reason, restated against current fact: the measurement precondition is now **satisfied** for the harness layer (§4 host legs, §5 numbers). What remains is an architecture-owner decision, plus one gap that measurement on this repository cannot close at all: the Reference-vs-NativeCore differential needs a **published NativeCore kernel artifact**, which does not exist. That is a dependency gap, not an unrun benchmark — running more of this seam will not produce it.
- Who must decide: architecture owner (kernel vendor/version/license, cache-key extras, invalidation, precision/LOD).
- NativeCore artifact hash must be verifiable **and** owner-approved before `nativecore-spatial-adapter` can leave pending. This card does not freeze any such hash.

**Blocked downstream (later cards whose live 执行前置 lists this gate):**

- R-00163 `[程序·Spatial] 实现 Revision-scoped Voxel Candidate 与 Occlusion Source`
- R-00193 `[程序·Mesh] 实现 Revision-scoped Voxel Mesh Source 构建`
- R-00194 `[程序·Collision] 实现 Revision-scoped Voxel Collision Source 构建`

Transitively: R-00166 (lists R-00163, not this gate).

**Continuable without this approval:** this evidence file, the measurement seam, and the optional `data/vox-d-007/` corpus; typed Source ports that refuse to cache and refuse to select an unapproved kernel.

## 8. Commands actually run

Measured 2026-08-29 on macOS (Darwin 25.5.0), Apple Silicon, at commit `13d515f`. The Windows transcript cited by the previous revision (`C:\Users\g923\AppData\Local\Temp\…\agent-R-00063.log`) is not in this repository and is superseded by the reproducible runner below. `rust-toolchain.toml` was not modified; the second leg goes through `rustup run <toolchain>` inside the runner.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test -p lumio-voxel-domain` | 0 | gatekeeping: a real linked test binary runs on this host. `cargo check` was not accepted as a substitute. |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/spatial_collision.rs` | 0 | clean; seam source untouched by this run |
| `benchmarks/decision_gates/run_seam_replay.sh spatial_collision` | 0 | `9 passed; 0 failed`; raw hashes in §5 |
| same runner, `SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin` | 0 | `9 passed; 0 failed`; diffs clean against the x86_64 leg |
| `node .spec/tools/spec-lint.mjs && node --test .spec/tools/spec-lint.test.mjs` | 0 | repository gate passes on this host; the symlink failure the previous revision hit was a Windows placeholder artefact |

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added.

Replay is the runner shown in §4; it resolves the hashed rlib filenames from cargo's JSON output rather than requiring a hand-typed `--extern` path.
