# Executable Task Cards

Baseline: `LGE-V1.3-2026-08-27`. Cards are ordered by dependency Wave. A Lane is serial; different lanes may run in parallel only when dependencies are satisfied. Each owned file appears on one card only.

## Wave summary

| Wave | Goal | Exit gate |
|---|---|---|
| W0 | Resolve authority/crate map and install architecture guards | exact seven-crate map; generated inputs protected |
| W1 | Revision/chunk/query/mutation preparation primitives | immutable cuts, four states, pure prepare |
| W2 | Commit, World lane/barrier/fault containment | sole publication path proven |
| W3 | Capture, persistence, restore shadow, streaming workers | all I/O remains off barrier |
| W4 | Restore/stream apply, Runtime orchestration, LocalEmbedded | race exclusion and two-tree isolation |
| W5 | Contract adapters and complete fixture evidence | unchanged architecture fixtures pass |

## VOX-T-0001 — Resolve frozen crate aliases and generated inputs

- **Wave / Lane:** `W0` / `GOV`
- **Package:** `integration`
- **Dependencies:** none
- **Decision gates:** none
- **Outcome:** A reviewed evidence record maps all seven physical crates and generated contract inputs to authoritative sources.

**Exclusive file ownership**

- `docs/evidence/crate-map-resolution.md`
- `docs/evidence/generated-input-inventory.md`

**Implementation steps**

1. Record exact ADR/module README source lines.
2. Verify L0–L5 edges and generated file locations.
3. Fail closed on any unresolved alias; do not create replacement crates.

**Required tests / fixtures**

- architecture/source inventory check

**Acceptance**

- Seven exact crate names and owners recorded.
- No Cargo/source file created.
- Baseline and source hashes recorded.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0002 — Install architecture dependency and generated-file guards

- **Wave / Lane:** `W0` / `GOV`
- **Package:** `integration`
- **Dependencies:** `VOX-T-0001`
- **Decision gates:** none
- **Outcome:** Automated checks reject forbidden dependency edges and handwritten generated-contract edits.

**Exclusive file ownership**

- `tools/architecture/check_dependency_direction.py`
- `tools/architecture/check_generated_clean.py`
- `tests/architecture/dependency_direction.rs`

**Implementation steps**

1. Encode only frozen layer rules.
2. Hash/compare generated outputs against architecture source.
3. Wire checks into existing spec validation entrypoint.

**Required tests / fixtures**

- negative fixtures for each forbidden edge
- generated-file mutation fixture

**Acceptance**

- Each forbidden edge fails with source reference.
- No public schema is reproduced by the checker.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0101 — Implement revision allocator and immutable read view

- **Wave / Lane:** `W1` / `CORE-A`
- **Package:** `revision`
- **Dependencies:** `VOX-T-0002`
- **Decision gates:** none
- **Outcome:** Monotonic internal revisions and stable pinned views exist without publication yet.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/revision/mod.rs`
- `<CRATE_DOMAIN>/src/revision/clock.rs`
- `<CRATE_DOMAIN>/src/revision/read_view.rs`
- `<CRATE_DOMAIN>/tests/revision_monotonicity.rs`

**Implementation steps**

1. Import generated revision/ID types.
2. Implement reservation/abandon/finalize lifecycle.
3. Implement immutable view pin lifetime.

**Required tests / fixtures**

- monotonic property test
- abandoned reservation test
- long-lived view test

**Acceptance**

- No revision reuse.
- No public contract duplicate.
- No hard-coded pin budget.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0102 — Implement single atomic published-state root

- **Wave / Lane:** `W1` / `CORE-A`
- **Package:** `revision`
- **Dependencies:** `VOX-T-0101`
- **Decision gates:** none
- **Outcome:** A sealed token can atomically replace a complete immutable world cut.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/revision/published.rs`
- `<CRATE_DOMAIN>/src/revision/publication.rs`
- `<CRATE_DOMAIN>/tests/publication_atomicity.rs`

