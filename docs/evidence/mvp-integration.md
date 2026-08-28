# MVP vertical slice — R-00146

- Card: R-00146 [测试·集成]
- Baseline: `LGE-V1.4-2026-08-27`
- Branch / HEAD at measurement: **`main` @ `4ced801`** — pushed; `origin/main` == `4ced801`
- rustc: `1.98.0 (88d9e12ae 2026-08-18)` host **`aarch64-apple-darwin`** (LLVM 22.1.8)
- Toolchain invocation: `cargo +1.98.0-aarch64-apple-darwin …` (rustup default host here is
  `x86_64-apple-darwin` under Rosetta; the aarch64 toolchain is selected explicitly)
- Harness: `lumio_voxel_test_support::mvp_harness::run_mvp_vertical_slice` → `MvpIntegrationReport`
- Tests: `crates/lumio-voxel-test-support/tests/mvp_vertical_slice.rs`

No P2 streaming. No invented Schema numbers.

## 0. What changed since the previous revision

The previous revision was **type-check 口径** — no `link.exe`, `cargo test` exit 101, none of the 10 steps
had ever executed. This revision is the **first real linked run**.

On first execution step 4 (`commit`) failed, cascading steps 5–10 to *"not reached"*. That was a real
production defect, now fixed.

**All 10 steps execute and pass, and the target itself now passes.** The `artifact_hashes` gate inside
`all_ok()` — the R-00143 cluster-A blocker that previously held this target red — is closed upstream and
mirrored in. Workspace: **158 passed / 0 failed**, exit 0.

## 1. Commands actually run

| # | Command | Exit | Reading |
| --- | --- | ---: | --- |
| 1 | `cargo +1.98.0-aarch64-apple-darwin test -p lumio-voxel-test-support --test mvp_vertical_slice --all-features` | **0** | **2 passed / 0 failed**; all 10 steps `ok: true` |
| 2 | `cargo +1.98.0-aarch64-apple-darwin test --workspace --all-features --no-fail-fast` | **0** | **158 passed / 0 failed** |
| 3 | `cargo +1.98.0-aarch64-apple-darwin test … -- --test-threads=1` | 0 | 158 / 0 (deterministic) |
| 4 | `cargo +1.98.0-aarch64-apple-darwin fmt --all -- --check` | 0 | clean |
| 5 | `cargo +1.98.0-aarch64-apple-darwin clippy --workspace --all-targets --all-features -- -D warnings` | 0 | clean |
| 6 | `cargo +1.98.0-aarch64-apple-darwin check --workspace --no-default-features` | 0 | clean |
| 7 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 8 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | 13 tests pass |
| 9 | `python3 tools/architecture/test_guards.py` | 0 | `ALL_PASS` |

## 2. Named steps — executed

Verbatim `detail` from the executed `MvpIntegrationReport`:

| # | Step | Runtime result |
| ---: | --- | --- |
| 1 | create | PASS — "Authority+Replica create; drive_to_running via adapter.admit; identities differ" |
| 2 | query | PASS — "four-state query via adapter; absent id NotLoaded; no implicit load" |
| 3 | prepare | PASS — "prepare reserved InFlight and did not publish" |
| 4 | commit | PASS — "commit swapped a complete new identity" **(was FAIL — see §3)** |
| 5 | duplicate_replay | PASS — "adapter.commit of the same TxnId returned the original receipt; identity unchanged" |
| 6 | capture | PASS — "capture released CaptureCut and pinned the live identity" |
| 7 | encode | PASS — "encode_capture wrote 461 bytes outside the barrier" |
| 8 | restore | PASS — "restore swapped a new identity (NotLoaded rematerialize)" |
| 9 | durability_ack | PASS — "old ack no-op; covering DurabilityAck clears latest" |
| 10 | close | PASS — "shutdown then stale origin StaleEpoch; replica identity unchanged" |

`authority_identity != replica_identity` holds — the dual instances are independent.

