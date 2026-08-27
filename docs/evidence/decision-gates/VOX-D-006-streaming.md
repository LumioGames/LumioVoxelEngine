# VOX-D-006 Streaming scheduling and backpressure profile

- Card: R-00062 / GATE-006
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/streaming_backpressure.rs`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam. It does not freeze numeric defaults, pick a default algorithm, edit Schema/ID/default config, or implement production streaming code.

Produces: `DecisionEvidenceVOXD006`; `StreamingProfileProposal{priority,concurrency,capacity,backpressure,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `1175b08808a3fc865f70ebfbfa66c576562864e2` (detached, includes R-00034 `8c49fba` and R-00041) |
| Architecture HEAD | `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550` (`main`, matches card lock) |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture `DECISIONS_PENDING.md` SHA-256 | `65d839c5732825a3392daf76e1b22797d1f97928b328df409ebc544b1191467f` |
| Architecture ADR-036 SHA-256 | `d8dec44c6ccd1e69fc4358a850a229310fc56100589be3559e3e9f62f0358d07` |
| Prerequisite R-00034 | Consumable. Workflow status `in_review` with evidence; worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline`. |
| Prerequisite R-00047 | **Unmet.** Live card is `backlog` / unimplemented. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `d0467f529132ef0b91227af1f8df26a5729e871873a1590b706f7fbbda32069d` exposes only crate-DAG / generated-clean guards. No `VoxelPortHarness`. No substitute harness was invented. |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen):**

- Queue **structure** (owners, producers, consumers, order, full-load *kind*, reliability, Origin Token, apply phase) in 仓内 ADR 0005. Capacity numbers are not frozen.
- Missing-chunk availability is `Ready` / `NotLoaded` / `Pending` / `Unavailable`.
- ADR-036: `DurabilityAck` shape, residency modes (`AllResident` / `DurableEviction` / `VolatileAllowed`), dirty-eviction fence. Dirty Unload requires coverage or an explicit volatile capability. Query must not implicit-load.
- Restore and Streaming Apply are mutually exclusive (仓内 ADR 0004).

**Open on this gate (architecture D-014 / VOX-D-006):**

- Priority scoring and eviction-candidate scoring / hysteresis.
- Concurrency of fetch/decode workers.
- Queue capacity.
- Backpressure and cancel thresholds.

Scoring/hysteresis that stays implementation-level does not change the baseline. New fence/protection shapes or new public residency modes require a new ADR, fixtures, and BaselineId.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion.

| id | priority | concurrency | capacity | backpressure | version | license | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fifo-deadline-reject` | FIFO within deadline bands (weights unfrozen) | single worker class; count unfrozen | bounded; size unfrozen | reject new demand at full (`QueueFull`) | unversioned | Apache-2.0 (in-tree) | ADR 0005 queue matrix | Stop if full-load silently drops tickets or Query starts loads. Not excluded: no measurements. |
| `demand-heat-hysteresis` | demand heat + deadline; dirty chunks scored only behind ADR-036 fence | separate fetch vs decode classes; counts unfrozen | bounded; size unfrozen | hysteresis watermarks (values unfrozen) cancel lowest-priority | unversioned | Apache-2.0 (in-tree) | D-014 `65d839c5…1191467f` | Stop if scoring evicts unprotected Dirty, or hysteresis is written as a production constant before approval. Not excluded: no measurements. |
| `lru-resident-cancel-lowest` | LRU among unload-eligible Ready chunks; load priority still deadline-based | shared pool; count unfrozen | bounded; size unfrozen | cancel-lowest then reject | unversioned | Apache-2.0 (in-tree) | ADR-036 fence `d8dec44c…f0358d07` | Stop if LRU considers Dirty without fence witness, or stale tickets overwrite newer slots. Not excluded: no measurements. |

No retry/timeout numeric policy is proposed. Those remain unfrozen and must not be invented in streaming workers.

## 4. Measurement plan (not executed)

Fixed once R-00047 is consumable: machine, toolchain, seed, corpus, schedule. Three runs per input; SHA-256 of raw traces must match for determinism axes. Statistics: throughput, queue watermark time series, p50/p95/p99 load/unload latency. No summary-only charts.

**Benchmark matrix** (card):

| axis | observe |
| --- | --- |
| burst demand | admit/reject counts; queue watermark |
| cold Chunk | load latency; four-state transitions |
| hot Chunk | merge/dedupe rate; no duplicate tickets |
| slow I/O | watermark; cancel/backpressure outcomes |
| cancel | ticket generation; no late apply |
| Dirty Unload fence | unload denied without Ack; admitted only with coverage |

**Fault matrix** (card):

| fault | required observable |
| --- | --- |
| queue full | `QueueFull`; no silent drop |
| expired ticket | no apply; no stale overwrite |
| wrong World / generation | reject; other World untouched |
| restore mutex | no interleaving with restore publish |
| missing DurabilityAck | Dirty stays resident; no silent unload |

**Replay commands (after R-00047):**

```text
cargo test -p lumio-voxel-test-support --all-features
# VoxelPortHarness + FaultInjector; three runs per (candidate, corpus, schedule)
```

## 5. Measurements

**未执行** because R-00047 is unmet. Correctness, determinism, and fault matrices have no raw results. No candidate is excluded. No numeric priority weight, concurrency, capacity, or backpressure threshold is written into production or this proposal.

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

**Blocked downstream (later cards whose live 执行前置 lists this gate):**

- R-00151 `[程序·Streaming] 实现显式 Demand、Ticket Coordinator 与 Source Port`

Transitively (not in this gate's own 执行前置, but blocked until R-00151 can consume an approved profile): R-00153, R-00155.

**Continuable without this approval:** this evidence file and the measurement seam; protocol work that keeps queues bounded *without* hard-coding the unfrozen numbers (must fail closed / upgrade rather than invent defaults).

## 8. Commands actually run

Full transcript: `tests-R-00062.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/*.rs` | 0 | after one rustfmt apply |
| `rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test benchmarks/decision_gates/streaming_backpressure.rs` | 0 | `tests::gate_remains_blocked` ok (`approval_status() == "blocked"`) |
| `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` (local junctions for `.claude/*` placeholders; not committed) |
| `node --import windows-symlink-junction.mjs --test .spec/tools/spec-lint.test.mjs` | 0 | 13/13 pass |
| `cargo fmt --all -- --check` | 0 | workspace members only; seams not in Cargo.toml |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | msvc check (no link) |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 (msvc: no `link.exe`; gnu: pre-existing live DAG metadata false-positive, not this card) | no `VoxelPortHarness`; measurements 未执行 |

Host `rust-toolchain.toml` stays `1.98.0` msvc. GNU rustc was used only to link seam tests; toolchain file was not modified.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added.
