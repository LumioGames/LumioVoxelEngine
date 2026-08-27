# VOX-D-004 Reservation 租约与 Receipt 容量 — Decision Evidence

- Card: R-00060 (`01a04391-98c9-7eaf-ae99-93389d23e851`) / GATE-004
- Gate: `VOX-D-004`
- Role: Voxel 性能与架构决策工程师
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Voxel worktree HEAD at record: `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (detached; `feat(R-00047): add deterministic harness, faults and fixture runner`)
- Recorded: 2026-08-28
- Host: `Lumio` / Microsoft Windows NT 10.0.26200.0 / rustc host `x86_64-pc-windows-msvc`
- Produces: `DecisionEvidenceVOXD004`; `ReservationReceiptProposal`
- Exclusive files: this document; `benchmarks/decision_gates/reservation_receipt.rs`
- Optional data dir `benchmarks/decision_gates/data/vox-d-004/`: **not created** (no executable replay, no raw traces)
- `approvalStatus=approved`
- Architecture owner approval: **`LGE-V1.4-VOX-D-P0-2026-08-28`** (Architecture `5f06822`)
- Selected internal family: `GenerationBoundLeaseFamily` (tick/generation lease, never wall clock)

This is a research gate plus owner confirmation. It does **not** freeze a public receipt-table capacity Schema column. It does **not** invent abort reasons. `approval_status()` returns `"approved"`.

**Explicit:** no Schema, ID Registry, ABI, generated Artifact, or default config was modified by this card. `Cargo.toml` / crate `lib.rs` were not edited.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Architecture baseline | `LGE-V1.4-2026-08-27` |
| Voxel HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Implementation blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| V1.3 `DECISION_GATES.md` SHA-256 | `4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2` |
| V1.3 `MANIFEST.sha256` file SHA-256 | `8c94d6b2680e331007dfab6961ef094a9745faee2084993f5ac0498f7161d3e6` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| `LICENSE` SHA-256 | `077012acbc9d54d0533b80bc7e9f23681f23319f1dd353a48e698ed053c9842e` |
| Toolchain (declared + observed) | `rust-toolchain.toml` channel `1.98.0` SHA-256 `f2c82031ae793bfe2c19b2f9828259fbb6eb79081849cca5294706ab46ff2d32`; `rustc 1.98.0 (88d9e12ae 2026-08-18)`; `cargo 1.98.0 (797e8a9bc 2026-08-05)` |
| Prerequisite R-00034 | Consumable (`in_review` with evidence; present on this history). |
| Prerequisite R-00047 | **Met.** Delivered at `b2f0d8a`. `VoxelPortHarness`, `DeterministicExecutor`, `FaultPoint`, `FaultInjector`, `run_fixture` are in `lumio-voxel-test-support`. |
| Prerequisite of R-00047: R-00045 | Met on this history (`c938868 feat(R-00045): consume V1.4 generated contract artifacts`). |
| `lumio-voxel-test-support/src/lib.rs` SHA-256 | `7e7bf9b24a8e31e55130bcd5c6336f748b2d456bdd4c8b20b3643082099ed742` |
| `reference_harness.rs` SHA-256 | `fcbc9274fe18eb028021b44e78e8e94e2da435e46ac4fdac3dbda3e94737ef1f` |
| `deterministic_executor.rs` SHA-256 | `46ae8ad5d6d4d27aa263a5d90918d35d8d606db12ea28aebc1433cd4125eec1e` |
| `fault_injection.rs` SHA-256 | `b39959ed9723619733c566bcd7b356073c6480671c4aba5b1f72c666a1fd3104` |
| Seam `reservation_receipt.rs` SHA-256 | `6fe271b63b1a00fbcba4b4984ce86e14317fbda995d10d343edf7b320ff45e1b` |

Historical unapproved source only (not authority, not a selected default): [`docs/LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md`](../../LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md) lists `VOX-D-004` as `unapproved` with rule “Expose configuration/measurement seam only; do not select a default or algorithm.”

## 2. Candidate list (names / kinds only)

No candidate is selected. The first row is not a conclusion. Exclude-reason values remain **pending-measurement**: the shipped harness is an echo port, not a receipt ledger, and this host could not link a binary replay.

| Name | Kind | Version | License | Source Hash | exclude-reason |
| --- | --- | --- | --- | --- | --- |
| GenerationBoundLeaseFamily | reservation-receipt-profile (lease bound to generation / revision, no wall clock) | unversioned-internal | Apache-2.0 (in-tree policy family, not a crate) | not-computed (policy name; not an implementation artifact) | pending-measurement |
| WallClockLeaseFamily | reservation-receipt-profile (TTL lease) | unversioned-internal | Apache-2.0 (in-tree policy family, not a crate) | not-computed (policy name; not an implementation artifact) | pending-measurement |
| AckPruneCapacityFamily | reservation-receipt-profile (capacity + prune at DurabilityAck safety point) | unversioned-internal | Apache-2.0 (in-tree policy family, not a crate) | not-computed (policy name; not an implementation artifact) | pending-measurement |

Selected default: **none**. Recommended candidate: **none**. `lease` / `capacity` / `pruneRule`: **null**.

## 3. Frozen contract vs internal candidate vs public value

| Class | What | Status |
| --- | --- | --- |
| Frozen contract (V1.4 schema exists) | ADR-025 / `schemas/voxel-mutation-receipt.schema.json`: participant receipt, `status(txnId)`, `CoDurableWithWorldState`; Prepare has no visible side effects; Commit is idempotent on `TxnId`; fingerprint conflict rejects before visible write; crash-recovery / pruning *protocol* (not numeric caps). Mutation is the sole write coordinator (ADR-0002). Receipt corpus labels used by this seam (`applied`, `duplicate`, `aborted-conflict`, `lost-result`, `pruned`) name that frozen protocol; they are not new Schema fields. Abort / reject **vocabulary already published** on the generated ID registry and reused here without addition: `RevisionConflict`, `ChunkUnavailable`, `StaleEpoch`, `InvalidHandle`, `EvidenceMissing`, `PartialLoadRolledBack`. | Frozen by architecture source. This card did not copy Schema fields and did not invent abort reasons. |
| Internal candidate | The three lease/capacity families in §2. Measurement-only names. Not compiled into `mutation/receipt_ledger.rs` or `VoxelConfigSnapshot`. | Internal. Not production policy. |
| Public value awaiting architecture-owner approval | Lease duration or generation window, receipt table capacity, prune numeric rule, and any generated config default. After approval, public config must be generated by the architecture repository. | Unapproved. |

## 4. Measurements — seam compiled; binary replay **未执行**

R-00047 is met. The exclusive seam now **drives** shipped `DeterministicExecutor` / `VoxelPortHarness` / `FaultPoint`. It is **not** a workspace member.

Statistical method (implemented in the seam, **not applied to a linked binary on this host**): three repeats of the same seed / corpus / schedule (`MEASUREMENT_SEED = 0x0000_D004`, `REPEAT_COUNT = 3`); SHA-256 of each raw encoded receipt trace via `lumio_voxel_contracts::sha256`; require byte-identical traces, trace hashes, and `snapshot_hash`. No summary-only charts. **No trace hashes are recorded below** — they were not produced by an executed binary, and this card does not invent them.

Executable corpus (ops use generated `schema_id` `"voxel-mutation-receipt"`):

- `applied`
- `duplicate`
- `aborted-conflict`
- `lost-result`
- `pruned`

Fault matrix mapped onto **shipped** `FaultPoint`s (recoverability from `FaultInjector`, not a new error table). `PostPublication` / `LostResult` stay unrecoverable:

| Decision-gate fault | Shipped `FaultPoint` | Shipped `error_id` | `recoverable` |
| --- | --- | --- | --- |
| `commit-intent-leak` | `PostPublication` | `PartialLoadRolledBack` | `false` |
| `applied-missing-result` | `LostResult` | `EvidenceMissing` | `false` |
| `lease-expired` | `PrePublication` | `InvalidHandle` | `true` |

`lease-expired` is a pre-visible-write reject (Prepare/Reservation path). It is **not** mapped to an invented `LeaseExpired` abort reason. Mapping uses the existing `PrePublication` / `InvalidHandle` pair.

Card-level capacity axes still **unmeasured** by the echo harness (need a real receipt ledger, R-00093): `repeated-txn`, `long-txn`, `crash-replay`, `capacity-pressure`, `prune-safety-point`. Negative axes `lease-boundary-race`, `fingerprint-conflict`, `capacity-exhaustion`, `restart-recovery` likewise cannot be decided by echo-port traces.

### Commands actually run

| # | Command | Exit | Notes |
| --- | --- | --- | --- |
| 1 | `cargo test -p lumio-voxel-test-support --all-features` | 101 | `error: linker 'link.exe' not found` (msvc). Libraries compiled; `lib test` failed to link. Expected on this host. |
| 2 | `cargo build --lib -p lumio-voxel-test-support --all-features` | 0 | `Finished dev profile ... in 0.02s` (rlibs already produced by step 1 compile). |
| 3 | `rustc --edition 2024 --crate-type rlib --crate-name vox_d_004_seam -L target/debug/deps --extern lumio_voxel_test_support=target/debug/deps/liblumio_voxel_test_support-6da53ff03bc58986.rlib --extern lumio_voxel_contracts=target/debug/deps/liblumio_voxel_contracts-a62c2cca441f7fde.rlib benchmarks/decision_gates/reservation_receipt.rs -o C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\seam-out\vox-d-004.rlib` | 0 | 163640 bytes. SHA-256 `61685d925a79e3c677d26915a2cacebd187cad9ddf0017c07fdcda32e7d0deb2`. Re-ran with `-D warnings` also exit 0. |
| 4 | rustfmt `--edition 2024` on the seam | 0 | Applied one formatting wrap; file is LF (`CR=0`). |
| 5 | Optional binary replay via `lld-link` / gnu `ld` | 1 | `kernel32.lib` / mingw libs missing. **未执行**. No exe, no traces. |
| 6 | `node .spec/tools/spec-lint.mjs` | 1 | Pre-existing Windows symlink unresolved (`.claude/agents`, `.claude/skills`, `.agents/skills`). Not caused by this card; this card did not edit `.spec/`. |

Log: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00060.log`.

