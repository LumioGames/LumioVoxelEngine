# Package 03 — Immutable Query Planning and Execution

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own deterministic read planning and execution over a pinned `ReadView`. Queries are read-only, may span chunks, and must return enough per-chunk status to preserve `Ready` / `NotLoaded` / `Pending` / `Unavailable`. Query code does not trigger I/O, mutate caches with externally visible semantics, advance revisions, acquire the write barrier, or silently substitute empty voxels for missing chunks unless an existing generated contract explicitly requires that behavior.

## 2. Physical placement

- Owning alias: `CRATE_DOMAIN` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - `src/query/mod.rs`.
  - `src/query/plan.rs`.
  - `src/query/execute.rs`.
  - `src/query/chunk_access.rs`.
  - `src/query/result_assembly.rs`.
  - `src/query/budget.rs` — consumes config snapshot; defines no constants.
  - `tests/query_cut_consistency.rs`.
  - `tests/query_missing_states.rs`.
  - `tests/query_determinism.rs`.

## 3. Internal types and ports

```text
QueryContext
  view: ReadView
  config: Arc<RuntimeConfigSnapshot>
  cancellation: CancellationView

QueryPlan
  ordered_chunk_accesses
  result_shape
  imported request identity/limits

ChunkAccessOutcome = Ready(ref) | NotLoaded(meta) | Pending(meta) | Unavailable(meta)
QueryExecutionOutcome
  imported response payload or adapter-ready internal result
  observed_world_revision
  per-chunk status evidence when required by contract
```

The package consumes generated request/response/error definitions through adapters. It does not create competing public DTOs.

## 4. Execution protocol

1. Runtime/world captures one `ReadView` before planning.
2. Planner validates the generated request and derives a canonical chunk order.
3. Executor resolves all accesses against the same directory root.
4. Ready chunks are read immutably; non-ready states are preserved in the internal outcome.
5. Assembler maps the outcome using the generated contract mapping table.
6. The read pin is released when the complete response no longer references the cut.

## 5. Determinism and cancellation

- Canonical iteration order is independent of hash-map seed and worker scheduling.
- Cancellation is observed at predeclared safe points; it never changes shared world state.
- Config budgets are captured once at query admission. A live config reload cannot alter an in-flight result.
- Parallel execution may be introduced only when merge order and floating/integer semantics remain fixture-identical.

## 6. Failure semantics

Malformed requests and budget rejection occur before expensive traversal and map only to existing generated errors. Missing chunk states are domain outcomes, not internal exceptions. Panic/invariant violations are contained by the world/runtime failure domain; query code does not fault unrelated worlds.

## 7. Verification surface

- Same request + same cut produces fixture-identical result across runs and thread schedules.
- A concurrent commit cannot mix old and new chunk versions in one response.
- Four-state matrix is covered at single- and multi-chunk boundaries.
- Cancellation leaves revision, dirty frontier, and streaming coordinator unchanged.
- Contract adapter tests prove no unmapped local error escapes.

## 8. Acceptance criteria

- Every query entry point requires a `ReadView` or `QueryContext`; none reads a mutable world directly.
- Query planning contains no I/O side effect.
- Missing chunks retain source-defined distinctions through response mapping.
- No numerical budget is selected while its decision gate is unapproved.
