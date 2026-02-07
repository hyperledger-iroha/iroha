---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2025-12-29T18:16:35.940439+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 外部审计简介
% Iroha 加密工作组
% 2026-01-30

# 概述

本简介包含了所需的工程和合规环境
Iroha 的 SM2/SM3/SM4 支持的独立审查。它针对的是审计团队
具有 Rust 密码学经验并熟悉中国国家
密码学标准。预期结果是一份书面报告，内容涵盖
实施风险、一致性差距和优先修复指导
在 SM 推出之前从预览转向生产。

# 程序快照

- **发布范围：** Iroha 2/3 共享代码库，确定性验证
  跨节点和 SDK 的路径，可在配置保护后面进行签名。
- **当前阶段：** SM-P3.2（OpenSSL/Tongsuo 后端集成）与 Rust
  实现已经交付用于验证和对称用例。
- **目标决策日期：** 2026-04-30（审核结果告知是否继续
  在验证器构建中启用 SM 签名）。
- **跟踪的关键风险：** 第三方依赖谱系、确定性
  混合硬件下的行为、运营商合规性准备情况。

# 代码和夹具参考

- `crates/iroha_crypto/src/sm.rs` — Rust 实现和可选的 OpenSSL
  绑定（`sm-ffi-openssl` 功能）。
- `crates/ivm/tests/sm_syscalls.rs` — IVM 哈希系统调用覆盖率，
  验证和对称模式。
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — Norito 有效负载
  SM文物往返。
- `docs/source/crypto/sm_program.md` — 程序历史记录、依赖性审计和
  推出护栏。
- `docs/source/crypto/sm_operator_rollout.md` — 面向操作员的支持和
  回滚程序。
- `docs/source/crypto/sm_compliance_brief.md` — 监管摘要和导出
  考虑因素。
- `scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — 用于 OpenSSL 支持的流的确定性烟雾控制。
- `fuzz/sm_*` 语料库 — 基于 RustCrypto 的模糊种子，涵盖 SM3/SM4 原语。

# 请求的审核范围1. **规格符合性**
   - 验证SM2签名验证、ZA计算和规范
     编码行为。
   - 确认SM3/SM4原语遵循GM/T 0002-2012和GM/T 0007-2012，
     包括计数器模式不变量和 IV 处理。
2. **确定性和恒定时间保证**
   - 检查分支、表查找和硬件调度以便节点执行
     跨 CPU 系列保持确定性。
   - 评估私钥操作的恒定时间声明并确认
     OpenSSL/Tongsuo 路径保留恒定时间语义。
3. **侧通道和故障分析**
   - 检查 Rust 和 中的时序、缓存和电源侧通道风险
     FFI 支持的代码路径。
   - 评估签名验证的故障处理和错误传播
     验证加密失败。
4. **构建、依赖性和供应链审查**
   - 确认 OpenSSL/Tongsuo 工件的可重复构建和出处。
   - 审查依赖树许可和审计覆盖范围。
5. **测试和验证利用批评**
   - 评估确定性烟雾测试、模糊测试和 Norito 装置。
   - 建议额外的覆盖范围（例如差异测试、基于属性的测试）
     证明）如果仍然存在差距。
6. **合规性和操作员指南验证**
   - 根据法律要求和预期交叉检查装运文件
     操作员控制。

# 交付成果和物流

- **开球：** 2026 年 2 月 24 日（虚拟，90 分钟）。
- **采访：** 加密货币工作组、IVM 维护者、平台操作员（根据需要）。
- **Artefact 访问：** 只读存储库镜像、CI 管道日志、固定装置
  输出和依赖 SBOM (CycloneDX)。
- **临时更新：**每周书面状态+风险标注。
- **最终交付成果（截至 2026 年 4 月 15 日）：**
  - 带有风险评级的执行摘要。
  - 详细的调查结果（每个问题：影响、可能性、代码参考、
    补救指导）。
  - 重新测试/验证计划。
  - 关于决定论、恒定时间姿态和合规性调整的声明。

## 参与状态

|供应商|状态 |开球 |现场窗口|笔记|
|--------|--------|----------|----------------|--------|
| Trail of Bits（中文练习）| 2026年2月21日执行的工作说明书 | 2026-02-24 | 2026-02-24–2026-03-22 |交货截止日期为2026年4月15日；张辉作为工程同行领导与 Alexey M. 的合作。每周状态通话周三 09:00 UTC。 |
| NCC 集团（亚太地区）|预留应急时段 |不适用（暂停）|暂定 2026-05-06–2026-05-31 |仅当高风险发现需要第二次时才激活； Priya N.（安全）和 NCC 集团参与部门于 2026 年 2 月 22 日确认了准备情况。 |

# 外展包中包含的附件- `docs/source/crypto/sm_program.md`
- `docs/source/crypto/sm_operator_rollout.md`
- `docs/source/crypto/sm_compliance_brief.md`
- `docs/source/crypto/sm_lock_refresh_plan.md`
- `docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"` 快照。
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — `cargo metadata` 导出 `iroha_crypto` 箱（锁定的依赖关系图）。
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — 最新的 `scripts/sm_openssl_smoke.sh` 运行（缺少提供商支持时跳过 SM2/SM4 路径）。
- `docs/source/crypto/attachments/sm_openssl_provenance.md` — 本地工具包出处（pkg-config/OpenSSL 版本说明）。
- 模糊语料库清单 (`fuzz/sm_corpus_manifest.json`)。

