# B0 verification matrix — R-00143

- Card: R-00143 [测试·B0]
- Baseline: `LGE-V1.4-2026-08-27`
- Branch / HEAD at measurement: `fix/rust-workspace-checks` @ `80a80c90f30dc1a8dc5769a3993c96c8aa64528f`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host **`aarch64-apple-darwin`** (LLVM 22.1.8)
- Toolchain invocation: `cargo +1.98.0-aarch64-apple-darwin …` (rustup default host on this machine is
  `x86_64-apple-darwin` under Rosetta; the aarch64 toolchain is selected explicitly)
- Harness: `lumio_voxel_test_support::b0_harness::run_b0_matrix` → `B0VerificationReport`
- Tests: `crates/lumio-voxel-test-support/tests/b0_contract_domain.rs`

## 0. What changed since the previous revision of this document

The previous revision recorded a **type-check 口径**: that host had no `link.exe`, every `cargo test`
ended at link with exit 101, and **no runtime assertion in this repository had ever executed**.

This revision is the **first real linked execution**. It is not a re-statement of the old evidence — it
is a different kind of evidence, and it changed the result: 21 assertions across 12 test targets failed
on first execution. Those failures were real defects, not flakiness (identical under `--test-threads=1`).

**Row 1 does not pass. This card is BLOCKED.** See §4.

## 1. Commands actually run

| # | Command | Exit | Reading |
| --- | --- | ---: | --- |
| 1 | `cargo +1.98.0-aarch64-apple-darwin test --workspace --all-features --no-fail-fast` | **101** | 153 passed / **5 failed** — all 5 from one root cause (§4) |
| 2 | `cargo +1.98.0-aarch64-apple-darwin test -p lumio-voxel-test-support --test b0_contract_domain --all-features` | **101** | 7 passed / 2 failed |
| 3 | `cargo +1.98.0-aarch64-apple-darwin test … -- --test-threads=1` | 101 | identical failure set → not parallelism/shared state |
| 4 | `cargo +1.98.0-aarch64-apple-darwin fmt --all -- --check` | 0 | clean |
| 5 | `cargo +1.98.0-aarch64-apple-darwin clippy --workspace --all-targets --all-features -- -D warnings` | 0 | clean |
| 6 | `cargo +1.98.0-aarch64-apple-darwin check --workspace --no-default-features` | 0 | clean |
| 7 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 8 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | 13 tests pass |
| 9 | `cargo +1.98.0-aarch64-apple-darwin check-crate-dag` | 0 | `check-crate-dag OK: 7 crates` |
| 10 | `cargo +1.98.0-aarch64-apple-darwin check-generated-clean` | 0 | `check-generated-clean OK` — **see the caveat in §4** |
| 11 | `python3 tools/architecture/check_crate_dag.py` | 0 | `check-crate-dag OK: 7 crates` |
| 12 | `python3 tools/architecture/check_generated_clean.py` | 0 | `check-generated-clean OK` (hashlib) |
| 13 | `python3 tools/architecture/test_guards.py` | 0 | `ALL_PASS` |

## 2. Matrix rows — executed, not type-checked

| # | Row | Runtime result |
| ---: | --- | --- |
| 1 | Artifact hash lock | **FAIL** — `verify_artifact_hashes: canonical-serializer-rust outputHash does not match package bytes` (§4) |
| 2 | Seven crate DAG | PASS — legal graph empty; extra-crate token rejected |
| 3 | Revision monotonic + abandon hole | PASS — world 0, hole 1, then 2; chunk domain independent |
| 4 | Pin / reclaim | PASS — `from_approved_snapshot`; last drop reclaims slot |
| 5 | Chunk four-state + illegal convert | PASS |
| 6 | `DirtyFrontier::covered_by` is pure | PASS — `except_covered` returns a new frontier |
| 7 | Publication capture old-or-new | PASS — concurrent capture saw complete old or complete new |
| 8 | Dual VoxelWorld independent captures | PASS — Authority/Replica captures stay independent |
| 9 | `GeneratedVoxelWorldPortAdapter` intern | PASS (after the fix in §3) |
| 10 | `DeterministicExecutor` two seeds | PASS — seeds `0xA11CE0` / `0xB05EED` same snapshot; hashmap fold ≠ vec fold |