**Implementation steps**

1. Define immutable aggregate roots.
2. Make token construction crate-private.
3. Audit post-swap path for infallibility.

**Required tests / fixtures**

- old-or-new root race
- stale token rejection
- fault injection before swap

**Acceptance**

- Exactly one visible publication primitive.
- Readers never observe mixed roots.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0201 — Implement chunk payload, slot, and directory roots

- **Wave / Lane:** `W1` / `CORE-A`
- **Package:** `chunk`
- **Dependencies:** `VOX-T-0102`
- **Decision gates:** none
- **Outcome:** Immutable payloads and explicit Ready/NotLoaded/Pending/Unavailable slots are representable and validated.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/chunk/mod.rs`
- `<CRATE_DOMAIN>/src/chunk/payload.rs`
- `<CRATE_DOMAIN>/src/chunk/slot.rs`
- `<CRATE_DOMAIN>/src/chunk/directory.rs`
- `<CRATE_DOMAIN>/tests/chunk_state_machine.rs`

**Implementation steps**

1. Import generated ChunkId/status adapters.
2. Implement exhaustive legal-transition validation.
3. Implement immutable directory builder.

**Required tests / fixtures**

- state transition table
- absence-does-not-collapse-state
- old-view COW test

**Acceptance**

- No Option/bool boundary erases four states.
- No I/O dependency.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0202 — Implement staged deltas and dirty frontier

- **Wave / Lane:** `W1` / `CORE-A`
- **Package:** `chunk`
- **Dependencies:** `VOX-T-0201`
- **Decision gates:** none
- **Outcome:** Private chunk edits freeze into replacement roots and revision-aware dirty records.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/chunk/delta.rs`
- `<CRATE_DOMAIN>/src/chunk/dirty.rs`
- `<CRATE_DOMAIN>/tests/chunk_cow_isolation.rs`
- `<CRATE_DOMAIN>/tests/dirty_ack_coverage.rs`

**Implementation steps**

1. Build copy-on-write delta/freeze path.
2. Record dirty revision without clearing API.
3. Implement pure ack-coverage calculation; World remains apply authority.

**Required tests / fixtures**

- COW isolation
- old ack preserves new dirty
- failed builder leaves published root

**Acceptance**

- Chunk package exposes no direct publish or dirty-clear operation.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0301 — Implement deterministic query planner

- **Wave / Lane:** `W1` / `READ`
- **Package:** `query`
- **Dependencies:** `VOX-T-0201`
- **Decision gates:** `VOX-D-001`, `VOX-D-002`
- **Outcome:** Generated requests become canonical immutable plans against one captured config/view.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/query/mod.rs`
- `<CRATE_DOMAIN>/src/query/plan.rs`
- `<CRATE_DOMAIN>/src/query/budget.rs`
- `<CRATE_DOMAIN>/tests/query_determinism.rs`

**Implementation steps**

1. Validate through generated adapters.
2. Canonicalize chunk order.
3. Consume config snapshot without selecting values.

**Required tests / fixtures**

- hash-seed determinism
- config snapshot stability
- invalid request no side effect

**Acceptance**

- Planner performs no I/O or world mutation.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0302 — Implement immutable query execution and result mapping

- **Wave / Lane:** `W1` / `READ`
- **Package:** `query`
- **Dependencies:** `VOX-T-0301`, `VOX-T-0201`
- **Decision gates:** none
- **Outcome:** Queries return deterministic source-compatible outcomes while preserving four chunk states.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/query/execute.rs`
- `<CRATE_DOMAIN>/src/query/chunk_access.rs`
- `<CRATE_DOMAIN>/src/query/result_assembly.rs`
- `<CRATE_DOMAIN>/tests/query_cut_consistency.rs`
- `<CRATE_DOMAIN>/tests/query_missing_states.rs`

**Implementation steps**

1. Resolve every access from one ReadView.
2. Keep missing-state evidence through assembly.
3. Map errors/status only through generated table.

