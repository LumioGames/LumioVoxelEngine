# VOX-D-001 Chunk 数值 Profile — Decision Evidence

- Card: R-00057 (`01a04390-c54a-72f3-aafe-625e6732fdf2`) / GATE-001
- Gate: `VOX-D-001`
- Role: Voxel 性能与架构决策工程师
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Voxel worktree HEAD at record: `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (detached; `feat(R-00047): add deterministic harness, faults and fixture runner`)
- Recorded: 2026-08-28
- Produces: `DecisionEvidenceVOXD001`; `ChunkProfileProposal`
- Exclusive files: this document; `benchmarks/decision_gates/chunk_profile.rs`
- `approvalStatus=approved`
- Architecture owner approval: **`LGE-V1.4-VOX-D-P0-2026-08-28`** (Architecture `5f06822`, [VOX-D-P0-OWNER-CONFIRMATION.md](../../../../LumioGameEngineArchitecture/docs/architecture/VOX-D-P0-OWNER-CONFIRMATION.md))
- Selected internal family: `IsolatedCubicExtentFamily` (adapter-internal; **no public Schema extent**)
- Owner-stated precondition (「macOS 环境首次真实跑通测试矩阵」): **partially met** — the matrix was linked
  and executed on `aarch64-apple-darwin` for the first time on 2026-08-28 (153 passed / 5 failed). The 5
  failures share one upstream root cause unrelated to this gate: a wrong SHA-256 round constant in the
  generated contract runtime (see [`../b0-verification.md`](../b0-verification.md) §4). This gate's own
  seam is still not a workspace member and `measure()` remains unexecuted (§4).

This is a research gate plus owner confirmation. It still does **not** freeze a public Chunk extent, world bound, page size, or overflow policy into generated config. `approval_status()` returns `"approved"` citing the owner confirmation. Concrete dimensions remain adapter-internal.

**Explicit:** no Schema, ID Registry, ABI, generated Artifact, or default config was modified by this card.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Architecture baseline | `LGE-V1.4-2026-08-27` |
| Voxel HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Implementation blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| V1.3 `DECISION_GATES.md` SHA-256 | `4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2` |
| V1.3 `MANIFEST.sha256` file SHA-256 | `8c94d6b2680e331007dfab6961ef094a9745faee2084993f5ac0498f7161d3e6` |
| Seam source SHA-256 (`chunk_profile.rs`) | `90c65e89b00030acc9da76282171bf2a36186f814e701744e4943f7759cb1601` |
| Toolchain (this host) | `rust-toolchain.toml` channel `1.98.0`; `rustc 1.98.0 (88d9e12ae 2026-08-18)`; `cargo 1.98.0 (797e8a9bc 2026-08-05)` |
| Prerequisite R-00034 | Consumable (`in_review` with evidence). Blueprint + ADR 0007 present. |
| Prerequisite R-00047 | **Consumable.** Commit `b2f0d8a`, status `in_review`. Ships `DeterministicExecutor`, `Schedule`, `Trace`, `VoxelPortHarness`, `GeneratedVoxelOperation`, `FaultPoint` / `FaultInjector`, `fixture_runner`. |
| Prerequisite of R-00047: R-00045 | Consumable at `c938868` (`feat(R-00045): consume V1.4 generated contract artifacts`). `SCHEMA_IDS` members used by the seam: `voxel-query`, `voxel-chunk-page`, `voxel-revision-stamp`, `voxel-mutation-receipt`. |

Historical unapproved source only (not authority, not a selected default): [`docs/LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md`](../../LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md) lists `VOX-D-001` as `unapproved` with rule “Expose configuration/measurement seam only; do not select a default or algorithm.”

## 2. Candidate list (names / kinds only)

No candidate is selected. The first row is not a conclusion. Families are measurement labels only.

| Name | Kind | Version | License | Source Hash | exclude-reason |
| --- | --- | --- | --- | --- | --- |
| IsolatedCubicExtentFamily | chunk-profile (extent independent of page) | unversioned-internal | N/A (policy family, not a crate) | not-computed | pending-executed-repeat-and-architecture-owner-approval |
| CoupledPageAxisExtentFamily | chunk-profile (page coupled to per-axis extent) | unversioned-internal | N/A (policy family, not a crate) | not-computed | pending-executed-repeat-and-architecture-owner-approval |

Selected default: **none**.

## 3. Frozen contract vs internal candidate vs public value

| Class | What | Status |
| --- | --- | --- |
| Frozen contract (V1.4 schema exists) | Architecture source ADR-024; `schemas/voxel-chunk-page.schema.json`; `common.schema.json#/$defs/voxelChunkId` (canonical `c:x:y:z` key); four-state presence `Ready` / `NotLoaded` / `Pending` / `Unavailable`; Port surface names in `modules/chunk/README.md`. Wire format and identity exist. Consumers still must not depend on a concrete extent. Generated `SCHEMA_IDS` / `STABLE_ERROR_IDS` consumed by the harness, not rewritten here. | Frozen by architecture source. This card did not copy Schema fields. |
| Internal candidate | The two profile families in §2. Measurement-only names for the harness matrix. Not compiled into `lumio-voxel-domain` / Port / `VoxelConfigSnapshot`. Occupancy and coordinates in the seam are payload bytes + `seq` only (no `const CHUNK=16`, no page-size / world-bound / overflow-policy constant). | Internal. Not production policy. |
| Public value awaiting architecture-owner approval | Chunk dimension, world bound, page size, coordinate overflow strategy, and any numeric default that would enter generated config. After approval, public config must be **generated by the architecture repository**, not handwritten here. | Unapproved. |

