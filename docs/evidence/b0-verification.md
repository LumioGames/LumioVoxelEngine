# B0 verification matrix — R-00143

- Card: R-00143 [测试·B0]
- Baseline: `LGE-V1.4-2026-08-27` (unchanged), schemaEpoch 1
- Branch / HEAD at measurement: **`main` @ `4ced801`** — pushed; `origin/main` == `4ced801`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host **`aarch64-apple-darwin`** (LLVM 22.1.8)
- Toolchain invocation: `cargo +1.98.0-aarch64-apple-darwin …` — **required**: this machine's rustup
  default host is `x86_64-apple-darwin` under Rosetta, so a plain `cargo` produces x86_64 binaries
- Harness: `lumio_voxel_test_support::b0_harness::run_b0_matrix` → `B0VerificationReport`
- Tests: `crates/lumio-voxel-test-support/tests/b0_contract_domain.rs`

## 0. Result

**All 10 rows pass. The workspace is green: `158 passed / 0 failed`, exit 0.**

Two earlier revisions of this document are superseded:

1. The original recorded a **type-check 口径** — that host had no `link.exe`, every `cargo test` ended at
   link with exit 101, and **no runtime assertion in this repository had ever executed**.
2. The second recorded the **first real linked execution** at `80a80c9`/`85df75e`: 21 failures on first
   run, reduced to 5, with row 1 BLOCKED on a generated-artifact defect.

This revision records the closed state. The blocker is resolved; see §3.

## 1. Commands actually run (on `4ced801`)

| # | Command | Exit | Output |
| --- | --- | ---: | --- |
| 1 | `cargo +1.98.0-aarch64-apple-darwin test --workspace --all-features --no-fail-fast` | **0** | **158 passed / 0 failed** |
| 2 | same, `-- --test-threads=1` | **0** | 158 passed / 0 failed (deterministic) |
| 3 | `cargo … test -p lumio-voxel-test-support --test b0_contract_domain --all-features` | 0 | 9 passed / 0 failed |
| 4 | `cargo … test -p lumio-voxel-contracts --test artifact_hashes` | 0 | 3 passed / 0 failed |
| 5 | `cargo … fmt --all -- --check` | 0 | clean |
| 6 | `cargo … clippy --workspace --all-targets --all-features -- -D warnings` | 0 | clean |
| 7 | `cargo … check --workspace --no-default-features` | 0 | clean |
| 8 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 9 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | pass 13 / fail 0 |
| 10 | `cargo … check-crate-dag` / `cargo … check-generated-clean` | 0 / 0 | `OK: 7 crates` / `OK` |
| 11 | `python3 tools/architecture/{check_crate_dag,check_generated_clean,test_guards}.py` | 0 | `OK` / `OK` / `ALL_PASS` |

## 2. Matrix rows — all executed, all pass

| # | Row | Result |
| ---: | --- | --- |
| 1 | Artifact hash lock | **PASS** — `verify_artifact_hashes` clean over all 12 packages |
| 2 | Seven crate DAG | PASS — legal graph empty; extra-crate token rejected |
| 3 | Revision monotonic + abandon hole | PASS |
| 4 | Pin / reclaim | PASS |
| 5 | Chunk four-state + illegal convert | PASS |
| 6 | `DirtyFrontier::covered_by` is pure | PASS |
| 7 | Publication capture old-or-new | PASS |
| 8 | Dual VoxelWorld independent captures | PASS |
| 9 | `GeneratedVoxelWorldPortAdapter` intern | PASS |
| 10 | `DeterministicExecutor` two seeds | PASS |

No row skipped, no `#[ignore]`, all 46 test targets report `0 ignored; 0 filtered out`.

## 3. Defects this card's linked execution exposed, and their disposition

The first linked run produced 21 failures across 12 targets (identical under `--test-threads=1`, so not
flakiness). Seven root causes, all closed:

| Cluster | Defect | Disposition |
| --- | --- | --- |
| B | `crate_dag::live_graph` passed `--workspace` to `cargo tree -p X`, which overrides `-p` and prints every member's tree, giving each crate the union of all depth-1 edges (incl. the impossible self-edge `contracts -> contracts`) | fixed, `956da90` |
| E | `revision_allocator` asserted `w0==0`, `c0==0`, then `assert_ne!(fw0, fc0)` — self-contradictory, zero detection power | fixed, `34ffdc1` |
| G | source guard `str::contains("clear_dirty")` matched a **doc comment** in `dirty.rs:258` reading *"Not named clear_dirty"* | fixed, `34ffdc1` |
| C | `CHUNK_PRESENCE`/`SCHEMA_IDS`/`BINDINGS` re-exported as `const`, which Rust inlines per crate — no canonical address, so `std::ptr::eq` compared distinct per-crate allocations. Two of three intern assertions had been passing only by luck of literal merging | fixed, `17ef95c`; see [ADR 0008](../../.spec/decisions/0008-interned-contract-tables-as-static.md) |
| F | `publish_once` consulted the used-token ledger **before** validating world/session, so a foreign token with a colliding per-authority id returned `HandleDoubleRelease` for a token never published | fixed, `17ef95c` |
| D | `commit.rs` read `new_root.identity()` **before** `prepare()`, which folds the replacement digest in via `incorporate_replacement`; the receipt recorded an identity that by construction is never published. Introduced by `9935902` (R-00104) | fixed, `17ef95c` |
| **A** | **generated SHA-256 round constant `K[28]` was `0xc6eabbdc`; FIPS 180-4 is `0xc6e00bf3`** | **closed upstream** — see §4 |

