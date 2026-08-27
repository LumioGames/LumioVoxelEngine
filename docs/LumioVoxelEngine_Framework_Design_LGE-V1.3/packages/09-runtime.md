# Package 09 — Runtime Hosting, World Registry, Jobs, and LocalEmbedded Isolation

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own process-level composition, World registry and handles, operation lifetimes, worker/job scheduling, snapshot-cut orchestration, persistence/streaming adapters, configuration snapshots, shutdown, and failure isolation between Worlds. Runtime does not own voxel mutation semantics, share one World tree between authority/presentation roles, or expose internal object pointers across the public boundary.

## 2. Physical placement

- Owning alias: `CRATE_RUNTIME` (`SOURCE_CRATE_MAP_REQUIRED`).
- Boundary adapter alias: `CRATE_FFI` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - `src/runtime/mod.rs`.
  - `src/runtime/registry.rs`.
  - `src/runtime/world_handle.rs`.
  - `src/runtime/operation.rs`.
  - `src/runtime/jobs.rs`.
  - `src/runtime/config_snapshot.rs`.
  - `src/runtime/snapshot_orchestrator.rs`.
  - `src/runtime/streaming_orchestrator.rs`.
  - `src/runtime/local_embedded.rs`.
  - `src/runtime/shutdown.rs`.
  - `tests/runtime_world_isolation.rs`.
  - `tests/runtime_snapshot_orchestration.rs`.
  - `tests/runtime_shutdown.rs`.
  - `tests/local_embedded_no_alias.rs`.

## 3. Owned state and types

```text
RuntimeHost
  registry: WorldRegistry
  jobs: JobSupervisor
  config_source + immutable current RuntimeConfigSnapshot
  persistence service adapters
  streaming service adapters
  operation registry

WorldHandle
  imported WorldId
  internal instance generation
  weak/owned command endpoint, never raw mutable World pointer

RuntimeOperation
  imported operation identity if defined
  kind/state/cancellation
  captured config snapshot
  diagnostic correlation
```

## 4. World registry

- Registry keys include an internal instance generation so stale handles cannot target a recreated World with the same imported ID.
- World-owned mutable state is not stored in global singletons.
- Registry operations are lifecycle operations, not a bypass around World command admission.
- Faulting one World does not poison registry locks or stop jobs for other Worlds.

## 5. Snapshot orchestration

1. Runtime creates an operation and captures config.
2. It requests a World snapshot cut; World returns `VoxelCaptureRef` under a short barrier.
3. Runtime submits background encode/store work and owns cancellation/progress.
4. On durable success, Runtime submits a `DurabilityAckCandidate` back to the exact World instance.
5. World applies coverage under its barrier; stale/closed instance cannot be accidentally acknowledged.
6. Runtime completes the operation using generated status/error mapping.

Runtime never holds the World barrier while awaiting a worker or store.

## 6. LocalEmbedded topology — two fully isolated trees

LocalEmbedded is a deployment topology, not permission to alias state. Runtime creates two independent World instances/trees for the roles defined by architecture. They must have distinct:

- `WorldCore`, publication roots, revision clocks, chunk payload objects, dirty frontiers, Txn ledgers, streaming coordinators/tickets, capture refs, config snapshots, and failure records;
- registries/instance generations and lifecycle operations;
- ownership graphs demonstrably free of shared mutable or immutable world-state `Arc` identity where the frozen architecture requires physical isolation.

Communication crosses the same generated serialized contract/message boundary used by remote topology, or an adapter proven equivalent by fixtures. Direct method calls that pass domain object references, shared chunk buffers, receipt-ledger entries, or capture refs across trees are forbidden.

## 7. Configuration

Runtime produces immutable `RuntimeConfigSnapshot` objects. Each World/operation/ticket captures the appropriate snapshot once. Approval-gated values (`VOX-D-001`–`VOX-D-008`) have no compiled fallback; startup/config validation rejects unresolved required values through existing semantics. Config reload creates a new snapshot and never mutates in-flight operations.

## 8. Shutdown and cancellation

Shutdown stops admission, cancels/drains operations according to frozen policy, closes Worlds independently, waits for capture refs/tickets as prescribed, and releases drivers only after dependents. No timeout is selected in this design. A cancellation token is advisory outside barrier and converted into explicit commands at safe points.

## 9. Verification surface

- World A fault/slow I/O does not block World B queries or commits.
- Stale handle after close/recreate cannot issue a command to the new instance.
- Snapshot barrier is released before job starts I/O.
- LocalEmbedded object-identity audit proves no cross-tree root/payload/ledger/ticket sharing.
- Remote vs LocalEmbedded fixture replay yields equivalent generated messages/outcomes.
- Config reload cannot change an in-flight query/load/snapshot policy.

## 10. Acceptance criteria

- Runtime is the only owner of operation and background-job lifecycles.
- World handles are generation-safe command endpoints.
- LocalEmbedded creates two trees, never two views of one tree.
- Every external failure is mapped through existing generated error schema.
