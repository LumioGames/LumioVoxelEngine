# VOX-D-002 Block 存储与压缩后端 — Decision Evidence

- Card: R-00058 (`01a04391-0687-775b-8f0d-2563b4d7de33`) / GATE-002
- Gate: `VOX-D-002`
- Role: Voxel 性能与架构决策工程师
- Architecture baseline: `LGE-V1.4-2026-08-27`
- Voxel worktree HEAD at record: `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` (`feat(R-00047): add deterministic harness, faults and fixture runner`)
- Recorded: 2026-08-28 (re-measure after R-00047)
- Produces: `DecisionEvidenceVOXD002`; `BlockStorageProposal`
- Exclusive files: this document; `benchmarks/decision_gates/block_storage.rs` (optional data dir unused)
- `approvalStatus=approved`
- Architecture owner approval: **`LGE-V1.4-VOX-D-P0-2026-08-28`** (Architecture `5f06822`)
- Selected internal family: `DenseUncompressedAdapter` (default codec identity `None`)
- Owner-stated precondition (「macOS 环境首次真实跑通测试矩阵」): **partially met** — the matrix was linked
  and executed on `aarch64-apple-darwin` for the first time on 2026-08-28 (153 passed / 5 failed). The 5
  failures share one upstream root cause unrelated to this gate: a wrong SHA-256 round constant in the
  generated contract runtime (see [`../b0-verification.md`](../b0-verification.md) §4). This gate's own
  seam is still not a workspace member and `measure()` remains unexecuted.

This is a research gate plus owner confirmation. It does not add crates or unaudited dependencies. `benchmarks/decision_gates/block_storage.rs` is **not** a workspace member. Public compressor defaults are not generated.

**Explicit:** no Schema, ID Registry, ABI, generated Artifact, or default config was modified by this card.

## 1. Delivery baseline (measured)

| Item | Value |
| --- | --- |
| Architecture baseline | `LGE-V1.4-2026-08-27` |
| Voxel HEAD | `b2f0d8a3763a02f805e29cbd101560ba7fdca77b` |
| V1.4 architecture document SHA-256 | `f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0` |
| Implementation blueprint SHA-256 | `32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa` |
| V1.3 `DECISION_GATES.md` SHA-256 | `4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2` |
| V1.3 `MANIFEST.sha256` file SHA-256 | `8c94d6b2680e331007dfab6961ef094a9745faee2084993f5ac0498f7161d3e6` |
| Seam `block_storage.rs` SHA-256 | `812f1219c606a541679e9887341a4abd156204e7381d408e104d2a7b22fbce51` |
| Toolchain (declared / ran) | `rust-toolchain.toml` channel `1.98.0`; `rustc 1.98.0 (88d9e12ae 2026-08-18)`; `cargo 1.98.0 (797e8a9bc 2026-08-05)` |
| Prerequisite R-00034 | Consumable (`in_review` with evidence). |
| Prerequisite R-00047 | **Met** at HEAD `b2f0d8a`. Shipped: `DeterministicExecutor` / `Schedule` / `Trace`, `VoxelPortHarness` / `GeneratedVoxelOperation`, `FaultPoint`. |
| Prerequisite of R-00047: R-00045 | Consumable (`c938868` generated artifacts). R-00037 `GateResult.ready=true`. |

Historical unapproved source only (not authority, not a selected default): [`docs/LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md`](../../LumioVoxelEngine_Framework_Design_LGE-V1.3/DECISION_GATES.md) lists `VOX-D-002` as `unapproved` with rule “Expose configuration/measurement seam only; do not select a default or algorithm.”

## 2. Candidate list (names / kinds only)

No candidate is selected. The first row is **not** a conclusion (`selected_backend() == None`). Exclude-reason values are placeholders until a real compressor is licensed and measured. No crate was fetched, so license / version / crate source Hash are not computed.

