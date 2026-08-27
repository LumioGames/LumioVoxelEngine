# LumioVoxelEngine Implementation Framework Design

**Baseline:** `LGE-V1.3-2026-08-27`  
**Repository commit inspected:** `UNAVAILABLE`  
**Deliverable type:** implementation-level framework design and executable task plan; no production Rust, no Cargo workspace/source scaffolding, no public ABI/Manifest/ErrorCode changes.

## 1. Authority and scope

This package lowers the already frozen LumioVoxelEngine architecture into file/type/port/state/test/task precision. It does not reopen ADR 0001–0006, alter the seven-crate map or L0–L5 direction, invent public protocol fields, select values for `VOX-D-001`–`VOX-D-008`, or implement gameplay/product logic.

Source precedence is fixed:

1. `LumioGameEngineArchitecture` at `LGE-V1.3-2026-08-27` — unique public contract/registry/fixture authority.
2. Effective LumioVoxelEngine ADR 0001–0006.
3. Frozen repository module README boundaries.
4. These package designs.
5. Task cards; a task may not override a higher source.

Any source conflict stops the affected task and produces an evidence report. “Convenient implementation” is not a conflict-resolution rule.

## 2. Frozen invariants carried into implementation

1. Seven physical crates and L0–L5 dependency direction are fixed.
2. `mutation` is the sole write coordinator.
3. Prepare has no visible side effect.
4. Commit is idempotent by imported `TxnId` and preserves the original receipt.
5. A CommitBatch cannot fail normally after its first visible write; an impossible invariant breach faults the owning World.
6. Runtime owns `SnapshotCut`; voxel owns only the internal immutable `VoxelCaptureRef`.
7. Snapshot uses a short World barrier followed by background encoding/I/O.
8. Dirty state clears only through a coverage-checked `DurabilityAck` applied under the World barrier.
9. Restore and streaming load are separate and mutually exclusive publication paths.
10. Missing chunks remain four states: `Ready`, `NotLoaded`, `Pending`, `Unavailable`.
11. LocalEmbedded uses two physically isolated World trees; same-process deployment does not permit state aliasing.
12. `VOX-D-001`–`VOX-D-008` remain unapproved gates; no numeric default is chosen in source or tests.

## 3. Layered dependency design

| Layer | Logical ownership | Allowed inward dependencies | Forbidden outward dependency |
|---|---|---|---|
| L0 | generated contracts/IDs/errors/fixtures | none beyond architecture-generated support | any domain/runtime implementation |
| L1 | revision, chunk immutable state | L0 | query/mutation/world/runtime |
| L2 | query, mutation prepare/plan | L0–L1 | concrete persistence/streaming/runtime drivers |
| L3 | World lifecycle, serial write lane, barrier | L0–L2 | concrete I/O SDKs and FFI entrypoints |
| L4 | capture adapter, persistence/restore, streaming workers | source-defined inward ports L0–L3 | FFI and application/product code |
| L5 | Runtime host, composition and boundary adapters | L0–L4 | product/gameplay repositories |
| Test | fixture/testkit | may depend on all for verification | no production crate depends back on it |

## 4. State ownership matrix

| State | Sole owner | Readers | Legal writer path |
|---|---|---|---|
| Current published roots/revision | World/Revision | pinned Query/Capture views | atomic publication under World lane |
| Chunk immutable payload/version | Chunk package | Query/Capture | unpublished builders, then root publication |
| Chunk presence state | Chunk root | Query/Streaming | World-applied mutation/stream command |
| Txn receipt ledger | Mutation within World | mutation replay/runtime diagnostics | commit pre-reservation + infallible finalize |
| Dirty frontier | World/Chunk metadata | Snapshot cut | mutation mark or World-applied DurabilityAck |
| Streaming tickets | Streaming coordinator + World admission token | Runtime/diagnostics | ticket protocol; worker never writes World |
| Restore operation/shadow state | Runtime/Persistence | runtime diagnostics | one World restore publication barrier |
| Snapshot operation | Runtime | boundary adapter | Runtime orchestrator |
| Voxel capture roots/pins | voxel capture package | persistence encoder | create under barrier; release by ref lifetime |
| Config policy | immutable RuntimeConfigSnapshot | all admitted operations | replacement snapshot, never live mutation |
| Fault record/lifecycle | target World | Runtime/query admission | World fault path only |

## 5. Canonical flows

### 5.1 Query

