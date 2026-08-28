# MvpReviewReport — R-00203

- Reviewer: independent reviewer (Claude Opus 5). Did **not** implement R-00143 / R-00145 / R-00146, did not
  author any reviewed commit, and did not participate in the previous review round.
- Baseline: `LGE-V1.4-2026-08-27`, `schemaEpoch = 1`
- Repo / branch: `LumioVoxelEngine` @ `main`
- Reviewed HEAD: **`4ced801`** (`Merge pull request #3 from LumioGames/fix/mirror-sha256-k28`)
  - `origin/main` = `4ced801` = local HEAD. Working tree clean at review open and at review close; the
    review target did **not** mutate during this round.
- Toolchain: `cargo +1.98.0-aarch64-apple-darwin` — `cargo 1.98.0 (797e8a9bc 2026-08-05)`,
  `rustc 1.98.0 (88d9e12ae 2026-08-18)`, host `aarch64-apple-darwin`. (rustup default on this machine is
  `x86_64-apple-darwin` under Rosetta and must not be used.)
- Upstream architecture, as consumed by the mirror: `LumioGameEngineArchitecture` @ **`bcc8eb9`**
  (`fix(contract-runtime): correct SHA-256 round constant K[28]`).
- Round: **re-review**. Previous verdict was RETURN (7 × P0/P1, 5 × MEDIUM, 3 × LOW).
- Verdict: **RETURN**

Nothing below is taken on the implementer's word. Every claim was re-derived from a command this reviewer
ran, or from an independent reimplementation of the algorithm in question. Two claims that looked like
findings on first measurement were **withdrawn** after re-verification (see §7); they are recorded there
rather than silently dropped.

---

## 1. Scope

| Card | Deliverable reviewed | Owner files |
| --- | --- | --- |
| R-00143 | B0 10-row matrix | `b0_harness.rs`, `tests/b0_contract_domain.rs`, `docs/evidence/b0-verification.md` |
| R-00145 | B2 12-row matrix | `b2_harness.rs`, `tests/b2_transaction_recovery.rs`, `docs/evidence/b2-verification.md` |
| R-00146 | MVP 10-step vertical slice | `mvp_harness.rs`, `tests/mvp_vertical_slice.rs`, `docs/evidence/mvp-integration.md` |

Also in scope because this wave changed them: the re-mirrored generated tree
(`crates/lumio-voxel-contracts/generated/`, 52 → 58 files), `tools/architecture/generated-lock.json`,
the `#[allow(dead_code)]` vendoring seam in `crates/lumio-voxel-contracts/src/lib.rs`, ADR 0008,
`.spec/knowledge/lessons.md`, and the artifact-gate record
`docs/evidence/v1.4-generated-artifact-gate.md` that the re-mirror invalidated.

Checklist dimensions applied: acceptance criteria, correctness, security, guardrails/standards, tests,
commit hygiene, sedimentation, plus the card's own dimensions (7-crate DAG, ownership, linearization,
dual-world isolation, error mapping, artifact provenance).

---

## 2. Verdict rationale

**RETURN.** The code is in good shape; the *evidence* is not.

Every technical blocker from the previous round is genuinely closed, and I verified each one independently
rather than accepting it (§3, §5). The workspace is green (158/158, 0 ignored), the mirror is byte-for-byte
reproducible from its upstream commit, and both Rust SHA-256 tables now match FIPS 180-4 exactly.

RETURN rests on two grounds, either sufficient:

1. **F-P0-1.** The three evidence documents that *are* the R-00143 / R-00145 / R-00146 deliverables are
   comprehensively false at HEAD. They still record measurement commit `80a80c9`, exit code 101,
   `153 passed / 5 failed`, a BLOCKED cluster A, a red `origin/main`, and card verdicts of
   BLOCKED / conditional. On their own face, no card meets its four acceptance criteria. This is the same
   defect class as last round's F-P1-3, recurring after it was fixed — and it is exactly what the first
   `lessons.md` entry was opened to prevent.
2. **F-P1-1.** The generated-artifact set actually consumed at HEAD has **no** Architecture Gate record.
   `docs/evidence/v1.4-generated-artifact-gate.md` — which declares itself the sole gate owner file — still
   attests a different artifact generation (`compilerHash 99a786e7…`, 6 of 12 `outputHash` values stale) and
   still says `ready: true` about it.

