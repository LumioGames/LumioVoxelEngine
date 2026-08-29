# VOX-D-003 Query 批次与预算 Profile — Decision Evidence

- Card: R-00059 (`01a04391-4a87-7cbc-9b45-dbf29d1ad406`) / GATE-003
- Gate: `VOX-D-003`
- Role: Voxel 性能与架构决策工程师
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Voxel worktree HEAD at record: `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (`feat(R-00047): add deterministic harness, faults and fixture runner`)
- Recorded: 2026-08-28
- Produces: `DecisionEvidenceVOXD003`; `QueryBudgetProposal`
- Exclusive files: this document; `benchmarks/decision_gates/query_budget.rs`; `benchmarks/decision_gates/data/vox-d-003/`
- `approvalStatus=approved`
- Architecture owner approval: **`LGE-V1.4-VOX-D-P0-2026-08-28`** (Architecture `5f06822`)
- Selected internal family: `StrictAdmissionBudgetFamily` (full-load action = generated `QueueFull` / `BudgetExceeded`)
- Owner-stated precondition (「macOS 环境首次真实跑通测试矩阵」): **partially met** — the matrix was linked
  and executed on `aarch64-apple-darwin` for the first time on 2026-08-28 (153 passed / 5 failed). The 5
  failures share one upstream root cause unrelated to this gate: a wrong SHA-256 round constant in the
  generated contract runtime (see [`../b0-verification.md`](../b0-verification.md) §4). This gate's own
  seam is still not a workspace member and `measure()` remains unexecuted.

This is a research gate plus owner confirmation. It does **not** freeze a public Query batch size, cost model, or quota Schema column. The four-state presence contract `Ready` / `NotLoaded` / `Pending` / `Unavailable` is already frozen by generated `CHUNK_PRESENCE`; this card does not change it.

**Explicit:** no Schema, ID Registry, ABI, generated Artifact, default config, or `Cargo.toml` / crate member list was modified by this card.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Architecture baseline | `LGE-V1.4-2026-08-27` |
| Voxel HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Implementation blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| V1.3 `DECISION_GATES.md` SHA-256 | `4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2` |
| V1.3 `MANIFEST.sha256` file SHA-256 | `8c94d6b2680e331007dfab6961ef094a9745faee2084993f5ac0498f7161d3e6` |
| Toolchain (declared / observed) | `rust-toolchain.toml` channel `1.98.0`; `rustc 1.98.0 (88d9e12ae 2026-08-18)`; `cargo 1.98.0 (797e8a9bc 2026-08-05)` |
| Prerequisite R-00034 | Consumable. Blueprint + ADR 0007 present; hashes above. |
| Prerequisite R-00047 | **Met** at `b2f0d8a`. `DeterministicExecutor` / `VoxelPortHarness` / `FaultPoint` / `FaultInjector` shipped in `lumio-voxel-test-support`. |
| Seam `query_budget.rs` SHA-256 | `17a8f6fffc6620ca1d5f494de05e149622091b55c0f48f2b2095245c23b3accf` |
| Corpus JSON SHA-256 | `f2f8fcd2d75badcfcad26ad20679f24dcc6fb5f6cc8a734c509971a62bb4f206` |
| Fault-map JSON SHA-256 | `e27673493daa42e6c8d8fe46fd3be1bbed05f8e67a30d1d57d79551e359ed0f2` |
| `deterministic_executor.rs` SHA-256 | `46ae8ad5d6d4d27aa263a5d90918d35d8d606db12ea28aebc1433cd4125eec1e` |
| `reference_harness.rs` SHA-256 | `fcbc9274fe18eb028021b44e78e8e94e2da435e46ac4fdac3dbda3e94737ef1f` |
| `fault_injection.rs` SHA-256 | `b39959ed9723619733c566bcd7b356073c6480671c4aba5b1f72c666a1fd3104` |

Historical unapproved source only (not authority, not a selected default): [`docs/LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md`](../../LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md) lists `VOX-D-003` as `unapproved` with rule “Expose configuration/measurement seam only; do not select a default or algorithm.” Queue capacity numbers remain unfrozen (also referenced from ADR-0005 / VOX-D-006).

## 2. Candidate list (names / kinds only)

No candidate is selected. The first row is not a conclusion. Exclude-reason values stay **pending-measurement** because this host cannot link a process that runs the production query engine (there is no query engine yet) and cannot link `cargo test` (`link.exe` missing).

| Name | Kind | Version | License | Source Hash | exclude-reason |
| --- | --- | --- | --- | --- | --- |
| StrictAdmissionBudgetFamily | query-budget-profile (hard batch + cost admission) | unversioned-internal | N/A (policy family, not a crate) | not-computed | pending-measurement (no numeric freeze) |
| ContinuationFirstBudgetFamily | query-budget-profile (continuation-bound remaining budget) | unversioned-internal | N/A (policy family, not a crate) | not-computed | pending-measurement (no numeric freeze) |
| ExplicitMissingQuotaFamily | query-budget-profile (explicit missing-chunk quota) | unversioned-internal | N/A (policy family, not a crate) | not-computed | pending-measurement (no numeric freeze) |

Selected default: **none**.

## 3. Frozen contract vs internal candidate vs public value

| Class | What | Status |
| --- | --- | --- |
| Frozen contract (V1.4 schema exists) | ADR-024 / `schemas/voxel-query.schema.json`: consistency modes, continuation binding, missing-chunk polymorphism; four-state `CHUNK_PRESENCE` = `Ready` / `NotLoaded` / `Pending` / `Unavailable`; Origin Token fields required when work leaves the Barrier (ADR-0005). Query/Mutation live in `lumio-voxel-ops`. Schema id `voxel-query` is in generated `SCHEMA_IDS`. | Frozen by architecture source. This card did not copy Schema fields and did not change presence states. |
| Internal candidate | The three budget-profile families in §2. Measurement-only names. Not compiled into `query/budget.rs` or `VoxelConfigSnapshot`. | Internal. Not production policy. |
| Public value awaiting architecture-owner approval | Batch limit, cost model weights, cancel granularity, missing-chunk quota, queue numeric capacity, and any generated config default. After approval, public config must be generated by the architecture repository. | Still ungenerated. The gate itself is `approvalStatus=approved` (`LGE-V1.4-VOX-D-P0-2026-08-28`), which selected `StrictAdmissionBudgetFamily` and explicitly kept batch/cost numerics adapter-internal — so no public numeric value follows from it. |

## 4. Measurements

R-00047 is met. The seam now calls shipped `DeterministicExecutor::run`, `VoxelPortHarness::{new,arm,execute,snapshot_hash}`, and `FaultPoint` / `FaultInjector::error_id`. Every op uses `schema_id = "voxel-query"`.

### 4.1 Corpus (harness schedule)

Seed `0x0000D003` (53251). Ops, in vec order (never HashMap iteration):

| seq | payload | notes |
| --- | --- | --- |
| 0 | `bound-query` | bound query |
| 1 | `continuation` | continuation of the bound request |
| 2 | `target-revision-unavailable` | named corpus case; harness success path (no new ErrorCode) |
| 3–6 | `Ready`, `NotLoaded`, `Pending`, `Unavailable` | labels taken from generated `CHUNK_PRESENCE` |

Three repeats of the same `Schedule` are encoded in `replay_corpus_three_times()` → `DeterministicExecutor::run` three times, then snapshot bytes compared.

`VoxelPortHarness::snapshot_hash` is SHA-256 over each committed op as `seq.to_le_bytes() || schema_id.as_bytes() || payload`. This host cannot link a binary (`link.exe` missing), so `DeterministicExecutor::run` was **not executed as a process**. The snapshot digest below is the SHA-256 of that same committed-op encoding, computed with Node `crypto.createHash('sha256')` (FIPS 180-4). It is not a production query-engine trace and is not an invented hex string.

| Run | Snapshot SHA-256 |
| --- | --- |
| 1 | `bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029` |
| 2 | `bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029` |
| 3 | `bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029` |

Committed encoding size: 215 bytes. Hashes identical across the three listed runs because the encoding is a pure function of the fixed schedule (the same property `DeterministicExecutor::run` has when linked: fresh `VoxelPortHarness`, vec order).

Throughput, tail latency, budget-accuracy, and result-size numbers: **not measured**. There is no production query planner (`R-00080` not in this card). No imaginary latency/throughput values.

### 4.2 Fault matrix (mapped onto shipped FaultPoints)

Do not invent ErrorCodes. Outcomes must be `STABLE_ERROR_IDS` values produced by `FaultInjector::error_id` on shipped `FaultPoint`s:

| Scenario | Mapped `FaultPoint` | `FaultInjector::error_id` (in `STABLE_ERROR_IDS`) | recoverable |
| --- | --- | --- | --- |
| unbound-continuation | `PrePublication` | `InvalidHandle` | true |
| budget-exceeded | `PostPublication` | `PartialLoadRolledBack` | false |
| stale-revision | `StaleCompletion` | `StaleEpoch` | true |

`BudgetExceeded` / `TargetRevisionUnavailable` exist in `STABLE_ERROR_IDS` but are **not** emitted by the shipped harness. This seam does not mint them. Numeric budget defaults remain unfrozen; mapping budget-exceeded onto `PostPublication` is a harness-level fault injection, not a chosen quota.

`measure_faults()` arms `VoxelPortHarness` and executes one `voxel-query` op per scenario. That function is compiled into the seam rlib; it was not process-executed (no linker).

### 4.3 Commands actually run

| # | Command | Exit |
| --- | --- | --- |
| 1 | `cargo test -p lumio-voxel-test-support --all-features` | **101** — `error: linker link.exe not found`; lib compiled, test binaries not linked |
| 2 | `cargo build -p lumio-voxel-test-support --lib --all-features` | **0** — `Finished dev profile` |
| 3 | `rustc --edition 2024 --crate-type rlib --crate-name vox_d_003_seam -L target/debug/deps --extern lumio_voxel_test_support=target/debug/deps/liblumio_voxel_test_support-6da53ff03bc58986.rlib --extern lumio_voxel_contracts=target/debug/deps/liblumio_voxel_contracts-a62c2cca441f7fde.rlib benchmarks/decision_gates/query_budget.rs -o …/seam-out/vox-d-003.rlib` | **0** — rlib 143378 bytes |

Log: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00059.log`.