| Name | Kind | Version | License | Source Hash | exclude-reason |
| --- | --- | --- | --- | --- | --- |
| DenseUncompressedAdapter | storage-backend (dense page, no compressor) | unversioned-internal | N/A (in-tree adapter family; not added) | not-computed | not-selected; pending-measurement |
| PaletteRleAdapter | storage-backend (palette + run encoding) | unversioned-internal | N/A (in-tree adapter family; not added) | not-computed | pending-measurement |
| ExternalLz4PageAdapter | storage-backend (OSS page compressor via Adapter) | pending-fetch | pending-license-audit | not-computed | pending-license-audit; unaudited crate forbidden |
| ExternalZstdPageAdapter | storage-backend (OSS page compressor via Adapter) | pending-fetch | pending-license-audit | not-computed | pending-license-audit; unaudited crate forbidden |

Selected default: **none**. Strong copyleft or unaudited crates are a stop condition; they were not introduced.

## 3. Frozen contract vs internal candidate vs public value

| Class | What | Status |
| --- | --- | --- |
| Frozen contract (V1.4 schema exists) | ADR-024 page envelope (`schemas/voxel-chunk-page.schema.json`); generated schema ids `voxel-chunk-page`, `voxel-query`, `voxel-mutation-receipt`; `common.schema.json` `CompressionCodec` identity enum (codec *name* space, not a default codec); Adapter / Storage Port isolation in `modules/chunk/README.md` and ADR-0006 (storage backends enter through domain Storage Port, never `ops` / `world`). | Frozen by architecture source. This card did not copy Schema fields. |
| Internal candidate | The four backend families in §2. Measurement-only names in the seam. Not compiled into production crates. No `Cargo.toml` dependency added. | Internal. Not production policy. |
| Public value awaiting architecture-owner approval | Default page representation, default compressor, dictionary policy, determinism/thread-count contract for the chosen backend, and any generated config field naming a backend. After approval, public config must be generated by the architecture repository. | Unapproved. |

## 4. Measurements

R-00047 is met. The seam `use`s shipped `DeterministicExecutor` / `Schedule`, `GeneratedVoxelOperation` / `VoxelPortHarness`, and `FaultPoint`. It does **not** implement or select a compressor.

Statistical method (encoded in seam; process not executed — see commands): three `DeterministicExecutor::run` of the identical `Schedule` (`SCHEDULE_SEED = 0x0002_D002`); compare `Trace` and `Trace.snapshot` for byte identity.

Corpus (payload **labels**, not compressor bytes): `air` / `repeated` / `high-entropy`, each crossed with generated schema ids `voxel-chunk-page` / `voxel-query` / `voxel-mutation-receipt` (9 ops).

Fault matrix mapped onto shipped unrecoverable-after-visible-write `FaultPoint`s (no unaudited codec crate):

| Label | `FaultPoint` | expected `recoverable` | stable error id |
| --- | --- | --- | --- |
| `corrupt-page` | `CorruptSnapshot` | false | `EvidenceDigestMismatch` |
| `mixed-backend` | `PostPublication` | false | `PartialLoadRolledBack` |
| `unaudited-codec` | `LostResult` | false | `EvidenceMissing` |

Real encode/decode ratios, random-access, COW isolation, peak memory, and the historical negative names (truncated page, corrupt dictionary, decompress cap, thread-count divergence, backend unavailable) were **not** measured: that would require a production Storage Port plus an audited compressor crate. No bytes under `benchmarks/decision_gates/data/vox-d-002/`. No invented run hashes.

Commands actually run (cwd = this worktree):

