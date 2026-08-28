# MvpReviewReport — R-00203

- Reviewer: independent reviewer (Claude Opus 5). Did **not** implement R-00143 / R-00145 / R-00146 and did
  not author any of the reviewed commits.
- Baseline: `LGE-V1.4-2026-08-27`
- Repo / branch: `LumioVoxelEngine` @ `fix/rust-workspace-checks`
- Reviewed HEAD: **`dc6926b`** (`docs(evidence): B0/B2/MVP 证据改写为 linked 口径,补记 VOX-D P0 裁决`)
  - Review opened against working-tree state on `80a80c9`; that state was committed mid-review as
    `17ef95c` + `34ffdc1` + `dc6926b`. Content verified byte-identical (see F-P1-1).
- Toolchain: `cargo +1.98.0-aarch64-apple-darwin` — `cargo 1.98.0 (797e8a9bc 2026-08-05)`,
  `rustc 1.98.0 (88d9e12ae 2026-08-18)`, host `aarch64-apple-darwin`, LLVM 22.1.8
- Upstream architecture: `LumioGameEngineArchitecture` @ `7f6c0c6`; owner freeze `5f06822` (pushed)
- Verdict: **RETURN**

Nothing in this report is taken on the implementer's word. Every claim below was re-derived from a command
this reviewer ran, or from an independent reimplementation of the algorithm in question.

---

## 1. Scope

| Card | Deliverable reviewed | Owner files |
| --- | --- | --- |
| R-00143 | B0 10-row matrix | `b0_harness.rs`, `tests/b0_contract_domain.rs`, `docs/evidence/b0-verification.md` |
| R-00145 | B2 12-row matrix | `b2_harness.rs`, `tests/b2_transaction_recovery.rs`, `docs/evidence/b2-verification.md` |
| R-00146 | MVP 10-step vertical slice | `mvp_harness.rs`, `tests/mvp_vertical_slice.rs`, `docs/evidence/mvp-integration.md` |

Also in scope, because this wave changed them: four production edits in `lumio-voxel-contracts`,
`lumio-voxel-domain`, `lumio-voxel-ops`; two domain test-guard edits; commits `956da90`, `97c7fd4`,
`17dd13d`, `0f8cf0c`, `80a80c9` from a concurrent actor.

Checklist dimensions applied: acceptance criteria, correctness, guardrails/standards, tests, commit
hygiene, sedimentation, plus the card's own dimensions (7-crate DAG, ownership, linearization, dual-world
isolation, error mapping).

---

## 2. Verdict rationale

**RETURN.** Three independent grounds, any one sufficient:

1. `cargo test --workspace --all-features` exits **101** (153 passed / 5 failed) at HEAD. A red gate is not
   an APPROVE regardless of who owns the root cause.
2. The delivering cards' own evidence documents record **FAIL** on their acceptance criteria:
   R-00143 criteria 2 and 4 FAIL; R-00146 criterion 4 FAIL, criterion 2 PARTIAL; R-00145 criterion 4
   PARTIAL. A card that self-reports FAIL cannot be approved.
3. R-00204 QA release gate is **BLOCKED** (`docs/evidence/qa/mvp-release-gate.md`) — the MVP has no QA
   execution evidence at all.

---

## 3. Findings

### CRITICAL

#### F-P0-1 — the generated Rust `sha256` is not SHA-256; every production digest in this repo is wrong

`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/sha256.rs:8`
Responsible: **upstream** `LumioGameEngineArchitecture` generator; mirrored under R-00045; surfaced by
R-00143 row 1.

Round constant `K[28] = 0xc6eabbdc`. FIPS 180-4 requires `0xc6e00bf3`. Verified two independent ways:

- Extracted both 64-entry `K` tables from the repo with a script and compared against constants derived
  from first principles (fractional parts of cube roots of the first 64 primes). Result:
  `generated_clean.rs` → 0 mismatches; the generated mirror → exactly one, at index 28.
- Reimplemented SHA-256 in Python twice, once with correct `K`, once with `K[28] = 0xc6eabbdc`:
  `sha256("")` = `e3b0c442…b855` (matches `hashlib`) vs `d86c89fc…57f1`.