**F-P0-3 from the previous round (R-00204 QA not executed) is explicitly NOT carried forward as a RETURN
ground.** `docs/evidence/qa/mvp-release-gate.md` states R-00204's own precondition: *「先验证 REV-MVP=APPROVE
且无未关闭 P0/P1 finding；否则不执行放行，直接 BLOCKED」*. Holding R-00203 open because R-00204 has not run,
when R-00204 cannot run until R-00203 approves, is a deadlock. R-00204 is a separate downstream card; its
non-execution is not a defect in this delivery. Treated as a scope note (§8), not a finding.

---

## 3. Status of the previous round's seven P0/P1 findings

| Prev. finding | Status | How verified |
| --- | --- | --- |
| **F-P0-1** generated Rust `sha256` `K[28]` wrong; Rust≠C# | **CLOSED** | Re-derived all 64 FIPS 180-4 constants from cube roots of the first 64 primes and compared both K tables in the repo: generated mirror **0 mismatches**, `generated_clean.rs` **0 mismatches**. Independently recomputed all 12 descriptor `outputHash` values with Python `hashlib`: **12 OK / 0 BAD**. The Rust `artifact_hashes` target passes over the same bytes, so the two independent implementations now agree — which is what proves the hasher, not just the data. |
| **F-P0-2** three cards, four criteria unmet | **NOT CLOSED** | Technically resolved for the hash blocker, but the criteria as recorded still read FAIL / PARTIAL / FAIL, criterion "replayable on the recorded commit" is now false in a *new* way, and R-00146 criterion 2 is unchanged. See **F-P0-1** and **F-P1-2** below. |
| **F-P0-3** R-00204 QA not executed | **NOT A FINDING against this card** | Circular precondition — see §2. Reclassified to §8. |
| **F-P1-1** review target mutated and committed mid-review | **CLOSED for this round** | `git rev-parse HEAD` = `4ced801` and `git status --porcelain` empty at review open and close. No mutation. (A related process note about *who* merged is in §8.) |
| **F-P1-2** `origin/main` is red | **CLOSED** | `origin/main` = `4ced801` = HEAD, and the full suite at that commit exits 0 with 158/158. |
| **F-P1-3** evidence documents assert falsifiable facts that are false | **NOT CLOSED — recurred** | `85df75e` corrected the two named statements at 18:03. `51c2836` + `e26d819` then merged at 20:01 without re-validating, and the documents are now false in ~10 places. Full list in **F-P0-1**. |
| **F-P1-4** two divergent SHA-256 implementations | **CLOSED as to divergence; residual is P2** | Both tables verified 0 mismatches against FIPS 180-4, so they no longer disagree. The duplicate implementation itself survives — `b0-verification.md` §6 committed to deleting it once the generated one was correct; it is now correct and the duplicate is still there (**F-P2-2**). |

---

## 4. Findings

### CRITICAL

#### F-P0-1 — the three delivering cards' evidence documents are false at HEAD, and on their face no card meets its acceptance criteria

`docs/evidence/b0-verification.md`, `docs/evidence/b2-verification.md`, `docs/evidence/mvp-integration.md`.
Responsible: **R-00143 / R-00145 / R-00146**.

These documents are not commentary — they are the cards' deliverables and the object this review is asked
to certify. Every row below is a statement in the document that I disproved by running a command at HEAD.

| Location | Document says | Measured at `4ced801` |
| --- | --- | --- |
| `b0-verification.md:5`, `b2-verification.md:5`, `mvp-integration.md:5` | measurement HEAD `80a80c90f30dc…` on branch `fix/rust-workspace-checks` | HEAD is `4ced801` on `main`; the mirror at `80a80c9` is a different artifact set |
| `b0-verification.md:27`, `b2-verification.md:26`, `mvp-integration.md:31` | `cargo test --workspace --all-features --no-fail-fast` → **101**, 153 passed / 5 failed | **exit 0**, **158 passed / 0 failed / 0 ignored** |
| `b0-verification.md:28` | `b0_contract_domain` → 7 passed / 2 failed | 9 passed / 0 failed |
| `mvp-integration.md:30` | `mvp_vertical_slice` → **101**, 1 passed / 1 failed | exit 0, 2 passed / 0 failed |
| `b0-verification.md:21`, `:45`, `:69` | row 1 FAIL; "**This card is BLOCKED**"; cluster A "BLOCKED — not fixable in this repo" | row 1 passes (`artifact_hashes_verify_ok ... ok`); cluster A resolved upstream and mirrored |
| `b0-verification.md:89` | "all **52** entries of `generated-lock.json`" | **58** entries; I verified all 58 against `hashlib`, 0 mismatches, 0 extra, 0 missing |
| `b0-verification.md:110-112` | upstream fix "in flight but **not yet committed**"; upstream `HEAD` `7f6c0c6` still carries `0xc6eabbdc` | landed upstream as `bcc8eb9`; `git show bcc8eb9:tools/lumio_generate.py` carries `0xc6e00bf3` |
| `b0-verification.md:141-142`, `b2-verification.md:119`, `mvp-integration.md:116` | "`origin/main` is currently **RED** … = `8e10823` … **does not contain** `17ef95c`/`34ffdc1`/`dc6926b`" | `origin/main` = `4ced801`, contains all three, and is green |
| `b0-verification.md:136` | "Verdict for this card: **BLOCKED**" | stale |
| `b2-verification.md:108` | "delivery **conditional**" | stale |
| `mvp-integration.md:107` | "delivery **BLOCKED** on cluster A" | stale |

