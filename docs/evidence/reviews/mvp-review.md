# MvpReviewReport — R-00203

- Reviewer: independent Voxel reviewer (Grok; did not implement P0 domain/query/mutation/world cards)
- Baseline: `LGE-V1.4-2026-08-27`
- Reviewed HEAD: `105ef06c` (`fix(R-00146): duplicate TxnId adapter.commit returns original receipt`; MVP harness originally `07886b9`)
- Architecture artifacts: published at `3d5e29db72b70c88fb61e392832afe2a762b25cb`
- Artifact gate: R-00037 `GateResult.ready=true` (`a0cd223a40fee07257c26b67a161e9edaae90f0d`)
- Artifact five-tuple (consumed copy): `baselineId=LGE-V1.4-2026-08-27` `schemaEpoch=1` `compilerHash=99a786e7241d6e8650b3bf17c8e9e731b483cc7096ee217c519ff24706d20b6b` `inputHash=84a2b4c80d3d2bc30be3a25a5f53a4380a9cd29a101d13fdf9688e561bfeeef1` `implementationDependencies=[]`
- Verdict: **RETURN**

This re-review replaces the `f4f2a64` / HEAD `1e07b76` snapshot (that report predated `07886b9`). On this HEAD the exclusive R-00146 files are present.

R-00066 **is shipped**. `crates/lumio-voxel-domain/src/lib.rs:10` is `pub mod config_snapshot;`. This report does **not** list R-00066 among undelivered P0 cards.

R-00146 **is shipped**. `crates/lumio-voxel-test-support/src/mvp_harness.rs` exists. `docs/evidence/mvp-integration.md` exists. `crates/lumio-voxel-test-support/src/lib.rs:14` is `pub mod mvp_harness;`. Live inventory status is `in_review`.

## 执行前置

卡面：「前置未满足立即交回」。R-00143 / R-00145 / R-00146 均有独占交付物且 live `in_review`。本审查产出 **RETURN**，不代修、不条件放行、不把 `cargo check` 当矩阵 PASS、不 APPROVE。

| 前置 | Tree | Consumable for APPROVE? |
|---|---|---|
| R-00143 [测试·B0] | `b0_harness.rs` / `tests/b0_contract_domain.rs` / `docs/evidence/b0-verification.md` at `72564f8` | type-check only (`cargo test` 101, no `link.exe`) |
| R-00145 [测试·B2] | `b2_harness.rs` / `tests/b2_transaction_recovery.rs` / `docs/evidence/b2-verification.md` at `1e07b76` | type-check only |
| R-00146 [测试·集成] | `mvp_harness.rs` / `tests/mvp_vertical_slice.rs` / `mvp-integration.md`; `lib.rs:14` `pub mod mvp_harness`; duplicate TxnId replay at `105ef06` | type-check only |

`crates/lumio-voxel-test-support/src/lib.rs` exports `b0_harness`, `b2_harness`, and `mvp_harness` in addition to the R-00047 harness modules.

## Scope vs delivered (this HEAD)

**Shipped and tree-verifiable (not an APPROVE of MVP):**

