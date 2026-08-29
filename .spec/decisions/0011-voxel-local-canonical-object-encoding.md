# 0011 · Voxel 自持类型化 canonical 对象编码，不再经上游 `canonical_object_pairs`

- 日期:2026-08-29
- 状态:生效

## 背景

上游发布的 `canonical_object_pairs`(vendored 于 `crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/lib.rs:40`)按名排序后直接拼接:成员名不转义、值不加引号原样拼、重复名不拒。本仓经 `pub use` 再导出并在六处生产点消费,另有四份各自复制的 `quote()`(只加引号、不转义)。

后果是可利用的:`canonical_fingerprint` 把调用方 `fields` 的名与值**双双裸推**,于是一个 `,"k":` 片段就能凭空造出或吞掉成员。语义不同的两个请求可以拿到同一指纹,`ReceiptLedger::check_entry` 判为重放,`commit` 直接返回上一份 receipt——请求从未执行,调用方却收到成功回执。正确判据应是 `RevisionConflict`。解码侧 `decode.rs` 按裸引号切分,与编码侧**错得一致**,所以 round-trip 反而通过,recanonicalize 守卫按构造恒过;`tests/mutation_receipt.rs` 的期望值内联复刻了同一份实现,断言在有无转义时恒成立。

裁决(`LumioGameEngineArchitecture/docs/plans/2026-08-29-canonical-object-pairs-adjudication.md` §3.2/§3.4)否决了「只补转义」:补转义仍是拼接,单射性仍载荷在「所有调用方都传对了」上。同一份裁决 §2 F1 认定该 helper **不是** `CanonicalJsonV1` 的实现——该形态的成员名文法 `^[A-Za-z][A-Za-z0-9]*$` 把本仓全部真实成员名(`txn_id`、`c:0:0:0`、`chunkRevision.c:0:0:0`)排除在外,把它宣称为该形态等于盖假合规章。

## 决策

- 编解码落 `lumio-voxel-ops::canonical`,单一实现:`CanonicalObject` + `CanonicalValue::{Text, Uint, TextArray}`。**值自持类型**,由编码器决定分隔方式,调用方交不出预编码字节。
- 字符串恒加引号并转义 `"`、`\` 与 C0 控制字符;整数裸出且无分隔符;成员按名的码点序排列。故 `" , : { } [ ]` 只出现在结构位。
- **重复成员名一律拒绝**,不作最后写入生效。`MutationRequest.fields` 中与契约成员同名的键因此被拒,`canonical_fingerprint` 随之返回 `Result`。
- `fields` 的值按其声明类型编码为字符串,**不按名特判成整数**——「这个名字其实是整数」正是要拆掉的那类「靠调用方传对」的锚。
- 解码是真正的逆:解析后重新编码并要求字节相同。解析器**故意比编码器宽**(接受任意成员顺序与任意 `\uXXXX`),让这道守卫承重,而不是按构造恒真。孤代理转义按具名错误拒绝,与 C# UTF-16 侧对称。
- 指纹输入含形态成员 `canonicalForm = VoxelCanonicalObjectV1`,让格式**自我识别**。该 id **不是**上游的 `CanonicalJsonV1`,也不是其未来的 `CanonicalObjectV1`。
- `lumio-voxel-contracts` **不再再导出** `canonical_object_pairs`。生成物不得手改,故在 `#[path]` 接缝处隔离并 `allow(dead_code)`。
- 期望摘要由 `tools/canonical/canonical_encoding_oracle.py`(按书面规则独立实现)产出,测试不得自产期望值。该脚本同时保留旧编码,仅供历史 receipt 回溯,不得被生产代码引用。

## 后果

- **指纹语义断代**:`fields` 非空的 mutation 指纹全部改变,`fields` 为空的也因形态成员改变。本仓无任何持久化(全 workspace 的 `fs::write` / `File::create` / `write_all` 只在两个测试文件),重放判定是「现算 vs 内存」,故幂等重放窗口等于进程生命周期,无需数据迁移。
- **snapshot / receipt / restore / query 四个面的字节不变**,因为这四处本来就把值分好了类型。改动前写出的 snapshot 改动后仍可 restore。逐面对拍表见 [`docs/evidence/canonical-encoding-goldens.md`](../../docs/evidence/canonical-encoding-goldens.md)。
- Host 侧若存过 receipt,其中的 fingerprint hex 不能再由当前代码复算;本仓查不到是否存在这类数据,未核实。
- 单射性仍是结构性论证而非机器证明——本仓无 fuzz 基建。这一条与裁决 §5 gap 1 同源,本条不消灭它。