Consequences for the acceptance criteria, which is why this is P0 and not P1:

- **R-00143 criterion 2** (`b0-verification.md:132`) is recorded **FAIL** ("Rust and C# hashers disagree").
  Substantively now satisfied — but the card face says FAIL.
- **R-00143 criterion 3** (`:133`, "report independently replayable on the recorded commit") is recorded
  PARTIAL and is now false in a *new* direction: replaying at the recorded commit `80a80c9` does not
  reproduce the artifact set at HEAD, and replaying at HEAD contradicts every number in the report. The
  report is currently replayable **nowhere**.
- **R-00143 criterion 4** (`:134`) and **R-00145 criterion 4** (`b2-verification.md:106`) are recorded
  FAIL / PARTIAL on two clauses. The first clause (`cargo test` exits 101) is closed. The second —
  "production code not modified by this test card" — is **not**: the cluster C/D/F changes remain in the
  tree, and `.spec/decisions/` records only the const→static half (ADR 0008). No in-repo record annotates
  the owner cards R-00056 / R-00142 / R-00078 / R-00104.
- **R-00146 criterion 4** (`mvp-integration.md:105`) is recorded **FAIL (explicit block)**.

The remediation is a re-measurement at `4ced801`, not a patch: rerun the commands, replace the numbers,
restate the card verdicts, and re-derive the acceptance table. This is the second consecutive round in which
evidence was validated and then invalidated by commits that landed after validation. `.spec/knowledge/lessons.md`
entry 1 prescribes 「交付前跑 … 并贴真实退出码」; a re-validation step at merge time is what is missing.

### HIGH

#### F-P1-1 — the generated-artifact set consumed at HEAD has no Architecture Gate record

`docs/evidence/v1.4-generated-artifact-gate.md` §1, §3, §7. Responsible: **R-00037 / R-00045**, triggered by
`51c2836`.

That file states at line 7: *"Gate owner file: this document only"* — it is the designated record. §7 asserts
`{"ready": true, "blockingEvidence": []}`. But §3's `ContractArtifactInventoryV1` describes a **different
artifact generation** from the one on disk:

| Field | Gate doc §3 | On disk at HEAD |
| --- | --- | --- |
| `compilerHash` | `99a786e7241d6e86…` | `3a46fc313ecf03ad…` |
| `inputHash` | `84a2b4c80d3d2bc3…` | `3a0436c9b1e48711…` |
| `outputHash` × 12 | 6 match, **6 stale** | — |

Stale: `canonical-serializer-csharp`, `language-binding-rust`, `language-binding-csharp`,
`contract-types-rust`, `contract-types-csharp`, `contract-runtime-rust`.

The consumed set is not a cosmetic bump — it **expands the public contract surface this repo re-exports**:
`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-types/src/lib.rs` gains
`"root-abi-bundle"` and `"canonical-digest-profile"` in `SCHEMA_IDS` (re-exported as
`pub static SCHEMA_IDS` at `crates/lumio-voxel-contracts/src/lib.rs:57` and used by `intern_schema`), plus
`ABI_VERSION` / `ABI_ENTRY_SYMBOL` / `ABI_TYPE_MAPPING` / `AbiTypeMapping`; `lumio-gen-language-binding`
gains `src/root_abi.rs`. `knowledge/standards/repository-architecture.md`「Architecture Gate」and AGENTS.md
「收口门槛」put public-contract changes behind a gate; none was recorded.

