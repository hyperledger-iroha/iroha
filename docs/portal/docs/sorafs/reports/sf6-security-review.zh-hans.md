---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 372a65e9493b53bc638c938fbc0ce0b7683a2111ed1b5f2a4f0320818559824c
source_last_modified: "2025-12-29T18:16:35.201601+00:00"
translation_last_reviewed: 2026-02-07
title: SF-6 Security Review
summary: Findings and follow-up items from the independent assessment of keyless signing, proof streaming, and manifest submission pipelines.
translator: machine-google-reviewed
---

# SF-6 安全审查

**评估窗口：** 2026-02-10 → 2026-02-18  
**审核线索：** 安全工程协会 (`@sec-eng`)、工具工作组 (`@tooling-wg`)  
**范围：** SoraFS CLI/SDK（`sorafs_cli`、`sorafs_car`、`sorafs_manifest`）、证明流 API、Torii 清单处理、Sigstore/OIDC 集成、CI 发布钩子。  
**文物：**  
- CLI 源和测试 (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii 舱单/证明处理程序 (`crates/iroha_torii/src/sorafs/api.rs`)  
- 发布自动化（`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`）  
- 确定性奇偶校验工具（`crates/sorafs_car/tests/sorafs_cli.rs`、[SoraFS Orchestrator GA 奇偶校验报告](./orchestrator-ga-parity.md)）

## 方法论

1. **威胁建模研讨会**映射了开发人员工作站、CI 系统和 Torii 节点的攻击者能力。  
2. **代码审查**重点关注凭证表面（OIDC 令牌交换、无密钥签名）、Norito 清单验证和证明流背压。  
3. **动态测试** 使用奇偶校验工具和定制模糊驱动器重放夹具清单和模拟故障模式（令牌重放、清单篡改、截断的证明流）。  
4. **配置检查**验证了 `iroha_config` 默认值、CLI 标志处理和发布脚本，以确保确定性、可审核的运行。  
5. **流程访谈** 与工具工作组发布所有者确认了修复流程、升级路径和审计证据捕获。

## 调查结果摘要

|身份证 |严重性 |面积 |寻找|分辨率|
|----|----------|------|---------|------------|
| SF6-SR-01 |高|无钥匙签名 | OIDC 令牌受众默认值隐含在 CI 模板中，存在跨租户重放的风险。 |在发布挂钩和 CI 模板中添加了显式 `--identity-token-audience` 强制（[发布过程](../developer-releases.md)、`docs/examples/sorafs_ci.md`）。现在，当观众被忽略时，CI 就会失败。 |
| SF6-SR-02 |中等|证明流 |背压路径接受无限的订阅者缓冲区，从而导致内存耗尽。 | `sorafs_cli proof stream` 通过确定性截断强制限制通道大小，记录 Norito 摘要并中止流； Torii 镜像已更新以绑定响应块 (`crates/iroha_torii/src/sorafs/api.rs`)。 |
| SF6-SR-03 |中等|清单提交 |当 `--plan` 不存在时，CLI 接受清单而不验证嵌入的块计划。 |除非提供 `--expect-plan-digest`，否则 `sorafs_cli manifest submit` 现在会重新计算和比较 CAR 摘要，拒绝不匹配并显示修复提示。测试涵盖成功/失败案例 (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 |低|审计追踪|发布清单缺少用于安全审查的签名批准日志。 |添加了 [发布流程](../developer-releases.md) 部分，要求在 GA 之前附加审核备忘录哈希值和签核票 URL。 |

所有高/中发现结果均在审查窗口期间修复，并通过现有的奇偶校验工具进行验证。不存在任何潜在的关键问题。

## 控制验证

- **凭证范围：** 默认 CI 模板现在强制要求明确的受众和颁发者断言； CLI 和发布助手都会快速失败，除非 `--identity-token-audience` 伴随 `--identity-token-provider`。  
- **确定性重播：**更新的测试涵盖正/负清单提交流程，确保不匹配的摘要保持非确定性故障，并在接触网络之前浮出水面。  
- **证明流式传输背压：** Torii 现在通过有界通道流式传输 PoR/PoTR 项目，并且 CLI 仅保留截断的延迟样本 + 五个故障示例，防止无限制的订阅者增长，同时保持确定性摘要。  
- **可观察性：** 证明流计数器 (`torii_sorafs_proof_stream_*`) 和 CLI 摘要捕获中止原因，为操作员提供审计面包屑。  
- **文档：** 开发人员指南（[开发人员索引](../developer-index.md)、[CLI 参考](../developer-cli.md)）调用了安全敏感标志和升级工作流程。

## 发布清单添加内容

发布经理在提升 GA 候选人时**必须**附上以下证据：

1. 最新安全审查备忘录（本文档）的哈希值。  
2. 链接到所跟踪的修复票证（例如，`governance/tickets/SF6-SR-2026.md`）。  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` 的输出显示明确的受众/发行者参数。  
4. 从奇偶校验工具 (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) 捕获日志。  
5. 确认 Torii 发行说明包含有界证明流遥测计数器。

未能收集上述工件会阻止 GA 签署。

**参考人工制品哈希值（2026 年 2 月 20 日签署）：**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## 出色的后续行动

- **威胁模型刷新：** 每季度或在添加主要 CLI 标志之前重复此审核。  
- **模糊测试覆盖范围：** 证明流传输编码通过 `fuzz/proof_stream_transport` 进行模糊测试，涵盖身份、gzip、deflate 和 zstd 有效负载。  
- **事件演练：** 安排操作员练习模拟令牌泄露和清单回滚，确保文档反映实践的程序。

## 批准

- 安全工程协会代表：@sec-eng (2026-02-20)  
- 工具工作组代表：@tooling-wg (2026-02-20)

将签署的批准与发布工件包一起存储。