**Required tests / fixtures**

- concurrent commit cut consistency
- four-state response matrix
- cancellation purity

**Acceptance**

- No implicit load trigger.
- No mixed revisions in one result.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0401 — Implement canonical request fingerprint and receipt lookup

- **Wave / Lane:** `W1` / `WRITE`
- **Package:** `mutation`
- **Dependencies:** `VOX-T-0202`
- **Decision gates:** `VOX-D-003`
- **Outcome:** TxnId replay can distinguish equivalent and conflicting canonical requests and preserve original receipts.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/mutation/mod.rs`
- `<CRATE_DOMAIN>/src/mutation/request_fingerprint.rs`
- `<CRATE_DOMAIN>/src/mutation/receipt_ledger.rs`
- `<CRATE_DOMAIN>/tests/mutation_txn_replay.rs`

**Implementation steps**

1. Use generated canonicalization rules.
2. Store immutable original receipt representation.
3. Expose reserve/finalize API without choosing retention policy.

**Required tests / fixtures**

- same Txn replay
- different fingerprint rejection
- receipt identity/equality

**Acceptance**

- Replay does not execute mutation again.
- No eviction default is chosen.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0402 — Implement side-effect-free Prepare

- **Wave / Lane:** `W1` / `WRITE`
- **Package:** `mutation`
- **Dependencies:** `VOX-T-0401`, `VOX-T-0202`
- **Decision gates:** none
- **Outcome:** All fallible validation, allocation, derived-index build, receipt build, and replacement-root staging occur privately.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/mutation/preconditions.rs`
- `<CRATE_DOMAIN>/src/mutation/prepare.rs`
- `<CRATE_DOMAIN>/src/mutation/plan.rs`
- `<CRATE_DOMAIN>/tests/mutation_prepare_purity.rs`

**Implementation steps**

1. Capture one base view.
2. Build immutable replacements off-tree.
3. Pre-reserve ledger/revision/bookkeeping.
4. Seal single-use prepared token.

**Required tests / fixtures**

- fault at every prepare boundary
- state snapshot before/after failures
- batch staging

**Acceptance**

- Lifecycle, roots, ledger, dirty frontier, and stream state are unchanged on failure.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0403 — Implement commit linearization and idempotent replay

- **Wave / Lane:** `W2` / `WRITE`
- **Package:** `mutation`
- **Dependencies:** `VOX-T-0402`, `VOX-T-0102`
- **Decision gates:** none
- **Outcome:** A final recheck followed by one visible publication and infallible finalize produces the original receipt.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/mutation/commit.rs`
- `<CRATE_DOMAIN>/tests/mutation_atomic_batch.rs`
- `<CRATE_DOMAIN>/tests/mutation_fault_escalation.rs`

**Implementation steps**

1. Recheck generation/preconditions/ledger.
2. Publish one aggregate root.
3. Move prebuilt dirty/receipt records.
4. Route impossible invariant to World fault port.

**Required tests / fixtures**

- old-or-new batch visibility
- concurrent duplicate Txn
- post-publication invariant trap

**Acceptance**

- No ordinary fallible operation after publication.
- Only mutation path can request mutation publication.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0501 — Implement World lifecycle and generation-safe command admission

- **Wave / Lane:** `W2` / `WORLD`
- **Package:** `world`
- **Dependencies:** `VOX-T-0402`
- **Decision gates:** none
- **Outcome:** World accepts/rejects typed commands according to frozen lifecycle and instance generation.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/world/mod.rs`
- `<CRATE_DOMAIN>/src/world/state.rs`
- `<CRATE_DOMAIN>/src/world/command.rs`
- `<CRATE_DOMAIN>/src/world/admission.rs`
- `<CRATE_DOMAIN>/tests/world_lifecycle.rs`

**Implementation steps**

1. Represent only source-defined lifecycle mapping.
2. Require internal instance generation.
3. Keep command payloads typed and private.

