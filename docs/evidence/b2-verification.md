# B2 verification matrix — R-00145

- Card: R-00145 [测试·B2]
- Baseline: `LGE-V1.4-2026-08-27`
- Branch / HEAD at measurement: `fix/rust-workspace-checks` @ `80a80c90f30dc1a8dc5769a3993c96c8aa64528f`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host **`aarch64-apple-darwin`** (LLVM 22.1.8)
- Toolchain invocation: `cargo +1.98.0-aarch64-apple-darwin …` (rustup default host here is
  `x86_64-apple-darwin` under Rosetta; the aarch64 toolchain is selected explicitly)
- Harness: `lumio_voxel_test_support::b2_harness::run_b2_matrix` → `B2VerificationReport`
- Tests: `crates/lumio-voxel-test-support/tests/b2_transaction_recovery.rs`

## 0. What changed since the previous revision

The previous revision was **type-check 口径** — that host lacked `link.exe`, `cargo test` ended at link
(exit 101), and none of the 12 rows had ever executed. This revision is the **first real linked run**.

On first execution 3 of the 12 rows failed. All three traced to one production defect, now fixed.

**All 12 rows now pass.** `b2_transaction_recovery`: **10 passed, 0 failed.**

## 1. Commands actually run

| # | Command | Exit | Reading |
| --- | --- | ---: | --- |
| 1 | `cargo +1.98.0-aarch64-apple-darwin test -p lumio-voxel-test-support --test b2_transaction_recovery --all-features` | **0** | **10 passed / 0 failed** |
| 2 | `cargo +1.98.0-aarch64-apple-darwin test --workspace --all-features --no-fail-fast` | 101 | 153 passed / 5 failed — none in B2; all 5 are the R-00143 cluster-A blocker |
| 3 | `cargo +1.98.0-aarch64-apple-darwin test … -- --test-threads=1` | 101 | identical set → not parallelism-dependent |
| 4 | `cargo +1.98.0-aarch64-apple-darwin fmt --all -- --check` | 0 | clean |
| 5 | `cargo +1.98.0-aarch64-apple-darwin clippy --workspace --all-targets --all-features -- -D warnings` | 0 | clean |
| 6 | `cargo +1.98.0-aarch64-apple-darwin check --workspace --no-default-features` | 0 | clean |
| 7 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 8 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | 13 tests pass |
| 9 | `cargo +1.98.0-aarch64-apple-darwin check-crate-dag` | 0 | `OK: 7 crates` |
| 10 | `python3 tools/architecture/test_guards.py` | 0 | `ALL_PASS` |

## 2. Matrix rows — executed

| # | Row | Runtime result |
| ---: | --- | --- |
| 1 | Query single-cut + permutation-independent `plan_hash` | PASS — execute binds one cut; stamp mismatch `InvalidHandle` |
| 2 | Query four-state / `NotLoaded` mapping | PASS — absent id → `NotLoaded`; identity unchanged |
| 3 | Query cancel / budget | PASS — `LoaderCancelled`; second walk `BudgetExceeded` |
| 4 | Prepare failure (wrong world) leaves ledger vacant | PASS — `SessionMismatch`; ledger `Vacant`; root identity unchanged |
| 5 | Prepare success does not publish | PASS — reserved `InFlight`, no publish |
| 6 | Commit atomic old-or-new + duplicate receipt | PASS **(was FAIL — see §3)** |
| 7 | Dual VoxelWorld isolation + target fault on A | PASS — `Io` rejected; trip A leaves B identity; shutdown ordered |
| 8 | CaptureCut then encode outside barrier | PASS |
| 9 | Restore preflight rejects truncated; success swaps | PASS — truncated bytes `InvalidHandle`; restore swapped a new identity |
| 10 | DurabilityAck covers latest; old ack no-op | PASS |
| 11 | Port adapter query/mutate/capture routes | PASS **(was FAIL — see §3)** |
| 12 | FaultInjector PrePublication recoverable vs PostPublication not | PASS |

## 3. The defect this card's first linked run exposed

Rows 6 and 11 (and `commit_is_atomic_and_duplicate_returns_same_receipt`,
`port_adapter_query_prepare_commit_capture`) failed with *"commit did not swap to a complete new identity"*.

Root cause — production, in `crates/lumio-voxel-ops/src/mutation/commit.rs`:

