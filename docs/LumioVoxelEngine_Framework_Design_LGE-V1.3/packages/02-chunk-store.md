# Package 02 — Chunk Store and Four-State Presence Model

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own immutable chunk payloads, the chunk directory root, copy-on-write staging, per-chunk revision/dirty metadata, and the exact missing-chunk distinction: `Ready`, `NotLoaded`, `Pending`, and `Unavailable`. This package never performs disk/network I/O, chooses a streaming policy, clears dirty state by itself, commits transactions, or collapses the four states into a boolean/nullable payload.

## 2. Physical placement

- Owning alias: `CRATE_DOMAIN` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - `src/chunk/mod.rs`.
  - `src/chunk/key.rs` — adapters around generated IDs only.
  - `src/chunk/payload.rs` — immutable payload representation.
  - `src/chunk/slot.rs` — four-state slot and legal transitions.
  - `src/chunk/directory.rs` — immutable directory root and builder.
  - `src/chunk/delta.rs` — staged edits.
  - `src/chunk/dirty.rs` — revision-aware dirty frontier.
  - `tests/chunk_state_machine.rs`.
  - `tests/chunk_cow_isolation.rs`.
  - `tests/dirty_ack_coverage.rs`.

## 3. Owned state and types

```text
ChunkDirectoryRoot       // immutable map/index from imported ChunkId to ChunkSlot
ChunkDirectoryBuilder    // unpublished copy-on-write staging
ChunkSlot =
  Ready(ChunkVersionRef)
  NotLoaded(NotLoadedMeta)
  Pending(PendingLoadMeta)
  Unavailable(UnavailableMeta)

ChunkVersion
  imported chunk revision
  immutable payload
  immutable derived-index refs

DirtyFrontier
  mark(chunk_id, committed_revision)
  covered_by(durability_ack) -> clear-set
```

The variant payloads are internal metadata. Public representations and status numeric values come solely from generated contracts.

## 4. Legal state behavior

| Current | Event | Next | Authority |
|---|---|---|---|
| `NotLoaded` | accepted load ticket | `Pending` | world-applied streaming command |
| `Pending` | matching successful load | `Ready` | world barrier |
| `Pending` | matching terminal failure | `Unavailable` | world barrier, frozen policy |
| `Pending` | cancellation/stale generation | source-defined state | world barrier; never direct worker write |
| `Unavailable` | approved retry/reset policy | source-defined state | world barrier |
| `Ready` | eviction decision | `NotLoaded` or source-defined state | world barrier |
| `Ready` | mutation commit | `Ready(new immutable version)` | mutation publication |

No transition is inferred merely from absence of a map entry. Directory construction must materialize or deterministically derive the correct source-defined state.

## 5. Copy-on-write edit path

1. Mutation prepare captures the base `ChunkVersionRef` for each touched chunk.
2. It applies edits to private builders and computes all derived indexes off-tree.
3. It freezes builders into immutable versions.
4. It builds an unpublished `ChunkDirectoryRoot` with all replacements.
5. Mutation commit publishes the complete root through Package 01.

## 6. Dirty semantics

- A successful committed change records the committed revision in `DirtyFrontier`.
- Snapshot capture reads but does not clear the frontier.
- Persistence produces a `DurabilityAck` identifying the covered cut using generated contract fields.
- Only a world-barrier command may apply the ack.
- Applying an ack clears an entry only when the ack covers that exact dirty revision; later edits remain dirty.

## 7. Invariants

- Ready payloads are immutable after publication.
- A load result cannot overwrite a newer generation or a chunk modified after its ticket basis.
- `Unavailable` is not equivalent to `NotLoaded`; `Pending` is not equivalent to either.
- Dirty and presence state are orthogonal.
- Derived indexes and payload revision publish together.

## 8. Verification surface

- Exhaustive transition-table tests, including rejected transitions.
- Stale streaming result race against mutation and eviction.
- Snapshot ack for revision R followed by mutation R+1 preserves dirty.
- Copy-on-write test proves old read views retain old bytes/indexes.
- Property test that directory build failure leaves the published root unchanged.

## 9. Acceptance criteria

- No `Option<ChunkPayload>` or boolean-loaded abstraction can erase four-state semantics at module boundaries.
- All state transitions pass through a world-authorized command.
- Dirty clearing is impossible without a coverage-checked `DurabilityAck` under the world barrier.
