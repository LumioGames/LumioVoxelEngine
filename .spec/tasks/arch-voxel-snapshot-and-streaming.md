---
status: pending
---

# 在架构源发布 Voxel Snapshot payload 与 Streaming 耐久回执契约

通用 `SnapshotHeader` 只是 envelope。运行中 Snapshot、Pin/COW 投影、Diff payload，以及 Dirty Chunk 的 DurabilityAck / 禁用 Unload Capability，必须在架构源冻结后本仓才能实现 P1。

## 涉及范围

- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/index.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/snapshot-header.schema.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/host-capability.schema.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/docs/adr/ADR-010-persistence-config.md`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/docs/architecture/DECISIONS_PENDING.md`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/valid/`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/invalid/`

本卡新增 Voxel Snapshot/Diff payload Schema、Cut 到 Voxel Revision 的投影字段、Streaming Load/Unload/Availability/DurabilityAck Schema，以及并发写、Pin 失效、磁盘满、回执丢失 Fixture。不修改本仓模块所有权表（Cut 仍归 Runtime，CaptureRef 仍归 Voxel）。

## 验收标准

- [ ] Voxel payload Schema 覆盖 Chunk 顺序、页索引、局部 Snapshot、Diff base/target，且明确不等于 `SnapshotHeader` envelope。
- [ ] Schema 或 ADR 写明：Runtime 固定 `SnapshotCut`；Voxel 只接收不可变 Cut 并输出 Canonical bytes。
- [ ] DurabilityAck 标识已覆盖的 Chunk 集合与 SnapshotId 或 WAL 位点；未 ack 的 Dirty 不得 Unload。
- [ ] HostCapability 可声明全驻留/禁用 Dirty Unload；Dedicated Server 默认不得驱逐未获恢复保障的 Dirty Chunk。
- [ ] 正向 Fixture：Cut 投影一致、Diff round-trip。反例 Fixture：Pin 失效仍宣称 Ready、未 ack 即 Unload、Hash 不匹配。
- [ ] 在架构源执行 `python3 tools/lumio_contract.py validate` 通过。

## 依赖

- arch-voxel-p0-contract-set

## 接口

Consumes: `arch-voxel-p0-contract-set` 产出的 WorldRevision、ChunkRevisionSet、ChunkId 与 World Context/Generation 类型名。

Produces: Voxel Snapshot/Diff payload Schema id；DurabilityAck 类型名；禁用 Unload 的 Capability 名。
