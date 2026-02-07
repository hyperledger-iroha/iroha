---
id: privacy-metrics-pipeline
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

# SoraNet 隐私指标管道

SNNet-8 为中继运行时引入了隐私感知遥测表面。的
中继现在将握手和电路事件聚合到分钟大小的桶中，
仅导出粗 Prometheus 计数器，保留单独的电路
不可链接，同时为操作员提供可操作的可见性。

## 聚合器概述

- 运行时实现位于 `tools/soranet-relay/src/privacy.rs` 中，如下所示
  `PrivacyAggregator`。
- 存储桶由挂钟分钟（`bucket_secs`，默认 60 秒）和
  存储在有界环中（`max_completed_buckets`，默认 120）。收藏家
  股票保留自己的有限积压（`max_share_lag_buckets`，默认 12）
  陈旧的 Prio 窗户会像抑制水桶一样被冲走，而不是漏水
  内存或屏蔽卡住的收集器。
- `RelayConfig::privacy` 直接映射到 `PrivacyConfig`，暴露调优
  旋钮（`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、
  `force_flush_buckets`、`max_completed_buckets`、`max_share_lag_buckets`、
  `expected_shares`）。生产运行时保留默认值，而 SNNet-8a
  引入安全聚合阈值。
- 运行时模块通过类型化助手记录事件：
  `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、
  `record_throttle_cooldown`、`record_capacity_reject`、`record_active_sample`、
  `record_verified_bytes` 和 `record_gar_category`。

## 中继管理端点

操作员可以通过以下方式轮询中继的管理监听器以获取原始观察结果
`GET /privacy/events`。端点返回以换行符分隔的 JSON
(`application/x-ndjson`) 包含镜像的 `SoranetPrivacyEventV1` 有效负载
来自内部 `PrivacyEventBuffer`。缓冲区保留最新的事件
到 `privacy.event_buffer_capacity` 条目（默认 4096）并耗尽
阅读，因此抓取工具应该足够频繁地轮询以避免出现间隙。活动涵盖
相同的握手、油门、验证带宽、有源电路和 GAR 信号
为 Prometheus 计数器供电，允许下游收集器归档
隐私安全的面包屑或提供安全的聚合工作流程。

## 继电器配置

操作员通过以下方式调整中继配置文件中的隐私遥测节奏
`privacy` 部分：

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

字段默认值符合 SNNet-8 规范，并在加载时进行验证：

|领域|描述 |默认|
|--------|-------------|---------|
| `bucket_secs` |每个聚合窗口的宽度（秒）。 | `60` |
| `min_handshakes` |存储桶可以发出计数器之前的最小贡献者计数。 | `12` |
| `flush_delay_buckets` |在尝试刷新之前要等待已完成的存储桶。 | `1` |
| `force_flush_buckets` |我们发射抑制桶之前的最大年龄。 | `6` |
| `max_completed_buckets` |保留存储桶积压（防止内存无界）。 | `120` |
| `max_share_lag_buckets` |抑制前收集股的保留窗口。 | `12` |
| `expected_shares` |合并前需要 Prio 收集者股份。 | `2` |
| `event_buffer_capacity` |管理流的 NDJSON 事件积压。 | `4096` |

将 `force_flush_buckets` 设置为低于 `flush_delay_buckets`，将
阈值，或禁用保留防护现在无法通过验证来避免
会泄漏每个中继遥测数据的部署。

`event_buffer_capacity` 限制还限制了 `/admin/privacy/events`，确保
爬虫不可能无限期地落后。

## Prio 收藏家分享

SNNet-8a 部署双收集器，发出秘密共享的 Prio 存储桶。的
Orchestrator 现在解析 `/privacy/events` NDJSON 流
`SoranetPrivacyEventV1` 条目和 `SoranetPrivacyPrioShareV1` 共享，
将它们转发到 `SoranetSecureAggregator::ingest_prio_share`。桶排放
一旦 `PrivacyBucketConfig::expected_shares` 贡献到达，镜像
中继行为。份额经过桶对齐和直方图形状验证
在合并为 `SoranetPrivacyBucketMetricsV1` 之前。如果结合起来
握手计数低于 `min_contributors`，存储桶导出为
`suppressed`，镜像中继内聚合器的行为。压抑
Windows 现在发出 `suppression_reason` 标签，以便操作员可以区分
`insufficient_contributors`、`collector_suppressed` 之间，
`collector_window_elapsed` 和 `forced_flush_window_elapsed` 场景
诊断遥测差距。 `collector_window_elapsed` 原因也引发
当 Prio 股价徘徊在 `max_share_lag_buckets` 以上时，让收藏家陷入困境
可见，而不会在内存中留下陈旧的累加器。

