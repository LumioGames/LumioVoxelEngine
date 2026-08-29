# VOX-D-005 Snapshot Pin/COW and sub-chunk Diff granularity

- Card: R-00061 / GATE-005
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28; re-measured 2026-08-29 on a linking host (see §4)
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/snapshot_cow.rs`; `benchmarks/decision_gates/data/vox-d-005/`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam that now drives the shipped R-00047 harness. It does not freeze numeric defaults, pick a default algorithm, edit Schema/ID/default config, or implement production snapshot code.

Produces: `DecisionEvidenceVOXD005`; `SnapshotCowProposal{pinBudget,diffGranularity,materializeRule,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (`feat(R-00047): add deterministic harness, faults and fixture runner`) |
| Architecture lock (card) | `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550` (`main` at planning) |
| Architecture working tree HEAD | `3d5e29db72b70c88fb61e392832afe2a762b25cb` (`main`) |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` (re-measured; matches lock) |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture `DECISIONS_PENDING.md` SHA-256 | lock `65d839c5732825a3392daf76e1b22797d1f97928b328df409ebc544b1191467f`; current HEAD `8cbcda49fb47b8951eb37f08e41cf4bccf51dff10a423743a04463b60cccbea3` |
| Architecture ADR-035 SHA-256 | `d55c8c6bc7c7381305a956a1f9f4f9f66dec4b70b8d4f0ae24cdabfd946b166c` (`.spec/decisions/ADR-035-voxel-snapshot-payload.md`; unchanged lock→HEAD) |
| `voxel-snapshot-payload.schema.json` SHA-256 | `b5e5d12767cfe2ba732e162be1c98068cce70fbd2b20974e844c6022545a6740` |
| Prerequisite R-00034 | Consumable. Worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline` (`8c49fba`). |
| Prerequisite R-00047 | **Met.** Commit `b2f0d8a`. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `7e7bf9b24a8e31e55130bcd5c6336f748b2d456bdd4c8b20b3643082099ed742` exports `deterministic_executor`, `reference_harness`, `fault_injection`, `fixture_runner`. No substitute harness was invented. |

Seam SHA-256 (this card, re-measured after rustfmt):

| Path | SHA-256 |
| --- | --- |
| `benchmarks/decision_gates/snapshot_cow.rs` | `11b0fe30361933471700cf49e447ca6762aac15e3ce4128558659002e3dd540d` |
| `benchmarks/decision_gates/data/vox-d-005/full-cut.json` | `4d41c10ab44d86c995b22ac9b280c1db6e89ff2c8e8935f4568b0d2cca6abc1a` |
| `benchmarks/decision_gates/data/vox-d-005/partial-aoi.json` | `9e3ae367a5c64565648a8a7cb6cb7f1f789983f5f1392c6b43bd2129d2c40116` |
| `benchmarks/decision_gates/data/vox-d-005/capture-ready-pinned.json` | `c311279e4a9b6dadce645dc7fd68dbf1fcb6675b938210047a9d41d007ed7367` |
| `benchmarks/decision_gates/data/vox-d-005/pin-expired.json` | `67df1f76dd326c7438bdebeb7a04bfb03a1417dba75ae8eeb5421a97ced51f2c` |

Architecture fixture **input** SHA-256 (not copied into this repo; not treated as pin/COW measurements):

| Fixture | SHA-256 |
| --- | --- |
| `fixtures/valid/voxel-snapshot-payload-full.json` | `4bfcbd0a7bcd45a11833b69481a31ddfebcb73ab1cd11577ea9faf2e388133e9` |
| `fixtures/valid/voxel-snapshot-payload-partial.json` | `7bf7656499b03950fe17292354dc96c88d15c111ce28568ea58611894d6cfe05` |
| `fixtures/valid/voxel-snapshot-capture-ready.json` | `cf02176e773762598adef8a24762a734e87bdb3ad2cb353da661676314deb00d` |
| `fixtures/valid/state-machine-voxel-snapshot-capture.json` | `84cc3f7d74b6405ac25948dcfc8bf382e8017d09662c831409902158103f2815` |
| `fixtures/invalid/voxel-snapshot-capture-pin-expired-ready.json` | `f750a618a83aba8a0a275946821b867de071476a24508c1d0cb9a91524075615` |
| `fixtures/invalid/voxel-snapshot-payload-bad-hash.json` | `9b3f86328883f1c892ee5bf7b7f9c276138f01da535d554429960ea228000db9` |
| `fixtures/invalid/voxel-snapshot-diff-no-advance.json` | `e8900650e1bcb7318c0394290b1be11f227eaf04f6f815b85471ccebbf0bb4bd` |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen, do not copy field layouts here):**

