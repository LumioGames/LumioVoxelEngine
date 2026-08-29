# MvpReviewReport — R-00203

- Reviewer: independent reviewer (Claude Fable 5). Did **not** implement any P0 card, did not author any
  reviewed commit, and did not participate in either previous review round.
- Baseline: `LGE-V1.4-2026-08-27`, `schemaEpoch = 1`
- Repo / branch: `LumioVoxelEngine` @ `main`
- Reviewed HEAD: **`0466ffd`** (`Merge pull request #7 from LumioGames/fix/r-00264-policy-v14-baseline`)
  - `origin/main` == `0466ffd` == the reviewed commit. Reviewed in an isolated `git worktree`; working tree
    clean at review open and close; the review target did not mutate during this round.
- Toolchain: `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0 (797e8a9bc 2026-08-05)`,
  host `aarch64-apple-darwin`, macOS.
- Upstream architecture as consumed by the mirror: `LumioGameEngineArchitecture` @ `bcc8eb9`.
  Upstream `origin/main` has since advanced to `3287bba` (PR #27) — a **reporting item**, not a failure of
  this delivery (§8).
- Round: **third review — re-review on the green baseline.** Round 1 RETURN; round 2 RETURN.
- Verdict: **RETURN**

Nothing below is taken on any implementer's word, and nothing is taken on a helper's word either. Every
finding in §5 was re-derived by this reviewer from a command run here or from an independent
reimplementation of the algorithm in question. Three candidate findings were **downgraded or withdrawn**
after that re-derivation; they are recorded in §6 rather than silently dropped.

---

## 0. What this round adds, and why

Round 2's report was returned at QA acceptance (SV-α, 2026-08-29) on one criterion only. Criteria 2, 3 and 4
passed. Criterion 1 — 「审查报告逐张覆盖全部 P0 Requirement、四条验收、完整 diff 与实际证据，无范围遗漏」 —
did not: §1 Scope enumerated the three delivering cards (R-00143 / R-00145 / R-00146) plus the wave's changed
surface, and the whole report referenced only five card numbers. A card-by-card pass over the P0 set was
missing.

This round adds **§4: a pass over all 35 P0 requirements in room RM-00003**, each with a conclusion, its four
acceptance criteria, and evidence anchors. The coverage list comes from the Workflow API — not from this
document's previous table of contents.

That pass is what produced this round's findings. Round 2 verified the three test cards and the wave's
changed files to a high standard; going card-by-card over the 26 implementation cards surfaced a class of
defect neither previous round had reason to look at, because neither previous round was reading the
production code of cards outside the wave. **The green suite did not catch them: the workspace is
158 / 0 / 0 at this HEAD, and the defects in §5 sit in paths those 158 tests do not exercise, or that they
exercise and assert the wrong way round.**

---

## 1. Scope — the full P0 set

**How the coverage list was derived.** `GET /requirements?roomId=<RM-00003>&limit=25`, cursor-paged to
`nextCursor == ""` (3 pages, 55 requirements), filtered `priority == "P0"` → **35 cards**. Cross-checked
against the umbrella card's own statement of blueprint topology: R-00002 says
「本蓝图创建 53 张 Requirement：P0 35、P2 18」, and `docs/evidence/qa/mvp-release-gate.md` independently says
「P0 35 张」. Three independent sources, same 35. The remaining 20 room members are 18 P2 cards plus the two
P1 cards opened later (R-00264, R-00290), outside this card's scope.

The 35 fall into four kinds, reviewed on their own terms:

| Kind | Count | How reviewed |
| --- | ---: | --- |
| Production / test implementation | 26 | Owner files, production code, tests, four criteria |
| Research decision gates | 5 (R-00037, R-00057, R-00058, R-00059, R-00060) | Evidence file, seam, approval reference |
| Umbrella requirement | 1 (R-00002) | Aggregate — satisfied iff its children are |
| Process | 2 (R-00203 this card, R-00204 downstream QA) | §4, §8 |

**Owner-file presence, machine-checked.** For each of the 34 cards with a declared owner-file set (R-00002 has
none), every declared path was tested at `0466ffd`: **34 / 34 cards, all declared files present, 0 missing.**
No P0 card is in the "declared but never created" state the P2 set is in.

**Commit anchors.** For each card the commit introducing its primary owner file was resolved and checked with
`git branch -r --contains` — the criterion round 2 adopted after the `d134046` / `5fb78fa` anchor failures.
**All 34 anchors reachable from `origin/main`; zero unpushed or rewritten anchors.**

**Diff scale reviewed.** Empty tree → `0466ffd` over `crates/`: 184 files / 22 625 lines, of which the
read-only generated mirror is 58 files / 1 281 lines, leaving **126 files / 21 344 lines of first-party code**.
Per crate (excluding the mirror): contracts 402, domain 4 112, ops 5 829, world 5 955, test-support 4 851,
project 5, migration 5. The last two are P2 skeletons and correctly carry no P0 obligation.

---

## 2. Evidence — commands this reviewer actually ran at `0466ffd`

| # | Command | Exit | Key output |
| --- | --- | ---: | --- |
| 1 | `cargo test --workspace --all-features --no-fail-fast` | **0** | **158 passed / 0 failed / 0 ignored** across 46 targets |
| 2 | same, `-- --test-threads=1` | **0** | 158 / 0 / 0 — identical; nothing order- or parallelism-dependent |
| 3 | `cargo fmt --all -- --check` | 0 | clean |
| 4 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 0 errors, 0 warnings |
| 5 | `cargo check --workspace --no-default-features` | 0 | clean |
| 6 | `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` |
| 7 | `node --test .spec/tools/spec-lint.test.mjs` | 0 | `pass 13`, `fail 0` |
| 8 | `python3 tools/architecture/check_crate_dag.py` | 0 | `OK: 7 crates` |
| 9 | `python3 tools/architecture/check_generated_clean.py` | 0 | `OK` |
| 10 | `python3 tools/architecture/test_guards.py` | 0 | `ALL_PASS`, incl. `cargo metadata seven members` |
| 11 | `cargo run -p lumio-voxel-test-support --example check-crate-dag` | 0 | `OK: 7 crates` |
| 12 | `cargo run -p lumio-voxel-test-support --example check-generated-clean` | 0 | `OK` |
| 13 | `shasum -a 256 -c docs/architecture/.baseline.sha256` | 0 | v1.4 mirror `OK` |
| 14 | `repository-policy.yml` readme job, replayed assertion by assertion (17 assertions) | 0 | all pass — now on the **v1.4** baseline (§3, F-P2-4 closed) |
| 15 | Python `hashlib`: recompute every `generated-lock.json` entry | — | **58 OK / 0 BAD / 0 missing**; 0 files on disk unlocked |
| 16 | Python: read the artifact five-tuple off the 12 live descriptors | — | `baselineId` `LGE-V1.4-2026-08-27` ×12, `schemaEpoch` 1 ×12, single `compilerHash 3a46fc31…`, single `inputHash 3a0436c9…` |
| 17 | Python: derive all 64 FIPS 180-4 `K` constants from cube roots of the first 64 primes; compare both repo tables | — | generated mirror **0 mismatches / 64**; `generated_clean.rs` **0 mismatches / 64**; `K[28] = 0xc6e00bf3` in both |
| 18 | `grep -rn "#\[ignore" crates/ tools/` | — | **0 matches**; all 46 `test result` lines report `0 ignored; 0 filtered out` |
| 19 | `unsafe` scan over `crates/` excluding the mirror | — | **0** occurrences outside `#![forbid(unsafe_code)]`, present in **70** files — compiler-enforced |
| 20 | Concurrency replay: `publication_atomicity` concurrent case ×20 | 0 | **20 / 20, 0 failures** |
| 21 | Concurrency replay: `mutation_commit` ×20 | 0 | **20 / 20, 0 failures** |
| 22 | Prepare-fault replay: `mutation_prepare --all-features` | 0 | **4 / 4** |
| 23 | Restore-corruption replay: `world_restore --all-features` | 0 | **7 / 7**, incl. truncated / empty / wrong-world preflight and bad schema epoch |
| 24 | Python: faithful port of `canonical_object_pairs` + `quote`, collision search | — | **collision found** — see F-P0-1 |
| 25 | `git branch -r --contains` on all 34 card anchors | — | **34 / 34 reachable from `origin/main`** |

Rows 20–23 are the independent replays the card requires (one concurrency schedule, one Prepare fault, one
Restore-corruption fixture), run here rather than read off a report.

---

## 3. Status of the previous rounds' findings

| Round-2 finding | Status at `0466ffd` | How verified |
| --- | --- | --- |
| **F-P0-1** three evidence docs false at HEAD | **CLOSED in substance; residual → F-P2-7** | `13d515f` rewrote all three. Every number they now assert reproduces here: 158/0/0, `b0_contract_domain` 9/0, `b2_transaction_recovery` 10/0, `mvp_vertical_slice` 2/0, `artifact_hashes` 3/0, `generated_clean` 4/0. Residual: they pin measurement to `4ced801` and assert 「`origin/main` == `4ced801`」, now false. |
| **F-P1-1** consumed artifact set has no gate record | **PARTIALLY closed — still open, see F-P1-6** | `13d515f` annotated `v1.4-generated-artifact-gate.md` marking `compilerHash 99a786e7…` / `inputHash 84a2b4c8…` / `"ready": true` historical and citing live `3a46fc31…` / `3a0436c9…`; ADR 0009 records the ADR-040/041 adoption. Not closed: §3's inventory body and §7's `"ready": true` still describe the superseded generation. |
| **F-P1-2** R-00146 criterion 2 structurally unprovable | **STILL OPEN, unchanged** | `reference_harness.rs` still exposes only `new` (`:35`), `arm` (`:42`), `execute` (`:46`), `snapshot_hash` (`:79`). No world-identity accessor. |
| **F-P2-1** `allow(dead_code)` module-wide, unsedimented | **sedimentation CLOSED; granularity open** | ADR 0009 exists and is indexed. Attribute still module-scoped. |
| **F-P2-2** production hasher has no known-vector test; duplicate hasher survives | **STILL OPEN — and worse than recorded, see F-P1-7** | Both `sha256_hex` remain. The only known-answer assertion is on the test-only copy. Additionally the generated crate's own `tests/chain.rs` is never compiled as a test target — the generated packages are not workspace members. |
| **F-P2-3** cut identity derived from `Debug` output | **STILL OPEN — and broader than recorded, see F-P2-8** | `publication/root.rs:113,115,117` unchanged; `chunk/replacement.rs:66` has the same pattern. |
| **F-P2-4** CI Architecture Gate gated v1.3 | **CLOSED** | `36850ec` (R-00264) moved every assertion to v1.4 (`repository-policy.yml:28,33,34,35`). All 17 assertions replayed green (§2 row 14). |
| **F-P2-5** three `GateSourceHashes` literals never recomputed | **STILL OPEN, unchanged** | `world_lifecycle.rs:42-47`, `world_barrier.rs:31-36` and siblings unchanged. |
| **F-P2-6** QA gate record stale | **STILL OPEN — broader than recorded** | `mvp-release-gate.md:12-13` still lists R-00203 and R-00146 as backlog, and its Traceability section still says only R-00002/34/37/41 have evidence. The whole record predates the wave. Its BLOCKED verdict remains correct. |
| **F-P3-1** unknown error ids collapse to `InvalidHandle` | **STILL OPEN — re-graded, see F-P2-9** | `port/error_mapping.rs:81` `_ => "InvalidHandle"`, and the test asserts the collapse rather than preventing it. |

### Decision gates — settled, not reopened

VOX-D-001..004 (P0; R-00057/58/59/60) carry `approvalStatus=approved` citing
`LGE-V1.4-VOX-D-P0-2026-08-28` (Architecture `5f06822`). VOX-D-005..008 (P2) carry `approvalStatus=approved`
citing `LGE-V1.4-VOX-D-P2-2026-08-29` (Architecture `origin/main` `997117e`, PR #16), applied here by
`dba284d`. Per that confirmation: measured invariants are binding, numeric axes stay adapter-internal,
VOX-D-007 keeps its dependency gap. `numeric_policy_frozen()` correctly stays `false`. **This review does not
reopen those rulings.** One consequence is recorded as a finding (F-P1-8) — not a challenge to the ruling, but
to the fact that nothing in the code notices when a gate's status changes.

---

## 4. Card-by-card pass over all 35 P0 requirements

Legend — **实现到位**: all four criteria met on the evidence available. **部分**: the deliverable exists and
substantially works, but at least one criterion is unmet, unproven, or proven by a test that does not test it.
**未动**: not started.

Test counts are from the `0466ffd` run (§2 row 1). "Anchor" is the commit introducing the card's primary owner
file; all are reachable from `origin/main`.

### 4.1 Foundation and contracts

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00034** 校准仓库规范与蓝图到 LGE-V1.4 | **部分** | `c8a7dc6` | — (docs) |

A1 **unmet**: `docs/architecture/LumioGameEngine_Architecture_v0.3.md:4,6`, `…v1.0.md:4,6`, `…v1.1.md:4,6` are
compatibility-pointer files that each state *"The normative architecture baseline is
LumioGameEngine_Architecture_v1.3.md"* and `ArchitectureBaselineId: LGE-V1.3-2026-08-27`. These are precisely
the entry points an external consumer following a historical filename lands on, and they point at V1.3. The
primary entries are clean (`README.md:7-11`, `modules/README.md:3-5`, `repository-architecture.md:12`,
`.spec/decisions/0007`), which is why this was missed: CI greps only the v1.4 file and README, and
`.baseline.sha256` locks only the v1.4 file. → **F-P2-10**.
A2, A3 **met**: blueprint carries only the ADR-0006 seven crates (`:13-23`), a linear acyclic graph
(`:25-76`), ten module landings (`:88-99`), single-owner shared-hotspot table (`:127-137`) consistent with the
exclusive-file table (`:302-330`); each module section gives stable port names, crate-private surface, failure
semantics and file set (`:143-301`) without copying schema fields.
A4 **met**: spec-lint 0 and `node --test` 13 pass, replayed here.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00041** 七 crate 工作区与依赖护栏 | **部分** | `1175b08` | `crate_dag` 6/6 |

A1 **met**: `Cargo.toml:3-11` seven members; `crate_dag.rs:53-82` asserts count == 7 and absence of
persistence/runtime/ffi/common; `test_guards.py:63-88` independently re-checks. Verified here (§2 rows 8–12).
A2 **unmet as written**: the criterion says *every* forbidden edge has a failing fixture. The allow-table
(`test-support/src/crate_dag.rs:21-69`) is a linear layering implying **23 forbidden member-to-member edges**
plus 5 non-edge rules. `tools/architecture/fixtures/` holds **3** negative fixtures — domain→world,
domain→test-support, and an extra `persistence` crate. 21 of 23 edges, three of four forbidden crate tokens,
both CoreEngine rules and the missing-frozen-crate rule have no fixture. It is "one representative per rule
class", not per edge. → **F-P2-11**.
A3 **met**: `check_generated_clean.py:32-40` + `generated_clean.rs:36-49` detect unlocked / mismatched /
missing; negative tests at `tests/generated_clean.rs:25-51,53-69`; `test_guards.py:54-61` writes a rogue file
and removes it; commands present in `testing.md:39-51` and `repository-policy.yml:47-60`.
A4 **met**, re-run here.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00045** 接入 V1.4 生成契约与 Fixture | **部分** | `1175b08` | `artifact_hashes` 3/3, `reexport_and_fixtures` 6/6 |

A1 **met**: `contracts/src/lib.rs:8-61` re-exports only through `#[path]` into the generated crates; `verify_one`
checks baselineId (`:159`), schemaEpoch (`:165`), `implementationDependencies: []` (`:172`) and outputHash vs
directory bytes (`:175-179`), requiring exactly 12 packages (`:139`). All 12 descriptor `outputHash` values and
all 58 lock entries independently recomputed here (§2 rows 15–16).
A2 **unmet**: two hand-written duplicates of generated values survive. (a) `test-support/src/generated_clean.rs:84-152`
is a complete hand-written FIPS 180-4 SHA-256 while the crate already depends on contracts, where 30+ other
sites use `lumio_voxel_contracts::sha256`. (b) `contracts/src/lib.rs:64` `pub const SCHEMA_EPOCH: u64 = 1;` is a
hand-copied generated value consumed by `config_snapshot.rs:108`, `restore_preflight.rs` and
`manifest_adapter.rs`; mitigated only because `verify_one:165` compares it against 12 descriptors. → **F-P1-7**.
A3 **met**: positive fixture `reexport_and_fixtures.rs:35-44`, negative `:47-55`; `artifact_hashes.rs:38-63`
tampers, expects `HashMismatch`, restores.
A4 **met**.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00047** 确定性 Reference、故障注入与 Harness | **部分** | `b2f0d8a` | `harness` 5/5 |

A1 **formally met, substantively hollow**: `harness.rs:31-40` asserts two runs produce identical trace and
snapshot bytes — but `deterministic_executor.rs:22-33` never reads `schedule.seed`; it is copied into
`Trace.seed` and nothing else. The execution body is a sequential fold with no randomness, so "same seed →
same result" is a tautology. No corpus concept, no differing-seed control. → **F-P2-12**.
A2 **met, and this one is genuinely good**: `fault_injection.rs:9-15` defines five fault points; `harness.rs:43-64`
is table-driven over **5/5**, asserting error_id and recoverability; the three unrecoverable points push to
`committed` before failing (`reference_harness.rs:58-72`), so they cannot masquerade as retryable.
A3 **partial**: `FixtureResult` (`fixture_runner.rs:8-15`) carries no op sequence, so the "minimal replay
bundle" cannot replay without re-reading the source file; and the expect-error branch (`fixture_runner.rs:49-55`)
has **zero coverage** — the only failure fixture (`unknown-schema.json`) is rejected at parse time (`:25-29`)
and never reaches it.
A4 **met**: `harness.rs:80-95` asserts no reverse production dependency via `live_graph`.

| Card | Conclusion | Anchor | Evidence |
| --- | --- | --- | --- |
| **R-00037** 核验 V1.4 生成契约 Artifact 发布门 | **部分** | `8401f5a` | `v1.4-generated-artifact-gate.md` |

A1, A3 **met** for the generation the document describes: twelve Rust/C# artifacts with the full five-tuple,
`implementationDependencies: []` verified. A2 **met** upstream (`stable outputHash: yes`).
A4 **met** for that generation. **But the document does not describe the artifacts on disk**: its inventory
body still carries `compilerHash 99a786e7…` / `inputHash 84a2b4c8…` while the live set is `3a46fc31…` /
`3a0436c9…`, and its `"ready": true` refers to the superseded set. An annotation added in `13d515f` marks these
historical and cites the current values, which is the honest interim step a consumer should not have to make —
but the gate record itself was never recomputed. → **F-P1-6**.

### 4.2 Decision gates (research)

| Card | Gate | Conclusion | Anchor |
| --- | --- | --- | --- |
| **R-00057** | VOX-D-001 Chunk profile | **实现到位** | `40265b1`, re-measured `49e84e6` |
| **R-00058** | VOX-D-002 Block storage | **实现到位** | `40265b1`, re-measured `49e84e6` |
| **R-00059** | VOX-D-003 Query budget | **实现到位** | `40265b1`, re-measured `49e84e6` |
| **R-00060** | VOX-D-004 Reservation/receipt | **实现到位** | `40265b1`, re-measured `49e84e6` |

All four: A1 evidence carries candidates, versions, licences, input hashes, raw measurements, statistical
method and replayable commands; A2 correctness/determinism/fault matrices carry real results, re-measured on a
linking host by `4705920` / `49e84e6` and driven by executable seam replays (`cc868e4`); A3 each separates
frozen contract, internal candidate and value-awaiting-approval, and none mutates schema, ID or default config
— `approval_status()` is a seam function, and the gates correctly refuse to self-approve; A4 owner approval
recorded (`LGE-V1.4-VOX-D-P0-2026-08-28`, Architecture `5f06822`).

**One systemic gap spanning all four, which belongs to R-00066 and is recorded there**: nothing connects these
documents' `approvalStatus` to the code that consumes gate evidence. → **F-P1-8**.

### 4.3 Configuration and concurrency

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00066** 不可变配置快照与 Capability 视图 | **部分** | `7a01dbd` | `config_snapshot` 6/6 |

A1 **partial**: the four P0 gates *are* checked one by one — `config_snapshot.rs:11` names VOX-D-001..004,
`:135-169` requires each evidence present, baselineId matching, five-tuple consistent across gates and digest
well-formed; `:166-168` collects non-`approved` gates and `:186-191` returns `TrustPolicyRejected` listing them.
**Rejecting blocked gates is real.** Two gaps: (a) the provenance five-tuple is used for cross-gate comparison
and then **discarded** — `Arc::new(Self{…})` (`:193-199`) stores no field carrying it, and `audit_summary()`
(`:222-224`) does not expose it, so a snapshot cannot be traced back to voxel_head / mirror sha / blueprint sha;
(b) the approved path has no fixture at all — the only fixture, `tests/fixtures/p0-gates-blocked.json`, is never
parsed by any code (the tests hand-build `DecisionEvidence` in Rust with hard-coded digests), and it still says
`blocked` while all four gate documents now say `approved`. → **F-P1-8**.
A2 **partial**: immutability is structural (`Arc`, no setters, `tests:226-229` greps for `fn set_`), but there
is **no reload API anywhere in the file**, so "concurrent reload, old and new operations each see one immutable
snapshot" is untested because the scenario does not exist in code. `tests:284-287` reads `config_hash` from two
independently-moved Arcs.
A3 **met**: missing → `EvidenceMissing`, unknown capability → `CapabilityMissing`, unapproved →
`TrustPolicyRejected`, digest mismatch → `EvidenceDigestMismatch`, all returning `Err` without producing an Arc.
One narrowing gap: `allow_list` takes the host capability set rather than `start_capabilities` (`:198`), so a
config declaring fewer capabilities than the host does not actually narrow. → **F-P2-13**.
A4 **met**.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00068** OriginToken、有界作业与完成信封 | **部分** | `31cb6a2` | `async_support` 4/4 |

A1 **unmet**: `OriginToken::try_new` (`origin.rs:55-64`) validates non-empty fields and the phase whitelist, but
**`configHash` is not in `OriginToken`** — it lives in `OriginEnvelope` (`origin.rs:101-106`), whose three
fields are public with no constructor and no validation. The single production gate,
`world/routing.rs:136-139`, reads:

```rust
fn check_config_hash(world: &VoxelWorld, hash: &str) -> Result<(), WorldError> {
    if hash.is_empty() { return Ok(()); }
    …
}
```

An empty `config_hash` **passes**. The criterion says a task missing the field must be unconstructible or fail
stably; it does the opposite. Verified by reading the only call path. → **F-P1-3**.
A2 **partial**: bounded submission is real (`bounded_port.rs:61-69` → `QueueFull`), but capacity is a caller
parameter (`:35-38`), not drawn from the approved snapshot, and `BoundedJobPort` has **no production
consumer** — `lumio-voxel-world` never constructs one.
A3 **partial**: `CompletionDisposition::Cancelled` is never produced by `validate_completion`
(`completion.rs:22-43`); `Late` is structurally unreachable on the production path because the expected phase
is taken from the envelope itself (`routing.rs:68,99`) and the basis revision is hard-coded 0 (`routing.rs:151`).
A4 **partial**: four real tests, all construction-level.

### 4.4 Revision

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00070** 单调分配器与 Reservation 生命周期 | **部分** | `31cb6a2` | `revision_allocator` 3/3 |

A1 **partial**: two independent monotone counters with `checked_add` before mutation (`allocator.rs:108-123`),
`abandon` sets a flag without rewinding (`:83-85`) — non-reuse is real. But "long sequence" is at most three
reservations per domain, and **concurrent reservation is neither supported nor tested**: `reserve_*` takes
`&mut self` with no atomics.
A2 **partial**: three stable error ids, counter unchanged before overflow. `finalize(); abandon(); finalize()`
and `finalize(); finalize()` report different ids for the same fact, untested.
A3 **partial**: no wall-clock (verified: zero `SystemTime|Instant|std::time|chrono` under domain/src), newtypes
not interchangeable. But the abandon-loop is an arbitrary-`WorldRevision` minting backdoor **used in production**
— `world/durability_ack.rs:153-166` spins a throwaway allocator `n` times to construct `WorldRevision(n)`,
O(n) and bypassing the allocator's uniqueness semantics. The same shape in the Restore path takes untrusted
input → **F-P1-2**.
A4 **unmet**: the criterion names 定向 / property / 并发. Property tests: **zero** (no proptest/quickcheck
anywhere, no dev-dependencies in the domain crate). Concurrency tests: **zero**, and the API cannot express them.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00071** 不可变 ReadView Pin 与回收边界 | **部分** | `3e57944` | `revision_read_view` 5/5 |

A1 **structurally met, evidence weaker than the wording**: `RevisionPin` holds the stamp by value (`pin.rs:148`)
and hands out only `&` (`:154`); there is no `&mut` path in the module, so immutability is type-level. But
"concurrent commit" is simulated on one thread (`:117`) and "config reload" by building a second registry
(`:120-122`) — `PinRegistry.snapshot` (`pin.rs:75`) is never re-read after construction, so live reload does not
exist in code.
A2 **met, and well covered**: `oldest_live` reads only slots (`retention.rs:21-33`), no TTL, no clock;
out-of-order drop keyed per pin (`pin.rs:175-187`); over-limit returns before writing any state (`:121-123`);
`destroy()` sets a flag without clearing slots (`:106-108`). Tests `:131-160`, `:196-217`, `:220-243`.
A3 **partial**: per-registry `Arc<Mutex<RegistryState>>` (`pin.rs:91`) makes cross-world writes impossible, but
the isolation test hands **the same `Arc<VoxelConfigSnapshot>` to both worlds** (`:165-166`) and makes no
address-level assertion, while the sibling card does (`publication_atomicity.rs:401,446` uses `!Arc::ptr_eq`).
Also `try_pin` does not check `world_id` (`pin.rs:115-118`).
A4 **partial**: five directed tests; no property, no concurrency.

### 4.5 Chunk

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00073** 不可变 Payload、四态 Slot 与 Directory Root | **部分** | `3e57944` | `chunk_state_machine` 5/5 |

A1 **partial**: four states are distinguishable and interned against the generated `CHUNK_PRESENCE`
(`slot.rs:46-54,101-107`; test `:22-66` asserts `names == CHUNK_PRESENCE`). But "item-by-item identical to the
generated state machine" cannot hold — there is no generated transition table for presence; the only generated
machine is `VoxelChunkResidency` (7 states), which the code and tests explicitly call a *different* machine
(`chunk/mod.rs:3-4`). `slot.rs:85-97` is hand-written. Of 16 from×to combinations, **2 are tested**.
Worse, `ChunkDirectoryBuilder::insert` (`directory.rs:38-42`) overwrites any slot without going through
`try_convert`, and the card's own test uses it to perform a transition `try_convert` would reject
(`chunk_state_machine.rs:124`). The four-state machine is advisory on the public builder API. → **F-P2-14**.
"Illegal transition has no side effect" **is** met and is structural: `try_convert(&self,…)` returns a new
value; `convert` computes `next` before `Arc::make_mut` (`directory.rs:52-60`).
A2 **met**: payload sealed at construction, `bytes: Arc<[u8]>` with no accessor or mutator
(`payload.rs:42-48,85-100`); COW via `freeze` + `Arc::make_mut`.
A3 **met on inspection** (no I/O, no world, no revision; only `lumio_voxel_contracts` and std) — but the guard
test (`chunk_state_machine.rs:241-263`) greps four tokens over **comment-inclusive** source, where the sibling
card strips comments first (`chunk_delta_dirty.rs:317`).
A4 **partial**: malformed-input coverage is genuinely thorough (12 malformed chunk ids, digest mismatch); no
property, no concurrency.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00076** Staged Delta 与 Dirty Frontier 纯计算 | **部分** | `26e3f3f` | `chunk_delta_dirty` 4/4 |

A1 **met structurally**: `ChunkDeltaBuilder::new` clones the root (`delta.rs:57-62`), `stage` touches only
`self.staged` (`:64-78`), `freeze(self)` only reads the baseline (`:80-91`); no `&mut ChunkDirectoryRoot`
anywhere in the crate. Frontier operations all take `&self` and return new values.
A2 **unmet — verified defect**: `covered_by` binds the ack's world-level cut and then never uses it —
`dirty.rs:233` `let _cut = SchemaRevision(ack.covered_world_revision);`. Coverage is decided purely per chunk on
`up_to_chunk_revision` (`:248-252`). An ack claiming `coveredWorldRevision = 0` while listing
`upToChunkRevision = 999` clears everything. **The test fixes this behaviour in place**: `:272-274` builds an ack
with `covered_world_revision = 4` against a chunk cut of 5 and asserts it covers. → **F-P1-4**.
A3 **met**: three files free of I/O / world / revision; the private `SchemaRevision(u64)` (`dirty.rs:13-14`) is
exactly why no revision service is called; `covered_by` yields a `DirtyCoverage` value and clears nothing.
A4 **partial**: the HashMap-order-independence test (`:164-225`) is real evidence; no property, no concurrency.

### 4.6 Publication

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00078** 单一原子 PublishedState Root | **部分** | `74ca752` | `publication_atomicity` 5/5 |

A1 **met, and this is the strongest concurrency evidence in the repo**: each capture clones one Arc under the
read lock and releases the guard before inspecting members (`authority.rs:63-69`); members are reachable only
through the root. `concurrent_captures_see_complete_old_or_complete_new` (`:525-580`) runs 4 readers × 64
iterations against 1 writer with a barrier, and every capture passes `assert_consistent_cut` (`:185-195`), which
cross-checks that stamp, directory and frontier are same-origin. Replayed 20× here, 0 failures (§2 row 20).
A2 **met**: one-shot seal (`prepared.rs:52-55`), non-`Clone` token (`:74-83`), by-value consumption
(`authority.rs:106`); stale / wrong-world / wrong-generation all rejected before the swap with identity
asserted unchanged.
A3 **unmet as written**: `authority.rs:135` comments *"No alloc/I/O/callback after this"*, but `:136-137` then
drops the previous `Arc<PublishedStateRoot>` — releasing the whole old cut inside the write lock if it was the
last reference — and drops the previous `RevisionPin`, whose `Drop` **acquires the pin registry mutex**
(`pin.rs:176-177`). So there is a second lock acquisition and unbounded destruction after the swap, and the
lock order "publication write lock → pin registry mutex" is nowhere recorded or tested. Visibility to readers is
still safe (that is A1); the A3 claim is what fails. **No test touches the post-swap path.** → **F-P2-15**.
A4 **partial**: five directed tests including one real threaded test; A3 has zero evidence; and every test uses
`empty_replacement()` (`:124-128`), so the digest folded into identity is always `sha256(&[])` and
`incorporate_replacement`'s discriminating power is never exercised.

**Also verified, on the round-2 carry-over**: `root.rs:113,115,117` still derive identity from `format!("{:?}")`
of directory, frontier and indexes. Two new observations this round: the `indexes` leg is a unit struct
(`root.rs:11`) whose `Debug` is a constant string, so that leg carries **zero information** and will silently
stop covering `AuxiliaryIndexes` if it ever gains fields; and the directory leg formats every payload byte into
a `String` on every root construction, so this is an O(total payload bytes) allocation on the publish path, not
only a stability problem. `identity()` is not internal — it is the base token at `publish_once:127` and crosses
crates into `RestoreReceipt` (`world/restore.rs:60,80`). → **F-P2-8**.

### 4.7 Query

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00080** 确定性计划器与预算校验 | **部分** | `88d527b` | `query_planner` 7/7 |

A1 **partial**: `planHash` covers canonicalized chunks + stamp + configHash + budget (`plan.rs:113-150`),
permutation-independence proven (`query_planner.rs:170`). Gap: the budget counts the **raw, pre-deduplication**
`request.chunk_ids.len()` (`plan.rs:63`) while canonicalization dedups afterwards (`:66`), so two requests with
identical `planHash` can differ on admission.
A2 **partial**: all validation precedes execution and only `view.stamp()` is read; four negative tests compare
full `Debug` hashes of root/stamp/directory. But the capability check is `config.capabilities().is_empty()`
(`plan.rs:61`) — it rejects only a wholly empty allow-list and requires no specific capability.
A3 **unmet — verified**: the plan does **not** fix one configuration snapshot. `QueryPlanner::plan()` takes a
`config: &VoxelConfigSnapshot` parameter (`plan.rs:56`) and never compares it against the `self.snapshot` bound
at construction; `self.snapshot` is used only by the `config_hash()` accessor (`:48-50`). A planner built from
snapshot A happily plans against snapshot B — and `query_planner.rs:291` does exactly that and asserts success.
→ **F-P1-5**.
A4 **met in form**: seven real tests, but the snapshot substitution is blessed rather than rejected.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00081** 单 Cut 只读执行与四态结果映射 | **实现到位（并发为单线程模拟）** | `edf472e` | `query_execution` 4/4, `query_missing_states` 1/1 |

A1 **met structurally**: `bind_cut` (`execute.rs:44-59`) compares world / context / generation / world_revision
and rejects with `InvalidHandle`; the whole walk reads one `&PublishedReadView` holding a single
`Arc<PublishedStateRoot>`, so a mixed cut is not constructible. `query_execution.rs:184` shows the old view
still serving the old cut. Single-threaded, as everywhere outside R-00078.
A2 **met**: `chunk_access.rs:43-67` interns against the generated `CHUNK_PRESENCE`; a directory miss maps to
`NotLoaded` without triggering a load; `query_missing_states.rs:147` covers all four states plus an absent id and
asserts the directory hash is unchanged after execution.
A3 **partial**: the returned buffer is bounded and leaks no internal reference — `query_execution.rs:325`
asserts the `Debug` output contains no `ChunkPayload` / `ChunkPage` / payload bytes / `Arc` / `0x`, which is
solid. But cancellation is caller-simulated: `execute_cancelled` (`execute.rs:35-41`) is a separate function that
unconditionally returns `LoaderCancelled`; there is no cancellation token observed during the walk.
A4 **met**.

### 4.8 Mutation

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00093** Canonical 指纹与 Txn Receipt Ledger | **部分** | `74ca752` | `mutation_receipt` 4/4 |

A1 **unmet — verified by independent reimplementation**: the canonical encoding does not escape, so distinct
requests collide. See **F-P0-1**; this is the round's most serious finding.
A2 **met**: a finalized entry re-finalized returns `Duplicate` with the original receipt and does not overwrite
(`receipt_ledger.rs:166-178`); same TxnId with a different fingerprint returns `RevisionConflict` consistently at
lookup, reserve and finalize (`:231-234`).
A3 **partial**: leases are generation-bound with no wall-clock (`reservation.rs:12`), correct per ADR. Capacity is
a caller parameter (`:91-94`), not from the snapshot. **Trimming is not implemented**: completed receipts are
never evicted, and once `entries.len() >= max_entries` every `reserve` returns `BudgetExceeded` permanently
(`:140-142`) — a monotone liveness cliff with no eviction path. → **F-P2-16**.
A4 **partial**: four real tests; no collision test, no reserved-key test, no eviction test (nothing to test).

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00096** 无可见副作用的 Prepare | **部分** | `88d527b` | `mutation_prepare` 4/4 |

A1 **partial**: Root and Dirty genuinely do not move — prepare reads the base and builds privately
(`prepare.rs:48-69`), `ChunkDeltaBuilder::new` clones, `freeze` does not touch the base; tests assert root
identity and dirty unchanged, replayed here 4/4 (§2 row 22). But the **Ledger's visible state does change on
failure**: prepare reserves first (`prepare.rs:28`) and only aborts after a seal failure (`:34`), while `abort`
removes the entry without decrementing `reserve_count` (`receipt_ledger.rs:152` `saturating_add`, no decrement
anywhere) — and `reserve_count()` is a public accessor (`:113`) that this repo's own tests use as the ledger's
observable state (`mutation_receipt.rs:135,143,…`). Severity is bounded: capacity is judged on
`entries.len()` (`receipt_ledger.rs:140`), which the abort does restore, so this is an accounting leak, not a
liveness one. → **F-P2-17**. Also `let _ = ledger.abort(request);` swallows abort failure.
A2 **met**: `PreparedMutation` is not `Clone` (`prepared_token.rs:10`), commit consumes by value
(`commit.rs:37-41`), wrong-world use rejected (`:109-116`). Weaker than the wording in one respect: commit does
not simply replay the token — it rebuilds the plan (`:58`), recomputes the overlay and stamp (`:60-62`), and
re-derives the world revision through an allocator loop (`:64,223-236`), several steps of which can fail.
A3 **partial**: duplicate chunk/cell keys fail the whole batch (`plan.rs:90-103`). Gap: an unrecognised edit key
is **silently dropped** rather than rejected — `plan.rs:75` `if !key.starts_with("c:") { continue; }` — so a
typo'd chunk key yields a batch that commits with one fewer edit, the opposite of all-or-nothing, untested.
The four-state precondition logic exists (`preconditions.rs:141-158`) but no test drives a mutation at the
`c:1`/`c:2`/`c:3` fixtures to assert `ChunkUnavailable`.
A4 **partial**.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00104** Commit 线性化与幂等重放 | **部分** | `9935902` | `mutation_commit` 4/4, `mutation_atomic_batch` 2/2 |

A1 **met structurally**: `publish_once` validates then performs a single prebuilt-Arc move under the write lock;
readers clone the Arc and release (`authority.rs:63-69`). `mutation_atomic_batch.rs:189` shows both chunks
flipping together with the old capture unchanged. Replayed 20× here, 0 failures (§2 row 21).
A2 **partial**: publish-once has three independent guards; Duplicate short-circuits before any prepare or
publish (`commit.rs:45-51`); conflict and stale fail before the swap with identity, dirty and ledger asserted
unchanged. But the main replay test is staged rather than observed: `mutation_commit.rs:242-263` uses a second
authority, a second ledger and a **different** txn id, then hand-calls `ledger2.finalize(…)` to plant the receipt
so commit takes the Duplicate branch. Genuine same-Txn replay is the last three lines (`:269-274`). "Concurrent
replay" is untestable as written — `commit` takes `&mut ReceiptLedger`, so the borrow checker already serialises it.
A3 **unmet as written**: the post-linearization path is `commit_finalize.rs:10-24`, which calls
`ledger.finalize(…)` **after** the swap and propagates its error (`:20-22`); `finalize`'s failure arms include
invalid_handle / session_mismatch / conflict. So an ordinary failure path exists after linearization, and if it
ever fires it leaves "world published, receipt unrecorded". It is unreachable in practice via the preceding
lookup, but the code does not make it unreachable. There are also allocations after the swap
(`receipt_ledger.rs:169,172`). I/O and external callbacks are genuinely absent throughout. Separately,
"invariant faults affect only the target world" has **no implementation** — there is no invariant-fault
detection or fencing, and both lock accessors recover from poisoning
(`authority.rs:143-153`, `unwrap_or_else(|p| p.into_inner())`). → **F-P2-18**.
A4 **partial**: six real tests; zero concurrency; the key replay leg is staged.

### 4.9 World

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00116** 生命周期、实例代与命令准入 | **实现到位（测试为抽样）** | `58fe42c` | `world_lifecycle` 8/8 |

A1 **met**: all three command classes check origin before touching state (`admission.rs:47-100`, `:51/:59/:81`),
`state.apply` follows (`:55`); legality is table-driven (`state.rs:72-80`). `world_lifecycle.rs:283-317`
enumerates all nine legal generated edges; `:333-357` asserts `StaleEpoch` with lifecycle unchanged. Illegal
edges are tested by one representative (`:320`) and read/write admission covers 5 of 8 states — the exhaustive
guarantee is in the code's explicit whitelist (`state.rs:82-90`), the tests sample it.
A2 **met, with the repo's cleanest isolation evidence**: `instance.rs:173-190` gives every instance its own
`PinRegistry` / `PublicationAuthority` / `ReceiptLedger` / `QueryPlanner`; the only `static` across world+ops is
the generation counter (`instance.rs:26`). `world_lifecycle.rs:416-466` asserts that after Authority publishes,
Replica's identity is byte-identical to before.
A3 **met**: `WorldStateView` (`instance.rs:44-51`) carries no host slot / session / thread / I/O type, and
`:469-484` asserts negatively against `WorldSlotHost` / `VoxelChunkResidency` / any `MACHINE_ID`.
A4 **met**.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00119** 串行写通道与 Typed Short Barriers | **部分** | `0399db9` | `world_barrier` 4/4, `world_command_order` 3/3 |

A1 **met, by a different mechanism than the wording suggests**: the unique lease is enforced by the borrow
checker — `write_lane.rs:34` takes `&mut VoxelWorld`, so `instance.write_occupied` is always false at
`try_acquire` and the `HandleDoubleRelease` arm (`:35-37`) is unreachable and untested. No global lock is real.
`world_command_order.rs:205-234` shows two worlds progressing independently, sequentially.
A2 **unmet — the guarantee has no implementation**: the five scopes exist (`barrier.rs:11-17`), but
`reject_forbidden` (`:29-35`) is a pure enum-to-error lookup that inspects and prevents nothing; no mechanism
stops `std::fs`, sleeping, an unbounded loop or a callback inside a barrier. The test (`world_barrier.rs:266-311`)
asserts `reject_forbidden(Io).error_id() == "LoaderTimeout"` — a match arm returning what the match arm says.
The counter-example is in this same repo: the Restore path contains a real input-driven unbounded loop
(F-P1-2) and the barrier machinery is oblivious to it. → **F-P1-9**.
A3 **partial**: ordering is reconstructed from the receipt's `old_root`/`new_root` chain plus capture identity
(`world_command_order.rs:182-202`); there is no independent Trace structure, and `DiagnosticsView`
(`diagnostics.rs:10-20`) is an instantaneous snapshot. Async-completion fencing is well covered (`:237-358`).
A4: A1 and A3 have real evidence; A2's evidence is empty.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00121** 目标实例故障隔离与有序关闭 | **部分** | `cf22b61` | `world_fault_isolation` 1/1, `world_shutdown` 3/3 |

A1 **met, with a real control group**: `fault.rs:32-52` touches only the target world;
`world_fault_isolation.rs:270-342` shows world B still querying, pausing, resuming and accepting writes after A
trips, with A's identity frozen and writes refused.
A2 **partial**: ordering and idempotence are covered (`world_shutdown.rs:166-218`; `shutdown.rs:16/29/44` each
return early on `sequence_reached`), stale origins refused (`:221-241`). But **no test reads the audit sink** —
"关闭顺序可审计" is inferred from lifecycle assertions; the audit record itself is never inspected. And the
generation-invalidation step is `generation + 1` (`shutdown.rs:122-125`), predictable and in the same namespace
as the allocator, so a forged gen+1 origin passes `check_origin` and is stopped only by the lifecycle table.
A3 **unmet in two respects**: `schema_id` and `error_id` *are* interned against the generated tables
(`events.rs:73,140-146`; `fault.rs:55-61`), but `incident_kind` is the hard-coded literal `"Simulation"`
(`events.rs:10`) validated against nothing; and **bounded redaction has no implementation** — `diagnostic_name`
is a caller-supplied `&'static str` checked only for non-emptiness (`fault.rs:39`), with no length bound, no
allow-list, and no test asserting a bundle excludes payload or keys. Sink failure cannot change domain state
structurally (`emit` returns `()`, `events.rs:115-121`), but `WorldFaultPort::trip` under a full or zero-capacity
sink is never tested. → **F-P2-19**.
A4 **partial**: one test for the isolation half is enough for A1 and not for A2/A3.

### 4.10 Snapshot, Restore, Durability

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00134** 不可变 VoxelCaptureRef 与 Canonical Codec Port | **部分** | `400dc47` | `snapshot_capture_ref` 6/6 |

A1 **met, with the repo's second real threaded test**: `snapshot_capture_ref.rs:224-271` spawns a thread with a
`Barrier`, publishes a new root concurrently, and asserts the capture's `root_identity` / `world_revision` /
`generation` / `config_hash` all come from the old cut and the encoded text contains `"worldRevision":0` and not
`:1`.
A2 **partial**: cross-instance use is rejected (`:361-368`, `capture_ref.rs:199`); drop/clone lifetime covered
(`:274-305`). But "no leak" is unverifiable — `PinRegistry` exposes no count (`pin.rs:79-144`), so nothing can
assert a pin returns on drop, and `PIN_CAPACITY = 16` exhaustion is untested. Also the `PinOrLease::Pin` arm is
dead on this path: capture always takes `Lease` (`capture.rs:37`).
A3 **unmet**: the repo contains **no generated fixture or reference bytes for a snapshot manifest** —
`generated/canonical/` holds only `canonical-digest-profile.json` and the descriptors have no
`snapshot-header` / `voxel-snapshot-payload` entry. The test (`:331-333`) recomputes the canonical string with
the same production helpers and compares to the production output: the code confirming itself, not a comparison
against reference bytes. And "no hand-written serializer" is only partly true — `manifest_adapter.rs:38-58`
hard-codes field names, `mod.rs:34-40` is a hand-written `quote()` **with no escaping** (see F-P0-1), and
`mod.rs:42-50` re-implements hex encoding that the generated runtime already provides. → **F-P2-20**.
A4: A1 is real evidence; A3 is not.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00135** Snapshot Cut 校验与 Capture 命令 | **实现到位** | `fc119d2` | `world_capture` 6/6 |

A1 **met**: admission plus a second validation against the same view (`capture_admission.rs:39-62,64-85`)
comparing world_id / context / generation / world_revision / artifact_hash / lease stamp; `capture.rs:31` takes
the view once and `:46` builds the ref from it. The cut arrives as `&RuntimeSnapshotCut` (`:27`) — Voxel neither
owns nor mutates it, so there is no ownership inversion. Weak spot: every positive test's cut is derived from
the world itself via `from_live` (`capture_admission.rs:24-36`), so the Runtime-supplies-the-cut direction is
never exercised with an independently constructed cut.
A2 **met**: inside the barrier only a lease clone and a struct construction (`capture.rs:33-51`), with
`drop(lease)` at `:51` before returning; `world_capture.rs:190-217` asserts the barrier is released before
encode, `:220-234` that the lane is re-acquirable, `:270-272` that failures leave no lane held.
A3 **met**: stale generation, wrong world and stamp mismatch each assert identity, dirty and lifecycle
unchanged (`:237-273`); a live capture does not block a subsequent prepare+commit (`:276-306`). The "pin
failure" clause is vacuous on this path (no pin is taken).
A4 **met**.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00136** Preflight、Shadow Root 与原子恢复 | **部分** | `ffee61c` | `world_restore` 7/7 |

A1 **unmet — verified defect**: structural corruption is well covered (re-canonicalize and compare whole
(`decode.rs:19-23`); magic / schema / epoch / world / generation / configHash / rootIdentity checks
(`restore_preflight.rs:130-186`); unknown fields rejected (`:232-246`)), and the 7/7 replay here confirms
truncated, empty, wrong-world and bad-epoch inputs are refused with the old world untouched (§2 row 23).
**But `worldRevision` has no upper bound** (`restore_preflight.rs:165` is a bare `require_u64`), and
`RestoreShadowBuilder::build` feeds it to `world_revision(n)` (`restore_shadow.rs:136-149`), which loops `n`
times reserving and abandoning. A snapshot with matching world_id / generation / configHash but a large
`worldRevision` hangs the build instead of failing stably. `chunk_revision(n)` (`:151-164`) is the same shape and
the number of `chunkRevision.*` entries is likewise unbounded (`restore_preflight.rs:209-230`). → **F-P1-2**.
A2 **met**: prepare → seal → single `publish_once` with the recheck first (`restore.rs:58-82`); tests assert
old/new root chaining and, on failure, identity + lifecycle + dirty all unchanged.
A3 **partial**: mutual exclusion is the borrow checker again. `restore_and_mutation_occupancy_are_serial`
(`world_restore.rs:415-453`) does not test what it names — `:441` exercises re-entering the *same* lease, and
`:446`'s `assert_stable_error("HandleDoubleRelease")` only checks the string exists in the generated table. The
"no Streaming Load" guarantee rests on a string scan over four hard-coded paths (`:488-515`), weaker than the
directory-walking equivalent in the snapshot card.
A4: real evidence, but A3's headline test is hollow.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00137** DurabilityAck 唯一 Dirty 清除路径 | **部分** | `6312d2a` | `durability_ack_apply` 7/7 |

A1 **met**: kind / world_id / context_id / generation validated and future cuts refused
(`durability_ack.rs:128-151`); coverage delegated to the domain. Tests cover covered-only clearing and old acks
producing empty coverage. (Note the domain-side coverage rule itself is defective — F-P1-4 — which is R-00076's.)
A2 **met**: duplicate, stale/out-of-order, wrong world, wrong generation, future revision and partial coverage
all covered, each asserting identity + dirty + lane state.
A3 **unmet as written**: the machine check is `production_src_has_no_clear_dirty_identifier`
(`durability_ack_apply.rs:441-486`), a source scan for the literals `fn clear_dirty` and `clear_dirty(`.
Renaming the function defeats it entirely. The genuinely strong fact is structural — `DirtyFrontier`'s only
subtractive operation is `except_covered` (`dirty.rs:259`) — but the test treats that as an aside (`:455-458`).
More importantly the claim itself does not hold: `restore_shadow.rs:95` constructs a fresh empty
`DirtyFrontier` which `restore.rs:58-82` publishes, so **a successful restore clears the entire dirty frontier**
via a second path the scan cannot see. → **F-P2-21**.
A4 **partial**.

### 4.11 Port

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00142** 生成 IVoxelWorldPort 总适配与所有权映射 | **部分** | `c51e5cd` | `generated_port_adapter` 9/9 |

A1 **partial**: error mapping is genuinely exhaustive in one direction — `STABLE_ERROR_IDS` has 43 entries and
`map_internal_error` has 43 explicit arms (`error_mapping.rs:38-80`), with `:411-417` iterating all 43 and
asserting identity mapping, so an upstream addition turns the test red. But the fallback `_ => "InvalidHandle"`
(`:81`) means an unknown id is silently rendered indistinguishable from a real `InvalidHandle`, and the test
named `unknown_error_id_mapping_cannot_succeed` (`:404-410`) **asserts the collapse** rather than preventing it.
There is no unknown-error fixture to compare against. → **F-P2-9**.
Method exhaustiveness cannot be checked against a generated method table (none is published — the generated
side offers only the schema id and binding name), so the only reference is the blueprint's ten stable names;
the adapter implements nine operations, and the two lists do not agree (`create_world` / `quiesce` / `destroy`
are in the blueprint but not the adapter; `admit` / `shutdown` are in the adapter but not the blueprint).
A2 **partial**: transfer, release, double release, use-after-release and stale generation are covered
(`:381-400`, `:347-375`); `ownership.rs:9-38` uses no raw pointers and `mod.rs:3` forbids unsafe. **Cancellation
is not covered at all** — the adapter has no cancel method, and `cancel: true` appears in no test in the repo.
A3 **met**: all four write paths go through `with_scope` (`routing.rs:36,56,87,111`), which acquires the write
lane before entering the barrier (`:118-125`); `:437-479` asserts seven members and no `extern "C"` / DllImport /
PInvoke in the port sources.
A4 **partial**: nine tests pass, but **`abort` has zero call sites in the entire repo** — `adapter.rs:77-82` →
`WorldRouter::abort` (`routing.rs:93`) → the barrier lease abort is a whole chain with no caller and no test,
in a card whose A1 is about exhaustive implementation and whose A4 is about real evidence. Verified by grep
across `crates/`. → **F-P2-22**.

### 4.12 Test matrices

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00143** B0 契约/Revision/Chunk/Publication 基础矩阵 | **实现到位** | `72564f8` | `b0_contract_domain` 9/9 |

A1–A4 **met at this HEAD**. All ten matrix rows are recorded PASS in `b0-verification.md` and the target runs
9/0 here. A2's Reference/Native agreement is carried by row 1 over all twelve packages, which I re-derived
independently (§2 rows 15–17). A3's replayability holds at `4ced801`, and `crates/` is byte-identical between
`4ced801` and `0466ffd` — see F-P2-7 for the anchor-text residual. A4's "production code not modified by this
test card" is satisfied for this round.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00145** B2 Query/Mutation/World/恢复故障矩阵 | **实现到位** | `1e07b76` | `b2_transaction_recovery` 10/10 |

A1–A4 **met at this HEAD**; all twelve rows PASS, target 10/0 here, and the Prepare-purity and Commit-atomicity
rows were replayed independently (§2 rows 20–22). The matrix tests what it says it tests; the criticisms in
§4.8 are of the *implementation's* uncovered paths, not of B2's own claims.

| Card | Conclusion | Anchor | Tests |
| --- | --- | --- | --- |
| **R-00146** MVP 端到端垂直链 | **部分** | `07886b9` | `mvp_vertical_slice` 2/2 |

A1, A3, A4 **met**: the ten-step chain runs through the generated port with auditable revisions, receipts and
hashes; dual instances are independent; the closing commands succeed.
A2 **still unmet, unchanged across three rounds**: "Native and Reference byte-identical on the same corpus"
cannot be asserted because `reference_harness.rs` has no world-identity accessor (§3, F-P1-2). The alignment
covers op sequence and payload only.

### 4.13 Umbrella and process cards

| Card | Conclusion | Note |
| --- | --- | --- |
| **R-00002** 原始需求 V1.4 完整框架落地 | **部分（P0 段）** | Aggregate. Its P0 half is delivered in the sense that all 26 implementation cards have shipped owner files and a green suite; it is **not** met in the sense that §5 records one CRITICAL and eight HIGH defects across those children. Its P2 half is untouched by design. |
| **R-00203** 本审查卡 | **本轮交付** | Owner file is this report. Round-2's acceptance gap — no card-by-card P0 pass — is closed by §4. Verdict on the reviewed object is RETURN (§5). |
| **R-00204** MVP 里程碑发布门 | **未动，正确地未动** | `mvp-release-gate.md` verdict BLOCKED. Its precondition is `REV-MVP = APPROVE`, which this round does not grant. Its record is stale in several places (§3, F-P2-6) but its verdict is right. |

---

## 5. Findings

Ordered by severity. Every one was re-derived by this reviewer; the method is stated with each.

### CRITICAL

#### F-P0-1 — the canonical encoding does not escape, so two different mutation requests can share a fingerprint

`crates/lumio-voxel-ops/src/mutation/fingerprint.rs:31-53`;
`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/lib.rs:40-52`.
Responsible: **R-00093** (and the generated canonicalizer upstream).

`canonical_object_pairs` sorts by key and concatenates `"key":value` with no escaping and no rejection of
duplicate keys. `canonical_fingerprint` inserts each caller-supplied field **value verbatim** (`:35`) while
quoting only `txn_id` and `world_id`. A value containing `,"` therefore forges structure.

I ported both functions to Python faithfully and searched for a collision. Found immediately:

| Request | `fields` | Canonical string |
| --- | --- | --- |
| A | `{"a": "1,\"b\":2"}` | `{"a":1,"b":2,"generation":1,"txn_id":"t","world_id":"w"}` |
| B | `{"a": "1", "b": "2"}` | `{"a":1,"b":2,"generation":1,"txn_id":"t","world_id":"w"}` |

Both hash to `1f782803503fa2868ffa5dbb093d461d17e847d61ff0668fdb1a0c8594985627`.

Why this is CRITICAL rather than a hygiene issue: the fingerprint **is** the idempotency predicate. Same TxnId +
same fingerprint returns the stored receipt *without executing* (`receipt_ledger.rs:166-178`); a different
fingerprint is rejected as a conflict (`:231-234`). A collision means a semantically different request can be
silently accepted as a replay of an earlier one and receive its receipt, having never run. Field values arrive
from the C# Runtime through the generated port, so this is an untrusted-input surface. Related: a caller field
literally named `txn_id` produces a duplicate key in the canonical string rather than an error.

**The same unescaped-encoding pattern is copied to three more sites**, so this is one defect surface, not one
site: `query/plan.rs:152` (plan hash), `mutation/commit.rs:268` (receipt), `snapshot/mod.rs:34-40` (snapshot
canonical bytes — and its decoder `decode.rs:74-98` splits on bare quotes and commas, so encoder and decoder are
wrong *consistently* and a round-trip test passes over a corrupted value). No test anywhere uses an id or field
value containing `"` or `,`.

### HIGH

#### F-P1-2 — a restore candidate's `worldRevision` is unbounded and drives an O(n) loop

`crates/lumio-voxel-ops/src/snapshot/restore_preflight.rs:165`;
`crates/lumio-voxel-ops/src/snapshot/restore_shadow.rs:136-164`. Responsible: **R-00136**.

Preflight validates magic, schema, epoch, world id, generation, configHash and rootIdentity, but reads
`worldRevision` with a bare `require_u64` and imposes no bound. `RestoreShadowBuilder::build` then calls
`world_revision(n)`, which constructs a throwaway `RevisionAllocator` and loops `n` times reserving and
abandoning to arrive at `WorldRevision(n)`. `chunk_revision(n)` is the same shape, and the number of
`chunkRevision.*` entries is also unbounded (`restore_preflight.rs:209-230`).

A snapshot whose world_id, generation and configHash match — i.e. one that passes every check — but whose
`worldRevision` is large makes `build` spin instead of failing. Snapshot bytes come from Host files and are
untrusted by the card's own framing ("所有损坏/不兼容输入在发布前稳定拒绝"). This is also precisely the
input-driven unbounded loop that R-00119 A2 forbids inside barrier work, which the barrier machinery cannot
detect (F-P1-9).

The root cause is shared with R-00070: an O(n) spin is the only way to mint a `WorldRevision(n)`, and the same
idiom is used with trusted input at `world/durability_ack.rs:153-166`, where it degrades linearly with world age.

#### F-P1-3 — an empty `configHash` passes the only production gate

`crates/lumio-voxel-world/src/world/routing.rs:136-139`. Responsible: **R-00068**.

```rust
fn check_config_hash(world: &VoxelWorld, hash: &str) -> Result<(), WorldError> {
    if hash.is_empty() { return Ok(()); }
    if hash != world.instance.snapshot.config_hash() { return Err(WorldError::session_mismatch()); }
    Ok(())
}
```

R-00068 A1 requires that a task leaving a barrier carry a complete generated Origin **and** configHash, and that
a missing field make the task unconstructible or fail stably. `configHash` is not part of `OriginToken` (which
does validate) but of `OriginEnvelope` (`origin.rs:101-106`), a three-field public struct with no constructor
and no validation — and the one gate that could catch an empty value waves it through. Every test constructs
`config_hash: String::new()`, so the bypass is the tested path and the mismatch arm has no coverage.

#### F-P1-4 — dirty coverage ignores the ack's world-level cut, and the test fixes that in place

`crates/lumio-voxel-domain/src/chunk/dirty.rs:233,248-252`. Responsible: **R-00076**.

`covered_by` binds the world cut and discards it: `let _cut = SchemaRevision(ack.covered_world_revision);`.
Coverage is decided purely per chunk on `up_to_chunk_revision`. An ack declaring `coveredWorldRevision = 0`
while listing `upToChunkRevision = 999` for a chunk clears that chunk.

R-00076 A2 requires that an old or wrong-generation ack cannot override newer dirty state. World id and
generation *are* checked (`:226-232`); the world revision — the ack's actual durability cut — is not.
`chunk_delta_dirty.rs:272-274` builds an ack with `covered_world_revision = 4` against a chunk cut of 5 and
asserts coverage, so the defect is pinned by a passing test. The world layer has an unrelated future-cut check
(`durability_ack.rs:147`), but the coverage decision does not consult it.

#### F-P1-5 — the query planner never binds the snapshot it was constructed with

`crates/lumio-voxel-ops/src/query/plan.rs:48-72`. Responsible: **R-00080**.

`plan()` takes `config: &VoxelConfigSnapshot` as a parameter and uses it for the capability check and the
recorded `config_hash`; it never compares it against `self.snapshot`, whose only remaining use is the
`config_hash()` accessor. A planner built from snapshot A plans against snapshot B without complaint —
`query_planner.rs:291` does exactly that and asserts success, so the substitution is blessed as expected
behaviour. R-00080 A3 requires the plan to fix one ReadView **and one configuration snapshot**.

#### F-P1-6 — the artifact set consumed at HEAD still has no recomputed gate record

`docs/evidence/v1.4-generated-artifact-gate.md` §3, §7. Responsible: **R-00037 / R-00045**. Carried from round 2.

`13d515f` added an honest in-place annotation marking `compilerHash 99a786e7…`, `inputHash 84a2b4c8…`, several
`outputHash` values and `"ready": true` as historical, and citing the live `3a46fc31…` / `3a0436c9…`. That was
the right interim step, and not recomputing on the consumer's behalf was the right call. But the gate record —
which declares itself the sole owner file — still attests a generation that is not on disk, and its
`"ready": true` still refers to that generation. **The artifacts themselves are sound**: I recomputed all 58 lock
entries and read the five-tuple off all 12 live descriptors (§2 rows 15–16), and both K tables match FIPS 180-4
exactly (row 17). The finding is the missing gate result, not bad artifacts.

#### F-P1-7 — the production hasher is the one without a known-answer test, and its own tests never compile

`crates/lumio-voxel-test-support/src/generated_clean.rs:84-152` vs
`crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/sha256.rs`. Responsible:
**R-00045 / R-00047**. Tracked as **R-00290**; restated because this round found it is worse than recorded.

Two SHA-256 implementations remain. The only known-answer assertion (`tests/generated_clean.rs:7`,
`e3b0c442…`) is on the **test-only** copy. Newly established this round: the generated crate's own
`tests/chain.rs` contains only a self-consistent round-trip and **is never compiled as a test target**, because
the generated packages under `crates/lumio-voxel-contracts/generated/rust/` are not workspace members — they are
pulled in via `#[path]` into a private module. So the hasher underpinning every identity, receipt and artifact
digest in the repo has no absolute-vector assertion anywhere in the build. This is the exact defect class that
let `K[28] = 0xc6eabbdc` survive from the project's first commit. Adding one assertion against
`lumio_voxel_contracts::sha256` closes it. Separately, `contracts/src/lib.rs:64`
`pub const SCHEMA_EPOCH: u64 = 1;` is a hand-copied generated value.

#### F-P1-8 — nothing connects the decision-gate documents to the code that consumes gate evidence

`crates/lumio-voxel-domain/src/config_snapshot.rs:135-199`;
`crates/lumio-voxel-domain/tests/fixtures/p0-gates-blocked.json`. Responsible: **R-00066**.

`from_generated` correctly refuses to start when a P0 gate is not `approved` — that logic is real and tested.
But the evidence it consumes is built in Rust inside the tests with hard-coded digests
(`config_snapshot.rs` tests `:42-61`, `:204`), and the repo's only gate fixture is **never parsed by any code**;
the tests only `contains`-match its text. Consequences, both live today:

1. All four VOX-D-001..004 documents now read `approvalStatus=approved` (`dba284d` and predecessors), while the
   fixture still says `blocked` and still records `voxelHead: b2f0d8a`. Nothing noticed.
2. The provenance five-tuple is validated for cross-gate consistency and then dropped — no field of
   `VoxelConfigSnapshot` carries it (`:193-199`) and `audit_summary()` does not expose it (`:222-224`), so A1's
   "来源五元组…可审计" does not hold for any consumer holding a snapshot.

This is the seam where a gate status change becomes invisible to the code that is supposed to gate on it.

#### F-P1-9 — barrier "forbidden work" rejection has no implementation

`crates/lumio-voxel-world/src/world/barrier.rs:29-35`;
`crates/lumio-voxel-world/tests/world_barrier.rs:266-311`. Responsible: **R-00119**.

R-00119 A2 requires that the five barrier scopes contain only the declared bounded in-memory work and that
I/O, waiting, callbacks and unbounded loops be rejected. `reject_forbidden` is a lookup mapping a
`ForbiddenWork` enum to an error id; it observes nothing and prevents nothing. The test asserts that
`reject_forbidden(Io).error_id() == "LoaderTimeout"` — that a match arm returns the value written in the match
arm. There is no execution-path evidence for the guarantee at all, and F-P1-2 is a live counter-example inside
the same repo.

### MEDIUM

- **F-P2-7** — the three evidence documents pin measurement to `4ced801` and assert 「`origin/main` == `4ced801`」
  (`b0-verification.md:5`, `b2-verification.md:5`, `mvp-integration.md:5`), which is false at `0466ffd`. The
  numbers themselves are *not* stale: `crates/` is byte-identical between the two commits (the range contains
  only docs, benchmarks, ADR 0009 and the CI fix), and every figure reproduces here. This is the third
  consecutive round in which evidence has been overtaken by a later merge; the mechanism, not the diligence, is
  what keeps failing. A document that recorded "measured at X; `crates/` unchanged between X and HEAD" would be
  self-maintaining. Responsible: R-00143 / R-00145 / R-00146.
- **F-P2-8** — cut identity is still derived from `Debug` output (`publication/root.rs:113,115,117`;
  `chunk/replacement.rs:66`). Two new observations: the `indexes` leg formats a unit struct, so it contributes a
  constant and covers nothing; and the directory leg formats every payload byte into a `String` on every root
  construction, making this an O(payload) allocation on the publish path. `identity()` crosses crates into
  `RestoreReceipt` (`world/restore.rs:60,80`). Responsible: R-00078.
- **F-P2-9** — unknown generated error ids collapse to `InvalidHandle` (`port/error_mapping.rs:81`) and the test
  asserts the collapse. The 43/43 identity-mapping test is a genuine guard against upstream additions; the
  fallback still makes an unknown id indistinguishable from a real one. Responsible: R-00142.
- **F-P2-10** — three compatibility-pointer files still name V1.3 as normative
  (`docs/architecture/LumioGameEngine_Architecture_v0.3.md:4,6` and the v1.0 / v1.1 siblings). These are the
  entry points a consumer following a historical filename hits. No gate can catch them: CI greps only the v1.4
  file and README, and `.baseline.sha256` locks only the v1.4 file. Responsible: R-00034.
- **F-P2-11** — the crate-DAG guard has 3 negative fixtures for 23 forbidden edges plus 5 rule classes; two rule
  classes (CoreEngine dependency, missing frozen crate) have none. Also `check_crate_dag.py` and `crate_dag.rs`
  are two hand-synchronised implementations of the same rules with no equivalence test. Responsible: R-00041.
- **F-P2-12** — `DeterministicExecutor` never reads `schedule.seed` (`deterministic_executor.rs:22-33`), so
  R-00047 A1's determinism assertion is a tautology; and the expect-error branch of the fixture runner has zero
  coverage because the only failure fixture is rejected at parse time. Responsible: R-00047.
- **F-P2-13** — `VoxelConfigSnapshot`'s allow-list is built from the host capability set, not
  `start_capabilities` (`config_snapshot.rs:198`), so a config declaring fewer capabilities than the host does
  not narrow. Responsible: R-00066.
- **F-P2-14** — `ChunkDirectoryBuilder::insert` (`directory.rs:38-42`) bypasses `try_convert`, and the card's own
  test uses it to perform a transition the state machine rejects (`chunk_state_machine.rs:124`). Two of sixteen
  transition combinations are tested. Responsible: R-00073.
- **F-P2-15** — `publish_once`'s post-swap comment ("No alloc/I/O/callback after this",
  `publication/authority.rs:135`) does not hold: `:136-137` drops the old root (potentially freeing the entire
  old cut under the write lock) and the old `RevisionPin`, whose `Drop` takes the pin registry mutex
  (`pin.rs:176-177`). Reader visibility is unaffected; the A3 claim is not. Also `used_ids` grows by one u64 per
  successful publish with no removal path, serving a branch the code documents as unreachable
  (`authority.rs:19,118-133`). Responsible: R-00078.
- **F-P2-16** — the receipt ledger has no eviction path: completed receipts are never removed and once
  `entries.len() >= max_entries` every reserve returns `BudgetExceeded` permanently
  (`receipt_ledger.rs:140-142`), while R-00093 A3 speaks of trimming. Responsible: R-00093.
- **F-P2-17** — a failed `prepare` leaves `reserve_count` incremented (`receipt_ledger.rs:152`, no decrement
  anywhere), which is public observable ledger state, against R-00096 A1's "完全不变". Bounded: capacity is judged
  on `entries.len()`, which the abort does restore. Responsible: R-00096.
- **F-P2-18** — `commit_finalize.rs:10-24` calls `ledger.finalize` *after* the swap and propagates its error, so
  an ordinary failure path exists after linearization (unreachable in practice, not by construction), and there
  are allocations after the swap. Separately, R-00104 A3's "invariant faults affect only the target world" has no
  implementation, and both lock accessors recover from poisoning (`authority.rs:143-153`). Responsible: R-00104.
- **F-P2-19** — R-00121 A3's bounded redaction has no implementation (`diagnostic_name` is an unbounded
  caller-supplied string checked only for non-emptiness, `fault.rs:39`), `incident_kind` is the literal
  `"Simulation"` validated against nothing (`events.rs:10`), and `trip` under a full sink is untested.
  Responsible: R-00121.
- **F-P2-20** — R-00134 A3 cannot be met as written: there are no generated fixtures or reference bytes for a
  snapshot manifest, and the test compares production output against a recomputation by the same production
  helpers. Responsible: R-00134.
- **F-P2-21** — the "sole dirty-clear entry" guard is a source scan for the identifier `clear_dirty`
  (`durability_ack_apply.rs:441-486`), defeated by renaming; and the claim is false as stated, because a
  successful restore publishes a fresh empty `DirtyFrontier` (`restore_shadow.rs:95` → `restore.rs:58-82`).
  The structural fact (only `except_covered` subtracts) is stronger than the test that is supposed to prove it.
  Responsible: R-00137.
- **F-P2-22** — `GeneratedVoxelWorldPortAdapter::abort` → `WorldRouter::abort` → barrier lease abort has **zero
  call sites** anywhere in `crates/` and zero test coverage, while passing clippy and compiling green.
  Responsible: R-00142.
- **F-P2-5, F-P2-6** — unchanged from round 2 (see §3).

### LOW

- Source-scan guards are used as primary acceptance evidence in four places (snapshot-no-fs, restore-no-streaming,
  durability-no-clear_dirty, adapter-no-pinvoke) with inconsistent strength: two walk a directory, two scan a
  hard-coded file list, one scans comment-inclusive source. They should be uniformly directory-walking and
  recorded as weak guards rather than headline evidence.
- Scaffolding comments leaked into production sources: `world/restore.rs:1-3` and `world/durability_ack.rs:1-3`
  carry `// ORCHESTRATOR MERGE world/mod.rs:`.
- `write_occupied` is duplicated in three places (`write_lane.rs:35`, `restore.rs:47`, `durability_ack.rs:64`)
  and is unreachable in all three; real exclusion comes from `&mut`. If any one later adopts interior
  mutability, the protection fails silently with all current tests green.
- Contract-table membership is asserted with discarded results in several places
  (`let _ = SCHEMA_IDS.contains(…)` at `origin.rs:65`, `completion.rs:23`, `preconditions.rs:150`,
  `commit.rs:310`) or with `debug_assert!`, which is compiled out in release.
- Property-based tests are absent repo-wide; the domain crate has no dev-dependencies.
- Real concurrency evidence exists in exactly two tests (`publication_atomicity.rs:525`,
  `snapshot_capture_ref.rs:243`). Every other "concurrent" criterion is met by sequential simulation.

---

## 6. Downgraded and withdrawn candidates

Recorded because the reason they were wrong matters for the next round.

1. **"A failed prepare permanently pins the ledger's capacity and world binding."** First reading suggested the
   un-decremented `reserve_count` consumed capacity. It does not: `reserve` judges capacity on
   `entries.len() >= max_entries` (`receipt_ledger.rs:140`) and `abort` removes the entry. The world/generation
   binding likewise survives deliberately, and each world instance owns its own ledger (`instance.rs:173-190`),
   so binding to that world is the intended semantics, not a leak. **Downgraded to F-P2-17** (accounting only).
2. **"The three evidence documents are stale again, as in round 2."** The anchor text is stale, the numbers are
   not. `git diff 4ced801..0466ffd -- crates/` is empty, and every figure reproduces here. Round 2's F-P0-1 was
   P0 because every number was wrong and every card verdict read BLOCKED; this is one false sentence.
   **Downgraded to F-P2-7.**
3. **"`world_fault_isolation` having a single test means R-00121 A1 is unproven."** A1 is in fact well proven —
   that one test carries a real control group (world B keeps querying, pausing, resuming and writing while A is
   tripped, with identities asserted both ways). The gaps are in A2 and A3, not A1. **Re-scoped into F-P2-19.**

---

## 7. Skips, ignores, weakened assertions, flakiness

**None found**, same as round 2 and re-verified here.

- `grep -rn "#\[ignore" crates/ tools/` → 0 matches. No `should_panic`, `todo!()` or `unimplemented!()` under
  `crates/`.
- All 46 `test result` lines report `0 ignored; 0 filtered out`.
- `--test-threads=1` produces an identical 158 / 0 / 0.
- 40 repeated concurrency runs (20 + 20) produced 0 failures.
- `unsafe`: zero occurrences outside `#![forbid(unsafe_code)]`, which appears in 70 files.

The caveat this round adds is that a green suite is not coverage: §5's CRITICAL and eight HIGH findings all sit
in paths these 158 tests do not exercise, or — in three cases (F-P1-4, F-P1-5, F-P2-9) — that they do exercise
and assert the defective behaviour as expected.

---

## 8. Scope notes for the main loop

- **Upstream has advanced.** `LumioGameEngineArchitecture` `origin/main` is now `3287bba` (PR #27); the mirror
  here reproduces `bcc8eb9` exactly. Per the standing ruling this is a **reporting item, not a failure** — the
  mirror is byte-consistent with the upstream commit it declares, all 58 lock entries verify, and the five-tuple
  is uniform. Re-mirroring is a separate scheduling decision.
- **R-00204 remains deadlocked against this card** and correctly stays BLOCKED. Its record additionally needs
  refreshing (F-P2-6): its precondition table still lists R-00203 and R-00146 as backlog, and its traceability
  section still says only four P0 cards have evidence.
- **`.spec/tasks/` holds only `README.md`.** Card acceptance state lives in the Workflow API, which this round
  read directly for the coverage list (§1). Anyone re-deriving §4 should do the same rather than trusting this
  document's table of contents — that is precisely how round 2's gap arose.
- **The findings in §5 belong to implementation cards, not to this one.** Nine of them (F-P0-1 and the eight
  HIGH) are in cards currently `in_review`. Whether they are fixed under those cards or split into new ones is
  the main loop's call; this report does not create cards.

---

## 9. Required before re-review

1. **F-P0-1** — escape values in the canonical encoding, or reject values containing the delimiter set, at all
   four sites (`mutation/fingerprint.rs`, `query/plan.rs`, `mutation/commit.rs`, `snapshot/mod.rs` + its decoder).
   Add a collision test using the pair in §5. The generated canonicalizer's contract is the upstream half of
   this and needs an architecture-side decision.
2. **F-P1-2** — bound `worldRevision` and the `chunkRevision.*` entry count in preflight, and replace the O(n)
   revision-minting loops with direct construction (both here and at `durability_ack.rs:153-166`).
3. **F-P1-3** — make `configHash` part of the validated `OriginToken`, or make the empty case fail in
   `check_config_hash`; add the negative test the current tests bypass.
4. **F-P1-4** — use `ack.covered_world_revision` in the coverage decision, and fix the test that pins the current
   behaviour.
5. **F-P1-5** — compare the `config` argument against the planner's bound snapshot, and invert
   `query_planner.rs:291` from asserting success to asserting rejection.
6. **F-P1-6** — recompute the Architecture Gate record for the artifact set actually on disk
   (`compilerHash 3a46fc31…`, `inputHash 3a0436c9…`, the 12 current `outputHash` values).
7. **F-P1-7** — assert a known vector against the **production** `lumio_voxel_contracts::sha256`, and delete the
   duplicate hand-written hasher (R-00290 already carries this).
8. **F-P1-8** — bind the gate documents to the code: parse the fixture, add an approved-path fixture, carry the
   provenance five-tuple on the snapshot, and add a drift check so a gate status change cannot pass unnoticed.
9. **F-P1-9** — either implement a real barrier-work guard or restate R-00119 A2 to what the borrow checker and
   scope typing actually prove. Carrying an assertion whose test is a tautology is the option that should not be
   chosen silently.
10. **F-P1-2 (R-00146 criterion 2)** — unchanged across three rounds: give the Reference harness an identity
    accessor, or rewrite the criterion, explicitly, to what it can prove.
11. **MEDIUM / LOW findings** — open cards; not individually blocking, but F-P2-7's mechanism (evidence pinned to
    an absolute commit that later merges invalidate) has now recurred three times and deserves a fix rather than
    another manual rewrite.