> **环境警告：** 当前的开发快照使用供应商的 OpenSSL 3.x 工具链（`openssl` crate `vendored` 功能），但 macOS 缺乏 SM3/SM4 CPU 内在函数，并且默认提供程序不公开 SM4-GCM，因此 OpenSSL 烟雾线束仍然跳过 SM4 覆盖和附件示例 SM2 解析。工作区依赖循环 (`sorafs_manifest ↔ sorafs_car`) 还会强制帮助程序脚本在发出 `cargo check` 故障后跳过运行。在 Linux 发布构建环境（启用了 SM4 且没有循环的 OpenSSL/Tongsuo）中重新运行捆绑包，以在外部审计之前捕获完整的奇偶校验。

# 候选审核合作伙伴及范围

|公司|相关经验|典型范围和可交付成果 |笔记|
|------|---------------------------------|--------------------------------------------|-----|
| Trail of Bits（CN密码学实践）| Rust 代码审查（`ring`、zkVM）、移动支付堆栈的先前 GM/T 评估。 |规范一致性差异 (GM/T 0002/3/4)、Rust + OpenSSL 路径的恒定时间审查、差异模糊测试、供应链审查、修复路线图。 |已经订婚了；在规划未来的刷新周期时保留该表是为了完整性。 |
| NCC 集团亚太区 |硬件/SOC + Rust 密码学红队，发表了 RustCrypto 原语和支付 HSM 桥的评论。 | Rust + JNI/FFI 绑定的整体评估、确定性策略验证、性能/遥测门审查、操作手册演练。 |保留作为应急措施；还可以为中国监管机构提供双语报告。 |
| Kudelski Security（区块链和加密货币团队）|对 Halo2、Mina、zkSync、Rust 中实现的自定义签名方案的审核。 |重点关注椭圆曲线正确性、转录完整性、硬件加速的威胁建模以及 CI/推出证据。 |对于有关硬件加速 (SM-5a) 和 FASTPQ 到 SM 交互的第二意见很有用。 |
|最小权限|基于 Rust 的区块链（Filecoin、Polkadot）的加密协议审计，可重现的构建咨询。 |确定性构建验证、Norito 编解码器验证、合规证据交叉检查、运营商通信审查。 |当监管机构要求代码审查之外的独立验证时，非常适合提供透明度/审计报告。 |

所有约定都要求上面列举的相同的工件捆绑包以及以下可选附加组件（具体取决于公司）：- **规范一致性和确定性行为：** SM2 ZA 推导、SM3 填充、SM4 轮函数和 `sm_accel` 运行时调度门的逐行验证，以确保加速永远不会改变语义。
- **侧通道和 FFI 审查：** 检查恒定时间声明、不安全代码块和 OpenSSL/Tongsuo 桥接层，包括针对 Rust 路径的差异测试。
- **CI/供应链验证：** `sm_interop_matrix`、`sm_openssl_smoke` 和 `sm_perf` 的复制与 SBOM/SLSA 证明一起使用，因此审计结果可以直接与发布证据联系起来。
- **面向运营商的抵押品：** 交叉检查 `sm_operator_rollout.md`、合规性归档模板和遥测仪表板，以确认文档中承诺的缓解措施在技术上是可执行的。

在确定未来审核的范围时，请重用此表，以使供应商优势与特定路线图里程碑保持一致（例如，支持 Kudelski 进行硬件/性能重度发布，支持 Trail of Bits 来支持语言/运行时正确性，支持 Least Authority 来确保可重现的构建保证）。

# 联络点

- **技术所有者：** Crypto WG 负责人 (Alexey M., `alexey@iroha.tech`)
- **项目经理：** 平台运营协调员（Sarah K.，
  `sarah@iroha.tech`)
- **安全联络员：** 安全工程 (Priya N., `security@iroha.tech`)
- **文档联络员：** Docs/DevRel 负责人（Jamila R.，
  `docs@iroha.tech`)