---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd9f85b2c7845414c27016f699da179e13c41c9b9e0ce5b178ab88a950744500
source_last_modified: "2025-12-29T18:16:35.142540+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-routed-trace-audit-2026q1
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::注意规范来源
此页面镜像 `docs/source/nexus_routed_trace_audit_report_2026q1.md`。保持两个副本对齐，直到剩余的翻译落地。
:::

# 2026 年第一季度路由跟踪审计报告 (B1)

路线图项目 **B1 — 路由跟踪审核和遥测基线** 需要
Nexus 路由跟踪计划的季度审查。该报告记录了
Q12026 审计窗口（一月至三月），以便治理委员会可以签署
Q2发射前的遥测姿态排练。

## 范围和时间表

|跟踪 ID |窗口 (UTC) |目标 |
|----------|--------------|------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 |在启用多通道之前验证通道准入直方图、队列八卦和警报流。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 |在 AND4/AND7 里程碑之前验证 OTLP 重播、diff 机器人奇偶校验和 SDK 遥测摄取。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 |在 RC1 削减之前确认治理批准的 `iroha_config` 增量和回滚准备情况。 |

每次排练都在类似生产的拓扑上运行，并带有路由跟踪
启用仪器（`nexus.audit.outcome` 遥测 + Prometheus 计数器），
已加载 Alertmanager 规则，并将证据导出到 `docs/examples/`。

## 方法论

1. **遥测收集。** 所有节点都发出结构化的
   `nexus.audit.outcome` 事件和随附指标
   （`nexus_audit_outcome_total*`）。帮手
   `scripts/telemetry/check_nexus_audit_outcome.py` 追踪 JSON 日志，
   验证事件状态，并将有效负载存档在
   `docs/examples/nexus_audit_outcomes/`.【脚本/遥测/check_nexus_audit_outcome.py:1】
2. **警报验证。** `dashboards/alerts/nexus_audit_rules.yml` 及其测试
   线束确保警报噪音阈值和有效负载模板保持不变
   一致。 CI 运行 `dashboards/alerts/tests/nexus_audit_rules.test.yml`
   每一次改变；在每个窗口期间手动执行相同的规则。
3. **仪表板捕获。** 操作员从
   `dashboards/grafana/soranet_sn16_handshake.json`（握手健康）和
   遥测概览仪表板将队列运行状况与审计结果关联起来。
4. **审稿人注释。** 治理秘书记录了审稿人姓名缩写，
   决定，以及 [Nexus 过渡说明](./nexus-transition-notes) 中的任何缓解票据
   和配置增量跟踪器 (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## 调查结果

|跟踪 ID |结果|证据|笔记|
|----------|---------|----------|--------|
| `TRACE-LANE-ROUTING` |通行证 |警报火灾/恢复截图（内部链接）+ `dashboards/alerts/tests/soranet_lane_rules.test.yml` 重播；遥测差异记录在 [Nexus 转换注释](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) 中。 |队列准入 P95 保持 612ms（目标 ≤750ms）。无需后续行动。 |
| `TRACE-TELEMETRY-BRIDGE` |通行证 |存档的结果有效负载 `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` 加上 `status.md` 中记录的 OTLP 重播哈希。 | SDK 修订盐与 Rust 基线相匹配； diff 机器人报告零增量。 |
| `TRACE-CONFIG-DELTA` |通过（缓解已关闭）|治理跟踪器条目 (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS 配置文件清单 (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + 遥测包清单 (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)。 |第二季度重新运行对批准的 TLS 配置文件进行哈希处理并确认零落后者；遥测清单记录插槽范围 912–936 和工作负载种子 `NEXUS-REH-2026Q2`。 |

所有跟踪在其内部至少产生一个 `nexus.audit.outcome` 事件
windows，满足Alertmanager护栏（`NexusAuditOutcomeFailure`
本季度保持绿色）。

## 后续行动

- 使用 TLS 哈希 `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` 更新路由跟踪附录；
  缓解措施 `NEXUS-421` 在过渡说明中关闭。
- 继续将原始 OTLP 回放和 Torii diff 工件附加到存档中
  为 Android AND4/AND7 评论提供同等证据。
- 确认即将进行的 `TRACE-MULTILANE-CANARY` 排练会重复使用相同的内容
  遥测助手，因此第二季度签核受益于经过验证的工作流程。

## 文物索引

|资产|地点 |
|--------|----------|
|遥测验证器 | `scripts/telemetry/check_nexus_audit_outcome.py` |
|警报规则和测试 | `dashboards/alerts/nexus_audit_rules.yml`，`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|结果负载示例 | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
|配置增量跟踪器 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|路由跟踪时间表和注释| [Nexus 过渡说明](./nexus-transition-notes) |

该报告、上述工件以及警报/遥测导出应该是
附在治理决策日志中以结束本季度的 B1。