| Card | Commit | What landed |
|---|---|---|
| R-00034 | `8c49fba` | V1.4 蓝图 / ADR 0007 |
| R-00037 | `a0cd223` | generated artifact gate ready |
| R-00041 | `1175b08` | 七 crate DAG |
| R-00045 | `c938868` | hash-locked generated artifacts |
| R-00047 | `b2f0d8a` | deterministic harness |
| R-00057..60 | `31cb6a2` / `54b488f` | P0 VOX-D measured; owner freeze `LGE-V1.4-VOX-D-P0-2026-08-28` |
| R-00066 | `7a01dbd` | `config_snapshot.rs`; `from_generated` rejects unapproved P0 gates; **no invented numeric defaults** |
| R-00068 / R-00070 | `31cb6a2` | OriginToken / Revision allocator |
| R-00071 / R-00073 | `3e57944` | Pin / Chunk four-state |
| R-00076 | `26e3f3f` | delta / dirty / replacement |
| R-00078 / R-00093 | `74ca752` | PublishedStateRoot / receipt ledger |
| R-00080 / R-00096 | `88d527b` | Query planner / Prepare |
| R-00081 / R-00104 | `edf472e` / `9935902` | Query execute / Commit |
| R-00116 / R-00119 | `58fe42c` / `0399db9` | World lifecycle / write lane |
| R-00121 / R-00134 | `cf22b61` / `400dc47` | fault isolation / VoxelCaptureRef |
| R-00135 / R-00136 | `fc119d2` / `ffee61c` | CaptureCut / Restore |
| R-00137 | `6312d2a` | DurabilityAck-only dirty clear |
| R-00142 | `c51e5cd` | generated voxel-world-port adapter |
| R-00143 | `72564f8` | B0 matrix harness |
| R-00145 | `1e07b76` | B2 matrix harness |
| R-00146 | `07886b9` + `105ef06` | MVP vertical slice via Port adapter; **duplicate TxnId drives `adapter.commit` and asserts the original receipt** |

`crates/lumio-voxel-domain/src/lib.rs:10` `pub mod config_snapshot` is present. R-00066 is **delivered**.

`crates/lumio-voxel-test-support/src/lib.rs:14` `pub mod mvp_harness` is present. R-00146 exclusive files are **in tree**.

## Duplicate TxnId (R-00146 详细要求)

Previous harness (`07886b9`) only second-prepared and required `RevisionConflict` / `InvalidHandle`. That did **not** meet 「重复 TxnId 返回原 receipt」.

This HEAD (`105ef06`):

- `MutationPreconditions::check` treats same-fingerprint `LookupOutcome::Duplicate` as idempotent replay (InFlight still rejected).
- `commit` looks up Duplicate **before** unpublished-base recheck and returns the stored receipt without a second `publish_once`.
- `mvp_harness::step_duplicate_replay` calls `GeneratedVoxelWorldPortAdapter::prepare_mutation` then **`adapter.commit`** on the same TxnId and asserts `replayed.payload.receipt == original.receipt` plus unchanged published identity.
- `tests/mvp_vertical_slice.rs` asserts the `duplicate_replay` detail names the original receipt and that only one `txn-mvp` receipt is recorded.

This is **type-checked**, not a linked test PASS.

## Why RETURN

1. `cargo test -p lumio-voxel-test-support --test mvp_vertical_slice --all-features` exit **101** (`linker link.exe not found`). Type-check is not a matrix PASS. Same for B0/B2.
2. QA live environment (R-00204) is not declared; `docs/evidence/qa/mvp-release-gate.md` is **BLOCKED**. Unit tests do not replace QA.
3. P2 Streaming/Spatial/Migration remain blocked on unapproved VOX-D-006/007/008. That does not undo R-00066 or R-00146.

This is not APPROVE, not a conditional pass, and not a claim that unit tests replaced QA.

## Commands actually run for this re-review

| Command | Exit | Reading |
|---|---:|---|
| `cargo check -p lumio-voxel-ops --tests --all-features` | 0 | type-check only |
| `cargo check -p lumio-voxel-test-support --tests --all-features` | 0 | type-check only |
| `cargo clippy -p lumio-voxel-ops --lib --all-features -- -D warnings` | 0 | lib clippy clean |
| `cargo clippy -p lumio-voxel-test-support --lib --all-features -- -D warnings` | 0 | lib clippy clean |
| `python tools/architecture/check_crate_dag.py` | 0 | 7 crates |
| `cargo test -p lumio-voxel-ops --test mutation_commit --all-features` | **101** | no `link.exe`; not PASS |
| `cargo test -p lumio-voxel-test-support --test mvp_vertical_slice --all-features` | **101** | no `link.exe`; not PASS |
