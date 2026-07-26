---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 31b279a990f47774972731f7f6b5181ed3b4625a7cd9d3e015b24c180e129c7b
source_last_modified: "2026-01-22T14:35:36.755283+00:00"
translation_last_reviewed: 2026-02-07
id: operations-playbook
title: SoraFS Operations Playbook
sidebar_label: Operations Playbook
description: Incident response guides and chaos drill procedures for SoraFS operators.
translator: machine-google-reviewed
---

:::注意规范来源
此页面反映了 `docs/source/sorafs_ops_playbook.md` 下维护的运行手册。保持两个副本同步，直到 Sphinx 文档集完全迁移。
:::

## 关键参考文献

- 可观测性资产：请参阅 `dashboards/grafana/` 下的 Grafana 仪表板和 `dashboards/alerts/` 中的 Prometheus 警报规则。
- 公制目录：`docs/source/sorafs_observability_plan.md`。
- Orchestrator 遥测表面：`docs/source/sorafs_orchestrator_plan.md`。

## 升级矩阵

|优先|触发器示例 |主要随叫随到 |备份|笔记|
|----------|--------------------------------|-----------------|--------|--------|
| P1 |全球网关中断，PoR 故障率 > 5%（15 分钟），复制积压每 10 分钟翻一番 |存储 SRE |可观察性 TL |如果影响超过 30 分钟，请与治理委员会联系。 |
| P2 |区域网关延迟 SLO 违规、编排器重试激增而不影响 SLA |可观察性 TL |存储 SRE |继续推出，但控制新清单。 |
| P3 |非关键警报（明显陈旧，容量 80–90%）|摄入量分类|行动公会 |下一个工作日内的地址。 |

## 网关中断/可用性下降

**检测**

- 警报：`SoraFSGatewayAvailabilityDrop`、`SoraFSGatewayLatencySlo`。
- 仪表板：`dashboards/grafana/sorafs_gateway_overview.json`。

**立即行动**

1. 通过请求率面板确认范围（单一提供商与车队）。
2. 通过在操作配置 (`docs/source/sorafs_gateway_self_cert.md`) 中切换 `sorafs_gateway_route_weights`，将 Torii 路由切换到健康的提供商（如果是多提供商）。
3. 如果所有提供商都受到影响，请为 CLI/SDK 客户端启用“直接获取”回退 (`docs/source/sorafs_node_client_protocol.md`)。

**分流**

- 根据 `sorafs_gateway_stream_token_limit` 检查流令牌利用率。
- 检查网关日志是否存在 TLS 或准入错误。
- 运行 `scripts/telemetry/run_schema_diff.sh` 以确保网关导出的架构与预期版本匹配。

**修复选项**

- 仅重新启动受影响的网关进程；避免回收整个集群，除非多个提供者出现故障。
- 如果确认饱和，则暂时将流令牌限制增加 10–15%。
- 稳定后重新运行自我认证 (`scripts/sorafs_gateway_self_cert.sh`)。

**事件发生后**

- 使用 `docs/source/sorafs/postmortem_template.md` 提交 P1 事后分析。
- 如果补救措施依赖于手动干预，则安排后续混乱演习。

## 证明失败峰值 (PoR / PoTR)

**检测**

- 警报：`SoraFSProofFailureSpike`、`SoraFSPoTRDeadlineMiss`。
- 仪表板：`dashboards/grafana/sorafs_proof_integrity.json`。
- 遥测：`torii_sorafs_proof_stream_events_total` 和 `sorafs.fetch.error` 事件以及 `provider_reason=corrupt_proof`。

**立即行动**

1. 通过标记清单注册表 (`docs/source/sorafs/manifest_pipeline.md`) 来冻结新的清单准入。
2. 通知治理部门暂停对受影响提供商的激励措施。

**分流**

- 检查 PoR 挑战队列深度与 `sorafs_node_replication_backlog_total`。
- 验证最近部署的证明验证管道 (`crates/sorafs_node/src/potr.rs`)。
- 将提供商固件版本与运营商注册表进行比较。

**修复选项**

- 使用 `sorafs_cli proof stream` 和最新清单触发 PoR 重放。
- 如果证明始终失败，请通过更新治理注册表并强制刷新 Orchestrator 记分板来从活动集中删除提供者。

**事件发生后**

- 在下一次生产部署之前运行 PoR 混沌演练场景。
- 在事后分析模板中汲取教训并更新提供商资格清单。

## 复制滞后/积压增长

**检测**

- 警报：`SoraFSReplicationBacklogGrowing`、`SoraFSCapacityPressure`。进口
  `dashboards/alerts/sorafs_capacity_rules.yml` 并运行
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  在升级之前，Alertmanager 会反映记录的阈值。
- 仪表板：`dashboards/grafana/sorafs_capacity_health.json`。
- 指标：`sorafs_node_replication_backlog_total`、`sorafs_node_manifest_refresh_age_seconds`。

