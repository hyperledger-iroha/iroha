---
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 887123dcafff50fb243d9788415b759da3691876e44b3cd7c800eede25a5ab09
source_last_modified: "2026-01-05T09:28:11.916414+00:00"
translation_last_reviewed: 2026-02-07
id: testnet-rollout
title: SoraNet testnet rollout (SNNet-10)
sidebar_label: Testnet Rollout (SNNet-10)
description: Phased activation plan, onboarding kit, and telemetry gates for SoraNet testnet promotions.
translator: machine-google-reviewed
---

:::注意规范来源
:::

SNNet-10 协调整个网络中 SoraNet 匿名覆盖的分阶段激活。使用此计划将路线图项目符号转化为具体的可交付成果、操作手册和遥测门，以便每个操作员在 SoraNet 成为默认传输之前了解期望。

## 启动阶段

|相|时间表（目标）|范围 |所需文物|
|--------|--------------------|--------------------|--------------------|
| **T0 — 封闭测试网** | 2026 年第四季度 | ≥3 个 ASN 中的 20-50 个中继由核心贡献者运营。 |测试网入门套件、守卫固定烟雾套件、基线延迟 + PoW 指标、掉电演练日志。 |
| **T1 — 公开测试版** | 2027 年第一季度 | ≥100 个继电器，启用保护轮换，强制退出绑定，SDK beta 默认为带有 `anon-guard-pq` 的 SoraNet。 |更新了入职工具包、操作员验证清单、目录发布 SOP、遥测仪表板包、事件演练报告。 |
| **T2 — 主网默认** | 2027 年第 2 季度（SNNet-6/7/9 完成时门控）|生产网络默认为SoraNet； obfs/MASQUE 传输和 PQ 棘轮强制执行已启用。 |治理批准会议纪要、仅直接回滚程序、降级警报、签署的成功指标报告。 |

**没有跳过路径**——每个阶段都必须在升级之前运送前一阶段的遥测和治理工件。

## 测试网入门套件

每个中继操作员都会收到一个包含以下文件的确定性包：

|文物|描述 |
|----------|-------------|
| `01-readme.md` |概述、联系点和时间表。 |
| `02-checklist.md` |飞行前检查表（硬件、网络可达性、防护策略验证）。 |
| `03-config-example.toml` |最小 SoraNet 中继 + 协调器配置与 SNNet-9 合规性块保持一致，包括固定最新防护快照哈希的 `guard_directory` 块。 |
| `04-telemetry.md` |连接 SoraNet 隐私指标仪表板和警报阈值的说明。 |
| `05-incident-playbook.md` |带有升级矩阵的断电/降级响应程序。 |
| `06-verification-report.md` |一旦冒烟测试通过，模板操作员就会完成并返回。 |

渲染副本位于 `docs/examples/soranet_testnet_operator_kit/` 中。每次促销都会刷新套件；版本号跟踪阶段（例如，`testnet-kit-vT0.1`）。

对于公共测试版 (T1) 操作员，`docs/source/soranet/snnet10_beta_onboarding.md` 中的简明入门简介总结了先决条件、遥测可交付成果和提交工作流程，同时指向确定性套件和验证器帮助程序。

`cargo xtask soranet-testnet-feed` 生成 JSON 源，该源聚合了阶段门模板引用的升级窗口、中继名册、指标报告、演练证据和附件哈希。首先使用 `cargo xtask soranet-testnet-drill-bundle` 签署钻探日志和附件，以便源可以记录 `drill_log.signed = true`。

## 成功指标

阶段之间的晋升通过以下遥测数据进行控制，收集至少两周：

- `soranet_privacy_circuit_events_total`：95%的电路完成，没有掉电或降级事件；剩余 5% 受 PQ 供应限制。
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`：每天 <1% 的提取会话会在计划的演练之外触发断电。
- `soranet_privacy_gar_reports_total`：预期 GAR 类别组合的偏差在 ±10% 以内；峰值必须由批准的政策更新来解释。
- PoW 选票成功率：3s目标窗口内≥99%；通过 `soranet_privacy_throttles_total{scope="congestion"}` 报告。
- 每个区域的延迟（第 95 个百分位）：电路完全构建后 <200 毫秒，通过 `soranet_privacy_rtt_millis{percentile="p95"}` 捕获。

仪表板和警报模板位于 `dashboard_templates/` 和 `alert_templates/` 中；将它们镜像到您的遥测存储库中，并将它们添加到 CI lint 检查中。在请求升级之前，使用 `cargo xtask soranet-testnet-metrics` 生成面向治理的报告。

阶段关提交必须遵循 `docs/source/soranet/snnet10_stage_gate_template.md`，它链接到存储在 `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` 下的可立即复制的 Markdown 表单。

## 验证清单

在进入每个阶段之前，操作员必须签署以下内容：

- ✅ 用当前入场信封签署的接力广告。
- ✅ 防护轮旋转烟雾测试（`tools/soranet-relay --check-rotation`）通过。
- ✅ `guard_directory` 指向最新的 `GuardDirectorySnapshotV2` 工件，并且 `expected_directory_hash_hex` 与委员会摘要匹配（中继启动记录经过验证的哈希值）。
- ✅ PQ 棘轮指标 (`sorafs_orchestrator_pq_ratio`) 保持在请求阶段的目标阈值之上。
- ✅ GAR 合规性配置与最新标签匹配（请参阅 SNNet-9 目录）。
- ✅ 降级警报模拟（禁用收集器，预计 5 分钟内发出警报）。
- ✅ 使用记录的缓解步骤执行 PoW/DoS 演练。

入门套件中包含预填充模板。操作员在收到生产凭证之前将完整的报告提交给治理帮助台。

## 治理和报告

- **变更控制：** 晋升需要治理委员会的批准，记录在理事会会议记录中并附在状态页面上。
- **状态摘要：**发布每周更新，总结继电器计数、PQ 比率、断电事件和未完成的操作项目（节奏开始后存储在 `docs/source/status/soranet_testnet_digest.md` 中）。
- **回滚：**维护已签署的回滚计划，在 30 分钟内将网络返回到上一阶段，包括 DNS/guard 缓存失效和客户端通信模板。

## 支持资产

- `cargo xtask soranet-testnet-kit [--out <dir>]` 将 `xtask/templates/soranet_testnet/` 中的入门套件具体化到目标目录中（默认为 `docs/examples/soranet_testnet_operator_kit/`）。
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` 评估 SNNet-10 成功指标并发出适合治理审查的结构化通过/失败报告。示例快照位于 `docs/examples/soranet_testnet_metrics_sample.json` 中。
- Grafana 和 Alertmanager 模板位于 `dashboard_templates/soranet_testnet_overview.json` 和 `alert_templates/soranet_testnet_rules.yml` 下；将它们复制到您的遥测存储库中或将它们连接到 CI lint 检查中。
- SDK/门户消息传递的降级通信模板位于 `docs/source/soranet/templates/downgrade_communication_template.md` 中。
- 每周状态摘要应使用 `docs/source/status/soranet_testnet_weekly_digest.md` 作为规范形式。

拉取请求应与任何人工制品或遥测更改一起更新此页面，以便推出计划保持规范。