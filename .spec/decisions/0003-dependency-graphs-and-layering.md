# 0003 · 用三张图描述依赖，逻辑模块不必等于 crate

- 日期:2026-08-27
- 状态:生效

## 背景

模块 README 用「上游/下游」同时表示编译依赖、调用方向和数据消费，总图与正文边不一致，无法导出无环 crate DAG。十个逻辑模块若机械映射为十个互相引用的 crate，会把 sibling 做成环。

## 决策

废除「上游/下游」。仓内只使用三种边：

- `depends on`：编译 / API 依赖。
- `called by`：控制流谁发起。
- `publishes / consumes`：事件与数据。

逻辑分层（物理 crate 可合并，不得反转）：

```text
L0  generated-contracts / NativeCore published types
L1  chunk-store  |  revision-ledger     （sibling，不互调）
L2  ReadView / WriteSet / CommitBatch / Availability Port / Storage Port
L3  query | mutation | snapshot | streaming
L4  spatial | mesh-collision
Tool  voxel-migration-node-provider
L5  world composition root
```

约束：

- `world` 依赖已启用模块；L0–L4 与 Tool 不得依赖 `world`。
- 源码只依赖 `LumioNativeCore` 与架构源生成物，不编译依赖 `LumioCoreEngine`。
- `query` 不控制 Load；`mutation` 不调用 `snapshot`；投影模块只经 ReadView，不直连 Chunk Storage。
- `spatial` 与 `mesh-collision` 的缓存必须 scoped 到 World Context/Generation。
- 不引入 generic common crate、全局单例或无界 Event Bus。

## 后果

当前不落地 Cargo。实现时按分层合并 crate，不按目录数量开 crate。跨仓 compile DAG 不变。