`b0_contract_domain`: **7 passed, 2 failed** (`artifact_hashes_verify_ok`, `run_b0_matrix_covers_ten_rows`
— both are row 1).

## 3. Defects this card's first linked run exposed, and their disposition

| Cluster | Defect | Disposition | Owner |
| --- | --- | --- | --- |
| B | `crate_dag::live_graph` passed `--workspace` to `cargo tree -p X`, which overrides `-p` and prints every member's tree; each crate received the union of all depth-1 edges, including the impossible self-edge `contracts -> contracts` | fixed (test-only) | R-00047 |
| E | `revision_allocator` asserted `w0==0`, `c0==0`, then `assert_ne!(fw0, fc0)` — self-contradictory, zero detection power | fixed; replaced with assertions that actually prove domain isolation | R-00070 |
| G | source guard `str::contains("clear_dirty")` matched a **doc comment** in `dirty.rs:258` reading *"Not named clear_dirty"* — prose documenting compliance | fixed; guard now strips comments before scanning | R-00076 |
| C | `CHUNK_PRESENCE`/`SCHEMA_IDS`/`BINDINGS` were re-exported `const`, which Rust inlines per crate, so no canonical address exists and `std::ptr::eq` compared distinct per-crate allocations. Two of the three intern assertions had been passing only by luck of literal merging | fixed (production, user-authorized): re-exported as `pub static` | R-00056 / R-00142 |
| F | `publish_once` consulted the used-token ledger **before** validating world/session, so a foreign token whose per-authority id collided numerically returned `HandleDoubleRelease` for a token never published anywhere | fixed (production, user-authorized): identity checks moved first | R-00078 |
| D | `commit.rs` read `new_root.identity()` **before** `prepare()`, which mutates the root via `incorporate_replacement`; the receipt therefore recorded an identity that by construction is never the published one | fixed (production, user-authorized) | R-00104 |
| **A** | **generated SHA-256 round constant `K[28]` is wrong** | **BLOCKED — not fixable in this repo** | upstream |

None of these was fixed by weakening an assertion, by `#[ignore]`, or by re-running until green.

## 4. BLOCKER — row 1 cannot pass: SHA-256 is broken in the generated contract runtime

`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/sha256.rs:8` carries round
constant `K[28] = 0xc6eabbdc`. The FIPS 180-4 value is `0xc6e00bf3`. Every other part of the routine is
correct, so the defect is a single constant — but `K[28]` is used in round 28 of every 64-round block
compression, therefore **every digest the Rust code produces is not SHA-256**:

```
sha256("") with the repo's K : d86c89fc171387b0a8793333e938280743f338afed0655c7b3b5ca75d34957f1
sha256("")真值 (FIPS 180-4)  : e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

The first value reproduces the observed assertion failure bit-for-bit.

Direction of the fault — **the artifacts and the lock are correct; the hasher is wrong**:

- all 52 entries of `tools/architecture/generated-lock.json` match a real SHA-256 of the files on disk
  (0 mismatches);
- all 12 `artifact.descriptor.json` `outputHash` values reproduce under real SHA-256 and under nothing else;
- `python3 tools/architecture/check_generated_clean.py` (uses `hashlib`) exits 0 over the identical bytes.

Blast radius beyond this card:

- The generated crate is compiled into the **production** crate `lumio-voxel-contracts` via `#[path]` and
  re-exported publicly as `sha256` / `sha256_hex` / `hash_chain_verify`. Every production consumer of those
  is emitting non-SHA-256 digests.
