# VOX-D-006 Streaming scheduling and backpressure profile

- Card: R-00062 / GATE-006
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28; re-measured 2026-08-29 on a linking host (see §4)
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/streaming_backpressure.rs`; optional `benchmarks/decision_gates/data/vox-d-006/`
- `approvalStatus`: `approved`
- Architecture owner approval: **`LGE-V1.4-VOX-D-P2-2026-08-29`** (Architecture `origin/main` `997117e`, PR #16, [VOX-D-P2-OWNER-CONFIRMATION.md](../../../../LumioGameEngineArchitecture/docs/architecture/VOX-D-P2-OWNER-CONFIRMATION.md); D-014 Confirmed) — no numeric profile generated, no candidate selected

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
| Seam source SHA-256 | `810bad946afab49c8aa8ee5bbd3387ad4cdcde096f7adef63729659adda02e8a` (`benchmarks/decision_gates/streaming_backpressure.rs`; was `153bb37b…46fe57ac3`, the §4 run's value — this revision applied owner confirmation `LGE-V1.4-VOX-D-P2-2026-08-29`: approval fields and gate test only, corpus and measurement paths untouched) |
| Corpus JSON SHA-256 | `2c3ce508ee360fd5617f495f2f86112ef73033212dddbe2d0c0d20a4bdf633f8` |
| Fault-map JSON SHA-256 | `57b4445c8fadd61711d855e6cd4ea4cc55a2d65a11d2ace42208fa7bf41cf15a` |
| Toolchain (declared) | `rust-toolchain.toml` channel `1.98.0`; `rustc 1.98.0 (88d9e12ae 2026-08-18)`; `cargo 1.98.0`. Unmodified by this run; the msvc host of the previous revision is superseded by the linking hosts in §4. |
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

## 4. Measurement seam — executed on a linking host

**Status: executed.** The previous revision reached its numbers through a Windows GNU cross-toolchain, used only because the msvc host had no `link.exe`. This gate was re-run on a host whose default toolchain links, so no substitute toolchain is involved and `cargo check` is not accepted as evidence.

Run of 2026-08-29, at repository commit `13d515f358ffeb182e9659d5bde4fa119496f711` (`origin/main`):

| leg | host triple | rustc | seam result |
| --- | --- | --- | --- |
| primary | `x86_64-apple-darwin` (Rosetta 2 on an Apple Silicon machine; rustup default host) | `1.98.0 (88d9e12ae 2026-08-18)`, pinned by `rust-toolchain.toml` | 4 passed / 0 failed |
| second | `aarch64-apple-darwin` (native) | `1.98.0 (88d9e12ae 2026-08-18)` | 4 passed / 0 failed; output byte-identical to the primary leg |

Generation commands:

```bash
benchmarks/decision_gates/run_seam_replay.sh streaming_backpressure
SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin SEAM_OUT_DIR=target/decision-gate-seams-aarch64 \
  benchmarks/decision_gates/run_seam_replay.sh streaming_backpressure