Direction of fault — **the artifacts are correct, the hasher is wrong.** I recomputed every generated
package's `outputHash` with Python `hashlib` using the same canonical algorithm as
`lumio-voxel-contracts/src/lib.rs:177-183`: **12 of 12 descriptors match, 0 mismatch.**

Blast radius (this is why it is critical, not merely a red test): the broken `sha256` is re-exported as
public API at `crates/lumio-voxel-contracts/src/lib.rs:36` and is the hash for every identity and receipt
in the domain:

- `lumio-voxel-domain/src/publication/root.rs:122` — `PublishedStateRoot` identity
- `lumio-voxel-ops/src/mutation/commit.rs:86,129` — `receipt_hash`
- `lumio-voxel-ops/src/mutation/fingerprint.rs:42` — txn fingerprint
- `lumio-voxel-ops/src/query/plan.rs:149`, `snapshot/codec_port.rs:159`, `snapshot/restore_shadow.rs:133`
- `lumio-voxel-domain/src/chunk/payload.rs:60`, `chunk/replacement.rs:71`

The C# half of the same generated contract
(`generated/csharp/Lumio.Gen.ContractRuntime/ContractRuntime.cs:16,23`) uses
`System.Security.Cryptography.SHA256.HashData` — the real algorithm. **Two generated implementations of one
hash-chain contract disagree on every input, including the genesis hash.** Any Server/Client pair that
hashes across the language boundary is silently incompatible today.

`.spec/rules/system.md`「生成物不得手改」correctly forbids fixing this in-repo. Upstream fix, regeneration,
re-emission of `outputHash`, and re-lock are required. Blocking.

#### F-P0-2 — no delivering card meets its four acceptance criteria

`docs/evidence/b0-verification.md:121-130`, `docs/evidence/b2-verification.md:99-110`,
`docs/evidence/mvp-integration.md` §4. R-00143 / R-00145 / R-00146.

Verified against raw test output rather than the summary tables. R-00143 criterion 2 FAIL (row 1),
criterion 4 FAIL. R-00146 criterion 2 PARTIAL (Reference harness cannot observe Native world identity, so
"Native and Reference byte-identical" is unproven), criterion 4 FAIL. R-00145 criterion 4 PARTIAL.

#### F-P0-3 — no QA evidence exists for the MVP

`docs/evidence/qa/mvp-release-gate.md`. R-00204.

Verdict BLOCKED; matrix not run; "P0 35 张中仅 R-00002/34/37/41 有执行证据". Unit tests do not substitute for
the QA gate. This is an evidence gap, which the card face makes an automatic RETURN.

### HIGH

#### F-P1-1 — the review target mutated during review, and the change was committed before review closed

Observed timeline (file mtimes and `git log`, all 2026-08-28):

| Time | Event |
| --- | --- |
| 17:38–17:39 | four production + two test files last modified (the state I was briefed on) |
| 17:40:00 | **PR #1 merged to `origin/main`** as `8e10823` |
| 17:43:25 | my first full `cargo test` completes (153/5) |
| 17:43–17:46 | `b0-verification.md`, `b2-verification.md`, `mvp-integration.md`, `VOX-D-001..004` rewritten **while review in flight** |
| ~17:50 | all 13 files committed as `17ef95c`, `34ffdc1`, `dc6926b` |

At session start `git status` listed 6 modified files; by 17:48 it listed 13. I had already read the
pre-rewrite `b0-verification.md` (Windows/type-check revision, `c51e5cd`), so the mutation is directly
attested, not inferred.

AGENTS.md「审查闭环」and rules/system.md「高风险改动在 reviewer 通过前不得提交」were not honoured: production
changes to `contracts`/`domain`/`ops` were committed before the reviewer returned a verdict.

Mitigation confirmed: I diffed `80a80c9..dc6926b -- crates/` against the four reverse patches I captured at
review open — **120 changed lines on both sides, identical when sorted**. My code findings therefore apply
unchanged to HEAD. The process failure stands regardless.

#### F-P1-2 — `origin/main` is red and does not contain the fixes

