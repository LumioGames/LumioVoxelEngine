# VOX-D-005 Snapshot Pin/COW and sub-chunk Diff granularity

- Card: R-00061 / GATE-005
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28
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

## 4. Measurement plan (harness seam)

Fixed: machine, toolchain (`rustc 1.98.0` / workspace `rust-toolchain.toml` msvc), seed `61`, corpus, schedule. Repeat each input three times and compare `Trace` / `snapshot_hash` equality. Statistics for pin/COW memory amplification, encoded bytes, and write tail remain **unfrozen** — the shipped harness does not pin pages or encode production snapshots. No summary-only charts. No invented hashes.

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

**Replay commands:**

```text
cargo test -p lumio-voxel-test-support --all-features
cargo build -p lumio-voxel-test-support --lib --all-features
rustc --edition 2024 --crate-type rlib --crate-name vox_d_005_seam -L target/debug/deps --extern lumio_voxel_test_support=<rlib> --extern lumio_voxel_contracts=<rlib> benchmarks/decision_gates/snapshot_cow.rs -o seam-out/vox-d-005.rlib
# when a host linker exists: rustc --test the same seam and run three-run replay_all()
```

## 5. Measurements

R-00047 is met. The seam compiles against shipped `DeterministicExecutor` / `VoxelPortHarness` / `FaultPoint` with schema_id `voxel-snapshot-payload`.

**Observed on this host:** compile-only. `cargo test -p lumio-voxel-test-support --all-features` cannot link (`link.exe` missing). `cargo build --lib` and `rustc --crate-type rlib` succeed. An extra `rustc --test` using `lld-link` failed for missing `kernel32.lib`. Therefore **raw three-run `snapshot_hash` values are not recorded** (not invented).

**Not observed / not frozen:** pin vs COW hold strategy, pin budget, memory amplification, encoded Diff bytes, write tail latency, materialize rule. `approval_status()` remains `"blocked"`. `numeric_policy_frozen()` remains `false`. No candidate is excluded.

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
- `approvalStatus`: **blocked**
- Who must decide: architecture owner, confirming D-014 / VOX-D-005 (date, owner, selected value, rejected alternatives, affected ADR/Manifest).
- What must be decided: pin-vs-COW hold strategy, pin budget field in the generated config snapshot, whether any sub-chunk encoding stays internal, materialize rule.

**Blocked downstream (later cards whose live 执行前置 lists this gate):** none known. Snapshot implementation cards R-00134 / R-00135 / R-00136 / R-00137 do not list R-00061. They may implement CaptureRef/Pin **APIs** but must not freeze VOX-D-005 numerics.

**Continuable without this approval:** this evidence file and the measurement seam; any code that consumes only frozen ADR-035 wire types and `voxel-snapshot-payload` capture states.

## 8. Commands actually run

Full transcript: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00061.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/snapshot_cow.rs` | 0 | clean after one rustfmt apply |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 | expected: linker `link.exe` not found (msvc); crate compiled then failed to link tests |
| `cargo build -p lumio-voxel-test-support --lib --all-features` | 0 | `Finished dev` profile |
| `rustc --edition 2024 --crate-type rlib --crate-name vox_d_005_seam -L target/debug/deps --extern lumio_voxel_test_support=<rlib> --extern lumio_voxel_contracts=<rlib> benchmarks/decision_gates/snapshot_cow.rs -o …/seam-out/vox-d-005.rlib` | 0 | wrote `vox-d-005.rlib` (415434 bytes after rustfmt recompile; no warnings) |
| extra `rustc --test` + `lld-link` | 1 | missing `kernel32.lib`; no test hashes |

Host `rust-toolchain.toml` stays `1.98.0` msvc. Toolchain file was not modified. `Cargo.toml` / crate `lib.rs` were not edited.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added; no increment is required for this evidence path.