```

Fixed: seed `0x0000_D006` (53254). Schema id `voxel-durability-ack`. Three runs per input; SHA-256 of raw traces must match. Seam source was unchanged by this run — `benchmarks/decision_gates/streaming_backpressure.rs` hashed to `153bb37b2c2c6f024f336cd23eee50ca09481d51946ccb797a9801246fe57ac3` at the time of measurement. A later revision applied the P2 owner confirmation to the seam's approval fields and gate test (new hash in §1); the corpus and the §5 measured values are unaffected.

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

Replay is the runner above; it resolves the hashed rlib filenames from cargo's JSON output instead of requiring a hand-typed `--extern` path.

## 5. Measurements

R-00047 is met. The seam drives `DeterministicExecutor::run`, `VoxelPortHarness::execute` / `arm`, and shipped `FaultPoint` values. `approval_status()` is now `"approved"` citing `LGE-V1.4-VOX-D-P2-2026-08-29` (§7). No scoring candidate is excluded. No numeric priority weight, concurrency, capacity, or backpressure threshold is written into production or this proposal.

Correctness / determinism (seed `0x0000_D006`, five `voxel-durability-ack` ops, three independent `DeterministicExecutor::run` calls):

| axis | result |
| --- | --- |
| run-to-run equality | three traces byte-identical (`Trace` + `snapshot_hash`) |
| `VoxelPortHarness::snapshot_hash` SHA-256 | `77733bfcace4c511f405d639966a2e834949c140140da80dd286a305776ca1da` (same on all three runs, and on both host legs) |
| seam `trace_digest` SHA-256 | `9b4da8700be802671497b0aad06baa676b344070db36e375637bcfd6b6bc9277` (same on all three runs, and on both host legs) |
| corpus outcomes | five ok outcomes; no error on the happy path |

These two values **replace** the `ce0637be…` / `3adf3722…` pair carried by the previous revision. The cause is a corrected SHA-256, not a change in streaming behaviour; §8 records the reproduction that establishes this.

Fault matrix (visible write `seq=0`, then armed mapped `FaultPoint` on `seq=1`):

| gate fault | visible write | faulted outcome | recoverable |
| --- | --- | --- | --- |
| `all-resident-evict` | error `None` | `PartialLoadRolledBack` | false |
| `uncovered-evict` | error `None` | `EvidenceMissing` | false |
| `missing-durability-point` | error `None` | `EvidenceDigestMismatch` | false |

Seam `--test` result on both legs: `4 passed; 0 failed` (`gate_remains_blocked`, `corpus_uses_generated_durability_ack_schema`, `three_runs_are_byte_identical`, `mapped_faults_are_unrecoverable_after_visible_write`).

These hashes are from that run. They are **not** production streaming throughput numbers: no priority weight, worker count, queue capacity, or backpressure watermark is measured or implied here, and none of the three §3 candidate families is ranked or excluded by this layer. The axes that would rank them (burst demand, cold/hot chunk, slow I/O, cancel, queue-full, p50/p95/p99 latency, watermark time series) need a production streaming coordinator, which does not exist in this repository; they were not modelled or estimated.

## 6. Proposal (approved; nothing numeric frozen)

```text
StreamingProfileProposal {
  priority: pending-architecture-owner,
  concurrency: pending-architecture-owner,
  capacity: pending-architecture-owner,
  backpressure: pending-architecture-owner,
  approvalStatus: approved,
  approvalReference: LGE-V1.4-VOX-D-P2-2026-08-29
}
```

Public configuration, if ever generated, is produced by the architecture repository.

## 7. Architecture owner approval

- Record: **`LGE-V1.4-VOX-D-P2-2026-08-29`** — Architecture repository `origin/main` `997117e` (PR #16), `docs/architecture/VOX-D-P2-OWNER-CONFIRMATION.md`; `DECISIONS_PENDING.md` D-014 marked Confirmed. Issued on delegated authority (delegation recorded in R-00257 and the session ledger, 2026-08-29).
- `approvalStatus`: **approved** — citing the confirmation id above. Per the confirmation: LGE-V1.4 **does not generate a public Voxel P2 numeric profile, and no strategy candidate is selected**. Approved means the owner decision on the open fields is made as recorded; it does **not** mean any number is frozen. §3 candidate sets stay open — none excluded, none preferred. The four public config fields (scoring, concurrency, capacity, backpressure) remain ungenerated; nothing lands as handwritten Port constants.
- Confirmed binding invariants (family level): ADR-036 durability-ack fence and residency shapes reaffirmed; mapped faults are **unrecoverable after a visible write**; fence replay must be byte-deterministic.
- Deferred, adapter-internal: priority scoring, concurrency, queue capacity, backpressure thresholds, eviction scoring/hysteresis. Unlock condition: a production streaming coordinator exists and burst/latency/watermark axes are measured.
- What was **not** decided here (already frozen before this gate): ADR-036 fence/residency shapes.
- This revision applies the confirmation to this document, the seam's `approval_status()` / `approval_reference()`, and the gate test (now `gate_approved_citing_owner_confirmation`). The §4–§5 measurement content is from the 2026-08-29 run at `13d515f` and is unchanged; §8's forensic record is untouched.

**Formerly blocked downstream (now unblocked by this approval, with the deferred axes still unfrozen):**

- R-00151 `[程序·Streaming] 实现显式 Demand、Ticket Coordinator 与 Source Port` — may proceed against the approved profile shape; must not hardcode the unfrozen numbers (fail closed / upgrade rather than invent defaults).

Transitively: R-00153, R-00155 (were blocked until R-00151 could consume an approved profile).

**Continuable:** protocol work that keeps queues bounded *without* hard-coding the unfrozen numbers; any code that consumes only frozen ADR-036 ack/fence/residency shapes.

## 8. Commands actually run

Measured 2026-08-29 on macOS (Darwin 25.5.0), Apple Silicon, at commit `13d515f`. The Windows transcript cited by the previous revision (`C:\Users\g923\AppData\Local\Temp\…\agent-R-00062.log`) is not in this repository and is superseded by the reproducible runner below.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo test -p lumio-voxel-domain` | 0 | gatekeeping: a real linked test binary runs on this host. `cargo check` was not accepted as a substitute. |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/streaming_backpressure.rs` | 0 | clean; seam source untouched by this run |
| `benchmarks/decision_gates/run_seam_replay.sh streaming_backpressure` | 0 | `4 passed; 0 failed`; hashes in §5 |
| same runner, `SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin` | 0 | `4 passed; 0 failed`; diffs clean against the x86_64 leg |

`rust-toolchain.toml` was not modified; the second leg goes through `rustup run <toolchain>` inside the runner.

### 8.1 Why the recorded hashes moved (root cause, verified)

The previous revision recorded `snapshot_hash = ce0637be…3426e8` and `trace_digest = 3adf3722…1e0d399a`. This run produces `77733bfc…776ca1da` and `9b4da870…b6bc9277`. Three candidate explanations were separated rather than assumed:

1. **Seam drift** — ruled out. At the time of that reproduction `streaming_backpressure.rs` still hashed to `153bb37b…46fe57ac3`, byte-identical to the value §1 recorded alongside the old numbers (the hash later changed only by the P2 approval revision — see §1).
2. **Host-dependent non-determinism** — ruled out. The x86_64 and aarch64 legs produce byte-identical output, and the old Windows-GNU values are reproducible on macOS (below), so the harness is host-independent across three OS/arch combinations.
3. **A corrected SHA-256** — confirmed. The generated `ContractRuntime` mirrored into `lumio-voxel-contracts` carried a wrong SHA-256 round constant, `K[28] = 0xc6eabbdc` where FIPS 180-4 specifies `0xc6e00bf3`. A single wrong constant pollutes every compression round, so that implementation returned a wrong digest for *every* input. It was corrected in `51c2836` (`fix(contracts): re-mirror generated artifacts with corrected SHA-256 K[28]`), which is an ancestor of the measured commit. `VoxelPortHarness::snapshot_hash` and the seam's `trace_digest` both route through that `sha256`, so both values necessarily moved.

Reproduction that pins it down — a worktree at `54b488f` (the commit that recorded the old numbers, which already carries this exact seam source and predates the fix), built and run on **this** macOS host:

| Command | Result |
| --- | --- |
| `git worktree add --detach <scratch> 54b488f` then the same runner | `4 passed; 0 failed` |
| observed `snapshot_hash` | `ce0637be74fd3e4d170ee1b307f759336d3f1ee04578d93e277f28790d3426e8` — exact match to the old record |
| observed `trace_digest` | `3adf3722ee3e8ba816daa0c2e6e5981263c2810078feef036f7bd5191e0d399a` — exact match to the old record |

Conclusion: the previously recorded numbers were **genuine measurements, not invented**, and they are now **superseded** because the digest function beneath them was defective. The temporary worktree was removed after the run.

Two independent checks confirm the current implementation is the correct one, using digests the seam does not compute for itself:

- VOX-D-005's `pin-expired` corpus yields `e3b0c442…7852b855`, the canonical SHA-256 of empty input; `printf '' | shasum -a 256` on this host returns the same value.
- Every `payload.*` line regenerated into `data/vox-d-008/measurements.txt` matches `printf '%s' <literal> | shasum -a 256` on this host.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added; no increment is required for this evidence path.