`origin/main` = `8e10823` (merge of PR #1, authored `Go1c`, 17:40:00), which contains `80a80c9` — a state
that fails 5 tests and carries F-P0-1 — and does **not** contain `17ef95c` / `34ffdc1` / `dc6926b`
(`git branch -r --contains 17ef95c` → empty). Main is now the *worse* of the two states: broken hasher,
and without the three production defect fixes.

#### F-P1-3 — evidence documents assert falsifiable facts that are false at HEAD

`docs/evidence/b0-verification.md` §4 and §6; same claims echoed in `b2-verification.md` §5 and
`mvp-integration.md` §5. R-00143 / R-00145 / R-00146.

Two claims fail verification:

1. *"Root cause is upstream: `tools/lumio_generate.py:168` … The correct constant appears nowhere in that
   repository."* — **False at HEAD.** `LumioGameEngineArchitecture/tools/lumio_generate.py:168` currently
   reads `0xc6e00bf3`, and that repo has 27 modified files including a regenerated
   `packages/rust/lumio-gen-contract-runtime/src/sha256.rs`. The committed upstream HEAD still carries the
   wrong constant (`git show HEAD:tools/lumio_generate.py | sed -n 168p` → `0xc6eabbdc`), so the fix is
   **in flight and uncommitted**, not absent. The remediation path must say so; "appears nowhere" invites
   the wrong follow-up.
2. *"`80a80c9` is not on `origin/main` (= `47cbfdd`)."* — **Stale.** `origin/main` = `8e10823` and
   contains `80a80c9`.

Neither is fabrication; both are staleness in a document whose entire purpose is to be the factual record.
Evidence must be re-validated at the moment of delivery.

#### F-P1-4 — two divergent SHA-256 implementations; a CI gate is green over data the shipped hasher cannot verify

`crates/lumio-voxel-test-support/src/generated_clean.rs:120` (correct `K`) vs
`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/sha256.rs:8` (wrong `K`).
Introduced by `0f8cf0c`. R-00045 / R-00047.

**On the specific question put to this review — is `0f8cf0c` "用绿灯掩盖真缺陷"? No.** Evidence against
masking:

- The authoritative tests are still red at HEAD: `published_hashes_match_locked_packages`,
  `tamper_fails_then_restore_passes`, `artifact_hashes_verify_ok`, `run_b0_matrix_covers_ten_rows`,
  `mvp_vertical_slice_step_count_and_dual_instance`.
- `0f8cf0c`'s message explicitly names the generated mirror as still broken and states that
  `artifact_hashes` remains red pending an upstream fix.
- Before the fix, `check-generated-clean` reported all 52 lock entries as mismatched — 52 false positives.
  The lock is correct (I verified 12/12 descriptors under real SHA-256), so the guard, not the data, was
  wrong. Fixing the guard restored true signal and narrowed red to exactly the real defect.

The residual hazard is real, though: `cargo check-generated-clean` now exits 0 while
`verify_artifact_hashes` fails over the same bytes. Anyone reading the CI gate list will see "generated
artifacts intact" while the shipped hasher cannot reproduce a single one of those hashes. b0-verification.md
§4 does carry this caveat; it needs to survive into whatever fixes cluster A.

### MEDIUM

#### F-P2-1 — `const` → `static` is a semver-breaking API change, recorded nowhere

`crates/lumio-voxel-contracts/src/lib.rs:49-51`. R-00056 / R-00142.

The change is **correct and necessary** (see §4), but `pub const` → `pub static` breaks any downstream
consumer using these tables in a const context (`const N: usize = CHUNK_PRESENCE.len();`, array lengths,
const generics) — statics cannot be read from const contexts. It also makes
`lumio_voxel_contracts::SCHEMA_IDS` a different kind of item from `lumio_gen_contract_types::SCHEMA_IDS`,
so any future crate that depends on the generated crate directly reintroduces the same latent
pointer-identity bug. No ADR, no note in `knowledge/`, no upstream report asking the generator to emit
`static`.

#### F-P2-2 — no sedimentation for three production semantic changes

`.spec/decisions/` unchanged; `.spec/knowledge/lessons.md` still "（暂无）"; `git status .spec/` empty.
AGENTS.md「改完沉淀」+ reviewer checklist item 7.

This wave changed (a) receipt bytes for every new commit, (b) `publish_once` error precedence, (c) a public
item kind. The evidence docs declare the gap but declaring is not sedimenting. `lessons.md` in particular
is the designated home for exactly the recurring failure this wave exposed — "type-check 口径 presented as
verification for months, hiding four latent defects" — and it is still empty.

#### F-P2-3 — cut identity is derived from Rust `Debug` output

`crates/lumio-voxel-domain/src/publication/root.rs:112-116` — `format!("{directory:?}")` and
`format!("{frontier:?}")` are folded into the identity hash. R-00078. Pre-existing, not introduced here.

`Debug` output is not a stable encoding: a derive change, a field reorder, or a std formatting change
silently changes every identity, and no C# implementation can reproduce it. Given F-P0-1 already establishes
that cross-language hash agreement is a contract requirement, this is a second, independent barrier to it.

#### F-P2-4 — the Architecture Gate in CI no longer gates the active baseline

`.github/workflows/repository-policy.yml:24-33` still asserts
`docs/architecture/LumioGameEngine_Architecture_v1.3.md` and greps `LGE-V1.3-2026-08-27`, while the active
baseline is v1.4. I reproduced the job locally: every grep passes only because the v1.3 file was never
removed. Actual v1.4 coverage comes solely from `sha256sum -c docs/architecture/.baseline.sha256`
(which does check v1.4 — verified OK).

#### F-P2-5 — gate-source hashes are decorative and cannot detect drift

`crates/lumio-voxel-world/tests/world_capture.rs:36` and 7 sibling test files, plus
`crates/lumio-voxel-test-support/src/b2_harness.rs:861`: `v13_decision_gates_sha256` is the hard-coded
literal `4850057d…ede2`, never recomputed from any artifact. I confirmed the neighbouring
`architecture_mirror_sha256` and `blueprint_sha256` literals **do** match real files
(`f1d36acf…`, `32e76066…`), which makes the unverifiable third value more misleading, not less. Four
`docs/evidence/decision-gates/*.md` files were edited in `dc6926b` with no hash consequence anywhere.

### LOW

#### F-P3-1 — hand-rolled comment stripper in a source guard

`crates/lumio-voxel-domain/tests/chunk_delta_dirty.rs:44-107`. R-00076.

`code_only()` handles line/block comments and normal string literals but **not** raw strings (`r"…"`,
`r#"…"#`) or char literals. A future `'/'` or `r"a/*b"` in a scanned file would desynchronize the parser
and silently drop real code from the guard's view. Currently latent: I confirmed none of the three scanned
files contains a raw string or a `'/'` literal. The underlying fix is legitimate — the guard was matching
the doc comment at `crates/lumio-voxel-domain/src/chunk/dirty.rs:258` ("Not named clear_dirty"), i.e. prose
documenting compliance.

#### F-P3-2 — `publish_once`'s `HandleDoubleRelease` branch is now unreachable

`crates/lumio-voxel-domain/src/publication/authority.rs:114`. R-00078.

`next_token_id` increases monotonically per authority and is never reset; `PublicationToken` is not `Clone`
and `publish_once` consumes it by value; `seal()` already guards double-seal. After the reordering, no
same-authority token id can reach the `used_ids` check twice, and foreign ids are rejected earlier. The
branch has no test covering it. Either prove a reachable path or record it as defence-in-depth.

#### F-P3-3 — unknown error ids collapse to `InvalidHandle`

`crates/lumio-voxel-world/src/port/error_mapping.rs:35`. R-00142. A generated id added upstream but not
added to this table degrades silently instead of failing loudly.

---

## 4. The four production edits: verdict on "为通过测试而改生产码"

**All four are genuine defect fixes. None weakens an assertion, and none is required by a test written this
wave.** Established by counterfactual, not by argument: I copied the tree to an isolated scratch directory,
reverted each change on its own, and ran the full workspace suite.

| Reverted change | Result | Newly failing tests | Test provenance |
| --- | --- | --- | --- |
| *(none — baseline)* | 153 / 5 | — | — |
| `authority.rs` order | 152 / **6** | `stale_wrong_world_and_double_token_leave_root_hash_unchanged` | `publication_atomicity.rs`, committed `74ca752` (R-00078) — **predates this wave**, unmodified |
| `commit.rs` + `prepared.rs` | 147 / **11** | `commit_is_atomic_and_duplicate_returns_same_receipt`, `capture_then_mutation_keeps_old_cut_identity`, `query_capture_uses_old_or_new_cut_not_mixed`, `same_world_mutation_commits_serialize_on_new_identity`, `port_adapter_query_prepare_commit_capture`, `run_b2_matrix_covers_twelve_rows` | R-00104 / R-00142 / R-00145, all pre-existing |
| `contracts/src/lib.rs` const→static | 150 / **8** | `chunk_presence_is_interned_and_illegal_convert_fails`, `port_adapter_interns_schema_and_binding`, `bindings_and_schema_ids_intern_voxel_world_port`, `run_b0_matrix_covers_ten_rows` | `b0_contract_domain.rs`, committed `72564f8` (R-00143) |
| domain test guards | 151 / **7** | `chunk_delta_dirty_source_has_no_publish_clear_or_fs`, `world_and_chunk_domains_are_independent_and_monotonic` | the two guards being fixed |

The direction is the opposite of test-appeasement: these were latent production defects that the cards'
**own** tests would have caught years earlier had the tests ever linked. The prior Windows host had no
`link.exe`, so every "verification" in this repo's history was a type-check.

Answers to the three questions the card asks specifically:

**`commit.rs` — is the new receipt semantics correct? Any locked fixture broken?**
Correct, and required. `PublishedStateRoot::new` computes identity with `replacement_digest = None`
(`root.rs:41`); `PublicationAuthority::prepare` then calls `incorporate_replacement`
(`authority.rs:83`), which *recomputes* identity with `Some(digest)` (`root.rs:75-82`). `publish_once`
installs the post-`incorporate` root. So the pre-`prepare` value the old code recorded was by construction
never observable by any reader. Worse for R-00146 specifically: `duplicate_receipt` reads
`view.root().identity()` (`commit.rs:135`), the *published* identity — so before the fix the original
receipt and its duplicate replay disagreed on `new_root`, which is exactly the "duplicate TxnId returns the
original receipt" requirement. Yes, receipt bytes change for new commits. No in-repo fixture depends on
them: the only 64-hex literal anywhere under `crates/*/tests` is the architecture mirror hash at
`publication_atomicity.rs:38` (verified to match the real file). Downstream consumers holding pre-fix
receipt bytes would see a difference — flagged in b2-verification.md §5, correctly.

**`authority.rs` — does the reorder change observable semantics? Is the precedence right?**
Yes, it changes semantics, in exactly one situation, and the new behaviour is the correct one. `token.id`
comes from a per-authority counter starting at 0 (`authority.rs:57`), so a *foreign* authority's token
routinely carries an id that collides numerically with a used id here. Under the old order such a token
returned `HandleDoubleRelease` — claiming a token was released twice when it was never published anywhere.
Concretely, in `stale_wrong_world_and_double_token_leave_root_hash_unchanged` both the `world-b/ctx-2/gen 9`
token and the `gen 2` token carry id 0 while `auth.used_ids = {0}`; the old order returned
`HandleDoubleRelease` for both, the test expects `SessionMismatch` and `StaleEpoch`. Identity-before-ledger
is the only ordering under which the ledger is consulted for tokens the ledger can actually describe. No
genuine double-publish is now missed — see F-P3-2.

**`contracts/src/lib.rs` — does const→static change public API semantics or ABI?**
Yes on API (F-P2-1), and the change is necessary. A `const` is inlined at each use site, so
`lumio-voxel-world` and `lumio-voxel-test-support` each materialized their own copy of the table and
`std::ptr::eq` compared distinct allocations. Interning is **production** intent, not a test invention:
`crates/lumio-voxel-world/src/port/adapter.rs:19-46` declares `PortEvidence` as "Interned schema id plus
generated rust binding name" and routes through `intern_schema()` / `intern_binding()`. The counterfactual
failure message is unambiguous: *"PortEvidence.binding_rust_type is not interned BINDINGS"*. Caveat worth
recording: I found no occurrence of "intern"/pointer identity in
`docs/architecture/LumioGameEngine_Architecture_v1.4.md`, so the invariant is repo-local (R-00142), not
traceable to the architecture contract.

---

## 5. Card dimensions checked

| Dimension | Result | How verified |
| --- | --- | --- |
| Seven-crate DAG | **OK** | `cargo check-crate-dag` → `OK: 7 crates`; `python3 tools/architecture/check_crate_dag.py` → 0; `test_guards.py` → `ALL_PASS` incl. `cargo metadata` seven members |
| World/Revision/Chunk ownership | **OK** | only `lumio-voxel-contracts` depends on the generated crates; no `lumio_gen_*` reference in domain/ops/world/test-support |
| Prepare/Commit/Restore/Ack linearization | **OK** | `publish_once_and_finalize` is the sole visible swap (`commit_finalize.rs:17-22`); `mutation_prepare` 4/4, `world_restore` 7/7, `b2_transaction_recovery` 10/10 |
| Dual World, no sharing | **OK** | `two_authorities_do_not_share_root_arc` passes; MVP step 1 detail "identities differ"; b0 row 8 ok |
| Error mapping | **OK, one caveat** | ids interned from `STABLE_ERROR_IDS`; unknown → `InvalidHandle` (F-P3-3) |
| Skips / `#[ignore]` / weakened asserts / flaky | **none found** | every one of 46 `test result` lines reports `0 ignored`; no `#[ignore]` in `crates/`; the two test edits were verified to *replace a self-contradictory assertion* and *remove a self-matching guard*, not to loosen (§4) |
| VOX-D-001..004 blocked→approved doc flip in `dc6926b` | **legitimate** | upstream `5f06822` `VOX-D-P0-OWNER-CONFIRMATION.md` explicitly authorizes `approvalStatus=approved`; commit is pushed to upstream origin |

### Independent replays required by the card

| Replay | Command | Result |
| --- | --- | --- |
| Concurrency schedule | `test -p lumio-voxel-domain --test publication_atomicity -- concurrent_captures_see_complete_old_or_complete_new --exact` ×30 | 30/30 pass, 0 failures |
| Concurrency (commit serialization) | `test -p lumio-voxel-ops --test mutation_commit -- same_world_mutation_commits_serialize_on_new_identity --exact` ×30 | 30/30 pass, 0 failures |
| Prepare fault | `test -p lumio-voxel-ops --test mutation_prepare --all-features` | exit 0, 4/4 — incl. `failed_precondition_wrong_world_leaves_ledger_vacant_and_root_unchanged`, `failed_stage_invalid_chunk_id_aborts_without_publish` |
| Restore corrupt fixture | `test -p lumio-voxel-world --test world_restore --all-features` | exit 0, 7/7 — incl. `preflight_rejects_truncated_empty_and_wrong_world_without_touching_world` |

No flakiness observed in 60 repeated concurrency runs.

---

## 6. Evidence — commands this reviewer actually ran

All at HEAD `dc6926b` unless noted. Full logs retained in the review scratchpad.

| # | Command | Exit | Key output |
| --- | --- | ---: | --- |
| 1 | `cargo +1.98.0-aarch64-apple-darwin fmt --all -- --check` | 0 | clean |
| 2 | `cargo +… clippy --workspace --all-targets --all-features -- -D warnings` | 0 | clean |
| 3 | `cargo +… check --workspace --no-default-features` | 0 | clean |
| 4 | `cargo +… check-crate-dag` | 0 | `check-crate-dag OK: 7 crates` |
| 5 | `cargo +… check-generated-clean` | 0 | `check-generated-clean OK` — see F-P1-4 |
| 6 | `cargo +… test --workspace --all-features --no-fail-fast` | **101** | **153 passed / 5 failed / 0 ignored** |
| 7 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 8 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | pass |
| 9 | `python3 tools/architecture/test_guards.py` | 0 | `ALL_PASS` |
| 10 | `python3 tools/architecture/check_crate_dag.py` | 0 | `OK: 7 crates` |
| 11 | `shasum -a 256 -c docs/architecture/.baseline.sha256` | 0 | `LumioGameEngine_Architecture_v1.4.md: OK` |
| 12 | repository-policy.yml doc assertions, replayed by hand | 0 | all pass — but see F-P2-4 |
| 13 | Python: extract both `K` tables, compare to FIPS 180-4 | — | `generated_clean.rs` 0 mismatches; generated mirror 1 mismatch at index 28 |
| 14 | Python: SHA-256 reimplemented with correct vs broken `K` | — | `e3b0c442…b855` vs `d86c89fc…57f1` |
| 15 | Python: recompute all 12 `outputHash` with `hashlib` | — | **12 OK / 0 BAD** — artifacts correct, hasher wrong |
| 16 | `git show HEAD:tools/lumio_generate.py \| sed -n 168p` (upstream) | 0 | `0xc6eabbdc` committed; working tree has `0xc6e00bf3`, 27 files modified, uncommitted |
| 17 | counterfactual ×4 in an isolated copy (§4) | 101 | 152/6, 147/11, 150/8, 151/7 |
| 18 | concurrency replay ×30 + ×30 | 0 | 0 failures |
| 19 | `test -p lumio-voxel-ops --test mutation_prepare` | 0 | 4/4 |
| 20 | `test -p lumio-voxel-world --test world_restore` | 0 | 7/7 |
| 21 | `test -p lumio-voxel-test-support --test b2_transaction_recovery` | 0 | 10/10, `run_b2_matrix_covers_twelve_rows` ok |
| 22 | `git diff 80a80c9 HEAD -- crates/` vs my open-of-review patches | 0 | 120 changed lines each side, identical |

### Verification of the implementer's headline claims

| Claim | Verdict |
| --- | --- |
| 153 passed / 5 failed | **CONFIRMED** — reproduced 3× (two runs in-repo, one in an isolated copy) |
| all 5 failures share one root cause (K[28]) | **CONFIRMED** — every failure message is `HashMismatch { artifact_id: "canonical-serializer-rust" }` or a gate over it |
| MVP 10 steps all `ok: true` | **CONFIRMED** — read verbatim from the panic payload; only `all_ok()`'s `artifact_hashes` conjunct is false |
| B0 rows 2–10 pass, row 1 fails | **CONFIRMED** — read from `B0CaseResult` array in raw output |
| B2 12/12 rows pass | **CONFIRMED** — independent run, exit 0 |
| lock + descriptors correct, hasher wrong | **CONFIRMED** — 12/12 under `hashlib` |
| C# uses real `SHA256.HashData` | **CONFIRMED** — `ContractRuntime.cs:16,23` |
| no `#[ignore]` / skipped rows / weakened assertions | **CONFIRMED** — 46/46 `test result` lines report `0 ignored` |
| fmt / clippy / no-default-features / spec-lint / guards green | **CONFIRMED** — all exit 0 |
| four production edits are real fixes, not test-appeasement | **CONFIRMED** by counterfactual (§4) |
| `0f8cf0c` masks a defect with a green light | **REFUTED** — the authoritative tests stay red and the commit discloses the mirror; but F-P1-4 records the residual divergent-gate hazard |
| "correct constant appears nowhere in the upstream repository" | **REFUTED** — present in the upstream working tree, uncommitted (F-P1-3) |
| "`80a80c9` is not on `origin/main` (= `47cbfdd`)" | **REFUTED** — `origin/main` = `8e10823`, contains `80a80c9` (F-P1-3) |

---

## 7. Required before re-review

1. **F-P0-1** — upstream `LumioGameEngineArchitecture`: commit the `K[28]` correction, regenerate, re-emit
   every affected `outputHash`, re-lock, re-mirror here. Until then the MVP cannot go green, and the Rust↔C#
   hash chain is incompatible in production.
2. **F-P0-3** — run R-00204 and attach real QA evidence.
3. **F-P1-2** — decide what to do about `origin/main` being red and missing the fixes.
4. **F-P1-3** — correct the two false statements in the evidence documents.
5. **F-P2-1 / F-P2-2** — record an ADR for the `const`→`static` public-API change and the receipt-byte /
   error-precedence changes; open the first `lessons.md` entry for the type-check-as-verification failure.
6. **F-P1-1** — restore the review closure order: no commit of `contracts`/`domain`/`ops` changes before a
   verdict.

## 8. Scope concerns for the main loop

- Three test cards (R-00143/145/146) modified production code owned by R-00056/R-00142, R-00078, R-00104.
  The changes are correct, but the owner cards need annotating and the boundary rule needs an explicit,
  recorded exemption rather than an inline "user-authorized" note.
- F-P2-3 (identity from `Debug` output) and F-P2-5 (unverifiable gate hashes) are pre-existing design
  issues outside this wave. Both bear on the same cross-language reproducibility that F-P0-1 exposes and
  probably want their own cards rather than a patch here.