No mixed `ChunkRevisionSet` was produced: the harness corpus does not assemble multi-chunk results. Implicit load is out of scope for this reference harness (ops are payload labels).

## 5. Approval

- `approvalStatus=approved`
- Architecture owner approval: **recorded**
- Approval reference: `LGE-V1.4-VOX-D-P0-2026-08-28` / Architecture commit `5f06822`
- Selected internal family: `StrictAdmissionBudgetFamily`
- Public batch/cost Schema columns remain **ungenerated**. Full-load *action* is the generated `QueueFull`/`BudgetExceeded` ids.

`QueryBudgetProposal` (no selected limits):

```json
{
  "name": "QueryBudgetProposal",
  "batchLimit": null,
  "costModel": "StrictAdmissionBudgetFamily",
  "cancelGranularity": null,
  "measurements": {
    "status": "harness-seam-compiled",
    "schemaId": "voxel-query",
    "seed": 53251,
    "repeatRuns": 3,
    "snapshotEncodingSha256": "bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029",
    "snapshotHashesIdentical": true,
    "provenance": "SHA-256 of VoxelPortHarness snapshot encoding; DeterministicExecutor::run not linked (no link.exe)",
    "throughputLatencyBudgetAccuracy": "not-measured",
    "exit": {
      "cargoTestAllFeatures": 101,
      "cargoBuildLibAllFeatures": 0,
      "rustcSeamRlib": 0
    }
  },
  "sourceHashes": {
    "architectureBaselineId": "LGE-V1.4-2026-08-27",
    "voxelHead": "b2f0d8a3763a02f805e29cbd101560ba7fdca77b",
    "architectureMirrorSha256": "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0",
    "v13DecisionGatesSha256": "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2",
    "blueprintSha256": "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa",
    "queryBudgetRsSha256": "17a8f6fffc6620ca1d5f494de05e149622091b55c0f48f2b2095245c23b3accf"
  },
  "approvalStatus": "approved",
  "approvalReference": "LGE-V1.4-VOX-D-P0-2026-08-28"
}
```