- Runtime owns `SnapshotCut`; Voxel owns `VoxelCaptureRef` (仓内 ADR 0001).
- Running snapshot: short barrier to pin, encode in background (仓内 ADR 0004). Restore is `decode → world.restore → materialize_pages + restore_stamps`, never Streaming Load.
- ADR-035 wire: canonical chunk order, `SameCutSameBytes`, chunk-granular strictly-advancing diffs, capture lifecycle `Requested → … → Ready` only while `pinState = Pinned`.
- Payload kinds, ErrorCode `SnapshotBaseMismatch` (1039), envelope remaining `snapshot-header`.
- `voxel-snapshot-payload` capture states (`$defs.voxelCaptureState`): `Requested`, `Cutting`, `Pinned`, `Encoding`, `Verified`, `Ready`, `Released`, `Cancelled`, `Failed`. Pin fence states are a different enum (`Requested` / `Pinned` / `Released` / `Expired` / `Invalidated`) and are not a pin-budget number.

**Open on this gate (architecture D-014 / VOX-D-005):**

- How the immutable capture view is held (pin-count vs copy-on-write).
- Pin budget / admission (no numeric default).
- Internal page/block-level diff encoding behind the frozen chunk-granular wire.
- Materialize rule (lazy pin vs eager copy vs hybrid dirty-frontier).

A finer **wire** diff granularity, new capture states, or new pin-fence shapes require a new architecture ADR, fixtures, and BaselineId. This card must not write those.

**Must not edit:** Schema, ID registry, generated default config, crate `Cargo.toml` / `lib.rs`, ADR index, knowledge index, architecture mirrors.

## 3. Candidates (no default selected)

`selectedDefault`: none. `recommendedCandidate`: none. The first row is **not** a conclusion.

License for in-tree strategy candidates is this repository (`Apache-2.0` per `LICENSE`). Version is unversioned policy, not a crate release. Source hash is the architecture decision text that defines the open question, not an implementation artifact.

| id | pinBudget (pending public) | diffGranularity (pending public) | materializeRule (pending public) | version | license | source Hash | stop / exclusion notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `pin-count-chunk-wire-diff` | pin-count over published pages; numeric cap unfrozen | keep ADR-035 chunk-granular wire; no internal sub-chunk encoding | lazy: hold pins until encode/release | unversioned | Apache-2.0 (in-tree) | ADR-035 `d55c8c6b…946b166c` | Stop if pin expiry can still emit `Ready`, or concurrent mutation mutates captured bytes. Not excluded: no pin/COW numeric matrix. |
| `page-cow-internal-page-diff` | COW page clones on first write after pin; numeric cap unfrozen | internal page/block encoding still emitted as chunk-granular wire | hybrid: COW dirtied pages, pin clean pages | unversioned | Apache-2.0 (in-tree) | D-014 lock `65d839c5…1191467f` (current HEAD file `8cbcda49…0cccbea3`) | Stop if internal encoding is leaked as a new public field. Changing wire granularity needs a new ADR. Not excluded: no pin/COW numeric matrix. |
| `eager-full-copy-chunk-diff` | copy covered pages at cut; pin budget becomes copy budget (still unfrozen) | ADR-035 chunk-granular wire only | eager materialize at cut, then drop live pins | unversioned | Apache-2.0 (in-tree) | 仓内 ADR 0004 (Pin/COW path) | Stop if cut barrier includes encode/fsync or if copy fails by silently sharing mutable pages. Not excluded: no pin/COW numeric matrix. |

No OSS codec/vendor is a candidate here. Canonical encode stays on the generated codec. Adding a compression backend is VOX-D-002, not this gate.

## 4. Measurement seam — executed on a linking host

**Status: executed.** The earlier revision of this section was a plan, because the implementer host was Windows msvc with no `link.exe` and could only compile the seam to an rlib. This gate was re-run on a host that links and executes real test binaries; `cargo check` is not accepted as evidence here.

Run of 2026-08-29, at repository commit `13d515f358ffeb182e9659d5bde4fa119496f711` (`origin/main`):