None was fixed by weakening an assertion, by `#[ignore]`, or by re-running until green.

## 4. Cluster A — closed

`K[28] = 0xc6eabbdc` made every digest the Rust code produced **not** SHA-256
(`sha256("")` → `d86c89fc…` instead of `e3b0c442…`). The generated crate is compiled into the production
crate `lumio-voxel-contracts` via `#[path]` and re-exported as `sha256` / `sha256_hex` /
`hash_chain_verify`, while the **C#** side of the same generated contract uses
`System.Security.Cryptography.SHA256.HashData` — so the two generated implementations of one hash-chain
contract disagreed on every input, from the genesis hash. `git log -S c6eabbdc` shows the constant was
wrong from introduction (`c938868` R-00045, `1175b08` R-00041): not a regression, and every artifact-hash
"verification" in this repo's history had run against a broken hasher.

Fixed upstream in `LumioGameEngineArchitecture` and mirrored here by `51c2836` (merged as `4ced801`).
Both Rust SHA-256 copies in this repo now carry `0xc6e00bf3` — the generated one and the hand-written
duplicate in `lumio-voxel-test-support/src/generated_clean.rs` — so the two no longer diverge.

Current artifact identity (all 12 descriptors agree):

- `compilerHash` `3a46fc313ecf03ad9f8ca26c4c9267db1fa97048122622b058a1515c8dde9115`
- `inputHash` `3a0436c9b1e48711afc5d48d95a594ebe0ae02eac2c207a32ecd98de6e003dfb`
- `contract-runtime-rust` `outputHash` `6145828934f64ec3b1071cce7b83372f3bc5fc0520aed59b34ca142e1c129985`
- `baselineId` `LGE-V1.4-2026-08-27`, `schemaEpoch` 1

The mirror is **58 files**, and `tools/architecture/generated-lock.json` holds **58 entries** (previously
52). The six added files are upstream artifacts published on the architecture repo's `origin/main`
(ADR-040 Root ABI, ADR-041 canonical/digest profiles). Per
[`workflow.md`](../../.spec/knowledge/standards/workflow.md) — public architecture changes originate
upstream and reach this repo as a read-only mirror — this is mirror synchronisation, not a locally
initiated ABI change. BaselineId is unchanged.

**This consumption did change a public contract surface**: `SCHEMA_IDS` gained `root-abi-bundle` and
`canonical-digest-profile`, and `lumio-voxel-contracts` re-exports `SCHEMA_IDS`. Recorded here rather
than left implicit; see §6.

## 5. Acceptance criteria

| # | Criterion | Status |
| ---: | --- | --- |
| 1 | B0 report covers artifact / seven crate / Revision / Chunk / Pin-COW / Dirty / single-root publication, no skipped rows | **PASS** — 10/10 executed |
| 2 | Reference/Native results, errors, states and hashes agree; expected-failure fixtures stably rejected | **PASS** — row 1 verifies all 12 packages; Rust and C# hashers now agree |
| 3 | Report independently replayable on the recorded commit/artifact/seed/schedule | **PASS** — replays deterministically on `4ced801`, which is on `origin/main` |
| 4 | All commands truly succeed and production code not modified by this test card | **PARTIAL** — all commands exit 0, but production code *was* modified under explicit user authorization (clusters C/D/F); see §6 |

## 6. Known gaps

- **Production code was modified by a test card.** Clusters C/D/F were fixed on explicit user
  authorization, overriding this card's 「不得为通过测试改生产代码」 boundary. Verified as genuine latent
  defects rather than assertion-weakening: an independent reviewer rolled each fix back in an isolated
  copy and confirmed each rollback re-breaks *that card's own* pre-existing tests. Owner cards
  R-00056/R-00142 (C), R-00078 (F), R-00104 (D) need annotating.
- **ADR-040 / ADR-041 consumption is not recorded in a repo decision or in the Architecture Gate doc.**
  `docs/evidence/v1.4-generated-artifact-gate.md` still carries the pre-fix `compilerHash`
  `99a786e7…` and `"ready": true`, and 6 of its 12 `outputHash` values are stale. `SCHEMA_IDS` gained two
  public entries with no corresponding deposit. Both are open.
- **`#[allow(dead_code)]` is applied at module granularity** on the two vendoring seams in
  `lumio-voxel-contracts/src/lib.rs`. Verified by counterfactual that every suppressed item is ADR-040
  Root ABI surface and none is a domain item, but the granularity is coarse and would also mask a future
  genuinely-dead domain item in those modules.
- **The hand-written SHA-256 duplicate in `lumio-voxel-test-support/src/generated_clean.rs` still exists.**
  It is a second implementation of a generated algorithm. Now that the generated one is correct, it should
  be deleted in favour of it. Neither implementation carries a known-answer test — the absence of one is
  precisely why the wrong constant survived from introduction.
- The upstream architecture repo has advanced past the commit this mirror corresponds to; the mirror is
  pinned, not tracking.