## 6. Blocked requirements and continuable scope

Cannot proceed until this freeze is architecture-owner approved **and** linked query-engine measurements exist:

- R-00066 `[程序·配置] 建立不可变 Voxel 配置快照` — consumes `DecisionEvidenceVOXD003`; must not fill a blocked batch/budget default.
- R-00080 `[程序·Query] 实现确定性计划器与预算校验` — depends on R-00059 / R-00068 / R-00078.
- Downstream query execution / world Port cards that admit batches against a config snapshot.

May continue (does **not** need this freeze):

- Work that does not require a frozen Query batch / budget profile.
- Independent work: crate DAG, artifact-gate evidence, sibling VOX-D cards that do not consume this profile.
- Consumption of R-00047 harness by other measurement seams.

Stop thresholds (qualitative; unchanged): implicit load; mixed revision in one result; budget miss that still mutates world; cancel that changes shared state.

## 7. Contract

No Schema / ID / default config was modified. Four-state presence was not changed. Query budget numbers were not frozen.

## 8. 2026-08-29 修复后复测 — retest on the corrected SHA-256

Retested 2026-08-29 on macOS (Darwin 25.5.0, Apple Silicon) at commit `cc868e4`, after `51c2836`
corrected the generated contract runtime's SHA-256 round constant `K[28] = 0xc6eabbdc → 0xc6e00bf3`
(forensics: [`VOX-D-006-streaming.md`](VOX-D-006-streaming.md) §8.1, [`../b0-verification.md`](../b0-verification.md) §4).
This gate is the only one of VOX-D-001..004 whose record contains an executed-style digest — the §4.1
snapshot `bf6aadfa…150029`, computed at record time with Node `crypto` because the host could not link.
Question answered: is that value polluted by the defective digest? **No — it reproduces bit-for-bit on
the fixed runtime.** §1–§7 stay unchanged; `approvalStatus=approved` is untouched.

