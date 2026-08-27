# 0006 · 按分层合并 crate，不按逻辑模块开仓

- 日期:2026-08-27
- 状态:生效

## 背景

十个逻辑模块若机械映射为十个 crate，会把 sibling 做成环，也会把测试支撑和生成契约散进领域实现。0003 已冻结分层与「禁止 generic common / 全局单例 / 无界 Event Bus」，但未给出物理 crate 名。Foundation 引入 Cargo 前必须有一张可执行的 map。

## 决策

物理 crate 按 0003 分层合并；逻辑模块仍是目录与 README 的边界，不是 crate 边界。

| Crate | 层 | 收录 | 不收录 |
| --- | --- | --- | --- |
| `lumio-voxel-contracts` | L0 | 架构源生成的 Schema/ID/错误/Capability 绑定；只读、不手改 | 任何领域逻辑 |
| `lumio-voxel-domain` | L1+L2 | `chunk` 存储与页、`revision` 账本、ReadView / WriteSet / CommitBatch / Availability / Storage Port | 互调服务；query/mutation API |
| `lumio-voxel-ops` | L3 | `query`、`mutation`、`snapshot`、`streaming`（feature 可关 snapshot/streaming） | 组合根；空间投影 |
| `lumio-voxel-world` | L5 | 组合根、Barrier 闸门、`IVoxelWorldPort` 实现、实例生命周期 | Chunk 内部布局；Host/Runtime |
| `lumio-voxel-project` | L4 | `spatial`、`mesh-collision`（可选 feature） | 权威写入 |
| `lumio-voxel-migration` | Tool | 节点提供者，独立二进制/lib | Tick 热路径 |
| `lumio-voxel-test-support` | 测试 | Reference Port、Golden/Property harness、故障注入、与 Native Port 共用的契约夹具 | 生产默认依赖 |

约束：

- `lumio-voxel-domain` 内 `chunk` 与 `revision` 仍是 sibling 模块，禁止服务互调；只接受 `mutation` 持有的受控 publish 能力。
- `lumio-voxel-world` 依赖已启用 crate；L0–L4 与 Tool 不得依赖 `lumio-voxel-world`。
- Reference Port 与 Native Port 只通过 `lumio-voxel-contracts` 与同一 Port 表面对话，不得各写一套字段。
- Storage 后端经 `Storage Port` 接入 `lumio-voxel-domain`，第三方实现不得进入 `ops`/`world`。
- 不设 `lumio-voxel-common`、全局单例、无界 Event Bus。

Foundation 最小集：`contracts` + `domain` + `ops`（query/mutation）+ `world` + `test-support`。`project`、`migration` 与 `ops` 的 snapshot/streaming feature 可晚于单域闭环启用。

## 后果

首次引入 Cargo 时按此开 crate 并更新 [`testing.md`](../knowledge/standards/testing.md) 的收口命令。目录仍按逻辑模块组织；crate 的 `mod` 映射不得反向依赖。跨仓 compile DAG 不变：源码只依赖 `LumioNativeCore` 与生成契约。
