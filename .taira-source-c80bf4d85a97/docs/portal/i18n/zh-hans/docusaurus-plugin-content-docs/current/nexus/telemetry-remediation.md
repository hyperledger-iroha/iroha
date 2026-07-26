---
id: nexus-telemetry-remediation
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 概述

路线图项目 **B2 — 遥测差距所有权** 需要发布的捆绑计划
每个突出的 Nexus 遥测间隙到信号、警报护栏、业主、
2026 年第一季度审核窗口开始之前的截止日期和验证工件。
本页镜像 `docs/source/nexus_telemetry_remediation_plan.md` 所以发布
工程、遥测操作和 SDK 所有者可以在发布之前确认覆盖范围
路由跟踪和 `TRACE-TELEMETRY-BRIDGE` 排练。

# 间隙矩阵

|间隙 ID |信号及警报护栏 |所有者/升级|截止日期（UTC）|证据与验证 |
|--------|------------------------------------|--------------------|------------------------|------------------------|
| `GAP-TELEM-001` |直方图 `torii_lane_admission_latency_seconds{lane_id,endpoint}`，带有警报 **`SoranetLaneAdmissionLatencyDegraded`**，当 `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 持续 5 分钟 (`dashboards/alerts/soranet_lane_rules.yml`) 时触发。 | `@torii-sdk`（信号）+ `@telemetry-ops`（警报）；通过 Nexus 路由跟踪 on-call 升级。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` 下的警报测试加上 `TRACE-LANE-ROUTING` 排练捕获显示已触发/恢复的警报和 Torii `/metrics` 抓取，存档于 [Nexus 过渡说明](./nexus-transition-notes)。 |
| `GAP-TELEM-002` |柜台 `nexus_config_diff_total{knob,profile}` 带护栏 `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` 门控部署 (`docs/source/telemetry.md`)。 | `@nexus-core`（仪表）→ `@telemetry-ops`（警报）；当计数器意外增加时，治理值班人员会收到传呼。 | 2026-02-26 |治理试运行输出存储在 `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 旁边；发布清单包括 Prometheus 查询屏幕截图以及证明 `StateTelemetry::record_nexus_config_diff` 发出差异的日志摘录。 |
| `GAP-TELEM-003` |当故障或丢失结果持续超过 30 分钟 (`dashboards/alerts/nexus_audit_rules.yml`) 时，事件 `TelemetryEvent::AuditOutcome`（指标 `nexus.audit.outcome`），并发出警报 **`NexusAuditOutcomeFailure`**。 | `@telemetry-ops`（管道）升级到 `@sec-observability`。 | 2026-02-27 | CI 门 `scripts/telemetry/check_nexus_audit_outcome.py` 归档 NDJSON 有效负载，并在 TRACE 窗口缺少成功事件时失败；警报屏幕截图附加到路由跟踪报告。 |
| `GAP-TELEM-004` |带护栏 `nexus_lane_configured_total != EXPECTED_LANE_COUNT` 的仪表 `nexus_lane_configured_total` 为 SRE 值班检查表提供数据。 |当节点报告目录大小不一致时，`@telemetry-ops`（计量/导出）升级为 `@nexus-core`。 | 2026-02-28 |调度程序遥测测试 `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` 证明发射；操作员将 Prometheus diff + `StateTelemetry::set_nexus_catalogs` 日志摘录附加到 TRACE 排练包中。 |

# 操作流程

1. **每周分类。** 业主报告 Nexus 准备电话的进展情况；
   拦截器和警报测试工件记录在 `status.md` 中。
2. **警报试运行。** 每个警报规则都附带一个
   `dashboards/alerts/tests/*.test.yml` 条目，以便 CI 执行 `promtool test
   每当护栏发生变化时都要遵守规则。
3. **审计证据。** 在 `TRACE-LANE-ROUTING` 期间和
   `TRACE-TELEMETRY-BRIDGE` 排练 on-call 捕获 Prometheus 查询
   结果、警报历史记录和相关脚本输出
   （`scripts/telemetry/check_nexus_audit_outcome.py`，
   `scripts/telemetry/check_redaction_status.py` 用于相关信号）和
   将它们与路由跟踪工件一起存储。
4. **升级。** 如果任何护栏在经过排练的窗口外起火，业主
   团队提交了引用此计划的 Nexus 事件通知单，包括
   恢复审计之前的指标快照和缓解步骤。

随着这个矩阵的发布——并引用自 `roadmap.md` 和
`status.md` — 路线图项目 **B2** 现在满足“责任、截止日期、
警报、验证”验收标准。