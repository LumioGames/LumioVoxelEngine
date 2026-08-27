# Package 05 — World Lifecycle, Serial Write Lane, and Barrier

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own each World instance’s lifecycle, command admission, single-writer serialization, short critical barriers, fault containment, and authority to apply mutation publications, snapshot cuts, durability acknowledgements, restore publication, and streaming results. World does not perform blocking I/O, encode snapshots, schedule transport, or redefine generated lifecycle/error fields.

## 2. Physical placement

- Owning alias: `CRATE_DOMAIN` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - `src/world/mod.rs`.
  - `src/world/state.rs`.
  - `src/world/command.rs`.
  - `src/world/write_lane.rs`.
  - `src/world/barrier.rs`.
  - `src/world/admission.rs`.
  - `src/world/fault.rs`.
  - `src/world/events.rs` — internal events only.
  - `tests/world_lifecycle.rs`.
  - `tests/world_barrier_order.rs`.
  - `tests/world_fault_isolation.rs`.

## 3. Owned state

```text
WorldCore
  identity: imported WorldId + internal instance generation
  lifecycle: internal representation mapped to generated contract
  published: PublishedCell
  revision_clock: RevisionClock
  write_lane: WorldWriteLane
  receipt_ledger: TxnReceiptLedger
  dirty_frontier: DirtyFrontier
  stream_admission: StreamAdmissionState
  restore_admission: RestoreAdmissionState
  fault_record: optional InternalFaultRecord
  config: Arc<RuntimeConfigSnapshot>
```

## 4. Command model

Only typed internal commands enter the serial lane:

```text
CommitPreparedMutation
BeginSnapshotCut
ApplyDurabilityAck
BeginRestorePublication / PublishRestoredRoot / AbortRestore
ApplyStreamingTicket / ApplyStreamingResult / ApplyStreamingFailure
Quiesce / Close
```

Commands carry imported IDs/contract objects by reference or generated adapters. No new public command schema is declared here.

## 5. Barrier protocol

A barrier is a **short, in-memory serialization point**, not a global stop-the-world lock and never an I/O scope.

1. Admission verifies lifecycle and mutual-exclusion rules.
2. The command is ordered behind already-admitted writes for that World.
3. The write lane grants a non-cloneable `WorldBarrierLease`.
4. The command reads or atomically replaces internal roots and updates prebuilt bookkeeping.
5. The lease is released before encoding, storage, network, callbacks, or long computation.

Barrier categories are explicit so tests can prove policy:

| Category | Allowed work inside | Forbidden inside |
|---|---|---|
| Mutation publish | final precondition + atomic root publish + infallible bookkeeping | staging, serialization, I/O |
| Snapshot cut | capture immutable roots/frontier + create ref | encoding/upload |
| Durability ack | coverage-check + clear covered dirty entries | store query/retry |
| Restore publish | final shadow-root validation + atomic publish | reading/decoding snapshot |
| Stream apply | ticket/generation check + root state transition | fetch/decompression |

## 6. Restore/streaming exclusion

- Restore admission closes the streaming-apply path for the target World and invalidates or drains tickets according to the frozen ADR.
- A streaming load cannot publish while restore publication is active.
- Restore cannot start while an incompatible streaming apply barrier is active.
- The exact drain/cancel policy must come from the effective ADR/approved decision; the implementation may not choose a timeout or retry value.

## 7. Fault containment

An invariant failure that could make the published cut or post-publication bookkeeping untrustworthy transitions only the owning World to `Faulted`, records a stable internal diagnostic reference, rejects new state-changing commands through existing error mapping, and leaves other Worlds/runtime workers operational. Ordinary I/O failures from persistence or streaming are domain results and do not fault the World unless they expose an internal invariant breach.

## 8. Ordering guarantees

- Commands within one World are linearized by the write lane.
- Different Worlds have no shared write lane and may progress independently.
- Queries pin a cut and do not hold the barrier.
- Config is captured at World creation or command admission according to frozen policy; live mutable globals are forbidden.
- Internal events are sequenced from committed state and cannot veto a commit.

## 9. Verification surface

- Model-based lifecycle tests for every legal/illegal command/state pair.
- Barrier trace proves no I/O or user callback occurs while leased.
- Restore vs streaming race has exactly one legal winner and no mixed root.
- Fault injection after publication faults only target World.
- Slow query does not block mutation publication after it pins a read view.
- Two Worlds can commit concurrently without shared ordering state.

## 10. Acceptance criteria

- All visible writes are reachable only through the World serial lane.
- Barrier scope is observable and bounded by work category rather than a guessed duration.
- Restore and streaming apply are mechanically mutually exclusive.
- World Faulted state is terminal/recoverable only as prescribed by the frozen architecture; this package does not invent recovery.