| # | Command | Exit | Key output |
| --- | --- | --- | --- |
| 1 | `cargo test -p lumio-voxel-test-support --all-features` | 1 | compiled `lumio-voxel-contracts` + `lumio-voxel-test-support`; lib-test link failed: linker `link.exe` not found (MSVC) |
| 2 | `cargo build -p lumio-voxel-test-support --lib --all-features` | 0 | Finished dev profile in 0.02s. rlibs: `liblumio_voxel_contracts-a62c2cca441f7fde.rlib`, `liblumio_voxel_test_support-6da53ff03bc58986.rlib` |
| 3 | `rustc --edition 2024 --crate-type rlib --crate-name vox_d_002_seam -L target/debug/deps --extern lumio_voxel_test_support=target\debug\deps\liblumio_voxel_test_support-6da53ff03bc58986.rlib --extern lumio_voxel_contracts=target\debug\deps\liblumio_voxel_contracts-a62c2cca441f7fde.rlib benchmarks/decision_gates/block_storage.rs -o C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\seam-out\vox-d-002.rlib` | 0 | compile artifact `vox-d-002.rlib` size 78828; SHA-256 `35c6437d30abe84b6bbba590110e0397402e716de9ffda1bac3c46aad9324cbc` (rlib bytes, **not** a corpus run hash) |

`where.exe link.exe` exit 1 (not on PATH). Full transcript: `C:\Users\g923\AppData\Local\Temp\grok-goal-05389858a0e6\implementer\agent-R-00058.log`.

## 5. Approval

- `approvalStatus=approved`
- Architecture owner approval: **recorded**
- Approval reference: `LGE-V1.4-VOX-D-P0-2026-08-28` / Architecture commit `5f06822`
- Selected internal family: `DenseUncompressedAdapter`
- External Lz4/Zstd remain rejected as V1 default (license audit pending).

`BlockStorageProposal` (no selected backend):

```json
{
  "name": "BlockStorageProposal",
  "backend": "DenseUncompressedAdapter",
  "version": null,
  "license": null,
  "benchmarks": {
    "status": "harness-seam-compiled",
    "exit": {
      "cargo_test_lumio_voxel_test_support_all_features": 1,
      "cargo_build_lib_all_features": 0,
      "rustc_vox_d_002_seam_rlib": 0
    },
    "reason": "R-00047 met; cargo test missing link.exe; rustc rlib compiled against shipped harness; three-run Trace.snapshot not executed as a process; no production backend selected"
  },
  "determinism": "not-executed-as-process; three identical Schedule runs and Trace.snapshot compare are encoded in the seam",
  "sourceHashes": {
    "architectureBaselineId": "LGE-V1.4-2026-08-27",
    "voxelHead": "b2f0d8a3763a02f805e29cbd101560ba7fdca77b",
    "architectureMirrorSha256": "f1d36acf33a1f5e8326a9e58d609fcf7d9fa85177f9b5b60bb3f4742c1afebd0",
    "v13DecisionGatesSha256": "4850057dd8926c11c8c3beebe109d18dffdb7e84cd451426d7d635860be5ede2",
    "blueprintSha256": "32e76066eb298aad20f4149760abbeddacb6d6c43e096945f1cf0ea75b2471aa",
    "blockStorageSeamSha256": "812f1219c606a541679e9887341a4abd156204e7381d408e104d2a7b22fbce51"
  },
  "approvalStatus": "approved",
  "approvalReference": "LGE-V1.4-VOX-D-P0-2026-08-28"
}
```

## 6. Blocked requirements and continuable scope

Cannot proceed until this freeze is architecture-owner approved **and** compressor measurements plus license audit exist:

- R-00066 `[程序·配置] 建立不可变 Voxel 配置快照` — must not fill a blocked backend default.
- R-00073 `[程序·Chunk] 实现不可变 Payload、四态 Slot 与 Directory Root` — depends on R-00057 / R-00058 / R-00066.
- Downstream chunk delta / publication / mutation cards that persist pages.

May continue (does **not** need this freeze):

- Nothing that requires a frozen Block storage / compression backend.
- Independent work: crate DAG, artifact-gate evidence, sibling VOX-D cards that do not select this backend.

Stop thresholds (qualitative): non-deterministic bytes across thread counts; unaudited or strong-copyleft dependency; decompress-bomb accepted; truncated/corrupt page accepted as valid payload; mixed backend or unaudited codec treated as a recoverable retry after a visible write.

## 7. Contract

No Schema / ID / default config was modified.

## 8. 2026-08-29 修复后复测 — retest on the corrected SHA-256

