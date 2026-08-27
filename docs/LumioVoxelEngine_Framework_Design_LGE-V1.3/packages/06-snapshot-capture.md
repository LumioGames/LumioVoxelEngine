# Package 06 — Snapshot Cut and VoxelCaptureRef

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Define the voxel-side capture mechanism for a coherent snapshot cut. The Runtime owns the snapshot operation and `SnapshotCut` orchestration. The voxel World only creates and holds an immutable `VoxelCaptureRef` during a short barrier, then background persistence consumes it after the barrier is released. This package does not write files, upload objects, choose codec/version fields, own runtime job scheduling, or clear dirty state.

## 2. Physical placement

- Domain capture side: `CRATE_DOMAIN` (`SOURCE_CRATE_MAP_REQUIRED`).
- Persistence-facing adapter side: `CRATE_PERSISTENCE` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - domain: `src/snapshot/capture.rs`, `src/snapshot/ref.rs`, `src/snapshot/manifest_view.rs`.
  - persistence adapter: `src/capture_input.rs`.
  - tests: `snapshot_cut_atomicity.rs`, `snapshot_ref_lifetime.rs`, `snapshot_dirty_frontier.rs`.

## 3. Ownership and types

```text
SnapshotCutRequest       // generated request or adapter; owned by Runtime
SnapshotCut              // Runtime-owned orchestration record
VoxelCaptureRef          // voxel-owned immutable pin, opaque outside voxel internals
  world identity/generation
  captured revision
  immutable published roots
  captured dirty-frontier view
  source contract/version refs
  internal lifetime token

CaptureReadPort
  enumerate_chunks(capture_ref)
  read_chunk(capture_ref, imported ChunkId)
  capture_metadata(capture_ref)
```

`VoxelCaptureRef` is not a public ABI handle unless the architecture source already defines one. It is an internal capability with explicit lifetime and cannot be constructed from raw IDs.

## 4. Cut protocol

1. Runtime admits a snapshot operation and asks the World for `BeginSnapshotCut`.
2. Under the World barrier, voxel captures the current immutable published roots, revision, contract-version references, and dirty-frontier view.
3. Voxel creates a lifetime-safe `VoxelCaptureRef` with all required allocations already complete.
4. World returns the ref/cut metadata and immediately releases the barrier.
5. Runtime schedules background encode/store work using the persistence port.
6. Completion yields a durability result; only a successful, source-valid `DurabilityAck` is later applied through the World barrier.
7. Dropping the capture ref releases pins; it does not mutate the World.

## 5. Consistency rules

- Every chunk and world metadata item in one capture comes from the same published cut.
- Later mutations create new immutable versions and cannot alter captured bytes.
- Capture may include a full cut or dirty subset only as defined by the existing manifest/decision contract; no implicit mode is selected here.
- A failed encoder/store leaves dirty entries untouched.
- Memory/pin budgets and admission behavior are configuration/decision gated; no hidden default.

## 6. Resource lifetime

The capture ref owns strong references or another proven immutable pin to every root needed by background encoding. It must not depend on a mutable registry entry that can be replaced during restore/close. Runtime cancellation drops the job and ref safely. World close waits/cancels according to frozen lifecycle policy without leaking refs.

## 7. Failure semantics

Failure before the ref is returned has no visible world side effect. Failure after cut creation but before durability is an operation failure only; the World remains dirty. Invalid/corrupt capture internals are invariant failures handled by World/runtime containment and never mapped to a newly invented public code.

## 8. Verification surface

- Concurrent mutation during encode: capture output remains at captured revision.
- Snapshot cut barrier trace contains only in-memory capture work.
- Encoder cancellation/drop releases all pins.
- Failed persistence produces no durability ack and clears no dirty entry.
- Restore/close races obey lifecycle admission and never yield a dangling ref.

## 9. Acceptance criteria

- Runtime, not voxel storage code, owns operation state and job scheduling.
- Voxel exposes only the minimum immutable read port required by persistence.
- Barrier ends before the first encode or I/O call.
- Dirty clearing is absent from this package.
