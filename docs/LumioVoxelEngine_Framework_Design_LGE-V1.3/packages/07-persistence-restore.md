# Package 07 — Persistence, DurabilityAck, and Restore

> Status: implementation-level design; no production code.  
> Governing baseline: `LGE-V1.3-2026-08-27`.  
> Public contract rule: import generated contract types; never duplicate their fields or numeric values here.  
> Decision rule: `VOX-D-001`–`VOX-D-008` remain `unapproved`; numeric defaults and policy choices stay blocked until approved.


## 1. Purpose and boundary

Own background snapshot encoding/decoding, durable-store ports, manifest compatibility checks, durability acknowledgement construction, and restore shadow-tree preparation. It does not own the snapshot cut, directly mutate a live World, directly clear dirty state, run streaming loads, or add/alter public Manifest/ABI fields.

Restore and streaming load are distinct, mutually exclusive paths. Restore builds and validates an unpublished complete state, then asks World to atomically publish it under a barrier. Streaming incrementally changes chunk presence through its own ticketed path.

## 2. Physical placement

- Owning alias: `CRATE_PERSISTENCE` (`SOURCE_CRATE_MAP_REQUIRED`).
- Generated-contract dependency: `CRATE_CONTRACT` (`SOURCE_CRATE_MAP_REQUIRED`), read-only/generated.
- Planned files:
  - `src/persistence/mod.rs`.
  - `src/persistence/encode.rs`.
  - `src/persistence/decode.rs`.
  - `src/persistence/store_port.rs`.
  - `src/persistence/manifest_adapter.rs`.
  - `src/persistence/durability_ack.rs`.
  - `src/restore/mod.rs`.
  - `src/restore/preflight.rs`.
  - `src/restore/shadow_builder.rs`.
  - `src/restore/publish_request.rs`.
  - `tests/persistence_roundtrip.rs`.
  - `tests/persistence_failure_dirty.rs`.
  - `tests/restore_atomicity.rs`.
  - `tests/restore_streaming_exclusion.rs`.

## 3. Ports and internal types

```text
DurableSnapshotStore
  begin_write(generated manifest identity)
  write_object(generated/object key, bytes)
  commit_manifest(generated manifest bytes)
  abort_write(...)
  read_manifest(...)
  read_object(...)

SnapshotEncoder
  encode(VoxelCaptureRef via CaptureReadPort, generated manifest builder)

DurabilityAckCandidate
  generated snapshot identity/cut coverage
  store commit proof required by source contract

RestorePreflight
  validated generated manifest
  compatibility evidence
  object plan

RestoreShadowState
  complete unpublished PublishedState candidate
  prebuilt indexes/ledger reset material exactly per frozen ADR
  validation evidence
```

Store and codec implementations depend on ports; the domain crate does not depend on concrete storage SDKs.

## 4. Durable write protocol

1. Consume the immutable capture outside the World barrier.
2. Build only fields permitted by the generated Manifest builder.
3. Encode objects deterministically according to source codec/version policy.
4. Write objects to a non-visible/temporary durable generation.
5. Commit the manifest/visibility marker according to the store contract.
6. Construct `DurabilityAckCandidate` only after durable commit succeeds.
7. Runtime submits the candidate to World; World validates cut coverage and clears only covered dirty entries under its barrier.

Retries must be idempotent using generated snapshot/object identities. Retry counts/backoff are unselected while decision gates remain unapproved.

## 5. Restore protocol

### Preflight and shadow build — outside World barrier

1. Runtime obtains restore admission for the target World; streaming admission is closed according to Package 05.
2. Read and parse the generated manifest.
3. Validate baseline/schema/capability/codec compatibility using generated registries.
4. Read/decode required objects and validate hashes/invariants.
5. Build immutable chunk versions, directory root, auxiliary root, and all source-required metadata in `RestoreShadowState`.
6. Preallocate publication/fault/diagnostic material.

### Publish — short World barrier

1. Submit a sealed restore publication request tied to World instance generation and restore admission token.
2. Recheck lifecycle, exclusion token, and shadow-state evidence.
3. Atomically publish the complete restored state once.
4. Apply source-defined revision/receipt/dirty/streaming-generation semantics using prebuilt infallible data.
5. Release the barrier and complete runtime operation state.

Any failure before publication leaves the prior published cut unchanged. No partial chunk-by-chunk restore is visible.

## 6. Compatibility and corruption

- Unknown public fields/versions/capabilities follow the generated compatibility policy; this design invents no fallback.
- Hash mismatch, truncation, and malformed object are distinguished internally but map only through existing error schema.
- A corrupt external snapshot is an operation failure, not automatically a World invariant fault, because it is rejected before publication.
- A violation detected after an allegedly validated shadow state crosses publication is an internal invariant failure and faults the target World.

## 7. Verification surface

- Golden fixtures from architecture source decode/encode without field drift.
- Crash matrix at every durable-write step never exposes a committed manifest that references missing objects.
- Store success but World ack failure leaves entries dirty and allows safe ack retry.
- Ack for old cut cannot clear later dirty revisions.
- Corrupt snapshot fails before publication and old World view remains unchanged.
- Restore and streaming apply race cannot interleave.
- Roundtrip tests compare semantic generated contracts, not implementation-specific byte layouts unless the source fixture requires exact bytes.

## 8. Acceptance criteria

- No live World reference is handed to codecs or storage drivers.
- Manifest construction goes through generated builders/adapters only.
- Restore publication is one atomic root replacement.
- Dirty clearing exists only in World `ApplyDurabilityAck`.
- Streaming code cannot call restore shadow/publication APIs.
