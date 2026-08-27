# Package 01 — Revision and Atomic Publication

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own the internal ordering and publication mechanism that makes a voxel world observable as coherent immutable cuts. This package owns revision allocation, publication eligibility, read pinning, and the single atomic swap of a fully staged world state. It does not own chunk payload semantics, mutation validation, I/O, streaming scheduling, snapshot encoding, runtime threads, or any public ABI definition.

The essential invariant is **one visible publication point**. A caller may allocate, validate, and stage before that point and may fail without visible side effects. Once publication occurs, all subsequently visible state and the transaction receipt must already be prebuilt so the remaining commit path is infallible under ordinary execution.

## 2. Physical placement

- Owning alias: `CRATE_DOMAIN` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - `src/revision/mod.rs` — private exports and invariants.
  - `src/revision/clock.rs` — monotonic revision allocator.
  - `src/revision/published.rs` — immutable published-state root and atomic publication cell.
  - `src/revision/read_view.rs` — pinned read views and lifetime accounting.
  - `src/revision/publication.rs` — prevalidated publication token.
  - `tests/revision_monotonicity.rs`.
  - `tests/publication_atomicity.rs`.
  - `tests/read_view_stability.rs`.

No generated contract file belongs in this package.

## 3. Owned state

| State | Owner | Mutation authority | Visibility |
|---|---|---|---|
| Next internal revision | `RevisionClock` | world write lane only | never directly public |
| Current immutable world roots | `PublishedCell` | validated `PublicationToken` only | read views |
| Active read pins | `ReadPinRegistry` | read-view acquire/drop | diagnostics only |
| Publication generation | `PublishedCell` | atomic publication | internal stale-token defense |

## 4. Internal types and ports

```text
RevisionClock
  reserve_next(current_cut) -> ReservedRevision
  finalize(reservation) -> RevisionStamp          // infallible move after validation

PublishedState
  revision: imported WorldRevision
  chunk_root: Arc<ChunkDirectoryRoot>
  auxiliary_root: Arc<WorldAuxiliaryRoot>
  publication_generation: InternalGeneration

PublicationToken             // sealed, non-cloneable, created only after all checks
ReadView                      // immutable Arc roots + imported revision value
PublishedCell
  load_view() -> ReadView
  publish(token) -> PublishedStateRef             // single visible write
```

`WorldAuxiliaryRoot` is an internal aggregate only. It must not be exposed as a new public contract. Its exact members are limited to already-owned world metadata and indexes required to make the cut coherent.

## 5. Algorithms

### Read capture

1. Atomically load the current `PublishedState` reference.
2. Register a read pin against its internal generation.
3. Return an immutable `ReadView`; no lock is held for the query lifetime.
4. On drop, release only the pin accounting. The view remains memory-safe through reference ownership.

### Publication

1. Require the world write lane/barrier lease.
2. Verify the token targets the current publication generation and reserved revision.
3. Confirm every fallible allocation, receipt serialization, index rebuild, and dirty-delta construction completed before entry.
4. Perform one atomic state-root replacement.
5. Finalize preallocated bookkeeping using infallible moves only.
6. Emit internal observability after the state is already coherent; telemetry failure must not affect commit outcome.

## 6. Invariants

- Revisions never move backward and are not reused within a world instance.
- A `ReadView` sees either the complete old cut or complete new cut, never a mixed pair of roots.
- `PublicationToken` cannot be forged outside the mutation/world coordination path.
- Revision reservation alone is not visible and may be abandoned.
- No queue limit, pin budget, retry count, or retention length is hard-coded while its `VOX-D-*` gate is unapproved.

## 7. Failure semantics

Pre-publication stale token, exhausted configured resource, or validation failure returns an internal failure mapped through the existing generated error table by the outer adapter. Unexpected invariant failure at/after publication is escalated to the owning World fault path; it is never translated into a novel public ErrorCode.

## 8. Verification surface

- Property: revisions are strictly monotonic under arbitrary successful/failed prepare sequences.
- Race: readers repeatedly capture while a writer publishes; each observed root pair shares one revision.
- Fault injection before publication: current view remains byte-for-byte equivalent.
- Fault injection after publication hooks: commit outcome remains successful or World becomes Faulted according to the frozen failure contract; no partial state is exposed.
- Long-lived read view: later commits do not mutate its roots.

## 9. Acceptance criteria

- Publication is represented by one auditable primitive.
- The post-publication path contains no allocator, serializer, I/O, callback into user code, or fallible index construction.
- Tests prove old/new-cut atomicity and abandoned reservation invisibility.
- All public revision values are imported from generated contracts.
