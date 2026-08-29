---
name: lessons
description: 经验教训——reviewer 反复退回的同类问题与 Agent 常犯坑;开工前与复盘沉淀时查
metadata:
  type: doc
  status: 已交付
---

# 经验教训（Lessons Learned）

复发问题的暂存区：记录 reviewer 反复退回的同类问题与 Agent 常犯的坑，让同一个坑不踩第三次。本文档是规范的**候选池**——条目在这里验证价值，稳定后升格，不在这里长住。

## 收录准入

- **同类问题第二次出现才收录**——单次偶发不收，防噪音。
- 来源：reviewer 退回报告、交回物的 known gaps、用户纠偏。
- 不收待办（走任务卡）；不收项目常识（进 `standards/` 或 feature 文档）。

## 条目格式

一条 lesson 一个小节，新条目加在「条目」节最上方（倒序）：

    ### <一句话规避规则>
    - 日期：YYYY-MM-DD
    - 现象：踩了什么坑、复发几次
    - 根因：为什么会发生
    - 规避：怎么做能不再犯（可验证的行为，不是口号）
    - 来源：reviewer 报告 / known gaps / 用户纠偏（附提交或任务标识）

## 升级路径

某条 lesson 被稳定复用（约第三次引用起）→ 升格为 `knowledge/standards/` 规则或 `rules/` 红线，原条目标注「已升格 → <落点>」，保留不删。

## 条目

### 「由 X 保证 / 与 X 同源」类桥接声称，必须在同一提交里对被声称的那一侧实跑，否则记「未验证」

- 日期：2026-08-29
- 现象：同一类桥接声称在两张卡上各栽一次。R-00045 卡面写「Python `dir_output_hash` 对 12 个包与 descriptor
  `outputHash` 全 OK；`verify_artifact_hashes` **同源算法**」——Python 用标准 SHA-256 确实全绿，Rust 侧
  `verify_artifact_hashes` 走的却是被污染的镜像实现（生成 runtime 的 K[28] 错误），该卡锚点 `c938868` 上
  `published_hashes_match_locked_packages` 与 `tamper_fails_then_restore_passes` 双双 FAILED。R-00041 卡面
  AC2/AC3 的证据取自 `python tools/architecture/test_guards.py`，而 CI 与收口门禁执行的是 Rust 侧
  `crate_dag.rs` / `generated_clean.rs`；同一时刻 Python 全绿、Rust 全红。
- 根因：桥接声称把「A 通过」外推成「B 通过」，中间那一步——A 与 B 真是同一份实现、同一条代码路径——从未被
  验证。它比缺证据更危险：读起来像证据，且外推方向永远朝着「已经通过」。被这样掩盖的护栏还会退化成
  「常量错但自洽」的全量误报器：红得毫无信息量，却看起来在工作。
- 规避：
  1. 证据只认**被验收项实际执行的那条代码路径**的输出。CI / 收口门禁跑哪一套，证据就得是哪一套；另一套
     实现的通过只能作为交叉验证的补充，不能顶替。
  2. 写下「同源 / 等价 / 由 X 保证」之前，在同一提交内 `grep` 出两侧实现并实跑比对，把命令与输出留进证据；
     做不到就把该项记「未验证」。
  3. 护栏类验收必须给**对照组**：制造违规 → 红，移除违规 → 绿。只有红没有绿证明不了检出力——K[28] 错误
     期间「写入手改文件后 check 失败」在任何输入下都成立，那不是护栏生效。
  4. 跨实现的哈希 / 序列化，用独立实现（`shasum -a 256`、`hashlib`、`SHA256.HashData`）对同一输入做 KAT 与
     全量复算，别让任一侧自证。
- 来源：QA 评论 `QA-EVIDENCE-RM00003-ODD-2026-08-29`（R-00041 / R-00045 判不通过）；修复提交 `956da90`
  （DAG 检查器误用 `--workspace`）/ `0f8cf0c` / `51c2836`（SHA-256 K[28]）；对照组与独立复算实录见两卡收口评论。

### 没有链接执行过的验证一律记「未执行」，`cargo check` / clippy / 类型检查都不算测试通过

- 日期：2026-08-28
- 现象：R-00143 / R-00145 / R-00146 三张测试卡连续三次把 `cargo check` + `clippy` exit 0 当作交付证据，
  卡面证据写成「实现覆盖 / 部分覆盖」，`cargo test` 的 exit 101（宿主缺 `link.exe`）被记为环境问题而非
  未验证。2026-08-28 换到能链接的宿主后首次真跑，**21 条断言失败、跨 12 个 test target**，其中包含 3 处
  生产缺陷和 1 处贯穿全项目历史的契约级缺陷（生成 SHA-256 轮常量 K[28] 错误，Rust 侧所有摘要都不是
  SHA-256，且与 C# 实现互不一致）。这些缺陷全部是各卡**自己的**用例本该抓到的。
- 根因：类型检查只证明「调用形状对得上」，不证明任何运行时断言成立。一旦允许用 check 口径填验收，
  证据链就与事实脱钩，而且脱钩得越久，攒下的潜伏缺陷越多——本次是攒了整个项目历史。
  次生根因：矩阵驱动函数（`run_b0_matrix` 等）返回结构化报告而非直接 panic，`cargo check` 通过时
  「报告里每一行 ok 都是 false」也毫无察觉。
- 规避：
  1. 交付前跑 `cargo test --workspace --all-features` 并贴真实退出码；退出码非 0 就不得写「通过」，
     写「未执行」或「失败」，并把矩阵**每一行**的 runtime 结果单独列出，不许只给汇总。
  2. 宿主不能链接时，这是**阻塞**，按卡面「前置不满足立即交回」处理，不是可以用 check 顶替的环境瑕疵。
  3. 首次在新宿主/新 host triple 上跑时，额外跑一遍 `-- --test-threads=1`，用来区分真实缺陷与并发污染
     （本次两者逐条一致，直接排除了 flaky 假设，省掉一整轮排查）。
  4. 只要有一个独立实现（本仓 Python guard 用 hashlib、C# 用 `SHA256.HashData`）能对同一输入交叉验证，
     就让它们互相比对；单侧自洽永远发现不了「常量错但自洽」这类缺陷。
- 来源：reviewer 报告 `docs/evidence/reviews/mvp-review.md`（verdict RETURN，F-P0-1/F-P0-2）；
  三卡 known gaps；提交 `17ef95c` / `34ffdc1`。
