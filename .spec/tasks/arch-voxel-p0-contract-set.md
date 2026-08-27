---
status: pending
---

# 在架构源发布 Voxel P0 公共契约与 participant receipt

Architecture Gate 尚未提供 World/Port、Chunk/Block/Page、Revision、Query 与 Mutation 参与者的 Schema、ID、错误和 Fixture。本仓模块 README 只保留候选接口，不得自行冻结字段。工作在 `LumioGameEngineArchitecture` 完成，再同步本仓只读镜像。

## 涉及范围

- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/index.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/common.schema.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/ids/index.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/docs/adr/ADR-003-cross-world-txn.md`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/schemas/cross-world-txn.schema.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/docs/architecture/DECISIONS_PENDING.md`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/index.json`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/valid/`
- `/Users/cui/LumioGames/LumioGameEngineArchitecture/fixtures/invalid/`

本卡将新增（名称可按源仓命名规则微调，但必须登记进 `schemas/index.json` 与 `ids/index.json`）：Voxel World/Port Schema、Chunk/Block/Page Schema、Voxel Revision/`ChunkRevisionSet` Schema、Query consistency Schema、Mutation participant receipt/status Schema，以及 Duplicate、Lost Result、RevisionConflict、Crash-between-markers、缺 Chunk、坐标边界的正反 Fixture。

不修改 Spatial/Mesh 公共 Wire Schema。不修改本仓 `modules/` 字段布局。

## 验收标准

- [ ] `schemas/index.json` 登记全部新增 P0 Voxel Schema，`owner` 为 VoxelEngine 或 Architecture，`priority` 为 P0。
- [ ] World 创建参数、Role、WorldId、Context/Generation、Capability、Handle 生命周期与 Port 方法/错误有 Schema，且本仓 README 不再复制字段布局。
- [ ] Chunk 坐标（含负坐标）、ChunkId、Block 值、页版本/长度/Hash/Compression 有 Schema 与边界/损坏 Fixture。
- [ ] Query 契约冻结：目标 Revision 在请求开始时绑定、多 Chunk 同一 WorldRevision、continuation 绑定原 Revision、缺 Chunk 不得伪装空世界。
- [ ] Mutation participant 状态为 Prepared/Applied/Aborted/Duplicate，不包含全局 CommitIntent；`status(txnId)` 在缓存淘汰与崩溃后的语义有 Schema。
- [ ] ADR-003 补充 participant receipt 耐久位置、retention/pruning handshake，以及 Duplicate、Lost Result、Crash-between-markers Fixture。
- [ ] 在架构源执行 `python3 tools/lumio_contract.py validate` 通过。
- [ ] 不把 Spatial/Mesh 内部缓存键写成跨仓 Schema。

## 依赖

无

## 接口

Consumes: 无。

Produces: 已登记的 P0 Voxel Schema 文件名与 ID；Query 一致性枚举名；participant receipt 类型名与 `status(txnId)` 返回值集合；ADR-003 中 receipt 耐久模型的选定条款。下游卡只引用这些生成名，不发明字段。
