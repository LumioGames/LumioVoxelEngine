# 0009 · 随镜像同步消费上游已发布的 ADR-040 / ADR-041 产物

- 日期:2026-08-28
- 状态:生效

## 背景

修正生成契约运行时的 SHA-256 轮常量 `K[28]`(见 [`docs/evidence/b0-verification.md`](../../docs/evidence/b0-verification.md) §4)
需要重新同步 `crates/lumio-voxel-contracts/generated/` 只读镜像。同步时上游 `LumioGameEngineArchitecture`
的 `origin/main` 已前进,携带两批**已发布**产物:

- `44f617b` ADR-040 Root ABI generated bundle
- `a4a7956` ADR-041 canonical / digest profiles

镜像因此从 52 文件增至 **58**,`tools/architecture/generated-lock.json` 同步为 58 条。

这不是可选项:`canonical-serializer-csharp` 与两个 `language-binding` 包各自新增了源文件,其 descriptor
声称的 `outputHash` 只有镜像带上这些文件才重算得出;只更新原 52 个文件会让摘要对不上、
`artifact_hashes` 变红(上游与本仓各自实测过)。

## 决策

- **按既定流程接受。** [`workflow.md`](../knowledge/standards/workflow.md) 规定「公共架构变更不从本仓直接
  发起:先在 `LumioGameEngineArchitecture` 完成 ADR、Schema、正向/失败 Fixture、Baseline 与契约校验,
  再同步本仓只读镜像」。两批产物已在上游 `origin/main` 完成并发布,同步镜像正是该流程的下半段;
  镜像缺这六个文件才是落后状态。
- **不视为本仓发起的 ABI 变更**,因此不触发「必须升级」的架构所有者裁决路径。依据:BaselineId 未变
  (`LGE-V1.4-2026-08-27`)、schemaEpoch 仍为 1、五元组由上游签发而非本仓填写。
- **登记随之而来的公共契约面变化**:生成的 `SCHEMA_IDS` 新增两项 —— `root-abi-bundle` 与
  `canonical-digest-profile`。`lumio-voxel-contracts` re-export `SCHEMA_IDS`,故本仓公共面确实随之扩大。
  本条即为该扩大的记录落点。
- **Voxel 域不消费 Root ABI 面。** 那批常量在本仓是 vendored 进私有 mod 且未 re-export 的不可达项,
  以 vendoring 接缝上的 `#[allow(dead_code)]` 容纳(见 [`b0-verification.md`](../../docs/evidence/b0-verification.md) §6),
  不为消除 lint 而把它们 re-export 进公共 API。

## 后果

- 本仓公共契约面随上游发布被动扩大。今后上游发布新产物时,镜像同步会再次带来同类扩大;
  若某次扩大触及 Voxel 实际消费的语义(而非本次这种纯新增、域内不消费),须另立 ADR 并升级架构所有者。
- `docs/evidence/v1.4-generated-artifact-gate.md` 自称唯一 Gate owner,但其 `compilerHash` / 部分
  `outputHash` / `GateResult.ready` 未随本次发布重算,现已在该文件内标注为历史值。
  **重算是 Gate 所有者的职责,消费方不就地改写。** 这是本条留下的已知缺口。
- 镜像是**钉住**的而非跟踪:上游 `origin/main` 已再次前进,本仓镜像仍对应引入本次修正的那个发布点。