Retested 2026-08-29 on macOS (Darwin 25.5.0, Apple Silicon) at commit `cc868e4`, after `51c2836`
corrected the generated contract runtime's SHA-256 round constant `K[28] = 0xc6eabbdc → 0xc6e00bf3`
(forensics: [`VOX-D-006-streaming.md`](VOX-D-006-streaming.md) §8.1, [`../b0-verification.md`](../b0-verification.md) §4).
Question answered: did any number recorded above come from the defective digest? **No.** §1–§7 stay
unchanged; `approvalStatus=approved` is untouched (the owner's signature is not this session's to re-issue).

### 8.1 Every §1 hash reproduces with system `shasum`(not polluted)

Each SHA-256 in §1 was recomputed with `/usr/bin/shasum -a 256` over `git show 54b488f:<path>`
(`54b488f` carries this document's recorded revision). All match bit-for-bit, including the seam
`block_storage.rs` (`812f1219c606a541679e9887341a4abd156204e7381d408e104d2a7b22fbce51`). The §4 rlib
SHA-256 (`35c6437d…`) is a Windows build artifact explicitly labeled "not a corpus run hash"; rlib
bytes are host/toolchain-dependent and are not re-derivable here — it carries no measurement content.

`block_storage.rs` at current HEAD hashes to
`f4acb63129754d9508dfc2cc40140c4ac0be624002e06050d71b582bca9677e1`, ≠ §1 — explained drift:
`31cb6a2` recorded the D-013 owner freeze (`"blocked"` → `"approved"` plus `approval_reference()` /
`selected_family()`); the seed/corpus/fault logic is byte-unchanged.

### 8.2 Three-run replay executed for the first time(§4 recorded process not executed)

§4 recorded the three-run `Trace.snapshot` compare as encoded-but-not-executed (no `link.exe`). It is
now executed through `benchmarks/decision_gates/run_seam_replay.sh block_storage_replay`; the committed
driver `block_storage_replay.rs` (added in `cc868e4`) only adds `--test` entry points via `#[path]` and
leaves the seam byte-untouched. Two legs (x86_64-apple-darwin Rosetta on the pinned `1.98.0`, and
`SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin`), plus a full-runner process re-run: all line-identical,
`rustc 1.98.0 (88d9e12ae 2026-08-18)`, 3 passed / 0 failed per leg.

First executed values (`SCHEDULE_SEED = 0x0002_D002`, 9 ops = 3 labels × 3 generated schema ids):

```text
VOX-D-002 run1 snapshot a73e12ba1bb503a56a62eaefa88385c25885c4a62a0bcce11f7876702f54192a
VOX-D-002 run2 snapshot a73e12ba1bb503a56a62eaefa88385c25885c4a62a0bcce11f7876702f54192a
VOX-D-002 run3 snapshot a73e12ba1bb503a56a62eaefa88385c25885c4a62a0bcce11f7876702f54192a
VOX-D-002 fault corrupt-page point=CorruptSnapshot error=Some("EvidenceDigestMismatch") recoverable=false
VOX-D-002 fault mixed-backend point=PostPublication error=Some("PartialLoadRolledBack") recoverable=false
VOX-D-002 fault unaudited-codec point=LostResult error=Some("EvidenceMissing") recoverable=false
```

The executed fault matrix equals the §4 planned table row-for-row; all three stay unrecoverable.
§4 recorded no run hash, so nothing in the original record is superseded; this is the first one.
Real compressor ratios / random-access / peak-memory remain unmeasured exactly as §4 states — they
need a production Storage Port plus an audited codec crate, which this retest does not add.

### 8.3 Counterfactual under the defective digest

The same replay at `54b488f` (pre-fix) yields snapshot
`afb6854b96ed9af057439d93630bd15a6a2943224dde310361148bc665d330b9` — what the defective runtime would
have produced. The 2026-08-28 record contains no such number, consistent with its "not executed as a
process" statement.

### 8.4 Verdict

**未被污染。** All static hashes reproduce under `shasum`; no executed run hash was ever recorded.
First executed dual-architecture replay added above; `approvalStatus` and §1–§7 unchanged.