`Runtime entry → World capture ReadView → Query plan → immutable chunk accesses → generated response mapping → release pin`.

No query step initiates I/O or changes load state. Demand generation is a separate explicit runtime/streaming action.

### 5.2 Mutation

`Generated request → canonical fingerprint/ledger lookup → immutable base view → private staging/validation/preallocation → sealed PreparedMutation → World serial lane → final recheck → one root publication → infallible dirty/ledger finalize → original receipt`.

All fallible work is before publication. Replay reads the stored receipt rather than executing prepare again.

### 5.3 Snapshot and durability

`Runtime operation → World snapshot-cut barrier → VoxelCaptureRef → release barrier → background encode/store → durable commit → DurabilityAckCandidate → World ack barrier → clear only covered dirty revisions`.

### 5.4 Restore

`Runtime restore admission (streaming closed) → manifest preflight/read/decode outside barrier → immutable shadow state → World restore barrier → one atomic publication → release admission`.

### 5.5 Streaming

`Explicit demand → World Pending/ticket admission → worker fetch/decode outside barrier → sealed result → World ticket/generation recheck → Ready/Unavailable publication`.

### 5.6 LocalEmbedded

`Authority Runtime/World tree ↔ generated serialized contract boundary ↔ presentation Runtime/World tree`. No domain object, root, payload buffer, ledger, ticket, capture ref, or mutable cache crosses the boundary by reference.

## 6. Visibility and failure rules

“Visible” means reachable from a newly captured `ReadView`, observable via generated response/event state, or recorded as committed Txn state. Private allocations/builders/reservations are not visible. The mutation design deliberately makes atomic root replacement the first visible write.

- Ordinary validation/resource/I/O failure before publication: operation fails through existing error mapping; old World cut remains.
- External snapshot/streaming corruption or unavailability: operation/ticket failure; no direct World fault.
- Internal invariant breach that may compromise a published cut or mandatory post-publication bookkeeping: target World transitions to Faulted; no ordinary partial-success error is fabricated.
- Runtime and other Worlds remain live unless the frozen architecture explicitly elevates the failure domain.

## 7. Concurrency model

- One serial state-changing lane per World; no process-global write lock.
- Queries and snapshot encoders consume immutable pinned roots without holding the barrier.
- Background I/O never receives mutable World references.
- Async completions carry World instance generation and operation/ticket basis to reject stale results.
- Config is snapshot-based, not a mutable global.
- Cross-World operations coordinate at Runtime without merging World write lanes.

## 8. Decisions and configuration gates

All eight decision IDs are carried as hard gates. The design provides seams, metrics, and benchmarks but no value/algorithm selection. See [`DECISION_GATES.md`](DECISION_GATES.md). A task blocked by a gate may implement interfaces and measurement only; production behavior requiring an unapproved value cannot be merged.

## 9. Package index

1. [`01-revision-publication.md`](packages/01-revision-publication.md)
2. [`02-chunk-store.md`](packages/02-chunk-store.md)
3. [`03-query.md`](packages/03-query.md)
4. [`04-mutation.md`](packages/04-mutation.md)
5. [`05-world-barrier.md`](packages/05-world-barrier.md)
6. [`06-snapshot-capture.md`](packages/06-snapshot-capture.md)
7. [`07-persistence-restore.md`](packages/07-persistence-restore.md)
8. [`08-streaming.md`](packages/08-streaming.md)
9. [`09-runtime.md`](packages/09-runtime.md)
10. [`10-integration-verification.md`](packages/10-integration-verification.md)

## 10. Task execution model

[`TASK_CARDS.md`](TASK_CARDS.md) orders implementation by Wave and serial Lane. Each card owns an explicit non-overlapping file set, lists dependencies, tests, forbidden changes, and evidence. The machine-readable source is [`task-cards.json`](task-cards.json).

## 11. Definition of done for the framework

- The seven frozen crates compile in the frozen dependency direction.
- Generated contract and architecture fixture conformance passes unchanged.
- State ownership has one writer for every mutable state.
- Mutation atomicity/idempotency and original-receipt replay are proven deterministically.
- Snapshot cut, background durability, ack coverage, restore atomicity, and streaming exclusion are proven.
- Four missing-chunk states survive all adapters.
- LocalEmbedded two-tree no-alias and remote-equivalence fixtures pass.
- Fault injection proves target-World containment.
- No unapproved decision value, new public field/code/capability, or source-authority duplicate is introduced.
