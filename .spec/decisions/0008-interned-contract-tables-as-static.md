# 0008 · 三张 interned 契约表以 `static` 而非 `const` 再导出

- 日期:2026-08-28
- 状态:生效

## 背景

`lumio-voxel-contracts` 原本以 `pub use` 直接再导出生成侧的 `CHUNK_PRESENCE`、`SCHEMA_IDS`、`BINDINGS`。
生成物把它们声明为 `pub const`。

Rust 的 `const` 是**逐使用点内联**的:每个消费 crate 会各自实体化一份副本,`&'static str` 数据的地址在
crate 之间没有任何保证。而本仓的 intern 语义(`ChunkSlot::presence`、`GeneratedVoxelWorldPortAdapter`
的 `intern_schema` / `intern_binding`)靠 `TABLE.iter().find(...)` 把表内元素**按引用**交回,调用方再用
`std::ptr::eq` 断言「这个标识符确实来自生成表,而不是手敲的重复字面量」。

`const` 与 `ptr::eq` 这两件事放在一起是不成立的:跨 crate 比较的是两份不同的 per-crate 分配。
2026-08-28 本机首次真实链接执行测试矩阵时暴露:三条 intern 断言中有两条其实一直只是靠链接器合并相同字面量
**侥幸通过**,第三条(`binding_rust_type`)没被合并,于是失败。此前 Windows 宿主缺 `link.exe`,这些断言从未
真正执行过,所以这个问题在历史上从未显形。

## 决策

- `lumio-voxel-contracts` 对 `CHUNK_PRESENCE`、`SCHEMA_IDS`、`BINDINGS` 三张表改用 `pub static` 绑定
  再导出,使全 workspace 只有一次实体化,intern 交回的引用因此具有稳定的规范地址。
- 其余再导出(`BASELINE_ID`、`MACHINE_IDS`、`STABLE_ERROR_IDS`、`Transition`、`VOXEL_WORLD_ROLES`、
  `Binding`、`machine_ids`、`state_transition_table`)保持 `pub use` 不变——它们不参与 `ptr::eq` 语义。
- **不修改生成物**。生成侧仍是 `const`;本条只改本仓手写的再导出层。让生成器改发 `static` 是上游选项,
  但那会改变生成 Artifact 的 hash 与基线五元组,不在本仓决策范围内。
- `ptr::eq` 形式的 intern 断言保留,不降级为值相等——值相等恰好丢掉这些断言唯一想证明的东西。

## 后果

- 这是对 `lumio-voxel-contracts` 公共 API 的**破坏性改动**:`static` 不能用于 const 上下文
  (数组长度、`match` 模式、其他 `const` 初始化)。已核实当前 workspace 内所有使用点都是运行时
  `.iter()` / `.contains()` 调用,无 const 上下文使用,故本次改动编译通过且行为不变。
  该 crate `publish = false`,消费者仅限本 workspace,影响面因此是封闭的。
- 未来若有消费者需要在 const 上下文使用这三张表,必须新增一条 ADR 取代本条,而不是改回 `const`——
  改回去会静默地重新引入 intern 断言侥幸通过的状态。
- 与 [0006](0006-crate-map.md) 的 crate 边界不冲突:没有新增 crate,依赖方向不变。
