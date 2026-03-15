---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20b155bf2418ccdfb4981e52af44816bed8fc256ba8e54a78f6b9b320450b8fc
source_last_modified: "2026-01-22T14:35:36.747633+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
---

:::注意规范来源
:::

## 概述

此操作手册记录了如何监控和分类 SoraFS pin 注册表及其复制服务级别协议 (SLA)。这些指标源自 `iroha_torii`，并通过 `torii_sorafs_*` 命名空间下的 Prometheus 导出。 Torii 在后台以 30 秒的间隔对注册表状态进行采样，因此即使没有操作员轮询 `/v2/sorafs/pin/*` 端点，仪表板仍保持最新状态。导入精心策划的仪表板 (`docs/source/grafana_sorafs_pin_registry.json`)，以获得直接映射到以下部分的即用型 Grafana 布局。

## 指标参考

|公制|标签|描述 |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) |按生命周期状态列出的链上清单清单。 |
| `torii_sorafs_registry_aliases_total` | — |注册表中记录的活动清单别名的计数。 |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) |复制订单积压按状态分段。 |
| `torii_sorafs_replication_backlog_total` | — |方便仪表镜像 `pending` 订单。 |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA 核算：`met` 统计截止日期内已完成的订单，`missed` 汇总延迟完成+到期的订单，`pending` 镜像未完成的订单。 |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) |聚合完成延迟（发布和完成之间的时期）。 |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) |挂单松弛窗口（截止日期减去发行纪元）。 |

所有仪表在每次快照拉取时都会重置，因此仪表板应以 `1m` 节奏或更快的速度进行采样。

## Grafana 仪表板

仪表板 JSON 附带七个面板，涵盖操作员工作流程。如果您喜欢构建定制图表，下面列出了查询以供快速参考。

1. **清单生命周期** – `torii_sorafs_registry_manifests_total`（按 `status` 分组）。
2. **别名目录趋势** – `torii_sorafs_registry_aliases_total`。
3. **按状态排序队列** – `torii_sorafs_registry_orders_total`（按 `status` 分组）。
4. **积压订单与过期订单** – 将 `torii_sorafs_replication_backlog_total` 和 `torii_sorafs_registry_orders_total{status="expired"}` 结合到表面饱和。
5. **SLA 成功率** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **延迟与截止时间松弛** – 叠加 `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` 和 `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`。当您需要绝对松弛地板时，使用 Grafana 转换添加 `min_over_time` 视图，例如：

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **错过订单（1 小时率）** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## 警报阈值

- **SLA 成功 < 0.95，持续 15 分钟**
  - 阈值：`sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - 操作：页面 SRE；开始复制积压分类。
- **待处理积压超过 10**
  - 阈值：`torii_sorafs_replication_backlog_total > 10` 持续 10 分钟
  - 操作：检查提供商可用性和 Torii 容量调度程序。
- **过期订单> 0**
  - 阈值：`increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - 行动：检查治理清单以确认提供商流失情况。
- **完成第95页>截止日期松弛平均值**
  - 阈值：`torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - 行动：验证提供商是否在截止日期前做出承诺；考虑发布重新分配。

### 示例 Prometheus 规则

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## 分类工作流程

1. **查明原因**
   - 如果 SLA 错过了峰值，而积压仍然很低，则应关注提供商的性能（PoR 失败、延迟完成）。
   - 如果积压随着稳定的错过而增加，请检查入场 (`/v2/sorafs/pin/*`) 以确认等待理事会批准的清单。
2. **验证提供商状态**
   - 运行 `iroha app sorafs providers list` 并验证公布的功能是否符合复制要求。
   - 检查 `torii_sorafs_capacity_*` 仪表以确认配置的 GiB 和 PoR 成功。
3. **重新分配复制**
   - 当积压订单 (`stat="avg"`) 低于 5 个周期时，通过 `sorafs_manifest_stub capacity replication-order` 发出新订单（舱单/CAR 包装使用 `iroha app sorafs toolkit pack`）。
   - 如果别名缺少活动清单绑定（`torii_sorafs_registry_aliases_total` 意外下降），则通知治理。
4. **记录结果**
   - 在 SoraFS 操作日志中记录事件注释以及时间戳和受影响的清单摘要。
   - 如果引入新的故障模式或仪表板，请更新此运行手册。

## 推出计划

在生产中启用或收紧别名缓存策略时，请遵循此分阶段过程：

1. **准备配置**
   - 使用商定的 TTL 和宽限窗口更新 `iroha_config` 中的 `torii.sorafs_alias_cache`（用户 → 实际）：`positive_ttl`、`refresh_window`、`hard_expiry`、`negative_ttl`、`revocation_ttl`、 `rotation_max_age`、`successor_grace` 和 `governance_grace`。默认值与 `docs/source/sorafs_alias_policy.md` 中的策略匹配。
   - 对于 SDK，通过其配置层（Rust / NAPI / Python 绑定中的 `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`）分发相同的值，以便客户端执行与网关匹配。
2. **分期试运行**
   - 将配置更改部署到镜像生产拓扑的临时集群。
   - 运行 `cargo xtask sorafs-pin-fixtures` 以确认规范别名装置仍在解码和往返；任何不匹配都意味着必须首先解决上游明显的漂移。
   - 使用涵盖新鲜、刷新窗口、过期和硬过期情况的综合证明来练习 `/v2/sorafs/pin/{digest}` 和 `/v2/sorafs/aliases` 端点。根据此 Runbook 验证 HTTP 状态代码、标头（`Sora-Proof-Status`、`Retry-After`、`Warning`）和 JSON 正文字段。
3. **在生产中启用**
   - 通过标准更改窗口推出新配置。首先将其应用到 Torii，然后在节点在日志中确认新策略后重新启动网关/SDK 服务。
   - 将 `docs/source/grafana_sorafs_pin_registry.json` 导入 Grafana（或更新现有仪表板）并将别名缓存刷新面板固定到 NOC 工作区。
4. **部署后验证**
   - 监视 `torii_sorafs_alias_cache_refresh_total` 和 `torii_sorafs_alias_cache_age_seconds` 30 分钟。 `error`/`expired` 曲线中的峰值应与策略刷新窗口相关；意外的增长意味着运营商必须在继续之前检查别名证明和提供商的健康状况。
   - 确认客户端日志显示相同的策略决策（当证明过时或过期时，SDK 将显示错误）。没有客户端警告表明配置错误。
5. **后备**
   - 如果别名发放落后并且刷新窗口频繁跳闸，请通过在配置中增加 `refresh_window` 和 `positive_ttl` 来暂时放宽策略，然后重新部署。保持 `hard_expiry` 完整，这样真正过时的证明仍然会被拒绝。
   - 如果遥测继续显示 `error` 计数升高，请通过恢复之前的 `iroha_config` 快照来恢复到之前的配置，然后打开事件以跟踪别名生成延迟。

## 相关材料

- `docs/source/sorafs/pin_registry_plan.md` — 实施路线图和治理背景。
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — 存储工作人员操作，补充了此注册表手册。