Beyond the gate file, **there is no record of the ADR-040 / ADR-041 adoption anywhere in this repo**:
`grep -rn "ADR-040\|ADR-041\|root-abi\|RootAbi\|canonical-digest-profile" .spec/ docs/` (excluding the
read-only architecture mirror) returns **nothing**. Only the commit message of `51c2836` carries it, and
commit messages are not the sedimentation target that AGENTS.md「改完沉淀」names.

**The artifacts themselves are sound — I verified provenance rather than assuming it.** Regenerating from
upstream `bcc8eb9` reproduces the mirror exactly: `compilerHash 3a46fc31…`, `inputHash 3a0436c9…`,
`rootAbi bundleDigest 25e78226…`, `stable outputHash: yes`, and **58 / 58 files byte-identical** to
`crates/lumio-voxel-contracts/generated/`. Upstream `lumio_contract.py validate` at that commit →
`Validated 174 fixture(s), 0 failure(s).` The finding is the **missing gate record**, not bad artifacts.

#### F-P1-2 — R-00146 acceptance criterion 2 remains unproven, and is unaffected by everything this wave fixed

`docs/evidence/mvp-integration.md:103`; `crates/lumio-voxel-test-support/src/reference_harness.rs:23-79`.
Responsible: **R-00146**.

Criterion 2 requires "Native and Reference byte-identical on the same corpus". It is recorded PARTIAL
because the Reference harness cannot observe Native world identity. I confirmed this is still structurally
true at HEAD: `VoxelPortHarness` exposes `new`, `arm`, `execute`, and `snapshot_hash()` — there is no
accessor for world identity, so the alignment covers op sequence and payload only.

This is an open acceptance-criterion gap independent of the SHA-256 cluster, and it was open in the previous
round too. It needs either a card that closes it (give the harness an identity view) or an explicit,
recorded rewrite of the criterion to what the harness can actually prove. Carrying it as a permanently
PARTIAL row is the option that should not be chosen silently.

### MEDIUM

#### F-P2-1 — the `#[allow(dead_code)]` seam is the right call, but it is module-wide and unsedimented

`crates/lumio-voxel-contracts/src/lib.rs:22,26` (`e26d819`). Responsible: **R-00045 / R-00056**.

**On the question the review was asked: the decision is appropriate and does not mask a real problem.**
Established by counterfactual, not argument — I copied the tree to a scratch directory, removed both
attributes, and ran clippy. Every suppressed item is ADR-040 Root ABI surface:
`ABI_VERSION`, `ABI_ENTRY_SYMBOL`, `ABI_SYMBOL_PREFIX`, `ABI_CALLING_CONVENTION`, `ABI_POINTER_WIDTH`,
`ABI_ENDIANNESS`, `ABI_TYPE_MAPPING`, `AbiTypeMapping`, `CALLING_CONVENTION`, `CAPABILITY_BITS`,
`ENTRY_SYMBOL`, `MAX_ALIGNMENT`, `POINTER_BYTES`, `ROOT_HEADER_BYTES`, `SLOT_OFFSETS`, `STRUCT_SIZES`,
`SYMBOL_PREFIX`, `TABLE_HEADER_BYTES`, `TARGET_PROFILE_ID`. Nothing domain-related is hidden. Voxel
legitimately does not consume the Root ABI — `repository-architecture.md`「所有权边界」excludes FFI/process
governance and ADR 0006 forbids an `ffi` crate. The two rejected alternatives are correctly rejected:
re-exporting would grow the public API to silence a lint, and editing generated files is forbidden by
`.spec/rules/system.md`「生成物不得手改」. `#[allow(dead_code)]` is also correctly narrower than `allow(unused)`.

Two residual points:

1. The attribute is **module-scoped**, and `lumio_gen_contract_types` also holds `SCHEMA_IDS`,
   `STABLE_ERROR_IDS`, `MACHINE_IDS`, `CHUNK_PRESENCE`, `Transition`. `dead_code` on that module is exactly
   how the ADR-040 surface announced its arrival in this wave; after this change, a future regeneration that
   adds contract-types items this repo *should* wire up will arrive silently. Worth a compensating check
   (e.g. asserting `SCHEMA_IDS` membership on regeneration) or an explicit note that the signal was traded away.
2. The decision is **not sedimented**. The structurally identical const→static decision got ADR 0008; this
   one got a commit message. AGENTS.md「改完沉淀」+ reviewer checklist item 7.

Minor precision note, same commit: the message states "22 个 dead_code error". The counterfactual produces
**20** errors per compilation unit over **19** unique items.

#### F-P2-2 — the production hasher has no direct known-vector assertion, and the duplicate implementation the docs promised to delete is still present