Note that step 5 now genuinely exercises `adapter.commit` on a duplicate TxnId and receives the original
receipt back. The previous revision recorded this as a known gap ("`PreparedMutation` can only move, so the
second call went through `prepare_mutation` instead of a second `commit`"); commit `105ef06` closed that
gap, and this run is the first execution that actually proves it.

## 3. The defects this run exposed, and the gate that used to hold the target red

### 3.1 The step-4 defect (fixed)

Step 4 originally reported *"commit did not publish a new identity"*. Root cause was production, in
`crates/lumio-voxel-ops/src/mutation/commit.rs`: it read `new_root.identity()` **before**
`PublicationAuthority::prepare(..)`, but `prepare` folds the replacement digest into the cut via
`incorporate_replacement`, so the recorded identity could never equal the published one. Full analysis and
the applied fix are in `b2-verification.md` §3. Fixed under explicit user authorization (§5).

### 3.2 The gate that used to hold this target red

`all_ok()` requires `steps.len() == STEP_COUNT`, every step `ok`, `authority_identity !=
replica_identity`, and `!artifact_hashes.starts_with("verify_artifact_hashes:")`. The first three always
held; the fourth failed because `verify_artifact_hashes()` returned
`HashMismatch { artifact_id: "canonical-serializer-rust" }` — the **cluster A** blocker (generated Rust
SHA-256 round constant `K[28]`). That is fixed upstream and mirrored in (`51c2836`, merged as `4ced801`),
so all four conjuncts now hold and the target passes. Full analysis in `b0-verification.md` §4.

## 4. Acceptance criteria

| # | Criterion | Status |
| ---: | --- | --- |
| 1 | Full MVP chain completes on the generated Port; steps, Revision, receipt, snapshot/restore/ack hashes auditable, no internal shortcuts | **PASS** — 10/10 steps executed through `GeneratedVoxelWorldPortAdapter` |
| 2 | Native and Reference byte-identical on the same corpus; races produce only permitted old-or-new traces | **PARTIAL** — Reference-side `DeterministicExecutor` hashes agree across seeds `0xA11CE0`/`0xB05EED`; Reference `VoxelPortHarness` still cannot observe Native world identity, so alignment is op seq/payload only |
| 3 | Dual instances share nothing; Prepare/Commit, Capture/Restore, Dirty Ack and shutdown failure domains satisfy B0/B2 evidence | **PASS** — steps 1, 3–6, 8–10; B2 12/12 rows pass |
| 4 | Full closing commands truly succeed and a replayable report is produced; missing external Artifact/Gate is an explicit block rather than a hand-written substitute | **PASS** — every closing command exits 0 on `4ced801`, which is on `origin/main`; nothing was hand-substituted while the gate was missing |

**Verdict for this card: slice PASS, target PASS, workspace green (158/0).** Criterion 2 remains PARTIAL for a structural reason (see §5), not for a blocker.

## 5. Known gaps

- **Production code modified by a test card.** The step-4 fix touches `lumio-voxel-ops` and
  `lumio-voxel-domain` (owners R-00104, R-00078), departing from this card's
  「不得为通过测试改生产代码」 boundary. Done on explicit user authorization; owner cards need annotating.
- **`origin/main` is green and contains this evidence's measurement point** (`4ced801`). Two earlier
  revisions of this bullet were wrong in opposite directions; both were true when written and were
  overtaken by events.
- **Criterion 2 is structurally unprovable with the current harness.** `reference_harness.rs` exposes only
  `snapshot_hash()` and has no world-identity accessor, so "Native and Reference byte-identical" cannot be
  asserted at all — only op sequence and payload are aligned. This is a harness capability gap, not a
  measurement that failed; closing it needs a Reference-side identity accessor.
- **Concurrent writer.** Five commits on `fix/rust-workspace-checks` were authored during this session by
  an actor other than the executing agent; the user confirmed them and elected to keep them.
- Four-state / post-restore Ready fixtures still go through copied B2 `PublicationAuthority` helpers
  rather than adapter traffic, because adapter mutation can only overlay `Ready` slots.
- `run_b0_matrix` / `run_b2_matrix` are reachable from `matrix_entry_points()` and are now genuinely
  executed by their own targets; this card's test only type-checks the function pointers.