No imaginary numbers. No invented run hashes. No memory / liveness / false-reject statistics.

## 5. Approval

- `approvalStatus=approved`
- Architecture owner approval: **recorded**
- Approval reference: `LGE-V1.4-VOX-D-P0-2026-08-28` / Architecture commit `5f06822`
- Selected internal family: `GenerationBoundLeaseFamily`
- Wall-clock leases rejected. Public table-capacity Schema columns remain ungenerated.

`ReservationReceiptProposal` (no selected lease / capacity / prune rule):

```json
{
  "name": "ReservationReceiptProposal",
  "lease": "GenerationBoundLeaseFamily",
  "capacity": null,
  "pruneRule": null,
  "corpus": [
    "applied",
    "duplicate",
    "aborted-conflict",
    "lost-result",
    "pruned"
  ],
  "schemaId": "voxel-mutation-receipt",
  "repeats": 3,
  "seed": "0x0000D004",
  "faults": [
    {
      "name": "commit-intent-leak",
      "faultPoint": "PostPublication",
      "errorId": "PartialLoadRolledBack",
      "recoverable": false
    },
    {
      "name": "applied-missing-result",
      "faultPoint": "LostResult",
      "errorId": "EvidenceMissing",
      "recoverable": false
    },
    {
      "name": "lease-expired",
      "faultPoint": "PrePublication",
      "errorId": "InvalidHandle",
      "recoverable": true
    }
  ],
  "measurements": {
    "status": "seam-compiled-not-executed",
    "exit": {
      "cargoTestAllFeatures": 101,
      "cargoBuildLib": 0,
      "rustcSeamRlib": 0,
      "binaryReplay": "未执行"
    },
    "reason": "R-00047 met; seam compiled against VoxelPortHarness; host cannot link executables (no link.exe / kernel32.lib); trace hashes not invented",
    "traceHashes": []
  },
  "sourceHashes": {
    "architectureBaselineId": "LGE-V1.4-2026-08-27",
    "voxelHead": "b2f0d8a3763a02f805e29cbd101560ba7fdca77b",
    "architectureMirrorSha256": "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0",
    "v13DecisionGatesSha256": "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2",
    "blueprintSha256": "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa",
    "r00047LibRsSha256": "7e7bf9b24a8e31e55130bcd5c6336f748b2d456bdd4c8b20b3643082099ed742",
    "seamRsSha256": "6fe271b63b1a00fbcba4b4984ce86e14317fbda995d10d343edf7b320ff45e1b",
    "seamRlibSha256": "61685d925a79e3c677d26915a2cacebd187cad9ddf0017c07fdcda32e7d0deb2"
  },
  "approvalStatus": "approved",
  "approvalReference": "LGE-V1.4-VOX-D-P0-2026-08-28"
}
```

## 6. Blocked requirements and continuable scope

Cannot proceed until this freeze is architecture-owner approved **and** executable three-run hashes exist on a machine that can link, using a receipt ledger that can actually size lease/capacity:

- R-00066 `[程序·配置] 建立不可变 Voxel 配置快照` — consumes `DecisionEvidenceVOXD004`.
- R-00093 `[程序·Mutation] 实现 Canonical 指纹与 Txn Receipt Ledger` — depends on R-00060 / R-00066 / R-00076.
- Downstream prepare/commit / world write-lane cards that size the receipt ledger.

May continue (does **not** need this freeze):

- Nothing that requires a frozen Reservation lease or receipt capacity.
- Independent work: crate DAG, artifact-gate evidence, sibling VOX-D cards that do not consume this profile.

Stop thresholds (qualitative; none frozen as numerics): re-execution of a committed `TxnId`; prune that drops a still-needed receipt; wall-clock lease that is non-deterministic across replays; capacity exhaustion that reports success. The echo harness cannot witness the first two; WallClockLeaseFamily remains a qualitative risk, **not** an exclusion.

## 7. Contract

No Schema / ID / default config was modified. No abort reason was added. `approval_status()` remains `"blocked"`.