`crates/lumio-voxel-test-support/tests/generated_clean.rs:4-9` vs
`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/sha256.rs`.
Responsible: **R-00045 / R-00047**.

`sha256_empty_matches_published_digest` asserts `generated_clean::sha256_hex(b"") == e3b0c442…b855` — on the
**test-only** copy. The **production** hasher, re-exported at `crates/lumio-voxel-contracts/src/lib.rs:44`
and used for every identity and receipt in the domain, has no equivalent one-line assertion; the vendored
`generated/rust/lumio-gen-contract-runtime/tests/chain.rs` is included via `#[path]` for `src/lib.rs` only
and is never compiled as a test target. Coverage is not absent — `artifact_hashes` did catch the K[28]
defect, and would catch a recurrence — but the direct guard sits on the copy that was never broken. Adding
the same assertion against `lumio_voxel_contracts::sha256` is a one-liner and closes the exact defect class,
in the spirit of `lessons.md` avoidance point 4.

Separately, `b0-verification.md:157-158` states the hand-written duplicate "should be deleted in favour of
the generated one once the generated one is correct." It is now correct; the duplicate remains.

#### F-P2-3 — cut identity is still derived from Rust `Debug` output

`crates/lumio-voxel-domain/src/publication/root.rs:112-118` — `format!("{directory:?}")`,
`format!("{frontier:?}")`, `format!("{indexes:?}")` are folded into the identity hash. Responsible: **R-00078**.
Pre-existing; **unchanged since the previous round**. A comment was added explaining the intent, but `Debug`
is still not a stable encoding: a derive change, field reorder, or std formatting change silently changes
every identity, and no C# implementation can reproduce it. Now that F-P0-1's root cause established
cross-language hash agreement as a live contract requirement, this is the remaining independent barrier to it.

#### F-P2-4 — the Architecture Gate in CI still gates the previous baseline

`.github/workflows/repository-policy.yml:24-33`. **Unchanged since the previous round.** The job still
asserts `docs/architecture/LumioGameEngine_Architecture_v1.3.md` exists, greps
`# LumioGameEngine V3 (v1.3)` in it, and greps `LGE-V1.3-2026-08-27` in both that file and `README.md`,
while the active baseline is v1.4. I replayed the whole job by hand: every assertion passes — but only
because the v1.3 file was never deleted and `README.md:11` still mentions `LGE-V1.3-2026-08-27` as a
historical source. Real v1.4 coverage comes solely from `sha256sum -c docs/architecture/.baseline.sha256`,
which does pin the v1.4 file (verified OK). A green job here is not evidence about v1.4.

#### F-P2-5 — the three `GateSourceHashes` literals are never recomputed, so none can detect drift

`crates/lumio-voxel-world/tests/world_capture.rs:31-39` and 11 sibling test files, plus
`b0_harness.rs:674`, `b2_harness.rs:861`, `mvp_harness.rs:807`. **Unchanged since the previous round.**

Correcting the previous round's characterisation: `v13_decision_gates_sha256` = `4850057d…ede2` is **not**
decorative — I scanned every file under `docs/` and it is the exact SHA-256 of
`docs/LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md`. The accurate finding is narrower and
applies to all three literals equally (`architecture_mirror_sha256`, `v13_decision_gates_sha256`,
`blueprint_sha256`): each is a hard-coded string that is never recomputed from the file it names, so if any
of those files drifts, no test fails. The neighbouring `voxel_head` literal `b2f0d8a3…` is a real commit
(`feat(R-00047)`) but is many commits stale.

#### F-P2-6 — the QA gate record contains a stale statement

`docs/evidence/qa/mvp-release-gate.md`. Responsible: **R-00204**. The 執行前置 table lists
`R-00146 INT-MVP | backlog | 无端到端垂直链`. R-00146 shipped (`07886b9` / `105ef06`); the slice runs
end-to-end and `mvp_vertical_slice` is green. Listed for accuracy, not as a block — R-00204's own verdict
correctly stays BLOCKED until this review approves.

### LOW

#### F-P3-1 — unknown generated error ids still collapse to `InvalidHandle`

`crates/lumio-voxel-world/src/port/error_mapping.rs:35-36`. **Unchanged.** An error id added upstream but
not added to this table degrades silently instead of failing loudly. Pre-existing, R-00142.

### Closed since the previous round

- **`code_only()` char literals** — `crates/lumio-voxel-domain/tests/chunk_delta_dirty.rs:312-316,76-92`.
  Char and escaped-char literals are now consumed wholesale so a bare `'"'` cannot flip the string state;
  the doc comment records that raw strings remain unmodelled and instructs adding handling before a scanned
  file uses one. Adequate for a LOW.
