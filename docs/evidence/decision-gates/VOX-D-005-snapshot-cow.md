# VOX-D-005 Snapshot Pin/COW and sub-chunk Diff granularity

- Card: R-00061 / GATE-005
- Role: Voxel 性能与架构决策工程师
- Recorded: 2026-08-28
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Gate owner files: this document; `benchmarks/decision_gates/snapshot_cow.rs`
- `approvalStatus`: `blocked`
- Architecture owner approval: none

This is a research gate. It records candidates and a measurement seam. It does not freeze numeric defaults, pick a default algorithm, edit Schema/ID/default config, or implement production snapshot code.

Produces: `DecisionEvidenceVOXD005`; `SnapshotCowProposal{pinBudget,diffGranularity,materializeRule,approvalStatus}`.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Voxel worktree HEAD | `1175b08808a3fc865f70ebfbfa66c576562864e2` (detached, includes R-00034 `8c49fba` and R-00041) |
| Architecture HEAD | `d3252a8886b4bfd56fbb08490c3db0e6fc8c9550` (`main`, matches card lock) |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| ADR 0007 SHA-256 | `250cc1cb86d3cecc1f3c0e9fa5096436a9e8a96d8020c949a979dd4d5801a158` |
| Architecture `DECISIONS_PENDING.md` SHA-256 | `65d839c5732825a3392daf76e1b22797d1f97928b328df409ebc544b1191467f` |
| Architecture ADR-035 SHA-256 | `d55c8c6bc7c7381305a956a1f9f4f9f66dec4b70b8d4f0ae24cdabfd946b166c` |
| Prerequisite R-00034 | Consumable. Workflow status `in_review` with evidence; worktree contains `docs(R-00034): adopt LGE-V1.4 implementation baseline`. |
| Prerequisite R-00047 | **Unmet.** Live card is `backlog` / unimplemented. `crates/lumio-voxel-test-support/src/lib.rs` SHA-256 `d0467f529132ef0b91227af1f8df26a5729e871873a1590b706f7fbbda32069d` exposes only crate-DAG / generated-clean guards. No `VoxelPortHarness`, deterministic executor, fault injector, or fixture runner. No substitute harness was invented. |

## 2. Frozen contract vs open fields

**Already frozen (do not reopen, do not copy field layouts here):**

- Runtime owns `SnapshotCut`; Voxel owns `VoxelCaptureRef` (仓内 ADR 0001).
- Running snapshot: short barrier to pin, encode in background (仓内 ADR 0004). Restore is `decode → world.restore → materialize_pages + restore_stamps`, never Streaming Load.
- ADR-035 wire: canonical chunk order, `SameCutSameBytes`, chunk-granular strictly-advancing diffs, capture lifecycle `Requested → … → Ready` only while `pinState = Pinned`.
- Payload kinds, ErrorCode `SnapshotBaseMismatch` (1039), envelope remaining `snapshot-header`.

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
| `pin-count-chunk-wire-diff` | pin-count over published pages; numeric cap unfrozen | keep ADR-035 chunk-granular wire; no internal sub-chunk encoding | lazy: hold pins until encode/release | unversioned | Apache-2.0 (in-tree) | ADR-035 `d55c8c6b…946b166c` | Stop if pin expiry can still emit `Ready`, or concurrent mutation mutates captured bytes. Not excluded: no measurements. |
| `page-cow-internal-page-diff` | COW page clones on first write after pin; numeric cap unfrozen | internal page/block encoding still emitted as chunk-granular wire | hybrid: COW dirtied pages, pin clean pages | unversioned | Apache-2.0 (in-tree) | D-014 `65d839c5…1191467f` | Stop if internal encoding is leaked as a new public field. Changing wire granularity needs a new ADR. Not excluded: no measurements. |
| `eager-full-copy-chunk-diff` | copy covered pages at cut; pin budget becomes copy budget (still unfrozen) | ADR-035 chunk-granular wire only | eager materialize at cut, then drop live pins | unversioned | Apache-2.0 (in-tree) | 仓内 ADR 0004 (Pin/COW path) | Stop if cut barrier includes encode/fsync or if copy fails by silently sharing mutable pages. Not excluded: no measurements. |

No OSS codec/vendor is a candidate here. Canonical encode stays on the generated codec. Adding a compression backend is VOX-D-002, not this gate.

## 4. Measurement plan (not executed)

Fixed once R-00047 is consumable: machine, toolchain (`rustc 1.98.0` / workspace `rust-toolchain.toml`), seed, corpus, schedule. Repeat each input three times and compare SHA-256 of raw traces. Statistics: median / p95 / p99 of memory amplification, encoded bytes, write tail latency; hash equality for determinism. No summary-only charts.

**Benchmark matrix** (card):

| axis | observe |
| --- | --- |
| long Pin | resident pin bytes vs published root bytes |
| high write during encode | write tail latency; capture bytes stay at cut revision |
| sparse Diff | encoded Diff size vs full payload |
| dense Diff | encoded Diff size; CPU of encode |
| multi Capture | peak memory with overlapping `VoxelCaptureRef` |

**Fault matrix** (card):

| fault | required observable |
| --- | --- |
| Pin over budget | `BudgetExceeded`; no `Ready`; previous Active untouched |
| Capture cancel | pins/buffers released; published root unchanged |
| concurrent write | captured pages immutable; live writes create new versions |
| corrupt Diff | reject; no published-root pollution; no early unpin of live refs |

**Replay commands (after R-00047):**

```text
cargo test -p lumio-voxel-test-support --all-features
# then, using VoxelPortHarness + FaultInjector, three runs per (candidate, corpus, schedule)
# hash raw traces; do not promote any candidate from this file
```

## 5. Measurements

**未执行** because R-00047 is unmet. Correctness, determinism, and fault matrices have no raw results. No candidate is excluded. No numeric pin budget, Diff grain, or materialize rule is written into production or this proposal.

`cargo test -p lumio-voxel-test-support --all-features` only exercises workspace DAG / generated-clean guards. That is not a Pin/COW measurement.

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

**Continuable without this approval:** this evidence file and the measurement seam; any code that consumes only frozen ADR-035 wire types.

## 8. Commands actually run

Full transcript: `tests-R-00061.log` (implementer scratch; not in this repo).

| Command | Exit | Result |
| --- | --- | --- |
| `rustfmt --edition 2024 --check benchmarks/decision_gates/*.rs` | 0 | after one rustfmt apply |
| `rustc +stable-x86_64-pc-windows-gnu --edition 2024 --test benchmarks/decision_gates/snapshot_cow.rs` | 0 | `tests::gate_remains_blocked` ok (`approval_status() == "blocked"`) |
| `node .spec/tools/spec-lint.mjs` | 0 | `spec-lint: OK` (local junctions for `.claude/*` placeholders; not committed) |
| `node --import windows-symlink-junction.mjs --test .spec/tools/spec-lint.test.mjs` | 0 | 13/13 pass |
| `cargo fmt --all -- --check` | 0 | workspace members only; seams not in Cargo.toml |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | msvc check (no link) |
| `cargo test -p lumio-voxel-test-support --all-features` | 101 (msvc: no `link.exe`; gnu: pre-existing live DAG metadata false-positive, not this card) | no `VoxelPortHarness`; measurements 未执行 |

Host `rust-toolchain.toml` stays `1.98.0` msvc. GNU rustc was used only to link seam tests; toolchain file was not modified.

Knowledge index was not edited (shared hotspot). No `knowledge/` document was added; no increment is required for this evidence path.
