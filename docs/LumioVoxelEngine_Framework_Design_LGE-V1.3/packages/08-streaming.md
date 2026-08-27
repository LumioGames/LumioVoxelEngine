# Package 08 — Streaming Coordination and Ticketed Load Apply

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own demand aggregation, load-ticket lifecycle, deduplication, cancellation, fetch/decode worker coordination, and submission of validated results to a World. It preserves the chunk four-state model and prevents stale asynchronous results from overwriting newer state. It does not publish roots directly, perform restore, infer missing chunks as empty, or share mutable state across LocalEmbedded trees.

## 2. Physical placement

- Owning alias: `CRATE_STREAMING` (`SOURCE_CRATE_MAP_REQUIRED`).
- World apply adapter depends inward on `CRATE_DOMAIN` interfaces only.
- Planned files:
  - `src/streaming/mod.rs`.
  - `src/streaming/demand.rs`.
  - `src/streaming/ticket.rs`.
  - `src/streaming/coordinator.rs`.
  - `src/streaming/source_port.rs`.
  - `src/streaming/decode.rs`.
  - `src/streaming/apply.rs`.
  - `src/streaming/cancel.rs`.
  - `tests/streaming_state_matrix.rs`.
  - `tests/streaming_stale_result.rs`.
  - `tests/streaming_restore_exclusion.rs`.
  - `tests/streaming_world_isolation.rs`.

## 3. Internal types and ports

```text
LoadDemand
  world instance generation
  imported ChunkId
  reason/priority through source-defined policy adapters

LoadTicket
  opaque ticket identity
  world instance generation
  chunk generation/basis revision
  cancellation generation
  captured config snapshot

ChunkSource
  fetch(ticket, generated object/location reference) -> bytes/result

DecodedChunkCandidate
  ticket
  immutable chunk version candidate
  validation evidence

StreamingCoordinator
  request(demand) -> ticket or deduplicated observation
  complete(candidate) -> World command
  fail(ticket, internal failure) -> World command
  cancel(scope)
```

Priorities, queue sizes, concurrency, retry counts, and eviction thresholds are decision/config inputs; no constants are set here.

## 4. Demand and ticket protocol

1. Query/game/runtime produces an explicit demand; query execution itself remains side-effect free.
2. Coordinator checks restore admission and existing ticket state.
3. World accepts a transition from `NotLoaded` (or source-approved retry state) to `Pending` under its write lane and returns a ticket basis.
4. Worker fetches and decodes outside the World barrier.
5. Worker submits a sealed candidate/failure carrying the exact ticket and basis.
6. World barrier rechecks World generation, restore exclusion, current slot state, ticket identity, and chunk basis.
7. Matching result publishes a new root with `Ready`; stale results are discarded/recorded according to frozen policy without altering newer state.

## 5. Four-state preservation

- `NotLoaded`: no active accepted ticket.
- `Pending`: one source-defined active load generation; duplicate demand observes/deduplicates rather than creating ambiguous writers.
- `Unavailable`: terminal/temporarily terminal result according to existing policy; not silently retried without approved transition.
- `Ready`: immutable payload at a known revision/generation.

Public status mapping uses generated contracts. The coordinator retains richer internal metadata without changing public schema.

## 6. Restore exclusion and lifecycle

Restore admission prevents new tickets and controls outstanding ones exactly as frozen ADR dictates. Completion from a pre-restore World generation cannot apply to a restored or recreated World. Close/fault cancels work without allowing worker callbacks to retain mutable World references.

## 7. Failure semantics

Network/store/decode failures become ticket results and may transition to `Unavailable` only through World. Cancellation/staleness is not reported as a novel public error. Coordinator panic/failure is contained to its runtime worker and affected operation; shared global state is avoided.

## 8. Verification surface

- Duplicate concurrent demands yield one accepted load generation.
- Mutation/eviction/restore between fetch and apply makes the result stale; no overwrite.
- All four states remain distinguishable in events/query outcomes.
- Retry behavior is blocked/configurable until corresponding decision approval.
- Two LocalEmbedded trees requesting same chunk receive distinct tickets/payload ownership.
- Queue saturation follows captured config and existing error mapping.

## 9. Acceptance criteria

- Worker threads cannot mutate World or `ChunkSlot` directly.
- Every apply crosses a sealed ticket revalidation under World barrier.
- Restore and streaming publication cannot overlap.
- No hidden numerical policy is embedded in coordinator code.