- **`publish_once`'s `HandleDoubleRelease` branch** —
  `crates/lumio-voxel-domain/src/publication/authority.rs:117-123`. Annotated as defence-in-depth, with the
  pre-reorder reachability (and why it only ever fired wrongly) recorded. Adequate.
- **`const` → `static` has no ADR** — closed by `.spec/decisions/0008-interned-contract-tables-as-static.md`,
  listed in `.spec/decisions/README.md:38`. The ADR correctly records the breaking-change surface,
  the `publish = false` blast-radius bound, and the "must supersede, must not revert" rule.
- **`lessons.md` empty** — closed by its first entry (「没有链接执行过的验证一律记『未执行』」). Well-scoped,
  with verifiable avoidance steps. Note that F-P0-1 above is a recurrence of the very class it records.

---

## 5. Skips, ignores, weakened assertions, flakiness

**None found.**

- `grep -rn "#\[ignore" crates/ tools/` → no matches. No `should_panic`, no `todo!()`, no `unimplemented!()`
  anywhere under `crates/`.
- All **46** `test result` lines in the full run report `0 ignored; 0 filtered out`.
- `--test-threads=1` produces the identical 158 / 0 / 0, so nothing is order- or parallelism-dependent.
- 60 repeated concurrency runs (30 + 30) produced 0 failures.
- The only `#[allow]` added this wave is the `dead_code` seam, assessed in F-P2-1; the other `#[allow]`s
  under `crates/` (`revision/allocator.rs:3`, `mutation/prepared_token.rs`) pre-date this wave.

### Independent replays required by the card

| Replay | Command | Result |
| --- | --- | --- |
| Concurrency schedule | `test -p lumio-voxel-domain --test publication_atomicity -- concurrent_captures_see_complete_old_or_complete_new --exact` ×30 | **30/30 pass, 0 failures** |
| Concurrency (commit serialization) | `test -p lumio-voxel-ops --test mutation_commit -- same_world_mutation_commits_serialize_on_new_identity --exact` ×30 | **30/30 pass, 0 failures** |
| Prepare fault | `test -p lumio-voxel-ops --test mutation_prepare --all-features` | exit 0, **4/4** — incl. `failed_precondition_wrong_world_leaves_ledger_vacant_and_root_unchanged`, `failed_stage_invalid_chunk_id_aborts_without_publish` |
| Restore corrupt fixture | `test -p lumio-voxel-world --test world_restore --all-features` | exit 0, **7/7** — incl. `preflight_rejects_truncated_empty_and_wrong_world_without_touching_world`, `preflight_rejects_bad_schema_epoch_and_config_hash` |

---

## 6. Evidence — commands this reviewer actually ran

All at HEAD `4ced801` unless noted.