**Required tests / fixtures**

- model-based lifecycle matrix
- stale instance command
- illegal transition no side effect

**Acceptance**

- No public lifecycle enum duplicate.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0502 — Implement serial write lane and typed short barriers

- **Wave / Lane:** `W2` / `WORLD`
- **Package:** `world`
- **Dependencies:** `VOX-T-0501`, `VOX-T-0403`
- **Decision gates:** none
- **Outcome:** All state-changing commands linearize per World; barrier scopes prohibit I/O and callbacks.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/world/write_lane.rs`
- `<CRATE_DOMAIN>/src/world/barrier.rs`
- `<CRATE_DOMAIN>/tests/world_barrier_order.rs`

**Implementation steps**

1. Implement non-cloneable lease.
2. Define mutation/cut/ack/restore/stream apply scopes.
3. Add trace hooks outside correctness path.

**Required tests / fixtures**

- command order model
- no-I/O barrier instrumentation
- slow reader nonblocking

**Acceptance**

- No process-global write lane.
- Barrier contains bounded work categories, not guessed timing.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0503 — Implement target-World fault containment

- **Wave / Lane:** `W2` / `WORLD`
- **Package:** `world`
- **Dependencies:** `VOX-T-0502`
- **Decision gates:** none
- **Outcome:** Invariant breaches fault only the target World and reject subsequent writes through existing mapping.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/world/fault.rs`
- `<CRATE_DOMAIN>/src/world/events.rs`
- `<CRATE_DOMAIN>/tests/world_fault_isolation.rs`

**Implementation steps**

1. Preallocate fault record path where required.
2. Separate external operation errors from invariants.
3. Emit non-vetoing internal events.

**Required tests / fixtures**

- two-world fault isolation
- event failure cannot change commit
- faulted admission matrix

**Acceptance**

- No new public ErrorCode.
- Other Worlds continue.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0601 — Implement immutable VoxelCaptureRef and capture read port

- **Wave / Lane:** `W3` / `SNAP`
- **Package:** `snapshot`
- **Dependencies:** `VOX-T-0502`
- **Decision gates:** `VOX-D-004`
- **Outcome:** Voxel can pin one coherent published cut for background readers without exposing mutable World.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/snapshot/capture.rs`
- `<CRATE_DOMAIN>/src/snapshot/ref.rs`
- `<CRATE_DOMAIN>/src/snapshot/manifest_view.rs`
- `<CRATE_DOMAIN>/tests/snapshot_ref_lifetime.rs`

**Implementation steps**

1. Bind ref to World generation/revision.
2. Own immutable roots/pins.
3. Expose minimal enumerate/read metadata port.

**Required tests / fixtures**

- ref survives later mutation
- drop releases pins
- close/restore race

**Acceptance**

- VoxelCaptureRef is internal and unforgeable.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0602 — Implement snapshot-cut World command

- **Wave / Lane:** `W3` / `SNAP`
- **Package:** `snapshot`
- **Dependencies:** `VOX-T-0601`
- **Decision gates:** none
- **Outcome:** World captures roots/revision/dirty frontier under a short barrier and returns immediately.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/snapshot/cut.rs`
- `<CRATE_DOMAIN>/tests/snapshot_cut_atomicity.rs`
- `<CRATE_DOMAIN>/tests/snapshot_dirty_frontier.rs`

**Implementation steps**

1. Add BeginSnapshotCut command handler.
2. Allocate required ref material before/inside approved in-memory scope.
3. Release barrier before job handoff.

**Required tests / fixtures**

- cut consistency race
- barrier trace
- failed cut no dirty clear

**Acceptance**

- No encoder or I/O reachable inside barrier.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0701 — Implement generated-manifest adapters and deterministic codec

- **Wave / Lane:** `W3` / `PERSIST`
- **Package:** `persistence`
- **Dependencies:** `VOX-T-0601`, `VOX-T-0002`
- **Decision gates:** none
- **Outcome:** Capture data roundtrips through generated manifest/codec rules without schema duplication.