### 8.1 Every §1 hash reproduces with system `shasum`(not polluted)

Each SHA-256 in §1 was recomputed with `/usr/bin/shasum -a 256` over `git show 54b488f:<path>`
(`54b488f` carries this document's recorded revision). All match bit-for-bit: the document set, the seam
`query_budget.rs` (`17a8f6fffc6620ca1d5f494de05e149622091b55c0f48f2b2095245c23b3accf`), the corpus /
fault-map JSON under `data/vox-d-003/`, and the three harness sources. The three harness sources
(`deterministic_executor.rs`, `reference_harness.rs`, `fault_injection.rs`) also hash **identically at
current HEAD** — the executor and the committed-op snapshot encoding never changed; only the `sha256`
beneath them was fixed.

`query_budget.rs` at current HEAD hashes to
`b350ff300da79b1e1be24289100dc9089f070da0e161878d26c6b6a0e5e6e824`, ≠ §1 — explained drift:
`31cb6a2` recorded the D-013 owner freeze (`"blocked"` → `"approved"` plus `approval_reference()` /
`selected_family()`); seed/corpus/fault logic byte-unchanged.

### 8.2 The recorded snapshot reproduces on the linked, fixed harness

§4.1's digest was a substitute computation (Node `crypto.createHash('sha256')` over the committed-op
encoding) because `DeterministicExecutor::run` could not be linked. Both gaps are now closed at once:
the real harness runs as a process, and its `sha256` is the corrected one. Runner:
`benchmarks/decision_gates/run_seam_replay.sh query_budget_replay`; the committed driver
`query_budget_replay.rs` (added in `cc868e4`) adds only `--test` entry points via `#[path]`, leaves the
seam byte-untouched, and **asserts** the recorded value from here on. Two legs (x86_64-apple-darwin
Rosetta on pinned `1.98.0`; `SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin`), plus a full-runner process
re-run: all line-identical, 3 passed / 0 failed per leg.

```text
VOX-D-003 seed 0x0000d003
VOX-D-003 op_count 7
VOX-D-003 committed_encoding_bytes 215
VOX-D-003 run1 snapshot bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029
VOX-D-003 run2 snapshot bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029
VOX-D-003 run3 snapshot bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029
VOX-D-003 recorded-2026-08-28 snapshot bf6aadfa5375cdaead9ec5dc65a7f2d3ad7b43063775c9b74ab77b38d9150029
VOX-D-003 matches_recorded_value true
VOX-D-003 fault unbound-continuation error=Some("InvalidHandle") recoverable=true stable=true
VOX-D-003 fault budget-exceeded error=Some("PartialLoadRolledBack") recoverable=false stable=true
VOX-D-003 fault stale-revision error=Some("StaleEpoch") recoverable=true stable=true
```

The committed encoding is 215 bytes, exactly as §4.1 recorded. A third, independent implementation
agrees: rebuilding the same 215-byte buffer (`seq_le64 ‖ "voxel-query" ‖ payload` over the seven §4.1
ops) and hashing it with Python `hashlib.sha256` on this host returns the same `bf6aadfa…150029`.
The executed fault matrix equals §4.2 row-for-row. Throughput / latency / budget-accuracy remain
not-measured exactly as §4.1 states (no production query planner in this card).

### 8.3 Counterfactual under the defective digest

The same replay rebuilt and run in a temporary worktree at `54b488f` (pre-fix) yields snapshot
`81ad871a715e686aec7b85071a53c6fa36bf70634f48c7a54f6777b7f9f8ad1a` ≠ the recorded value. Had the
2026-08-28 session routed its substitute digest through the then-defective contract runtime instead of
Node `crypto`, the record would contain that wrong value. It does not — the recorder's claimed
provenance ("FIPS 180-4") is confirmed from both directions.

### 8.4 Verdict

**未被污染,且已由链接执行确认。** The one executed-style number in this record (`bf6aadfa…150029`)
is now triple-sourced: Node `crypto` (2026-08-28 record), the linked harness on the fixed `sha256`
(both architectures), and Python `hashlib` — all identical; the defective runtime provably produces a
different value. `approvalStatus` and §1–§7 unchanged.