## 4. Measurement driver (wired) — three repeats **not executed**

R-00047 is now consumable. `benchmarks/decision_gates/chunk_profile.rs` is still **not** a workspace member. `pub fn measure()` builds a `Schedule` and drives the **shipped** types:

- `lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace}`
- `lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness}`
- `lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint}`
- `lumio_voxel_contracts::SCHEMA_IDS` (lookup by generated member; never a handwritten schema id)

### 4.1 Seed / corpus / schedule (encoded, not a production default)

- Seed (documented, measurement-only): `MEASURE_SEED = 0x0000_D001_0000_0057` (VOX-D-001 / R-00057).
- Corpus families: `sparse`, `dense`, `boundary-coords`, `negative-coords`, `extreme-coords`, `cold-read`, `hot-read`, `bulk-edit`.
- Encoding: `payload = kind NUL x_le32 y_le32 z_le32 occupancy`; `seq` is schedule order. Coordinate integers are corpus labels (`i32::MIN` / `i32::MAX` appear only as extreme-coordinate **samples**), not world bounds.
- Schema ids: `voxel-chunk-page` (sparse/dense/fault probe), `voxel-query` (coord + cold/hot read), `voxel-mutation-receipt` + `voxel-revision-stamp` (bulk-edit).
- Planned statistical method: `DeterministicExecutor::run(&schedule)` three times; compare `Trace` (`seed`, `outcomes`, `snapshot: [u8; 32]`) for byte identity (`PartialEq`). Do not report p50/p95/p99 without executed traces.

`measure()` contains that three-repeat comparison. This host cannot **execute** it: `cargo test` / any bin needs `link.exe`.

**未执行：cargo test/bin 缺 link.exe.** No raw traces were written. No snapshot SHA-256 is invented. Exclusive data dir `benchmarks/decision_gates/data/vox-d-001/` was **not** created (no executed bytes to store).

### 4.2 Negative / fault matrix (encoded against the shipped injector)

`VoxelPortHarness::arm` mapping. Error id and recoverable flag are taken from `FaultInjector::error_id` / `FaultInjector::recoverable` (not a second error table). Visible-write points stay unrecoverable in the shipped injector.

