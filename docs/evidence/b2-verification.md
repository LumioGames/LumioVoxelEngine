# B2 verification matrix — R-00145

- Card: R-00145 [测试·B2]
- Baseline: `LGE-V1.4-2026-08-27`
- Worktree HEAD at measurement: `72564f886842b639431dae91584ecd02536f0aec`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host `x86_64-pc-windows-msvc`
- Harness: `lumio_voxel_test_support::b2_harness::run_b2_matrix` → `B2VerificationReport`
- Tests: `crates/lumio-voxel-test-support/tests/b2_transaction_recovery.rs`

This host has **no `link.exe`**. `cargo test` is a linker failure, **not** a matrix PASS. `cargo check` exit 0 is type-check only and is **not** a test PASS.

## 1. Commands actually run

CWD: Voxel worktree. Interpreter for Python: `C:\Users\g923\AppData\Local\Programs\Python\Python312\python.exe`.

| # | Command | Exit | Honest reading |
| --- | --- | ---: | --- |
| 1 | `cargo check -p lumio-voxel-test-support --lib --all-features` | 0 | lib type-checks; **not** a test PASS |
| 2 | `cargo check -p lumio-voxel-test-support --tests --all-features` | 0 | `b2_transaction_recovery` type-checks; **not** a test PASS |
| 3 | `cargo clippy -p lumio-voxel-test-support --lib --all-features -- -D warnings` | 0 | lib clippy clean |
| 4 | `cargo test -p lumio-voxel-test-support --test b2_transaction_recovery --all-features -- --nocapture` | **101** | `error: linker \`link.exe\` not found` (MSVC). **No test ran. Not PASS.** |
| 5 | `python.exe tools/architecture/check_crate_dag.py` | 0 | `check-crate-dag OK: 7 crates` |
| 6 | `git diff --stat` on production crates (`contracts` / `domain` / `ops` / `world` / `project` / `migration`) | 0 | empty |

`link.exe` is not on PATH. Default host is `x86_64-pc-windows-msvc`.

## 2. Matrix rows (no skipped rows)

Runtime column is **not executed**: the only `cargo test` invocation ended at link (exit 101). Each row is a public case function called from `run_b2_matrix()` and from `b2_transaction_recovery.rs`. The test binary type-checks against the shipped APIs (command 2).

| # | Row | Case | Shipped entry points driven | Runtime |
| ---: | --- | --- | --- | --- |
| 1 | Query single-cut + permutation-independent plan_hash | `case_query_single_cut_plan_hash` | `QueryPlanner::from_approved_snapshot`, `plan`, `QueryExecutor::execute`; stamp mismatch `InvalidHandle` | not run (101) |
| 2 | Query four-state / NotLoaded mapping | `case_query_four_state` | `QueryExecutor::execute` directory lookup; absent id → `NotLoaded` | not run (101) |
| 3 | Query cancel / budget | `case_query_cancel_budget` | `QueryExecutor::execute_cancelled` / `walk` | not run (101) |
| 4 | Prepare failure (wrong world) | `case_prepare_wrong_world` | `prepare`; ledger `Vacant`; root `.identity()` unchanged | not run (101) |
| 5 | Prepare success does not publish | `case_prepare_does_not_publish` | `prepare`, `canonical_fingerprint`, `ReceiptLedger::lookup` InFlight | not run (101) |
| 6 | Commit atomic old-or-new + duplicate receipt | `case_commit_atomic_duplicate` | `commit`; duplicate receipt; conflict fingerprint `RevisionConflict` | not run (101) |
| 7 | Dual VoxelWorld isolation + target fault on A | `case_dual_world_fault_isolation` | `VoxelWorld::create` ×2; `WorldCommand` lifecycle via endpoint; `reject_forbidden(ForbiddenWork::Io)`; `WorldFaultPort::trip`; `WorldShutdown::{begin,drain,finalize}` | not run (101) |
| 8 | CaptureCut then encode outside barrier | `case_capture_encode_outside_barrier` | `capture` + `encode_capture` after barrier | not run (101) |
| 9 | Restore preflight reject truncated; success swap | `case_restore_preflight_and_swap` | `RestorePreflight::validate` + `RestoreShadowBuilder::build` + `restore` | not run (101) |
| 10 | DurabilityAck covers latest; old ack no-op | `case_durability_ack_covers_latest` | `apply_durability_ack` with `DurabilityAckEvidence`; `covered_by` / `except_covered` | not run (101) |
| 11 | Port adapter query/mutate/capture routes | `case_port_adapter_routes` | `GeneratedVoxelWorldPortAdapter` query / prepare / commit / capture | not run (101) |
| 12 | FaultInjector PrePublication recoverable vs PostPublication not | `case_fault_injector_recoverable` | `FaultInjector::recoverable` | not run (101) |

`run_b2_matrix()` fills `B2VerificationReport { baseline, commit, cases }`. `baseline` is generated `BASELINE_ID`. `commit` is `git rev-parse HEAD` at call time (not executed here).

## 3. Cargo test stderr (verbatim, last run)

```text
error: linker `link.exe` not found
  |
  = note: program not found

note: the msvc targets depend on the msvc linker but `link.exe` was not found

note: please ensure that Visual Studio 2017 or later, or Build Tools for Visual Studio were installed with the Visual C++ option

note: VS Code is a different product, and is not sufficient

error: could not compile `lumio-voxel-test-support` (test "b2_transaction_recovery") due to 1 previous error
```

Exit: **101**. Zero tests executed. Do not treat commands 1–3 as a substitute PASS.

## 4. Diff boundary

Exclusive / allowed files:

- `crates/lumio-voxel-test-support/src/b2_harness.rs` (create)
- `crates/lumio-voxel-test-support/tests/b2_transaction_recovery.rs` (create)
- `docs/evidence/b2-verification.md` (create)
- `crates/lumio-voxel-test-support/src/lib.rs` — `pub mod b2_harness;`

No production crate `src/` edits (`contracts` / `domain` / `ops` / `world` / `project` / `migration`). `Cargo.toml` not edited.

## 5. Concerns

- Matrix assertions have not run on this host. Type-check + clippy are not evidence that concurrent-looking isolation, restore swap, or durability coverage passed.
- Prerequisite cards were `in_review` on the card face; this harness calls their shipped APIs in-tree.
- `cargo test` exit 101 is the MSVC linker gap, identical to B0. It is not a matrix PASS.