## Torii 摄取端点

Torii 现在公开两个遥测门控 HTTP 端点，以便中继和收集器
可以在不嵌入定制传输的情况下转发观察结果：

- `POST /v1/soranet/privacy/event` 接受
  `RecordSoranetPrivacyEventDto` 有效负载。身体包裹着一个
  `SoranetPrivacyEventV1` 加上可选的 `source` 标签。 Torii 验证
  针对活动遥测配置文件的请求、记录事件并响应
  带有 HTTP `202 Accepted` 以及包含以下内容的 Norito JSON 信封
  计算的桶窗口（`bucket_start_unix`、`bucket_duration_secs`）和
  中继模式。
- `POST /v1/soranet/privacy/share` 接受 `RecordSoranetPrivacyShareDto`
  有效负载。机身带有 `SoranetPrivacyPrioShareV1` 和可选的
  `forwarded_by` 提示操作员可以审核收集器流量。成功
  提交返回 HTTP `202 Accepted` 和 Norito JSON 信封总结
  收集器、存储桶窗口和抑制提示；验证失败映射到
  遥测 `Conversion` 响应以保留确定性错误处理
  跨收藏家。编排器的事件循环现在会发出这些共享
  轮询中继，使 Torii 的 Prio 累加器与中继桶保持同步。

两个端点均遵循遥测配置文件：它们发出“503 服务”
当指标被禁用时不可用。客户端可以发送 Norito 二进制文件
(`application/x.norito`) 或 Norito JSON (`application/x.norito+json`) 机构；
服务器通过标准 Torii 自动协商格式
提取器。

## Prometheus 指标

每个导出的桶携带 `mode` (`entry`, `middle`, `exit`) 和
`bucket_start` 标签。发出以下度量标准系列：

|公制|描述 |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` |握手分类为 `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`。 |
| `soranet_privacy_throttles_total{scope}` |油门计数器为 `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |由限制握手造成的聚合冷却持续时间。 |
| `soranet_privacy_verified_bytes_total` |通过盲法测量证明验证带宽。 |
| `soranet_privacy_active_circuits_{avg,max}` |每个桶的平均和峰值有源电路。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT 百分位估计（`p50`、`p90`、`p99`）。 |
| `soranet_privacy_gar_reports_total{category_hash}` |按类别摘要键入的哈希治理行动报告计数器。 |
| `soranet_privacy_bucket_suppressed` |由于未达到贡献者阈值，存储桶被扣留。 |
| `soranet_privacy_pending_collectors{mode}` |收集器共享累加器待组合，按中继模式分组。 |
| `soranet_privacy_suppression_total{reason}` |使用 `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` 抑制存储桶计数器，以便仪表板可以归因隐私差距。 |
| `soranet_privacy_snapshot_suppression_ratio` |最后一次消耗的抑制/消耗比率 (0-1)，对于警报预算很有用。 |
| `soranet_privacy_last_poll_unixtime` |最近成功轮询的 UNIX 时间戳（驱动收集器空闲警报）。 |
| `soranet_privacy_collector_enabled` |当隐私收集器被禁用或无法启动时，仪表会翻转为 `0`（驱动收集器禁用警报）。 |
| `soranet_privacy_poll_errors_total{provider}` |按中继别名分组的轮询失败（解码错误、HTTP 失败或意外状态代码的增量）。 |

没有观察的桶保持沉默，保持仪表板整洁，无需观察
制作零填充窗口。

## 操作指导

1. **仪表板** – 将上述指标按 `mode` 和 `window_start` 分组绘制图表。
   突出显示缺失的窗口以暴露收集器或继电器问题。使用
   `soranet_privacy_suppression_total{reason}` 区分贡献者
   对间隙进行分类时收集器驱动的抑制造成的不足。 Grafana
   资产现在运送一个专用的 **“抑制原因 (5m)”** 面板，由这些人员提供
   计数器加上 **“Suppressed Bucket %”** 计算统计数据
   `sum(soranet_privacy_bucket_suppressed) / count(...)` 每个选择所以
   运营商可以一目了然地发现预算违规情况。 **收藏家分享
   积压**系列 (`soranet_privacy_pending_collectors`) 和**快照
   抑制率**统计数据突出了收藏家和预算漂移
   自动运行。
2. **警报** – 从隐私安全计数器发出警报：PoW 拒绝峰值，
   冷却频率、RTT 漂移和容量拒绝。因为计数器是
   每个存储桶内都是单调的，基于率的简单规则效果很好。
