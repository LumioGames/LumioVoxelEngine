# Package 10 — Composition, Contract Adapters, Fixtures, and Verification

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Provide the composition root and verification surface that proves the seven frozen crates/modules operate together without changing public contracts. It owns adapters to generated contracts, fixture runners, deterministic test clocks/executors, fault-injection ports, topology-equivalence harnesses, architecture dependency checks, and evidence reports. It does not own gameplay, product policy, new ABI fields, production storage credentials, or alternative module boundaries.

## 2. Physical placement

- Contract adapter: `CRATE_FFI` (`SOURCE_CRATE_MAP_REQUIRED`).
- Test support: `CRATE_TESTKIT` (`SOURCE_CRATE_MAP_REQUIRED`).
- Generated input: `CRATE_CONTRACT` (`SOURCE_CRATE_MAP_REQUIRED`), never hand-edited.
- Planned files:
  - boundary: `src/adapter/mod.rs`, `src/adapter/generated_types.rs`, `src/adapter/error_mapping.rs`, `src/adapter/ownership.rs`.
  - testkit: `src/fixture_runner.rs`, `src/deterministic_executor.rs`, `src/fault_injection.rs`, `src/topology_harness.rs`, `src/model_oracle.rs`.
  - integration tests: `tests/contract_fixtures.rs`, `tests/end_to_end_mutation.rs`, `tests/snapshot_restore.rs`, `tests/streaming_races.rs`, `tests/local_embedded_equivalence.rs`, `tests/failure_domains.rs`.
  - architecture checks: `tests/dependency_direction.rs`, `tests/no_generated_edits.rs`.

## 3. Contract adapter rules

- Generated public structs/enums/IDs/ErrorCodes/Manifest/Capabilities are imported, not mirrored.
- Conversion is total and audited: every generated variant is handled; unknown/forward-compatible behavior follows source policy.
- Internal diagnostics never leak as new numeric codes or public fields.
- Memory ownership, handle lifetime, thread affinity, and buffer release follow the existing ABI contract exactly.
- No adapter may call internal publication APIs except through Runtime/World ports.

## 4. Composition graph

```text
Generated Contracts (L0)
        ↓
Revision + Chunk primitives (L1)
        ↓
Query + Mutation (L2)
        ↓
World + Barrier (L3)
        ↓
Snapshot/Persistence + Streaming services (L4)
        ↓
Runtime + boundary composition (L5)
        ↓
Testkit/fixtures depend on all; production crates never depend on testkit
```

The exact seven physical crate names remain those in the frozen crate map; aliases in `00_SOURCE_INVENTORY.md` prevent this design from silently inventing replacements.

## 5. Fixture matrix

| Fixture family | Proves |
|---|---|
| Generated contract/ABI fixtures | byte/field/error/capability compatibility required by source registry |
| Revision/publication model | monotonic cuts and old-or-new visibility |
| Txn replay | original receipt preservation and request-fingerprint conflict behavior |
| Query missing-state | exact Ready/NotLoaded/Pending/Unavailable mapping |
| Snapshot/durability | cut consistency, crash-safe store visibility, ack coverage |
| Restore | corruption rejection and one-shot atomic publication |
| Streaming race | ticket generation, stale result suppression, restore exclusion |
| LocalEmbedded/remote equivalence | two-tree isolation with equivalent messages/outcomes |
| Failure domains | target World fault containment and multi-World progress |

Use fixture IDs and expected bytes only from the architecture source registry. This package names families, not new public fixtures.

## 6. Deterministic harness

`DeterministicExecutor` controls task ordering, cancellation, and injected completions without wall-clock sleeps. `ModelOracle` represents only frozen state-machine rules. `FaultInjector` exposes named pre-publication and store/streaming boundaries; there is no ordinary recoverable injection after the first visible mutation write because the implementation must make that region infallible.

## 7. Architecture enforcement

Automated checks must reject:

- dependency edges from lower layers to Runtime/FFI/testkit;
- production dependencies on concrete storage/network SDKs outside owning adapter crate;
- handwritten copies or edits of generated contract files;
- public exports of internal `WorldCore`, roots, builders, tickets, capture refs, or publication tokens;
- direct chunk/root writes outside mutation/World;
- blocking I/O symbols reachable while a `WorldBarrierLease` is held;
- cross-tree object references in LocalEmbedded composition.

## 8. End-to-end scenarios

1. Create World → query `NotLoaded` → demand/load → `Pending` → apply → `Ready`.
2. Prepare mutation failure → verify zero visible change.
3. Commit batch → query old/new cut atomicity → replay `TxnId` returns original receipt.
4. Snapshot cut → mutate while encoding → durable ack clears only covered dirty revision.
5. Corrupt restore → old cut unchanged; valid restore publishes once; no streaming apply overlaps.
6. LocalEmbedded command replication between two isolated trees → compare against remote-topology fixture.
7. Force invariant failure after publication boundary → only target World Faulted; another World continues.

## 9. Acceptance criteria

- All architecture-source fixtures pass without changing expected public data.
- Dependency and generated-file checks run in CI before implementation tests.
- No test relies on sleep-based race timing when deterministic scheduling can represent it.
- Evidence identifies source fixture/ADR/baseline and exact implementation commit.
