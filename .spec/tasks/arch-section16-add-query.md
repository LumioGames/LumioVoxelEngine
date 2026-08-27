---
status: pending
---

# 架构源 §16 VoxelEngine 首批模块补上 query

架构源 §16 首批只列 `world/chunk/revision/mutation/snapshot/streaming`。本仓已把 `query` 拆为独立 P0 模块，这是实现粒度细化，需要回源仓模块地图，避免后续仓库按 §16 把 Query 藏进 chunk。

## 涉及范围

- `/Users/cui/LumioGames/LumioGameEngineArchitecture/docs/architecture/LumioGameEngine_Architecture_v1.0.md`

不修改 Schema、ID Registry 或本仓 `modules/README.md` 的模块清单（本仓已含 query）。若该节同步到本仓只读镜像，镜像更新与 Baseline 规则按架构源发布流程执行，不在本卡改镜像。

## 验收标准

- [ ] §16 VoxelEngine 首批子模块包含 `query`，且不把 Query 描述为 chunk 或 world 的内部细节。
- [ ] `spatial`、`migration`、`mesh-collision` 仍可留在后续子模块；不得把 mesh-collision 升为 V1 必需 Port 依赖。
- [ ] 正文不改变仓库所有权：VoxelEngine 仍不拥有 Gameplay、ECS、Host 或跨域 SnapshotCut。

## 依赖

无