| leg | host triple | rustc | seam result |
| --- | --- | --- | --- |
| primary | `x86_64-apple-darwin` (Rosetta 2 on an Apple Silicon machine; rustup default host) | `1.98.0 (88d9e12ae 2026-08-18)`, pinned by `rust-toolchain.toml` | 5 passed / 0 failed |
| second | `aarch64-apple-darwin` (native) | `1.98.0 (88d9e12ae 2026-08-18)` | 5 passed / 0 failed; output byte-identical to the primary leg |

Generation commands (the runner resolves the hashed rlib names from cargo's JSON output, replacing the hand-typed `rustc` line the previous revision carried):

```bash
benchmarks/decision_gates/run_seam_replay.sh snapshot_cow
SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin SEAM_OUT_DIR=target/decision-gate-seams-aarch64 \
  benchmarks/decision_gates/run_seam_replay.sh snapshot_cow
```

Fixed: seed `61`, corpus, schedule. Each input repeated three times in-process and compared for `Trace` / `snapshot_hash` equality; the whole runner was additionally re-executed three times and diffed. Statistics for pin/COW memory amplification, encoded bytes, and write tail remain **unfrozen** — the shipped harness does not pin pages or encode production snapshots. No summary-only charts. No invented hashes.

The seam source is unchanged by this run: `benchmarks/decision_gates/snapshot_cow.rs` still hashes to `11b0fe30361933471700cf49e447ca6762aac15e3ce4128558659002e3dd540d`, the value recorded in §1.

**Harness corpus** (schema_id `voxel-snapshot-payload`):

| id | schedule |
| --- | --- |
| `full-cut` | capture states `Requested → Cutting → Pinned → Encoding → Verified → Ready → Released` via `DeterministicExecutor::run` |
| `partial-aoi` | single Partial-AOI labeled op via `DeterministicExecutor::run` |
| `capture-ready-pinned` | happy path through `Ready` (no `Released`); pin still held |
| `pin-expired` | Ready-claim op with shipped `FaultPoint::StaleCompletion` |

**Harness faults** (card names mapped onto shipped `FaultPoint`; no sixth point invented):

| card fault | FaultPoint | error id | recoverable | committed |
| --- | --- | --- | --- | --- |
| `pin-expired-ready` | `StaleCompletion` | `StaleEpoch` | true | no (`Ready` + expired pin must not publish) |
| `payload-bad-hash` | `CorruptSnapshot` | `EvidenceDigestMismatch` | false | yes (harness commits then flags digest mismatch) |
| `diff-no-advance` | `PrePublication` | `InvalidHandle` | true | no (reject before publication; `SnapshotBaseMismatch` exists in `STABLE_ERROR_IDS` but is not emitted by any shipped FaultPoint) |

**Production axes still open** (not frozen, not executed as snapshot encoder benches):

| axis | observe when a production snapshot encoder exists |
| --- | --- |
| long Pin | resident pin bytes vs published root bytes |
| high write during encode | write tail latency; capture bytes stay at cut revision |
| sparse Diff | encoded Diff size vs full payload |
| dense Diff | encoded Diff size; CPU of encode |
| multi Capture | peak memory with overlapping `VoxelCaptureRef` |

## 5. Measurements

R-00047 is met. The seam executes against shipped `DeterministicExecutor` / `VoxelPortHarness` / `FaultPoint` with schema_id `voxel-snapshot-payload`. `replay_all()` ran; `measurements_executed()` returned `true`.

Corpus, three in-process repeats each, `Trace.snapshot` hex (identical across all three runs and across both host legs):

| corpus id | three-run identical | `snapshot_hash` SHA-256 |
| --- | --- | --- |
| `full-cut` | yes | `f546668ef35ac88474c07cd2b41a3be0234c297215dcde5a4044490afe89b660` |
| `partial-aoi` | yes | `8f47cbbd6c5061a950b4534611c07a5f80458d1fb49e5c06ba15fc9365ec0fc5` |
| `capture-ready-pinned` | yes | `ddd17aa557eacfc027e2da8baf243be2a3c18d98fed760e71c32caa76506aca9` |
| `pin-expired` | yes | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Fault matrix, three repeats each:

| fault id | error id | recoverable | committed |
| --- | --- | --- | --- |
| `pin-expired-ready` | `StaleEpoch` | true | **false** |
| `payload-bad-hash` | `EvidenceDigestMismatch` | false | true |
| `diff-no-advance` | `InvalidHandle` | true | **false** |

Read of the `pin-expired` row: its `snapshot_hash` is the canonical SHA-256 of empty input, i.e. the harness committed set is empty. A `Ready` claim behind an expired pin published nothing, which is the behaviour §3 lists as the stop condition for `pin-count-chunk-wire-diff`. That candidate is therefore **not** eliminated by this run; neither are the other two. No candidate is excluded and none is preferred — this layer measures determinism and fault semantics, not pin/COW cost.

That same value is an independent correctness check on the in-tree SHA-256: `printf '' | shasum -a 256` on this host returns the identical digest.

**Not observed / not frozen:** pin vs COW hold strategy, pin budget, memory amplification, encoded Diff bytes, write tail latency, materialize rule. These need a production snapshot encoder, which does not exist in this repository; they were not modelled or estimated. `approval_status()` remains `"blocked"`. `numeric_policy_frozen()` remains `false`.

Generated `VoxelSnapshotCapture` transitions cover the happy path `Requested → … → Released`. Schema capture states also include `Cancelled` and `Failed`. The seam treats the schema enum as the frozen contract and only asserts generated transitions are a subset. That difference is not a new capture state invented here.

`cargo test -p lumio-voxel-test-support --all-features` is the harness crate test, not a Pin/COW measurement.

## 6. Proposal (not approved)

```text
SnapshotCowProposal {
  pinBudget: pending-architecture-owner,   // not a number
  diffGranularity: pending-architecture-owner,
  materializeRule: pending-architecture-owner,
  approvalStatus: blocked
}
```

Internal candidate ids in §3 remain a list. Public configuration after approval must be generated by the architecture repository, not handwritten here.

## 7. Architecture owner approval

- Record: **none**
- `approvalStatus`: **blocked** — unchanged by this run. Executing the seam is not self-approval; nothing here selects a default.
- Blocked reason, restated against current fact: the measurement precondition ("没有测量就没有据") is now **satisfied** for the harness layer — the seam runs on a linking host and §5 carries reproducible numbers. What remains is (a) an architecture-owner decision on the four open fields, and (b) production-cost axes that no repository-side harness can supply without a snapshot encoder. (b) does not block the owner from deciding (a) on the frozen-contract grounds in §2–§3; it does block any numeric cost comparison between the three candidates.
- Who must decide: architecture owner, confirming D-014 / VOX-D-005 (date, owner, selected value, rejected alternatives, affected ADR/Manifest).
- What must be decided: pin-vs-COW hold strategy, pin budget field in the generated config snapshot, whether any sub-chunk encoding stays internal, materialize rule.

**Blocked downstream (later cards whose live 执行前置 lists this gate):** none known. Snapshot implementation cards R-00134 / R-00135 / R-00136 / R-00137 do not list R-00061. They may implement CaptureRef/Pin **APIs** but must not freeze VOX-D-005 numerics.

**Continuable without this approval:** this evidence file and the measurement seam; any code that consumes only frozen ADR-035 wire types and `voxel-snapshot-payload` capture states.

## 8. Commands actually run

Measured 2026-08-29 on macOS (Darwin 25.5.0), Apple Silicon, at commit `13d515f`. The Windows transcript cited by the previous revision (`C:\Users\g923\AppData\Local\Temp\…\agent-R-00061.log`) is not in this repository and is superseded by the reproducible runner below.

| Command | Exit | Result |
| --- | --- | --- |
| `git fetch origin` then `cargo test -p lumio-voxel-domain` | 0 | gatekeeping: a real linked test binary runs on this host (19 tests across 5 binaries). `cargo check` was not accepted as a substitute. |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/snapshot_cow.rs` | 0 | clean; seam source untouched by this run |
| `benchmarks/decision_gates/run_seam_replay.sh snapshot_cow` | 0 | compiles the seam with `rustc --test` and **runs** it: `5 passed; 0 failed`; hashes in §5 |
| same runner, `SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin` | 0 | `5 passed; 0 failed`; output diffs clean against the x86_64 leg |
| runner re-executed three times, outputs diffed | 0 | identical across separate processes |

`rust-toolchain.toml` was not modified: the aarch64 leg goes through `rustup run <toolchain>` inside the runner. `Cargo.toml` and crate `lib.rs` were not edited; the seam stays outside the cargo workspace.

**Supersession note.** The previous revision of this gate recorded no raw hashes, so nothing here is contradicted. Sibling gates VOX-D-006/007/008 did record hashes, and those values are superseded — see VOX-D-006 §8, which documents the SHA-256 K[28] defect and the reproduction that confirms the old numbers were genuine rather than invented.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added; no increment is required for this evidence path.