| Scenario | `FaultPoint` | Injector error id | recoverable | visible-write |
| --- | --- | --- | --- | --- |
| illegal-dimension | `PrePublication` | `InvalidHandle` | true | no |
| extreme-coordinate | `StaleCompletion` | `StaleEpoch` | true | no |
| memory-pressure | `LostResult` | `EvidenceMissing` | false | yes |
| memory-pressure | `PostPublication` | `PartialLoadRolledBack` | false | yes |
| cross-profile-misread | `CorruptSnapshot` | `EvidenceDigestMismatch` | false | yes |

Runtime outcomes of `arm` + `execute` were **not** observed on this host (same linker gap). The mapping is what `measure()` will record when a host can link.

### 4.3 Commands actually run

| # | Command | Exit | Key output |
| --- | --- | --- | --- |
| 1 | `cargo test -p lumio-voxel-test-support --all-features` | **101** | See verbatim block below. Expected: no MSVC `link.exe`. |
| 2 | `cargo build -p lumio-voxel-test-support --lib --all-features` | **0** | Finished dev profile [unoptimized + debuginfo] target(s) in 0.01s (rlib; no linker). |
| 3 | `rustc --edition 2024 --crate-type rlib --crate-name vox_d_001_seam` (full command below) | **0** | empty stdout/stderr; wrote `vox-d-001.rlib` (171664 bytes). `measure` / `approval_status` / `VOX-D-001` / `DeterministicExecutor` / `voxel-chunk-page` present in the rlib. |
| 4 | Execute `measure()` three repeats / write trace hashes | **未执行：cargo test/bin 缺 link.exe** | No invented hashes. |

Verbatim `cargo test` (this host):

```text
   Compiling lumio-voxel-test-support v0.0.0 (C:\Users\g923\.grok\worktrees\lumiogames-lumiovoxelengine\subagent-01a0443e-2787-72b1-bd2f-ece0ecaa9412\crates\lumio-voxel-test-support)
error: linker `link.exe` not found
  |
  = note: program not found

note: the msvc targets depend on the msvc linker but `link.exe` was not found

note: please ensure that Visual Studio 2017 or later, or Build Tools for Visual Studio were installed with the Visual C++ option

note: VS Code is a different product, and is not sufficient

error: could not compile `lumio-voxel-test-support` (example "check-generated-clean") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `lumio-voxel-test-support` (example "check-crate-dag") due to 1 previous error
error: could not compile `lumio-voxel-test-support` (lib test) due to 1 previous error
error: could not compile `lumio-voxel-test-support` (test "crate_dag") due to 1 previous error
error: could not compile `lumio-voxel-test-support` (test "generated_clean") due to 1 previous error
error: could not compile `lumio-voxel-test-support` (test "harness") due to 1 previous error
```