**Exclusive file ownership**

- `<CRATE_PERSISTENCE>/src/persistence/mod.rs`
- `<CRATE_PERSISTENCE>/src/persistence/encode.rs`
- `<CRATE_PERSISTENCE>/src/persistence/decode.rs`
- `<CRATE_PERSISTENCE>/src/persistence/manifest_adapter.rs`
- `<CRATE_PERSISTENCE>/tests/persistence_roundtrip.rs`

**Implementation steps**

1. Wrap generated builders.
2. Implement deterministic object ordering.
3. Reject incompatibility per generated registry.

**Required tests / fixtures**

- architecture golden fixtures
- unknown version policy fixture
- deterministic encode

**Acceptance**

- No handwritten Manifest field/value.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0702 — Implement durable-store transaction and ack candidate

- **Wave / Lane:** `W3` / `PERSIST`
- **Package:** `persistence`
- **Dependencies:** `VOX-T-0701`, `VOX-T-0602`
- **Decision gates:** `VOX-D-005`
- **Outcome:** Manifest becomes visible only after referenced objects are durable; only then can an ack candidate exist.

**Exclusive file ownership**

- `<CRATE_PERSISTENCE>/src/persistence/store_port.rs`
- `<CRATE_PERSISTENCE>/src/persistence/durability_ack.rs`
- `<CRATE_PERSISTENCE>/tests/persistence_failure_dirty.rs`

**Implementation steps**

1. Define SDK-neutral store port.
2. Implement object/write/manifest commit ordering.
3. Construct source-valid ack candidate after commit.

**Required tests / fixtures**

- crash step matrix
- retry idempotency harness
- store failure leaves dirty

**Acceptance**

- Persistence cannot clear World dirty state.
- No retry/backoff default.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0703 — Implement World durability-ack apply