| # | Command | Exit | Key output |
| --- | --- | ---: | --- |
| 1 | `cargo +1.98.0-aarch64-apple-darwin test --workspace --all-features --no-fail-fast` | **0** | **158 passed / 0 failed / 0 ignored** across 46 targets |
| 2 | `cargo +… test --workspace --all-features --no-fail-fast -- --test-threads=1` | **0** | 158 / 0 / 0 — identical |
| 3 | `cargo +… fmt --all -- --check` | 0 | no output |
| 4 | `cargo +… clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 0 errors, 0 warnings |
| 5 | `cargo +… check --workspace --no-default-features` | 0 | clean |
| 6 | `cargo +… check-crate-dag` | 0 | `check-crate-dag OK: 7 crates` |
| 7 | `cargo +… check-generated-clean` | 0 | `check-generated-clean OK` |
| 8 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 9 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | `pass 13`, `fail 0` |
| 10 | `python3 tools/architecture/check_crate_dag.py` | 0 | `OK: 7 crates` |
| 11 | `python3 tools/architecture/check_generated_clean.py` | 0 | `check-generated-clean OK` |
| 12 | `python3 tools/architecture/test_guards.py` | 0 | `ALL_PASS`, incl. `cargo metadata seven members` |
| 13 | `shasum -a 256 -c docs/architecture/.baseline.sha256` | 0 | v1.4 mirror `OK` |
| 14 | `repository-policy.yml` readme job, replayed assertion by assertion | 0 | all pass — but see F-P2-4 |
| 15 | Python: derive all 64 FIPS 180-4 `K` from cube roots; compare both repo tables | — | generated mirror **0 mismatches**; `generated_clean.rs` **0 mismatches** |
| 16 | Python `hashlib`: recompute all 12 descriptor `outputHash` | — | **12 OK / 0 BAD**; every `baselineId = LGE-V1.4-2026-08-27`, `schemaEpoch = 1` |
| 17 | Python `hashlib`: verify all `generated-lock.json` entries | — | **58 OK / 0 BAD**; 0 files on disk unlocked, 0 locked files missing |
| 18 | Byte-compare mirror vs `git archive bcc8eb9 packages` (upstream) | — | **58 identical / 0 differ**; only `.gitignore`, `README.md`, `rust/Cargo.toml`, `rust/Cargo.lock` unmirrored — the four this mirror has always excluded |
| 19 | `python3.11 tools/lumio_contract.py generate` from a clean `bcc8eb9` archive | 0 | `compilerHash 3a46fc31…`, `inputHash 3a0436c9…`, `rootAbi bundleDigest 25e78226…`, `stable outputHash: yes`; regenerated tree **58/58 byte-identical** to the mirror |
| 20 | `python3.11 tools/lumio_contract.py validate` on `bcc8eb9` | 0 | `Validated 174 fixture(s), 0 failure(s).` |
| 21 | Counterfactual: strip both `#[allow(dead_code)]`, rerun clippy in a scratch copy | **101** | 20 errors / unit, 19 unique items, **all** ADR-040 Root ABI (F-P2-1) |
| 22 | Concurrency replays ×30 + ×30 | 0 | 0 failures |
| 23 | `test -p lumio-voxel-ops --test mutation_prepare --all-features` | 0 | 4/4 |
| 24 | `test -p lumio-voxel-world --test world_restore --all-features` | 0 | 7/7 |
| 25 | Python: compare `v1.4-generated-artifact-gate.md` §3 against live descriptors | — | **6 of 12 `outputHash` stale**, `compilerHash` stale, `inputHash` stale (F-P1-1) |
| 26 | `grep -rn "#\[ignore" crates/ tools/`; scan all `test result` lines | — | no matches; 46/46 report `0 ignored; 0 filtered out` |
| 27 | Scan every file under `docs/` for SHA-256 `4850057d…ede2` | — | matches `docs/LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md` (F-P2-5) |

### Verification of the delivery's headline claims

| Claim | Verdict |
| --- | --- |
| `test --workspace --all-features --no-fail-fast` → exit 0, 158 passed / 0 failed | **CONFIRMED** |
| `-- --test-threads=1` → same 158 / 0 | **CONFIRMED** |
| `fmt` 0 / `clippy` 0 / `check --no-default-features` 0 | **CONFIRMED** |
| `spec-lint` 0; `spec-lint.test` 13 pass | **CONFIRMED** |
| `check-crate-dag` / `check-generated-clean` OK on both the cargo and python paths | **CONFIRMED** (4 commands) |
| `test_guards.py` → `ALL_PASS` | **CONFIRMED** |
| `artifact_hashes` 3 passed / 0 failed; `generated_clean` 4 passed / 0 failed | **CONFIRMED** |
| upstream fixed `K[28]` and republished; mirror synced 52 → 58 files | **CONFIRMED** — both K tables now 0 mismatches vs FIPS 180-4 |
| the mirror is byte-identical to upstream, nothing hand-edited | **CONFIRMED** — 58/58 vs `bcc8eb9`, and 58/58 vs an independent regeneration |
| BaselineId unchanged at `LGE-V1.4-2026-08-27` | **CONFIRMED** — on all 12 descriptors, `schemaEpoch` 1 |
| the six new files are required for the declared `outputHash` to reproduce | **PARTLY CONFIRMED** — true for the three inside package directories (`RootAbi.cs`, `root_abi.rs`, `CanonicalProfile.cs`); `abi/lumio_core.h`, `abi/root-abi-bundle.json`, `canonical/canonical-digest-profile.json` sit outside every package dir and do not enter any `outputHash`. Mirroring them is still right (full-fidelity mirror, and `generated-lock.json` covers all 58), but the stated reason does not apply to them. |
| `51c2836`'s stated precondition — land only after the upstream commit reaches architecture `origin/main` | **CONFIRMED SATISFIED** — the fix reached architecture `origin/main` as `bcc8eb9`; the mirror matches it exactly |
| `#[allow(dead_code)]` is the least-bad option and hides nothing real | **CONFIRMED** by counterfactual — all 19 suppressed items are ADR-040 ABI surface (F-P2-1 records two residuals) |
| evidence documents are current | **REFUTED** — false in ~10 places (F-P0-1) |
| the artifact set consumed at HEAD is gated | **REFUTED** — gate record describes a different generation (F-P1-1) |