Full rustc seam command (exit 0; log: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00057.log`):

```text
rustc --edition 2024 --crate-type rlib --crate-name vox_d_001_seam -L target/debug/deps --extern lumio_voxel_test_support=C:\Users\g923\.grok\worktrees\lumiogames-lumiovoxelengine\subagent-01a0443e-2787-72b1-bd2f-ece0ecaa9412\target\debug\deps\liblumio_voxel_test_support-6da53ff03bc58986.rlib --extern lumio_voxel_contracts=C:\Users\g923\.grok\worktrees\lumiogames-lumiovoxelengine\subagent-01a0443e-2787-72b1-bd2f-ece0ecaa9412\target\debug\deps\liblumio_voxel_contracts-a62c2cca441f7fde.rlib benchmarks/decision_gates/chunk_profile.rs -o C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\seam-out\vox-d-001.rlib
```

Compile-check of `measure()` is the evidence that the driver is wired to shipped types. Do not treat that as an executed determinism matrix.

## 5. Approval

- `approvalStatus=approved`
- Architecture owner approval: **recorded**
- Approval reference: `LGE-V1.4-VOX-D-P0-2026-08-28` / Architecture commit `5f06822`
- Selected internal family: `IsolatedCubicExtentFamily`
- This card does not invent Schema numbers. Public extent remains ungenerated.
- Seam: `approval_status() -> "approved"`; `selected_family() -> "IsolatedCubicExtentFamily"`.

`ChunkProfileProposal` (no selected candidate, no limits):

```json
{
  "name": "ChunkProfileProposal",
  "candidate": "IsolatedCubicExtentFamily",
  "measurements": {
    "status": "driver-compiled-not-executed",
    "exit": "未执行：cargo test/bin 缺 link.exe",
    "seed": "0x0000_D001_0000_0057",
    "repeatsPlanned": 3,
    "traceHashes": null,
    "reason": "R-00047 VoxelPortHarness is consumable at b2f0d8a; measure() compiled against shipped types; this host cannot link cargo test/bin"
  },
  "limits": null,
  "sourceHashes": {
    "architectureBaselineId": "LGE-V1.4-2026-08-27",
    "voxelHead": "b2f0d8a3763a02f805e29cbd101560ba7fdca77b",
    "architectureMirrorSha256": "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0",
    "v13DecisionGatesSha256": "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2",
    "blueprintSha256": "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa",
    "seamSha256": "90c65e89b00030acc9da76282171bf2a36186f814e701744e4943f7759cb1601"
  },
  "approvalStatus": "approved",
  "approvalReference": "LGE-V1.4-VOX-D-P0-2026-08-28"
}
```

## 6. Blocked requirements and continuable scope

Owner confirmation is recorded. Public Schema extent is still ungenerated. Three-repeat matrix remains unexecuted on this host (`link.exe` missing). Downstream:

- R-00066 `[程序·配置] 建立不可变 Voxel 配置快照` — consumes `DecisionEvidenceVOXD001` `approvalStatus` and source hashes; must not fill defaults for a blocked gate.
- R-00073 `[程序·Chunk] 实现不可变 Payload、四态 Slot 与 Directory Root` — depends on R-00057 / R-00058 / R-00066.
- Downstream of those (R-00076, R-00078, query/mutation/world cards that assume a config snapshot with approved extent).

May continue (does **not** need this freeze):

- Nothing that requires a frozen Chunk numeric profile.
- Independent work already outside this gate: crate DAG (R-00041), generated artifacts (R-00045), harness (R-00047), sibling VOX-D-005–008 evidence cards that do not consume this profile.

Stop thresholds (qualitative; no numeric SLA frozen): non-deterministic encode; overflow wrap that aliases another chunk; Port consumers compiling a concrete extent; cross-profile page misread accepted as valid; visible-write faults reported as recoverable.

## 7. Contract

No Schema / ID / default config was modified. Root `Cargo.toml` / `Cargo.lock` / crate manifests were not edited; the seam is not a workspace member.

## 8. 2026-08-29 修复后复测 — retest on the corrected SHA-256

Retested 2026-08-29 on macOS (Darwin 25.5.0, Apple Silicon) at commit `cc868e4`, after `51c2836`
(`fix(contracts): re-mirror generated artifacts with corrected SHA-256 K[28]`) replaced the wrong round
constant `K[28] = 0xc6eabbdc` (FIPS 180-4: `0xc6e00bf3`) in the generated contract runtime. Root-cause
forensics: [`VOX-D-006-streaming.md`](VOX-D-006-streaming.md) §8.1 and [`../b0-verification.md`](../b0-verification.md) §4.
Question answered here: did any number recorded above come from the defective digest? **No.**
§1–§7 are the 2026-08-28 record and stay unchanged; `approvalStatus=approved` is untouched — the
`LGE-V1.4-VOX-D-P0-2026-08-28` signature is the architecture owner's, not this session's to re-issue.

### 8.1 Every §1 hash reproduces with system `shasum`(not polluted)

Each SHA-256 recorded in §1 was recomputed with `/usr/bin/shasum -a 256` (FIPS 180-4) over
`git show 54b488f:<path>` — `54b488f` is the commit that carries this document's recorded revision
(identical bytes at the recorded worktree HEAD `b2f0d8a` for these files). The architecture mirror,
blueprint, V1.3 `DECISION_GATES.md`, V1.3 `MANIFEST.sha256`, seam `chunk_profile.rs`
(`90c65e89b00030acc9da76282171bf2a36186f814e701744e4943f7759cb1601`), and toolchain values all match
bit-for-bit. The defective implementation returns a wrong digest for **every** input, so a match with
`shasum` proves these values never went through it.

`chunk_profile.rs` at current HEAD hashes to
`afa26ff839ca77c2a6e33dace39fd8c48da1296d347c5023ccb9eeb7f9a10f60`, ≠ §1 — explained drift, not
tampering: `31cb6a2` recorded the D-013 owner freeze (`approval_status()` `"blocked"` → `"approved"`,
plus new `approval_reference()` / `selected_family()`); the seed/corpus/fault logic is byte-unchanged
(`git diff 54b488f HEAD -- benchmarks/decision_gates/chunk_profile.rs`).

### 8.2 `measure()` executed for the first time(§4 recorded 未执行)

§4/§4.3 recorded that no host could link a binary; `traceHashes` was `null` and stayed honest. The gate
seam is now executed as a process through `benchmarks/decision_gates/run_seam_replay.sh chunk_profile_replay`.
The committed driver `benchmarks/decision_gates/chunk_profile_replay.rs` (added in `cc868e4`) only adds
`--test` entry points via `#[path]`; the seam file itself stays byte-untouched, so the §1 seam hash above
remains the drift check. Two legs, plus a full-runner process re-run, all line-identical:

| Leg | Toolchain | Result |
| --- | --- | --- |
| x86_64-apple-darwin (Rosetta) | pinned `1.98.0` → `rustc 1.98.0 (88d9e12ae 2026-08-18)` | 3 passed / 0 failed |
| aarch64-apple-darwin (native) | `SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin` | 3 passed / 0 failed; output line-identical to x86_64 leg |

First executed values (`MEASURE_SEED = 0x0000_D001_0000_0057`, 10 ops, three
`DeterministicExecutor::run` repeats):

```text
VOX-D-001 traces_byte_identical true
VOX-D-001 snapshot 089e8bb0d24f64f48071989735314c4ab5d11e3d9cde16355e14891624165bc7
VOX-D-001 negative illegal-dimension point=PrePublication error=InvalidHandle recoverable=true visible_write=false matches_injector=true
VOX-D-001 negative extreme-coordinate point=StaleCompletion error=StaleEpoch recoverable=true visible_write=false matches_injector=true
VOX-D-001 negative memory-pressure point=LostResult error=EvidenceMissing recoverable=false visible_write=true matches_injector=true
VOX-D-001 negative memory-pressure point=PostPublication error=PartialLoadRolledBack recoverable=false visible_write=true matches_injector=true
VOX-D-001 negative cross-profile-misread point=CorruptSnapshot error=EvidenceDigestMismatch recoverable=false visible_write=true matches_injector=true
```

The executed negative matrix equals the §4.2 planned table row-for-row; visible-write faults stay
unrecoverable. The snapshot above is the first executed one for this gate — §4 recorded none, so there
is no old/new comparison to make; nothing in the original record is superseded by it.

### 8.3 Counterfactual under the defective digest

The same replay rebuilt and run in a temporary worktree at `54b488f` (pre-fix, defective `K[28]` still
in the mirror) yields snapshot `4d5e27970cc8d81ce0988c370725142a70904a900b338658027eda71d8608bac` —
what the record would have contained had the 2026-08-28 session invented an "executed" number through
the then-defective runtime. It recorded `null` instead. The worktree was removed after the run.

### 8.4 Verdict

**未被污染。** No number in this document came from the defective SHA-256: all static hashes reproduce
under `shasum`, and no executed trace hash was ever recorded (`traceHashes: null` was accurate). This
section adds the first executed dual-architecture replay; `approvalStatus` and §1–§7 are unchanged.