3. **事件响应** – 首先依赖聚合数据。当进行更深层次的调试时
   必要时，请求中继重放存储桶快照或进行盲检查
   测量证明而不是收集原始流量日志。
4. **保留** – 经常刮擦以避免超过
   `max_completed_buckets`。出口商应将 Prometheus 输出视为
   规范源并在转发后删除本地存储桶。

## 抑制分析和自动运行SNNet-8 的接受取决于证明自动收集器的存在
健康且抑制保持在政策范围内（每个桶的 ≤10%）
任何 30 分钟窗口内的中继）。现在满足该要求所需的工具
随树而船；操作员必须将其纳入每周的例行公事中。新的
Grafana 抑制面板镜像下面的 PromQL 片段，提供 on-call
团队在需要回退到手动查询之前实时了解情况。

### 用于抑制审查的 PromQL 配方

操作员应随身携带以下 PromQL 助手；两者都被引用
在共享 Grafana 仪表板 (`dashboards/grafana/soranet_privacy_metrics.json`) 中
和警报管理器规则：

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

使用比率输出确认 **“Suppressed Bucket %”** 统计数据仍低于
政策预算；将尖峰检测器连接到 Alertmanager 以获得快速反馈
当贡献者数量意外下降时。

### 离线存储桶报告 CLI

工作区公开 `cargo xtask soranet-privacy-report` 用于一次性 NDJSON
捕获。将其指向一个或多个中继管理导出：

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

帮助程序通过 `SoranetSecureAggregator` 流式传输捕获，打印
抑制摘要到标准输出，并可选择写入结构化 JSON 报告
通过 `--json-out <path|->`。它具有与现场收集器相同的旋钮
（`--bucket-secs`、`--min-contributors`、`--expected-shares` 等），让
操作员在分类时重播不同阈值下的历史捕获
一个问题。将 JSON 与 Grafana 屏幕截图一起附加，以便 SNNet-8
抑制分析门仍然可审计。

### 第一个自动运行清单

治理仍然需要证明第一次自动化运行满足
抑制预算。助手现在接受 `--max-suppression-ratio <0-1>` 所以
当抑制的存储桶超过允许的范围时，CI 或操作员可能会快速失败
窗口（默认 10%）或尚不存在存储桶时。推荐流程：

1. 从中继管理端点以及协调器的端点导出 NDJSON
   `/v1/soranet/privacy/event|share` 流进
   `artifacts/sorafs_privacy/<relay>.ndjson`。
2. 使用策略预算运行助手：

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   该命令打印观察到的比率，并在预算达到时以非零值退出
   超过**或**，当没有桶准备好时，表明遥测尚未准备好
   尚未生产用于运行。实时指标应该显示
   `soranet_privacy_pending_collectors` 向零耗尽并且
   `soranet_privacy_snapshot_suppression_ratio` 保持在相同的预算范围内
   当运行执行时。
3. 之前使用 SNNet-8 证据包归档 JSON 输出和 CLI 日志
   翻转传输默认值，以便审阅者可以重放确切的工件。

## 后续步骤 (SNNet-8a)

- 集成双 Prio 收集器，将其共享摄取连接到
  运行时，以便继电器和收集器发出一致的 `SoranetPrivacyBucketMetricsV1`
  有效负载。 *（完成 — 请参阅 `ingest_privacy_payload`
  `crates/sorafs_orchestrator/src/lib.rs` 和随附的测试。）*
- 发布共享的 Prometheus 仪表板 JSON 和警报规则，涵盖
  抑制差距、收集器健康状况和匿名限制。 *（完成 - 参见
  `dashboards/grafana/soranet_privacy_metrics.json`，
  `dashboards/alerts/soranet_privacy_rules.yml`，
  `dashboards/alerts/soranet_policy_rules.yml`，和验证夹具。）*
- 产生差分隐私校准工件中描述的
  `privacy_metrics_dp.md`，包括可复制的笔记本和治理
  消化。 *（完成——笔记本+由以下人员生成的文物
  `scripts/telemetry/run_privacy_dp.py`； CI 包装器
  `scripts/telemetry/run_privacy_dp_notebook.sh` 通过以下命令执行笔记本
  `.github/workflows/release-pipeline.yml` 工作流程；治理摘要提交于
  `docs/source/status/soranet_privacy_dp_digest.md`。）*

当前版本提供了 SNNet-8 基础：确定性、
隐私安全的遥测技术，可直接插入现有的 Prometheus 抓取器
和仪表板。差分隐私校准工件已就位，
发布管道工作流程使笔记本输出保持新鲜，剩余的
工作重点是监控第一次自动运行以及扩展抑制
警报分析。