---

## 7. Withdrawn findings (recorded, not silently dropped)

Two candidate findings were retracted after re-verification. Both are logged because the *reason* they were
wrong matters for the next round.

1. **"The published upstream artifacts are not reproducible from upstream source."** First measurement
   showed `abi/root-abi-bundle.json`, `canonical/canonical-digest-profile.json` and
   `csharp/…/CanonicalProfile.cs` differing on regeneration, with `compilerHash` mismatching. Cause: the
   `LumioGameEngineArchitecture` working copy **moved during this review** (`origin/main` went
   `bcc8eb9` → `b8f8c50`, local `HEAD` `6ac5266` → `7bdad78`) and I had regenerated with a newer generator
   than the one that published the mirror. Re-run from a clean `git archive bcc8eb9`, the mirror reproduces
   **58/58 byte-identical** with matching `compilerHash` / `inputHash` / `bundleDigest`. **No defect.**
2. **"`v13_decision_gates_sha256` is a decorative literal matching nothing."** Carried over from the
   previous round. The value is the real SHA-256 of `DECISION_GATES.md`. The surviving, narrower finding is
   F-P2-5 (never recomputed, therefore cannot detect drift).

The first item is also a standing hazard for anyone re-measuring: **pin the upstream commit and regenerate
from an archive**, never from a live sibling checkout.

---

## 8. Scope concerns for the main loop

- **R-00204 is deadlocked against this card.** `docs/evidence/qa/mvp-release-gate.md` makes
  `REV-MVP = APPROVE` its execution precondition, so R-00204 cannot produce evidence while R-00203 returns.
  Do not carry "QA not executed" as a finding against R-00203 in future rounds; it is a sequencing fact.
  Once F-P0-1 and F-P1-1 close, R-00203 approves and R-00204 becomes runnable.
- **Both PRs were merged to `main` before this review returned a verdict.** `eb611b5` (PR #2, 20:01:02) and
  `4ced801` (PR #3, 20:01:53) landed while the standing verdict was RETURN. I checked authorship before
  raising this as a guardrail breach and it is **not** one: `gh pr view` reports both were merged by the
  human account `Go1c` (`is_bot: false`), so this was the user's call, not an agent bypassing
  `.spec/rules/system.md`「高风险改动在 reviewer 通过前不得提交」. Noted only because it means `main` currently
  carries changes this review is returning, so the fixes for F-P0-1 / F-P1-1 land on top rather than in a
  pre-merge branch.
- **`.spec/tasks/` holds only `README.md`.** The acceptance criteria and the "owner cards need annotating"
  gap live in an external task system, so I could verify neither in-repo. F-P0-1's criterion-4 clause and
  the R-00056 / R-00142 / R-00078 / R-00104 annotations need confirming there.
- **F-P2-3 (identity from `Debug`) and F-P2-4 (CI gating v1.3) are pre-existing design issues** outside this
  wave, both open across two rounds. They probably want their own cards rather than a patch here — F-P2-3 in
  particular now bears directly on the cross-language reproducibility that F-P0-1's root cause exposed.

## 9. Required before re-review

1. **F-P0-1** — re-measure `b0-verification.md`, `b2-verification.md`, `mvp-integration.md` at `4ced801`:
   real commands, real exit codes, real counts, restated card verdicts, and a re-derived acceptance table.
   Add a re-validation step so evidence cannot be invalidated by a later merge without being rerun.
2. **F-P1-1** — record an Architecture Gate result for the artifact set actually consumed
   (`compilerHash 3a46fc31…`, `inputHash 3a0436c9…`, the 12 current `outputHash` values), and sediment the
   ADR-040 / ADR-041 adoption — the contract surface this repo re-exports changed and nothing in `.spec/`
   or `docs/` says so.
3. **F-P1-2** — close R-00146 criterion 2 or rewrite it, explicitly, to what the Reference harness can prove.
4. **F-P2-1 / F-P2-2** — sediment the `allow(dead_code)` decision the way ADR 0008 sediments const→static;
   add a known-vector assertion against the production `sha256`; delete the duplicate hand-written hasher
   as `b0-verification.md` §6 undertook to.
5. **F-P2-3 / F-P2-4 / F-P2-5 / F-P3-1** — open cards; not blocking this delivery.
