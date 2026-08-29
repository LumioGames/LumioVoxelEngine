# 0010 · generated-clean 守卫保留自己的 SHA-256，不复用被审计树内的实现

- 日期:2026-08-29
- 状态:生效

## 背景

仓内有两份手写 SHA-256 压缩函数,且长期都没有 known-answer 测试:

- `crates/lumio-voxel-contracts/generated/rust/lumio-gen-contract-runtime/src/sha256.rs`——生成物,服务 Hash 链与 canonical 校验;
- `crates/lumio-voxel-test-support/src/generated_clean.rs`——`generated_clean` 守卫自带的一份,用来对 `crates/lumio-voxel-contracts/generated` 整棵树逐文件求哈希,比对 `tools/architecture/generated-lock.json`,以发现「生成目录被手改」。

架构仓曾出过 K[28] 轮常量写错、对任意输入算错摘要、无 KAT 守护而长期未被发现的事故(同族问题,R-00290 因此立卡)。表面上「两份实现应当合一」,但两份实现处在**不同信任域**:`lumio-gen-contract-runtime/src/sha256.rs` 本身就是 lock 清单里的一条被锁条目,即被守卫审计的对象之一。

## 决策

- **两份实现保留,不合一。** `generated_clean` 守卫继续使用它自己的 SHA-256,不改为调用 `lumio_voxel_contracts::sha256`。
  理由:用被审计树里的哈希器去审计该树,构成信任自证循环——篡改生成目录的人只要一并改掉 `sha256.rs`,即可让被篡改的文件通过自己的哈希比对;同样,该生成实现一旦有 bug,守卫会静默地对所有文件放行。守卫的独立性是它的全部价值。
- **保留的代价用测试补齐,而不是用信任补齐**:`crates/lumio-voxel-test-support/tests/sha256_kat.rs` 用 FIPS 180-4 公布向量同时锁住**两份**实现,并以长度扫描做差分,确保两份副本不漂移。
- 两份实现各自的存在理由写进代码注释,避免后续被「顺手合并」。

## 后果

- 接受一份约 60 行的算法重复,换取守卫对被审计对象的独立性。
- 重复的漂移风险由 KAT 与差分测试承担;`cargo test --workspace --all-features` 已在 `.github/workflows/repository-policy.yml` 的门禁内,故 KAT 由 CI 强制执行。
- `tools/architecture/check_generated_clean.py` 用标准库 `hashlib` 做同一份 lock 的第二次独立校验,与 Rust 守卫互为旁证,本条不改动它。
- 若将来引入经审计的第三方 SHA-256 crate,应当替换的是**生成侧**;守卫侧是否跟进需另开 ADR 重新评估独立性。
