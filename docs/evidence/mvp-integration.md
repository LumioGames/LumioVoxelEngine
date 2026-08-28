# MVP vertical slice — R-00146

- Card: R-00146 [测试·集成]
- Baseline: `LGE-V1.4-2026-08-27`
- Worktree HEAD at measurement: `f4f2a6450a99a0a9498c44df6481897217bcfc62`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host `x86_64-pc-windows-msvc`
- Harness: `lumio_voxel_test_support::mvp_harness::run_mvp_vertical_slice` → `MvpIntegrationReport`
- Tests: `crates/lumio-voxel-test-support/tests/mvp_vertical_slice.rs`

This host has **no `link.exe`**. `cargo test` is a linker failure, **not** a slice PASS. `cargo check` exit 0 is type-check only and is **not** a test PASS.

No P2 streaming. No invented Schema numbers.

## 1. Commands actually run

CWD: Voxel worktree. Interpreter for Python: `C:\Users\g923\AppData\Local\Programs\Python\Python312\python.exe`.

| # | Command | Exit | Honest reading |
| --- | --- | ---: | --- |
| 1 | `cargo check -p lumio-voxel-test-support --lib --all-features` | 0 | lib type-checks; **not** a test PASS |
| 2 | `cargo check -p lumio-voxel-test-support --tests --all-features` | 0 | `mvp_vertical_slice` type-checks; **not** a test PASS |
| 3 | `cargo clippy -p lumio-voxel-test-support --lib --all-features -- -D warnings` | 0 | lib clippy clean |
| 4 | `cargo test -p lumio-voxel-test-support --test mvp_vertical_slice --all-features -- --nocapture` | **101** | `error: linker \`link.exe\` not found` (MSVC). **No test ran. Not PASS.** |
| 5 | `python.exe tools/architecture/check_crate_dag.py` | 0 | `check-crate-dag OK: 7 crates` |
| 6 | `git diff --stat` on production crates (`contracts` / `domain` / `ops` / `world` / `project` / `migration`) | 0 | empty |

`link.exe` is not on PATH. Default host is `x86_64-pc-windows-msvc`.

## 2. Named steps (no skipped row)

Runtime column is **not executed**: the only `cargo test` invocation ended at link (exit 101). Each row is a named step recorded by `run_mvp_vertical_slice()` and asserted from `mvp_vertical_slice.rs`. The test binary type-checks against the shipped APIs (command 2).

| # | Step | Shipped entry points driven | Runtime |
| ---: | --- | --- | --- |
| 1 | create | `VoxelWorld::create` ×2 via `intern_local_embedded_pair(Authority, Replica)`; lifecycle `Initialize/Prime/Start` via `GeneratedVoxelWorldPortAdapter::admit` | not run (101) |
| 2 | query | four-state ids + absent `c:4:0:0` through `adapter.query`; no implicit load (absent / NotLoaded / Pending / Unavailable do not become Ready) | not run (101) |
| 3 | prepare | `adapter.prepare_mutation` with `world_revision` + `c:0:0:0/cell-0`; identity unchanged | not run (101) |
| 4 | commit | `adapter.commit`; new published identity; receipt recorded | not run (101) |
| 5 | duplicate_replay | second `adapter.prepare_mutation` of the same TxnId then `adapter.commit`; original receipt bytes/hash returned; published identity unchanged | not run (101) |
| 6 | capture | `adapter.capture` + `RuntimeSnapshotCut::from_live`; CaptureCut released | not run (101) |
| 7 | encode | `encode_capture` on the captured ref outside the barrier | not run (101) |
| 8 | restore | `RestorePreflight::validate` + `RestoreShadowBuilder::build` + `adapter.restore` | not run (101) |
| 9 | durability_ack | `adapter.apply_durability_ack`; old ack must not clear newer dirty; covering ack clears | not run (101) |
| 10 | close | `adapter.shutdown` then stale-origin `adapter.query` → `StaleEpoch`; replica identity unchanged | not run (101) |

`run_mvp_vertical_slice()` fills `MvpIntegrationReport { baseline, commit, artifact_hashes, config_hash, steps, receipts, snapshot_hash, restored_hash, trace_hash, commands, authority_identity, replica_identity }`. `baseline` is generated `BASELINE_ID`. `commit` is `git rev-parse HEAD` at call time (not executed here). `trace_hash` is `DeterministicExecutor::run` on the MVP schema corpus with seeds `0xA11CE0` / `0xB05EED`. Reference `VoxelPortHarness` cannot observe Native world identity; alignment is op seq/payload snapshot only.

`run_b0_matrix` / `run_b2_matrix` are type-checked via `matrix_entry_points()` and the test `b0_and_b2_matrix_entry_points_typecheck`. They are **not invoked** (their runtime also needs `link.exe`).

Fixture helpers copied from `b2_harness.rs` (not adapter traffic): four-state / Ready directory publish through `PublicationAuthority`. Adapter mutation can only overlay `Ready` slots, so Pending/Unavailable cannot be inserted via the port. RestoreShadowBuilder rematerializes NotLoaded and an empty dirty frontier; the durability step therefore re-seeds Ready and mutates once before old/covering ack.

## 3. Cargo test stderr (verbatim, last run)

```text
error: linker `link.exe` not found
  |
  = note: program not found

note: the msvc targets depend on the msvc linker but `link.exe` was not found

note: please ensure that Visual Studio 2017 or later, or Build Tools for Visual Studio were installed with the Visual C++ option

note: VS Code is a different product, and is not sufficient

error: could not compile `lumio-voxel-test-support` (test "mvp_vertical_slice") due to 1 previous error
```

Exit: **101**. Zero tests executed. Do not treat commands 1–3 as a substitute PASS.

## 4. Diff boundary

Exclusive / allowed files:

- `crates/lumio-voxel-test-support/src/mvp_harness.rs` (create)
- `crates/lumio-voxel-test-support/tests/mvp_vertical_slice.rs` (create)
- `docs/evidence/mvp-integration.md` (create)
- `crates/lumio-voxel-test-support/src/lib.rs` — `pub mod mvp_harness;`

No production crate `src/` edits (`contracts` / `domain` / `ops` / `world` / `project` / `migration`). `Cargo.toml` not edited. `b0_harness.rs` / `b2_harness.rs` not edited.

## 5. Concerns

- Slice assertions have not run on this host. Type-check + clippy are not evidence that query four-state, commit identity swap, restore, durability coverage, or dual-instance isolation passed.
- Duplicate replay now drives `adapter.commit` of the same TxnId after a same-fingerprint second `prepare_mutation`. The shipped commit path returns the original receipt without a second publish. Runtime still not executed here (`link.exe` missing).
- Four-state / post-restore Ready fixtures go through copied B2 `PublicationAuthority` helpers, not the adapter.
- Prerequisite cards R-00142 / R-00143 / R-00145 were `in_review` on the card face; this harness calls their shipped APIs in-tree.
- `cargo test` exit 101 is the MSVC linker gap, identical to B0/B2. It is not a slice PASS.
