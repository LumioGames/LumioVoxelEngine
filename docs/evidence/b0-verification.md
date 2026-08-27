# B0 verification matrix — R-00143

- Card: R-00143 [测试·B0]
- Baseline: `LGE-V1.4-2026-08-27`
- Worktree HEAD at measurement: `c51e5cdb6d54fe5b3f427d4db261795053579648`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host `x86_64-pc-windows-msvc`
- Harness: `lumio_voxel_test_support::b0_harness::run_b0_matrix` → `B0VerificationReport`
- Tests: `crates/lumio-voxel-test-support/tests/b0_contract_domain.rs`

This host has **no `link.exe`**. `cargo test` is a linker failure, **not** a matrix PASS. `cargo check` exit 0 is type-check only and is **not** a test PASS.

## 1. Commands actually run

CWD: Voxel worktree. Interpreter for Python: `C:\Users\g923\AppData\Local\Programs\Python\Python312\python.exe`.

| # | Command | Exit | Honest reading |
| --- | --- | ---: | --- |
| 1 | `cargo check -p lumio-voxel-test-support --lib --all-features` | 0 | lib type-checks; **not** a test PASS |
| 2 | `cargo check -p lumio-voxel-test-support --tests --all-features` | 0 | `b0_contract_domain` type-checks; **not** a test PASS |
| 3 | `cargo clippy -p lumio-voxel-test-support --lib --all-features -- -D warnings` | 0 | lib clippy clean |
| 4 | `cargo test -p lumio-voxel-test-support --test b0_contract_domain --all-features -- --nocapture` | **101** | `error: linker \`link.exe\` not found` (MSVC). **No test ran. Not PASS.** |
| 5 | `python.exe tools/architecture/check_crate_dag.py` | 0 | `check-crate-dag OK: 7 crates` |
| 6 | `python.exe tools/architecture/test_guards.py` | 0 | `ALL_PASS` (DAG fixtures + generated-clean + seven `cargo metadata` members) |
| 7 | `git diff --stat` (tracked) | 0 | `Cargo.lock`, `crates/lumio-voxel-test-support/Cargo.toml`, `src/lib.rs` only among tracked files |
| 8 | Extra: `cargo test ... --target x86_64-pc-windows-gnu` | 1 | `rust-lld` missing MinGW libs (`-lkernel32`, `-lmingw32`, …). Still no linked test binary |

`link.exe` is not on PATH. Default host is `x86_64-pc-windows-msvc`.

`git diff --stat` on production crates (`contracts` / `domain` / `ops` / `world` / `project` / `migration`) is empty.

## 2. Matrix rows (no skipped rows)

Runtime column is **not executed**: the only `cargo test` invocation ended at link (exit 101). Each row is a public case function called from `run_b0_matrix()` and from `b0_contract_domain.rs`. The test binary type-checks against the shipped APIs (command 2).

| # | Row | Case | Shipped entry points driven | Runtime |
| ---: | --- | --- | --- | --- |
| 1 | Artifact hash lock | `case_artifact_hash_lock` | `lumio_voxel_contracts::verify_artifact_hashes`; `generated_clean::violations`; `SCHEMA_IDS` / `STABLE_ERROR_IDS` / `BINDINGS` / `CHUNK_PRESENCE` | not run (101) |
| 2 | Seven crate DAG | `case_seven_crate_dag` | `crate_dag::{violations, SEVEN_CRATES}` on `dag-legal.json` (empty) and `dag-forbidden-persistence.json` (forbidden extra crate token) | not run (101); Python DAG check exit 0 |
| 3 | Revision monotonic + abandon hole | `case_revision_monotonic` | `RevisionAllocator` reserve / abandon / finalize; `STABLE_ERROR_IDS` `InvalidHandle` / `HandleDoubleRelease` | not run (101) |
| 4 | Pin / reclaim | `case_pin_reclaim` | `VoxelConfigSnapshot::from_generated`; `PinRegistry::from_approved_snapshot`; clone/drop refcount then reclaim | not run (101) |
| 5 | Chunk four-state + illegal convert | `case_chunk_four_state` | `CHUNK_PRESENCE` intern via `ChunkSlot::presence`; `try_convert` / `ChunkDirectoryBuilder::convert` refuse `Unavailable → Ready` | not run (101) |
| 6 | DirtyFrontier covered_by is pure | `case_dirty_frontier_pure` | `DirtyFrontier::covered_by` + `DurabilityAckEvidence`; does not clear; `except_covered` returns a new frontier | not run (101) |
| 7 | Publication capture old-or-new | `case_publication_old_or_new` | `PublicationAuthority::capture` / `prepare` / `publish_once`; concurrent readers see complete old or complete new identity, never mixed stamp/dir | not run (101) |
| 8 | Dual VoxelWorld instances | `case_dual_voxel_world` | `VoxelWorld::create` ×2; `intern_local_embedded_pair`; independent `publication_authority().capture()` | not run (101) |
| 9 | Port schema / binding intern | `case_port_schema_intern` | `GeneratedVoxelWorldPortAdapter::{schema_id, evidence}` interned from `SCHEMA_IDS` / `BINDINGS` | not run (101) |
| 10 | DeterministicExecutor two seeds | `case_deterministic_executor` | `DeterministicExecutor::run` seeds `0xA11CE0` / `0xB05EED`; same schedule → same trace/snapshot; `hashmap_fold_payloads` ≠ `vec_fold_payloads` | not run (101) |

`run_b0_matrix()` fills `B0VerificationReport { baseline, commit, artifact_ok, dag_ok, cases }`. `baseline` is generated `BASELINE_ID`. `commit` is `git rev-parse HEAD` at call time (not executed here).

## 3. Cargo test stderr (verbatim, last run)

```text
error: linker `link.exe` not found
  |
  = note: program not found

note: the msvc targets depend on the msvc linker but `link.exe` was not found

note: please ensure that Visual Studio 2017 or later, or Build Tools for Visual Studio were installed with the Visual C++ option

note: VS Code is a different product, and is not sufficient

error: could not compile `lumio-voxel-test-support` (test "b0_contract_domain") due to 1 previous error
```

Exit: **101**. Zero tests executed. Do not treat commands 1–3 as a substitute PASS.

## 4. Diff boundary

Exclusive / allowed files:

- `crates/lumio-voxel-test-support/src/b0_harness.rs` (create)
- `crates/lumio-voxel-test-support/tests/b0_contract_domain.rs` (create)
- `docs/evidence/b0-verification.md` (create)
- `crates/lumio-voxel-test-support/src/lib.rs` — `pub mod b0_harness;`
- `crates/lumio-voxel-test-support/Cargo.toml` — path deps `domain`, `ops` (`features = ["snapshot"]`), `world`
- `Cargo.lock` — test-support now lists those three path deps

No production crate `src/` edits (`contracts` / `domain` / `ops` / `world` / `project` / `migration`).

## 5. Concerns

- Matrix assertions have not run on this host. Type-check + clippy are not evidence that concurrent capture, pin reclaim, or hashmap-vs-vec fold passed.
- Prerequisite cards R-00047 / R-00078 / R-00142 were `in_review` on the card face; this harness calls their shipped APIs in-tree.
- `HashMap` fold inequality is order-dependent (same anti-pattern as `tests/harness.rs`); not observed here because the test never linked.
