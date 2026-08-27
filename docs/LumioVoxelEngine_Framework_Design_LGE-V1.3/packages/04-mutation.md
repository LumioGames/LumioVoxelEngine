# Package 04 — Mutation Prepare/Commit and TxnId Idempotency

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

`mutation` is the sole write coordinator. It owns request fingerprinting, idempotency lookup, optimistic preconditions, private staging, commit-plan sealing, canonical receipt construction, and submission to the World write lane. No other package may publish chunk/world roots or make a mutation visible.

Prepare has **zero visible side effects**. Commit is idempotent by imported `TxnId`. A repeated equivalent request returns the original stored receipt, not a recomputed approximation. A repeated `TxnId` with a different canonical request fingerprint is rejected before any visible write using only existing contract semantics.

## 2. Physical placement

- Owning alias: `CRATE_DOMAIN` (`SOURCE_CRATE_MAP_REQUIRED`).
- Planned files:
  - `src/mutation/mod.rs`.
  - `src/mutation/request_fingerprint.rs`.
  - `src/mutation/prepare.rs`.
  - `src/mutation/preconditions.rs`.
  - `src/mutation/plan.rs`.
  - `src/mutation/commit.rs`.
  - `src/mutation/receipt_ledger.rs`.
  - `tests/mutation_prepare_purity.rs`.
  - `tests/mutation_txn_replay.rs`.
  - `tests/mutation_atomic_batch.rs`.
  - `tests/mutation_fault_escalation.rs`.

## 3. Internal types

```text
CanonicalRequestFingerprint   // derived from generated canonicalization rules
PrepareContext                // immutable ReadView + captured config + imported request
PreparedMutation              // sealed, single-use
  base_publication_generation
  request_fingerprint
  frozen_chunk_replacements
  frozen_auxiliary_root
  prebuilt_dirty_delta
  prebuilt_original_receipt
  pre-reserved ledger/publication resources

TxnReceiptRecord
  txn_id
  canonical_request_fingerprint
  immutable original receipt representation
  committed revision/cut reference as defined by source contract

TxnReceiptLedger
  lookup(txn_id)
  reserve_for(prepared)
  publish_infallibly(record)
```

## 4. Prepare protocol — no visible side effects

1. Canonicalize and fingerprint the imported request.
2. Consult the receipt ledger read-only:
   - matching fingerprint: return replay outcome with original receipt;
   - differing fingerprint: return existing source-defined conflict outcome;
   - absent: continue.
3. Capture one immutable base view.
4. Validate lifecycle, permissions/capabilities through existing adapters, chunk presence, and preconditions.
5. Build all edited chunk versions and derived indexes privately.
6. Build the replacement directory/auxiliary roots privately.
7. Allocate revision reservation, ledger capacity, dirty delta, event data, and canonical receipt representation.
8. Seal a non-cloneable `PreparedMutation`.

No load-state transition, dirty mark, revision publication, event emission, ledger insertion, or cache mutation is allowed in this phase.

## 5. Commit protocol

1. Enter the World serial write lane with the prepared token.
2. Recheck lifecycle, `TxnId` ledger, base publication generation, and all commit-time preconditions.
3. If a matching receipt appeared concurrently, return that original receipt without publication.
4. If anything is stale, fail before visibility; discard the token.
5. Construct a `PublicationToken` from already frozen/preallocated members.
6. Perform the single visible root publication.
7. Finalize dirty frontier and receipt record using infallible moves/pre-reserved slots.
8. Return the prebuilt canonical receipt.

The implementation must be structured so “first visible write” is auditable in code review. No fallible callback, allocation, serialization, I/O, lock acquisition with timeout, or generated-contract conversion may occur after it.

## 6. Batch semantics

A batch is staged and validated as one unit. It is not a loop of independently visible commits. Every touched chunk and auxiliary index is represented in one replacement root. If a supposedly impossible invariant is violated at/after visibility, the World transitions to Faulted under Package 05; the path must not return an ordinary partial-failure result.

## 7. Receipt retention and decision gates

Retention, eviction, persistence, and memory budget follow the exact approved `VOX-D-*` decisions. Until approval, implementation may define the port and benchmark harness but may not choose a count, TTL, eviction algorithm, or fallback that weakens replay guarantees.

## 8. Verification surface

- Prepare purity snapshot: all published roots, lifecycle, ledger, dirty frontier, and streaming states unchanged on every prepare failure.
- Same `TxnId` + same canonical request returns byte/field-equivalent original receipt across retries.
- Same `TxnId` + different fingerprint has no visible side effect.
- Arbitrary allocation/fault injection before publication leaves old cut intact.
- No injected ordinary failure point exists after publication; invariant trap faults only that World.
- Batch observers see all-old or all-new touched chunks.

## 9. Acceptance criteria

- Static/code-review evidence identifies the sole publication statement.
- Only this package can produce a sealed publication request for mutation.
- Receipt replay does not rerun mutation logic.
- Every post-publication operation is mechanically infallible or routed to World fault containment.