`commit` read `new_root.identity()` **before** calling `PublicationAuthority::prepare(..)`. But `prepare`
mutates the root it is handed: `authority.rs` calls `new_root.incorporate_replacement(&replacement)`, and
`PublishedStateRoot::incorporate_replacement` recomputes `identity` with `Some(replacement.digest())` in
place of the `None` (`[0u8;32]`) placeholder used by `PublishedStateRoot::new`. A `ChunkReplacement`
digest is never all-zero, so the recomputed identity **always** differs. The identity `commit` recorded in
`CommitEvidence.new_root` was therefore by construction never the identity `publish_once` made visible.

The publish itself was always correct — the world *did* swap to a complete new cut. Only the receipt's
claim about which cut that was disagreed.

Corroboration that the *test* expectation was right and the code was wrong: the tree's other two
publishers, `world/src/world/restore.rs:76-79` and `world/src/world/durability_ack.rs:110-116`, both read
the new identity as `published.root().identity()` **after** `publish_once`, and their tests (rows 9, 10,
`world_restore`, `durability_ack_apply`) always passed. `commit.rs` was the lone deviation.

The direct unit tests passed only because none of them compared `receipt.evidence.new_root` against the
authority's published identity — `mutation_commit.rs` asserts `auth.capture().root().identity() == hash2`
where `hash2` is itself read from a capture, never from evidence.

Introduced by `9935902` (R-00104), which first wrote `let new_identity = new_root.identity();` ahead of
`prepare`. **Not** caused by `105ef06`, which only reordered the ledger `Duplicate` lookup ahead of
`recheck_prepared`; that commit's dedup key (txn_id + canonical fingerprint) is correct and unrelated.

Fix applied (production, under explicit user authorization — this departs from the card's
「不得为通过测试改生产代码」 boundary, recorded in §5):

- `crates/lumio-voxel-domain/src/publication/prepared.rs` — new `new_root_identity()` accessor returning
  the identity of the bound cut *after* `prepare` folded the replacement digest in.
- `crates/lumio-voxel-ops/src/mutation/commit.rs` — `new_identity` is now read from `publication`
  after `prepare`, before `seal`.

This intentionally changes the receipt bytes of new commits: the receipt now attests the cut that actually
became visible. Ordering discipline is preserved — receipt bytes are still prebuilt before `publish_once`,
and `publish_once` remains the sole visible swap. It also makes the original commit's `evidence.new_root`
agree with the duplicate-replay path, which already read `view.root().identity()`; the two disagreed before.

Two further production defects surfaced by the same run are recorded in `b0-verification.md` §3
(cluster C: `const` re-export defeating pointer interning; cluster F: `publish_once` check order).

## 4. Acceptance criteria

| # | Criterion | Status |
| ---: | --- | --- |
| 1 | B2 report covers all transactions, lifecycle, Capture/Restore/Ack and fault interleavings, no implicit skips | **PASS** — 12 rows executed, none skipped |
| 2 | Prepare failure leaves all state unchanged; Commit all-old/all-new with duplicate returning the original receipt; target fault does not cross World | **PASS** — rows 4, 5, 6, 7 |
| 3 | Native/Reference outcome, error, receipt, snapshot bytes and Trace conform under the generated Port | **PASS** — row 11 |
| 4 | All commands truly succeed, report replayable, test card did not modify production code | **PARTIAL** — B2's own target exits 0; but the workspace run exits 101 on the R-00143 blocker, and production code *was* modified under user authorization |

**Verdict for this card: 12/12 rows PASS; delivery conditional.** B2 itself is green. The workspace is not,
because of cluster A (owner: `LumioGameEngineArchitecture`), and criterion 4 is not cleanly met.

## 5. Known gaps

- **Production code modified by a test card.** The cluster-D fix touches `lumio-voxel-ops` and
  `lumio-voxel-domain`, which belong to R-00104 and R-00078, not to this card. Done on explicit user
  authorization; those owner cards need annotating.
- **Receipt bytes changed for new commits.** No locked fixture depends on them, and
  `mutation_receipt` / `mutation_commit` / `mutation_atomic_batch` stay green — but downstream consumers
  that recorded pre-fix receipt bytes would see a difference.
- **`origin/main` is currently RED.** It is `8e10823` (merge of PR #1), which **contains** `80a80c9` (5
  failing tests, cluster-A P0) and **does not contain** the fixes `17ef95c` / `34ffdc1` / `dc6926b` — the
  cluster-D fix recorded in §3 is therefore *not* on the published branch. An earlier revision said
  `origin/main` = `47cbfdd` with these commits unpushed; that was true when first measured and is now
  false (the merge happened mid-session).
- **Concurrent writer.** Five commits on this branch were authored during the session by another actor;
  the user confirmed and elected to keep them. Measurements were re-verified after `80a80c9`.
- Reference `VoxelPortHarness` still cannot observe Native world identity; alignment remains op
  seq/payload snapshot only.