**立即行动**

1. 验证积压范围（单个提供商或队列）并暂停非必要的复制任务。
2. 如果积压订单被隔离，则通过复制调度程序临时将新订单重新分配给备用提供商。

**分流**

- 检查 Orchestrator 遥测是否存在可能级联积压的重试突发。
- 确认存储目标有足够的空间 (`sorafs_node_capacity_utilisation_percent`)。
- 查看最近的配置更改（块配置文件更新、证明节奏）。

**修复选项**

- 使用 `--rebalance` 选项运行 `sorafs_cli` 以重新分发内容。
- 为受影响的提供商水平扩展复制工作人员。
- 触发清单刷新以重新对齐 TTL 窗口。

**事件发生后**

- 安排一次容量演习，重点关注提供商饱和故障。
- 更新 `docs/source/sorafs_node_client_protocol.md` 中的复制 SLA 文档。

## 修复积压和 SLA 违规

**检测**

- 警报：
  - `SoraFSRepairBacklogHigh`（队列深度 > 50 或最旧的排队年龄 > 4h，持续 10m）。
  - `SoraFSRepairEscalations`（> 3 次升级/小时）。
  - `SoraFSRepairLeaseExpirySpike`（> 5 个租约到期/小时）。
  - `SoraFSRetentionBlockedEvictions`（过去 15m 中的主动修复阻止了保留）。
- 仪表板：`dashboards/grafana/sorafs_capacity_health.json`。

**立即行动**

1. 识别受影响的提供商（队列深度峰值）并暂停它们的新 pin/复制订单。
2. 验证修复工作人员的活跃度并在安全的情况下增加工作人员的并发性。

**分流**

- 将 `torii_sorafs_repair_backlog_oldest_age_seconds` 与 4 小时 SLA 窗口进行比较。
- 检查 `torii_sorafs_repair_lease_expired_total{outcome=...}` 是否有崩溃/时钟偏差模式。
- 查看重复清单/提供商对的升级票证并验证证据包。

**修复选项**

- 重新指派或重新启动停滞的维修人员；通过正常索赔流程清除孤立租赁。
- 修复排水管时限制新销，以防止额外的 SLA 压力。
- 如果升级持续存在，则升级至治理并附上修复审核工件。

## 保留/GC 检查（只读）

**检测**

- 警报：`SoraFSCapacityPressure` 或持续 `torii_sorafs_storage_bytes_used` > 90%。
- 仪表板：`dashboards/grafana/sorafs_capacity_health.json`。

**立即行动**

1. 运行本地保留快照：
   ```bash
   iroha app sorafs gc inspect --data-dir /var/lib/sorafs
   ```
2. 捕获仅过期视图以进行分类：
   ```bash
   iroha app sorafs gc dry-run --data-dir /var/lib/sorafs
   ```
3. 将 JSON 输出附加到事件单以进行审计。

**分流**

- 确认舱单报告 `retention_epoch=0`（无到期日）与有截止日期的舱单报告。
- 在GC JSON输出中使用`retention_sources`来查看哪个约束设置有效
  保留（`deal_end`、`governance_cap`、`pin_policy` 或 `unbounded`）。交易和治理上限
  通过清单元数据键 `sorafs.retention.deal_end_epoch` 提供
  `sorafs.retention.governance_cap_epoch`。
- 如果 `dry-run` 报告清单已过期但容量仍处于固定状态，请验证否
  主动修复或保留策略会覆盖块驱逐。
  容量触发的扫描按最近最少使用的顺序逐出过期清单
  `manifest_id` 决胜局。

**修复选项**

- GC CLI 是只读的。不要在生产中手动删除清单或块。
- 升级为保留政策调整或容量扩展的治理
  当过期数据累积而没有自动驱逐时。

## 混沌练习节奏

- **季度**：组合网关中断 + Orchestrator 重试风暴模拟。
- **一年两次**：跨两个提供商进行 PoR/PoTR 故障注入并进行恢复。
- **每月抽查**：使用暂存清单的复制滞后场景。
- 通过以下方式在共享运行手册日志 (`ops/drill-log.md`) 中跟踪演练：

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- 在提交之前验证日志：

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- 对于即将进行的演习使用 `--status scheduled`，对于已完成的运行使用 `pass`/`fail`，当行动项保持开放状态时使用 `follow-up`。
- 使用 `--log` 覆盖目的地以进行试运行或自动验证；如果没有它，脚本将继续更新 `ops/drill-log.md`。

## 事后分析模板

对每个 P1/P2 事件和混沌演习回顾使用 `docs/source/sorafs/postmortem_template.md`。该模板涵盖时间表、影响量化、影响因素、纠正措施和后续验证任务。