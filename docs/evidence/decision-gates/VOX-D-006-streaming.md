# VOX-D-006 Streaming scheduling and backpressure profile

- Card: R-00062 / GATE-006
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28 (re-measure after R-00047)
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/streaming_backpressure.rs`; optional `benchmarks/decision_gates/data/vox-d-006/`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam on the shipped R-00047 harness. It does not freeze numeric defaults, pick a default algorithm, edit Schema/ID/default config, or implement production streaming code. Eviction scoring and streaming budgets stay unfrozen. ADR-036 `voxel-durability-ack` fence and residency shapes stay frozen.

Produces: `DecisionEvidenceVOXD006`; `StreamingProfileProposal{priority,concurrency,capacity,backpressure,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (detached; includes R-00047 `feat(R-00047): add deterministic harness, faults and fixture runner`) |
| Architecture HEAD | `3d5e29db72b70c88fb61e392832afe2a762b25cb` (`main`; card lock was `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550`) |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| 仓内 ADR 0005 SHA-256 | `a6c5a172f718cc43e870a27d0e199a74d1bd19551d95b316475496e9d34510cb` |
| Architecture `DECISIONS_PENDING.md` SHA-256 | `8cbcda49fb47b8951eb37f08e41cf4bccf51dff10a423743a04463b60cccbea3` (D-014 scoring still unfrozen; file hash moved vs the earlier `65d839c5…` snapshot because of the 2026-08-28 CanonicalSerializer checksum-domain confirmation, not a VOX-D-006 freeze) |
| Architecture ADR-036 SHA-256 | `d8dec44c6ccd1e69fc4358a850a229310fc56100589be3559e3e9f62f0358d07` (`.spec/decisions/ADR-036-voxel-streaming-durability-ack.md`) |
| `schemas/voxel-durability-ack.schema.json` SHA-256 | `518b0ba9dba75157644e0824d023b96e34464720d0663dddf30c106b455da279` (hashed; fields not copied) |
| Seam source SHA-256 | `153bb37b2c2c6f024f336cd23eee50ca09481d51946ccb797a9801246fe57ac3` (`benchmarks/decision_gates/streaming_backpressure.rs`) |
| Corpus JSON SHA-256 | `2c3ce508ee360fd5617f495f2f86112ef73033212dddbe2d0c0d20a4bdf633f8` |
| Fault-map JSON SHA-256 | `57b4445c8fadd61711d855e6cd4ea4cc55a2d65a11d2ace42208fa7bf41cf15a` |
| Toolchain (declared) | `rust-toolchain.toml` channel `1.98.0`; host `rustc 1.98.0 (88d9e12ae 2026-08-18)` msvc; `cargo 1.98.0` |
| Prerequisite R-00034 | Consumable. Worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline` (`8c49fba`). |
| Prerequisite R-00047 | **Met.** Commit `b2f0d8a3763a02f805e29cbd101560ba7fdca77b`. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `7e7bf9b24a8e31e55130bcd5c6336f748b2d456bdd4c8b20b3643082099ed742` exports `deterministic_executor`, `reference_harness`, `fault_injection`, `fixture_runner`. No substitute harness was invented. |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen):**

- Queue **structure** (owners, producers, consumers, order, full-load *kind*, reliability, Origin Token, apply phase) in 仓内 ADR 0005. Capacity numbers are not frozen.
- Missing-chunk availability is `Ready` / `NotLoaded` / `Pending` / `Unavailable`.
- ADR-036: `DurabilityAck` coverage shape, residency modes (`AllResident` / `DurableEviction` / `VolatileAllowed`), dirty-eviction fence. Dirty Unload requires coverage or an explicit volatile capability. Query must not implicit-load.
- Restore and Streaming Apply are mutually exclusive (仓内 ADR 0004).
- Generated schema id `voxel-durability-ack` and stable error `DirtyChunkNotDurable` (consumed, not re-specified).

**Open on this gate (architecture D-014 / VOX-D-006):**

- Priority scoring and eviction-candidate scoring / hysteresis.
- Concurrency of fetch/decode workers.
- Queue capacity.
- Backpressure and cancel thresholds.

Scoring/hysteresis that stays implementation-level does not change the baseline. New fence/protection shapes or new public residency modes require a new ADR, fixtures, and BaselineId.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion. The durability-ack fence replay does **not** rank these families.

| id | priority | concurrency | capacity | backpressure | version | license | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fifo-deadline-reject` | FIFO within deadline bands (weights unfrozen) | single worker class; count unfrozen | bounded; size unfrozen | reject new demand at full (`QueueFull`) | unversioned | Apache-2.0 (in-tree) | ADR 0005 `a6c5a172…d34510cb` | Stop if full-load silently drops tickets or Query starts loads. Not excluded: scoring unmeasured. |
| `demand-heat-hysteresis` | demand heat + deadline; dirty chunks scored only behind ADR-036 fence | separate fetch vs decode classes; counts unfrozen | bounded; size unfrozen | hysteresis watermarks (values unfrozen) cancel lowest-priority | unversioned | Apache-2.0 (in-tree) | D-014 `8cbcda49…0cccbea3` | Stop if scoring evicts unprotected Dirty, or hysteresis is written as a production constant before approval. Not excluded: scoring unmeasured. |
| `lru-resident-cancel-lowest` | LRU among unload-eligible Ready chunks; load priority still deadline-based | shared pool; count unfrozen | bounded; size unfrozen | cancel-lowest then reject | unversioned | Apache-2.0 (in-tree) | ADR-036 fence `d8dec44c…f0358d07` | Stop if LRU considers Dirty without fence witness, or stale tickets overwrite newer slots. Not excluded: scoring unmeasured. |

No retry/timeout numeric policy is proposed. Those remain unfrozen and must not be invented in streaming workers.

## 4. Measurement plan

Fixed: machine = this worktree host; toolchain = workspace `rust-toolchain.toml` `1.98.0` msvc for rlib compile; GNU `stable-x86_64-pc-windows-gnu` / `rustc 1.98.0` only to link the seam `--test` binary (toolchain file not modified). Seed `0x0000_D006` (53254). Schema id `voxel-durability-ack`. Three runs per input; SHA-256 of raw traces must match.

**Executed corpus** (fixture-id tokens, not schema field copies; architecture names in `fixtures/index.json`):

| id | architecture fixture id |
| --- | --- |
| `snapshot-point-ack` | `voxeldur/ack-snapshot-point` |
| `wal-point-ack` | `voxeldur/ack-wal-point` |
| `dirty-covered-evict` | `voxeldur/fence-dirty-covered-evict` |
| `dirty-uncovered-deny` | `voxeldur/fence-dirty-uncovered-deny` |
| `residency-all-resident` | `voxeldur/residency-all-resident` |

**Executed faults** mapped onto shipped `FaultPoint` values. After a visible write these points are unrecoverable (`FaultInjector::recoverable == false`):

| gate fault | shipped `FaultPoint` | stable error id | recoverable |
| --- | --- | --- | --- |
| `all-resident-evict` | `PostPublication` | `PartialLoadRolledBack` | false |
| `uncovered-evict` | `LostResult` | `EvidenceMissing` | false |
| `missing-durability-point` | `CorruptSnapshot` | `EvidenceDigestMismatch` | false |

**Not executed** (need production streaming coordinator; not invented here): burst demand, cold/hot Chunk, slow I/O, cancel; queue-full, expired ticket, wrong World, restore mutex. No p50/p95/p99 load/unload latency and no queue-watermark time series.

**Replay commands:**

```text
cargo test -p lumio-voxel-test-support --all-features
cargo build --lib
rustc --edition 2024 --crate-type rlib --crate-name vox_d_006_seam -L target/debug/deps --extern lumio_voxel_test_support=<rlib> --extern lumio_voxel_contracts=<rlib> benchmarks/decision_gates/streaming_backpressure.rs -o <seam-out>/vox-d-006.rlib
```

Host msvc cannot link test binaries (`link.exe` missing). Three-run execution used GNU rustc `--test` against GNU `cargo build --lib` rlibs; see §8.

## 5. Measurements

R-00047 is met. The seam drives `DeterministicExecutor::run`, `VoxelPortHarness::execute` / `arm`, and shipped `FaultPoint` values. `approval_status()` remains `"blocked"`. No scoring candidate is excluded. No numeric priority weight, concurrency, capacity, or backpressure threshold is written into production or this proposal.

Correctness / determinism (seed `0x0000_D006`, five `voxel-durability-ack` ops, three independent `DeterministicExecutor::run` calls):

| axis | result |
| --- | --- |
| run-to-run equality | three traces byte-identical (`Trace` + `snapshot_hash`) |
| `VoxelPortHarness::snapshot_hash` SHA-256 | `ce0637be74fd3e4d170ee1b307f759336d3f1ee04578d93e277f28790d3426e8` (same on all three runs) |
| seam `trace_digest` SHA-256 | `3adf3722ee3e8ba816daa0c2e6e5981263c2810078feef036f7bd5191e0d399a` (same on all three runs) |
| corpus outcomes | five ok outcomes; no error on the happy path |

Fault matrix (visible write `seq=0`, then armed mapped `FaultPoint` on `seq=1`):

| gate fault | visible write | faulted outcome | recoverable |
| --- | --- | --- | --- |
| `all-resident-evict` | error `None` | `PartialLoadRolledBack` | false |
| `uncovered-evict` | error `None` | `EvidenceMissing` | false |
| `missing-durability-point` | error `None` | `EvidenceDigestMismatch` | false |

GNU `--test` result: `4 passed; 0 failed` (`gate_remains_blocked`, `corpus_uses_generated_durability_ack_schema`, `three_runs_are_byte_identical`, `mapped_faults_are_unrecoverable_after_visible_write`).

These hashes are from that run. They are not production streaming throughput numbers.

## 6. Proposal (not approved)

```text
StreamingProfileProposal {
  priority: pending-architecture-owner,
  concurrency: pending-architecture-owner,
  capacity: pending-architecture-owner,
  backpressure: pending-architecture-owner,
  approvalStatus: blocked
}
```

Approved public configuration must be generated by the architecture repository.

## 7. Architecture owner approval

- Record: **none**
- `approvalStatus`: **blocked**
- Who must decide: architecture owner, confirming D-014 / VOX-D-006 scoring, concurrency, capacity, backpressure.
- What must be decided: those four public config fields; they must land via generated config snapshot, not handwritten Port constants.
- What is **not** being decided here: ADR-036 fence/residency shapes (already frozen).

**Blocked downstream (later cards whose live 执行前置 lists this gate):**

- R-00151 `[程序·Streaming] 实现显式 Demand、Ticket Coordinator 与 Source Port`

Transitively (not in this gate's own 执行前置, but blocked until R-00151 can consume an approved profile): R-00153, R-00155.

**Continuable without this approval:** this evidence file and the measurement seam; protocol work that keeps queues bounded *without* hard-coding the unfrozen numbers (must fail closed / upgrade rather than invent defaults); any code that consumes only frozen ADR-036 ack/fence/residency shapes.

## 8. Commands actually run

Full transcript: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00062.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/streaming_backpressure.rs` | 0 | after one rustfmt apply |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 | expected: msvc `linker link.exe not found`; lib compiled, tests not linked |
| `cargo build --lib` | 0 | `Finished dev profile [unoptimized + debuginfo]`; rlibs in `target/debug/deps` (`liblumio_voxel_test_support-6da53ff03bc58986.rlib`, `liblumio_voxel_contracts-a62c2cca441f7fde.rlib`) |
| `rustc --edition 2024 --crate-type rlib --crate-name vox_d_006_seam -L target/debug/deps --extern lumio_voxel_test_support=<rlib> --extern lumio_voxel_contracts=<rlib> benchmarks/decision_gates/streaming_backpressure.rs -o …/seam-out/vox-d-006.rlib` | 0 | wrote `vox-d-006.rlib` (170060 bytes); SHA-256 `3dd5657e747e3cb98b1b7e11d96bdf724a5b89163723a2e1a40e5f4c5e782b20` |
| `cargo +stable-x86_64-pc-windows-gnu build --lib --target-dir target-gnu` | 0 | extra; GNU rlibs for `--test` link only; `target-gnu/` removed after the run |
| `rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test --crate-name vox_d_006_seam … -o …/seam-out/vox-d-006-test.exe` | 0 | extra; GNU used only because host msvc has no `link.exe`; `rust-toolchain.toml` not modified |
| `vox-d-006-test.exe --nocapture` | 0 | `4 passed; 0 failed`; snapshot/digest hashes in §5 |

Host `rust-toolchain.toml` stays `1.98.0` msvc.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added; no increment is required for this evidence path. No commit.