- The **C#** side of the same generated contract (`csharp/Lumio.Gen.ContractRuntime/ContractRuntime.cs`)
  uses `System.Security.Cryptography.SHA256.HashData` — the real algorithm. So the two generated
  implementations of one hash-chain contract **disagree on every input, starting from the genesis hash**
  (`HashChain.Sha256(Array.Empty<byte>())`). This is a cross-language contract divergence.
- `git log -S c6eabbdc` shows the constant was wrong from introduction (`c938868` R-00045, `1175b08`
  R-00041). It is not a regression — every artifact-hash "verification" in this repo's history ran against
  a broken hasher.

Root cause is **upstream**: `LumioGameEngineArchitecture/tools/lumio_generate.py:168` (the generator) and
`LumioGameEngineArchitecture/packages/rust/lumio-gen-contract-runtime/src/sha256.rs:8`. The correct
constant appears nowhere in that repository.

Per `.spec/rules/system.md` (**生成物不得手改**) this cannot be fixed here. It requires an upstream
correction, regeneration, re-emission of the affected `outputHash` values, and a re-lock.

**Caveat on command 10.** `cargo check-generated-clean` and `check-generated-clean.py` now exit 0 while
`verify_artifact_hashes` still fails. That is not a contradiction — it is the divergence itself: commit
`0f8cf0c` corrected `K[28]` in the **test-only** copy (`lumio-voxel-test-support/src/generated_clean.rs`)
while the generated production copy stays broken. The repository currently contains two Rust SHA-256
implementations that disagree. **A green `check-generated-clean` must not be read as evidence that the
artifact hash chain verifies.**

## 5. Acceptance criteria

| # | Criterion | Status |
| ---: | --- | --- |
| 1 | B0 report covers artifact / seven crate / Revision / Chunk / Pin-COW / Dirty / single-root publication, no skipped rows | **PASS** — 10 rows executed, none skipped |
| 2 | Reference/Native results, errors, states and hashes agree; expected-failure fixtures stably rejected | **FAIL** — row 1: Rust and C# hashers disagree; artifact hash chain does not verify |
| 3 | Report independently replayable on the recorded commit/artifact/seed/schedule | **PARTIAL** — replays deterministically on `80a80c9`, but that commit is **not pushed to origin** (see §6) |
| 4 | All commands truly succeed and production code not modified by this test card | **FAIL** — `cargo test` exits 101; production code *was* modified under explicit user authorization (clusters C/D/F), which departs from this card's stated boundary |

**Verdict for this card: BLOCKED.** Blocking single item: cluster A, owner = `LumioGameEngineArchitecture`.

## 6. Known gaps

- **Cluster A is unresolved and is the sole remaining test failure.** Requires an upstream fix.
- **Evidence cites unpushed commits.** `80a80c9` and its four predecessors on `fix/rust-workspace-checks`
  are not on `origin/main` (`origin/main` = `47cbfdd`). This violates the standing rule that evidence may
  only cite pushed commits; recorded here explicitly rather than silently.
- **Production code was modified by a test card.** Clusters C/D/F were fixed on explicit user
  authorization, overriding this card's 「不得为通过测试改生产代码」 boundary. The changes are genuine
  defect fixes, not assertion-weakening, but they belong to R-00056/R-00142, R-00078 and R-00104 and
  those cards need annotating.
- **Concurrent writer.** Commits `956da90`, `97c7fd4`, `17dd13d`, `0f8cf0c`, `80a80c9` were authored during
  this session by an actor other than the executing agent; the user confirmed they are theirs and elected
  to keep them. Measurements here were taken after `80a80c9` and re-verified immediately before recording.
- The duplicated hand-written SHA-256 in `lumio-voxel-test-support/src/generated_clean.rs` is a second
  implementation of a generated algorithm, which the card boundary discourages. It should be deleted in
  favour of the generated one once the generated one is correct.