- **Wave / Lane:** `W3` / `WORLD`
- **Package:** `persistence`
- **Dependencies:** `VOX-T-0702`, `VOX-T-0502`
- **Decision gates:** none
- **Outcome:** A generation/cut-valid ack clears only dirty entries it covers under World barrier.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/world/durability_ack.rs`
- `<CRATE_DOMAIN>/tests/durability_ack_apply.rs`

**Implementation steps**

1. Validate World generation and generated ack evidence.
2. Compute covered set.
3. Apply clear in serial lane; preserve newer dirty revisions.

**Required tests / fixtures**

- old ack vs new mutation
- duplicate ack idempotency
- wrong World rejection

**Acceptance**

- This is the only dirty-clear path.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0710 — Implement restore preflight and shadow builder

- **Wave / Lane:** `W3` / `RESTORE`
- **Package:** `persistence`
- **Dependencies:** `VOX-T-0701`, `VOX-T-0202`
- **Decision gates:** none
- **Outcome:** A complete immutable candidate state is decoded/validated without touching the live World.

**Exclusive file ownership**

- `<CRATE_PERSISTENCE>/src/restore/mod.rs`
- `<CRATE_PERSISTENCE>/src/restore/preflight.rs`
- `<CRATE_PERSISTENCE>/src/restore/shadow_builder.rs`
- `<CRATE_PERSISTENCE>/tests/restore_atomicity.rs`

**Implementation steps**

1. Validate generated compatibility/hashes.
2. Build all chunk/index roots off-tree.
3. Prebuild source-required reset/bookkeeping data.

**Required tests / fixtures**

- corruption matrix
- build failure old-world unchanged
- fixture roundtrip

**Acceptance**

- No live World mutable reference in persistence.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0711 — Implement restore admission and atomic publication

- **Wave / Lane:** `W4` / `RESTORE`
- **Package:** `persistence`
- **Dependencies:** `VOX-T-0710`, `VOX-T-0502`, `VOX-T-0801`
- **Decision gates:** `VOX-D-006`
- **Outcome:** A sealed shadow state publishes once under an exclusive restore barrier, never interleaved with streaming apply.

**Exclusive file ownership**

- `<CRATE_PERSISTENCE>/src/restore/publish_request.rs`
- `<CRATE_DOMAIN>/src/world/restore.rs`
- `<CRATE_PERSISTENCE>/tests/restore_streaming_exclusion.rs`

**Implementation steps**

1. Obtain restore admission token.
2. Close/coordinate streaming tickets per frozen policy.
3. Final validate and atomically publish.
4. Release admission.

**Required tests / fixtures**

- restore vs stream race
- failure before publish
- stale World generation

**Acceptance**

- No partial chunk restore is visible.
- No timeout/retry policy invented.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0801 — Implement demand/ticket coordinator and source port

- **Wave / Lane:** `W3` / `STREAM`
- **Package:** `streaming`
- **Dependencies:** `VOX-T-0502`, `VOX-T-0201`
- **Decision gates:** `VOX-D-007`, `VOX-D-008`
- **Outcome:** Explicit demand creates/deduplicates generation-safe tickets and drives NotLoaded→Pending only through World.

**Exclusive file ownership**

- `<CRATE_STREAMING>/src/streaming/mod.rs`
- `<CRATE_STREAMING>/src/streaming/demand.rs`
- `<CRATE_STREAMING>/src/streaming/ticket.rs`
- `<CRATE_STREAMING>/src/streaming/coordinator.rs`
- `<CRATE_STREAMING>/src/streaming/source_port.rs`
- `<CRATE_STREAMING>/tests/streaming_state_matrix.rs`

**Implementation steps**

1. Define SDK-neutral source port.
2. Bind tickets to World/chunk generation.
3. Capture config per ticket.

**Required tests / fixtures**

- duplicate demand
- four-state transition matrix
- restore admission rejection

**Acceptance**

- No queue/concurrency/retry constants.
- Query does not call coordinator implicitly.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0802 — Implement worker decode and sealed apply result

- **Wave / Lane:** `W3` / `STREAM`
- **Package:** `streaming`
- **Dependencies:** `VOX-T-0801`, `VOX-T-0701`
- **Decision gates:** none
- **Outcome:** Workers fetch/decode candidates outside World and submit ticket-bound results/failures.

**Exclusive file ownership**

- `<CRATE_STREAMING>/src/streaming/decode.rs`
- `<CRATE_STREAMING>/src/streaming/apply.rs`
- `<CRATE_STREAMING>/src/streaming/cancel.rs`
- `<CRATE_STREAMING>/tests/streaming_stale_result.rs`

**Implementation steps**

1. Decode using source-compatible adapter.
2. Never retain mutable World pointer.
3. Make cancellation/stale completion explicit.

**Required tests / fixtures**

- cancel during fetch
- decode corruption
- World recreate stale result

**Acceptance**

- Worker cannot publish roots directly.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0803 — Implement World streaming apply handler

- **Wave / Lane:** `W4` / `WORLD`
- **Package:** `streaming`
- **Dependencies:** `VOX-T-0802`, `VOX-T-0502`
- **Decision gates:** none
- **Outcome:** World revalidates ticket/basis and atomically transitions matching slot; stale results cannot overwrite newer state.

**Exclusive file ownership**

- `<CRATE_DOMAIN>/src/world/streaming_apply.rs`
- `<CRATE_STREAMING>/tests/streaming_restore_exclusion.rs`
- `<CRATE_STREAMING>/tests/streaming_world_isolation.rs`

**Implementation steps**

1. Check lifecycle/restore exclusion.
2. Check ticket, World generation, chunk basis.
3. Publish replacement root or reject stale result.

**Required tests / fixtures**

- mutation vs load race
- restore vs apply race
- two-World isolation

**Acceptance**

- All four states preserved.
- No stale overwrite.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0901 — Implement Runtime registry and generation-safe World handles

- **Wave / Lane:** `W4` / `RUNTIME`
- **Package:** `runtime`
- **Dependencies:** `VOX-T-0503`
- **Decision gates:** none
- **Outcome:** Runtime hosts independent Worlds and rejects stale handles after close/recreate.

**Exclusive file ownership**

- `<CRATE_RUNTIME>/src/runtime/mod.rs`
- `<CRATE_RUNTIME>/src/runtime/registry.rs`
- `<CRATE_RUNTIME>/src/runtime/world_handle.rs`
- `<CRATE_RUNTIME>/src/runtime/operation.rs`
- `<CRATE_RUNTIME>/tests/runtime_world_isolation.rs`

**Implementation steps**

1. Implement registry with internal instance generations.
2. Route commands through endpoints only.
3. Contain operation/fault state per World.

**Required tests / fixtures**

- stale handle
- parallel two-World progress
- fault isolation

**Acceptance**

- No raw mutable World pointer escapes.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0902 — Implement immutable config snapshots and job supervisor

- **Wave / Lane:** `W4` / `RUNTIME`
- **Package:** `runtime`
- **Dependencies:** `VOX-T-0901`
- **Decision gates:** `VOX-D-001`, `VOX-D-002`, `VOX-D-003`, `VOX-D-004`, `VOX-D-005`, `VOX-D-006`, `VOX-D-007`, `VOX-D-008`
- **Outcome:** Operations/tickets capture immutable policy and shutdown safely controls workers.

**Exclusive file ownership**

- `<CRATE_RUNTIME>/src/runtime/config_snapshot.rs`
- `<CRATE_RUNTIME>/src/runtime/jobs.rs`
- `<CRATE_RUNTIME>/src/runtime/shutdown.rs`
- `<CRATE_RUNTIME>/tests/runtime_shutdown.rs`

**Implementation steps**

1. Validate required approved decision values.
2. Create new snapshots on reload.
3. Implement source-defined admission/drain/cancel phases.

**Required tests / fixtures**

- reload in-flight stability
- shutdown with capture/ticket
- worker panic containment

**Acceptance**

- No live mutable global config.
- No timeout default.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0903 — Implement snapshot and streaming orchestrators

- **Wave / Lane:** `W4` / `RUNTIME`
- **Package:** `runtime`
- **Dependencies:** `VOX-T-0902`, `VOX-T-0602`, `VOX-T-0703`, `VOX-T-0803`
- **Decision gates:** none
- **Outcome:** Runtime owns operation state, releases barriers before I/O, and routes generation-safe completions.

**Exclusive file ownership**

- `<CRATE_RUNTIME>/src/runtime/snapshot_orchestrator.rs`
- `<CRATE_RUNTIME>/src/runtime/streaming_orchestrator.rs`
- `<CRATE_RUNTIME>/tests/runtime_snapshot_orchestration.rs`

**Implementation steps**

1. Orchestrate cut→job→ack.
2. Orchestrate demand→ticket→worker→apply.
3. Map outcomes through generated adapters.

**Required tests / fixtures**

- barrier release before I/O
- stale ack/result
- cancel/retry operation

**Acceptance**

- No persistence/streaming worker directly mutates World.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-0904 — Implement LocalEmbedded two-tree composition

- **Wave / Lane:** `W4` / `RUNTIME`
- **Package:** `runtime`
- **Dependencies:** `VOX-T-0903`
- **Decision gates:** none
- **Outcome:** LocalEmbedded creates two physically independent trees connected only by generated-contract-equivalent messages.

**Exclusive file ownership**

- `<CRATE_RUNTIME>/src/runtime/local_embedded.rs`
- `<CRATE_RUNTIME>/tests/local_embedded_no_alias.rs`

**Implementation steps**

1. Instantiate independent registries/Worlds/coordinators.
2. Enforce message serialization/equivalent adapter.
3. Add object-identity graph audit.

**Required tests / fixtures**

- no shared roots/payloads/ledgers/tickets/capture refs
- remote-equivalence trace
- one-tree fault isolation

**Acceptance**

- No direct domain object references cross trees.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-1001 — Implement total generated-contract adapters

- **Wave / Lane:** `W5` / `VERIFY`
- **Package:** `integration`
- **Dependencies:** `VOX-T-0002`, `VOX-T-0901`
- **Decision gates:** none
- **Outcome:** Boundary conversion is total, source-compatible, and cannot bypass Runtime/World authority.

**Exclusive file ownership**

- `<CRATE_FFI>/src/adapter/mod.rs`
- `<CRATE_FFI>/src/adapter/generated_types.rs`
- `<CRATE_FFI>/src/adapter/error_mapping.rs`
- `<CRATE_FFI>/src/adapter/ownership.rs`

**Implementation steps**

1. Wrap generated types directly.
2. Implement exhaustive error/status mapping.
3. Enforce handle/buffer ownership rules from source.

**Required tests / fixtures**

- generated ABI fixtures
- unknown/forward variant fixture
- lifetime misuse negative test

**Acceptance**

- No new public type/field/code/capability.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-1002 — Implement deterministic executor, model oracle, and fault injection

- **Wave / Lane:** `W5` / `VERIFY`
- **Package:** `integration`
- **Dependencies:** `VOX-T-0503`
- **Decision gates:** none
- **Outcome:** Concurrency/failure tests are deterministic and encode only frozen state rules.

**Exclusive file ownership**

- `<CRATE_TESTKIT>/src/deterministic_executor.rs`
- `<CRATE_TESTKIT>/src/model_oracle.rs`
- `<CRATE_TESTKIT>/src/fault_injection.rs`
- `<CRATE_TESTKIT>/src/fixture_runner.rs`

**Implementation steps**

1. Provide controlled task completion.
2. Model lifecycle/chunk/ticket transitions.
3. Expose named pre-publication and I/O fault points.

**Required tests / fixtures**

- self-tests for schedule exploration
- model/implementation differential harness

**Acceptance**

- Production crates do not depend on testkit.
- No recoverable injection after mutation visibility.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.

## VOX-T-1003 — Run end-to-end architecture fixture matrix

- **Wave / Lane:** `W5` / `VERIFY`
- **Package:** `integration`
- **Dependencies:** `VOX-T-0711`, `VOX-T-0803`, `VOX-T-0904`, `VOX-T-1001`, `VOX-T-1002`
- **Decision gates:** none
- **Outcome:** All frozen architecture fixtures and cross-module scenarios pass with evidence tied to exact source revisions.

**Exclusive file ownership**

- `<CRATE_TESTKIT>/src/topology_harness.rs`
- `<CRATE_TESTKIT>/tests/contract_fixtures.rs`
- `<CRATE_TESTKIT>/tests/end_to_end_mutation.rs`
- `<CRATE_TESTKIT>/tests/snapshot_restore.rs`
- `<CRATE_TESTKIT>/tests/streaming_races.rs`
- `<CRATE_TESTKIT>/tests/local_embedded_equivalence.rs`
- `<CRATE_TESTKIT>/tests/failure_domains.rs`

**Implementation steps**

1. Load fixture registry from architecture output.
2. Run remote and LocalEmbedded topologies.
3. Emit evidence with seed/schedule/commit/source hash.

**Required tests / fixtures**

- full fixture matrix
- dependency guard
- generated clean guard

**Acceptance**

- Expected public data is unchanged.
- All failure domains and no-alias properties pass.

**Forbidden changes**

- Public ABI, Manifest, ErrorCode, Capability, ID registry, or architecture fixture definitions.
- ADR 0001–0006, frozen module boundaries, physical crate map, or L0–L5 direction.
- Any numerical default/algorithm still blocked by `VOX-D-001`–`VOX-D-008`.
- Production gameplay/business logic.
