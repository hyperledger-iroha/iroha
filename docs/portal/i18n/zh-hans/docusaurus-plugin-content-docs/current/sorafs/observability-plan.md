---
id: observability-plan
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

## 目标
- 为网关、节点和多源协调器定义指标和结构化事件。
- 提供 Grafana 仪表板、警报阈值和验证挂钩。
- 制定 SLO 目标以及错误预算和混乱演习政策。

## 公制目录

### 网关表面

|公制|类型 |标签|笔记|
|--------|------|--------|--------|
| `sorafs_gateway_active` |仪表（UpDownCounter）| `endpoint`、`method`、`variant`、`chunker`、`profile` |通过 `SorafsGatewayOtel` 发出；跟踪每个端点/方法组合的正在进行的 HTTP 操作。 |
| `sorafs_gateway_responses_total` |专柜| `endpoint`、`method`、`variant`、`chunker`、`profile`、`result`、`status`、`error_code` |每个完成的网关请求都会递增一次； `result` ∈ {`success`,`error`,`dropped`}。 |
| `sorafs_gateway_ttfb_ms_bucket` |直方图| `endpoint`、`method`、`variant`、`chunker`、`profile`、`result`、`status`、`error_code` |网关响应的首字节延迟时间；导出为 Prometheus `_bucket/_sum/_count`。 |
| `sorafs_gateway_proof_verifications_total` |专柜| `profile_version`、`result`、`error_code` |在请求时捕获的证明验证结果（`result` ∈ {`success`，`failure`}）。 |
| `sorafs_gateway_proof_duration_ms_bucket` |直方图| `profile_version`、`result`、`error_code` | PoR 收据的验证延迟分布。 |
| `telemetry::sorafs.gateway.request` |结构化活动| `endpoint`、`method`、`variant`、`result`、`status`、`error_code`、`duration_ms` | Loki/Tempo 关联的每个请求完成时发出的结构化日志。 |

`telemetry::sorafs.gateway.request` 事件镜像具有结构化有效负载的 OTEL 计数器，并呈现 `endpoint`、`method`、`variant`、`status`、`error_code` 和 `duration_ms` Loki/Tempo 关联，而仪表板使用 OTLP 系列进行 SLO 跟踪。

### 健康证明遥测

|公制|类型 |标签|笔记|
|--------|------|--------|--------|
| `torii_sorafs_proof_health_alerts_total` |专柜| `provider_id`、`trigger`、`penalty` |每次 `RecordCapacityTelemetry` 发出 `SorafsProofHealthAlert` 时递增。 `trigger` 区分 PDP/PoTR/两者失败，而 `penalty` 捕获抵押品是否实际上被削减或被冷却抑制。 |
| `torii_sorafs_proof_health_pdp_failures`，`torii_sorafs_proof_health_potr_breaches` |仪表| `provider_id` |违规遥测窗口内报告的最新 PDP/PoTR 计数，以便团队可以量化提供商超出政策的程度。 |
| `torii_sorafs_proof_health_penalty_nano` |仪表| `provider_id` | Nano-XOR 数量在最后一次警报时大幅削减（当冷却时间抑制强制执行时为零）。 |
| `torii_sorafs_proof_health_cooldown` |仪表| `provider_id` |当后续警报暂时静音时，布尔量表（`1` = 警报被冷却抑制）浮出水面。 |
| `torii_sorafs_proof_health_window_end_epoch` |仪表| `provider_id` |为与警报相关的遥测窗口记录的纪元，以便操作员可以将 Norito 伪影关联起来。 |

这些提要现在为 Taikai 查看器仪表板的健康证明行提供支持
(`dashboards/grafana/taikai_viewer.json`)，为 CDN 运营商提供实时可见性
分为警报量、PDP/PoTR 触发组合、惩罚和冷却状态
提供者。

相同的指标现在支持两个 Taikai 查看器警报规则：
`SorafsProofHealthPenalty` 每当
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` 增加
最后 15 分钟，而 `SorafsProofHealthCooldown` 会在以下情况下发出警告：
提供者仍处于冷却状态五分钟。两个警报都位于
`dashboards/alerts/taikai_viewer_rules.yml` 因此 SRE 可以立即接收上下文
每当 PoR/PoTR 执法升级时。

### Orchestrator 表面

|指标/事件 |类型 |标签|制片人|笔记|
|----------------|------|--------|----------|--------|
| `sorafs_orchestrator_active_fetches` |仪表| `manifest_id`，`region` | `FetchMetricsCtx` |会议目前正在进行中。 |
| `sorafs_orchestrator_fetch_duration_ms` |直方图| `manifest_id`，`region` | `FetchMetricsCtx` |持续时间直方图（以毫秒为单位）； 1ms→30s桶。 |
| `sorafs_orchestrator_fetch_failures_total` |专柜| `manifest_id`、`region`、`reason` | `FetchMetricsCtx` |原因：`no_providers`、`no_healthy_providers`、`no_compatible_providers`、`exhausted_retries`、`observer_failed`、`internal_invariant`。 |
| `sorafs_orchestrator_retries_total` |专柜| `manifest_id`、`provider_id`、`reason` | `FetchMetricsCtx` |区分重试原因（`retry`、`digest_mismatch`、`length_mismatch`、`provider_error`）。 |
| `sorafs_orchestrator_provider_failures_total` |专柜| `manifest_id`、`provider_id`、`reason` | `FetchMetricsCtx` |捕获会话级禁用/故障计数。 |
| `sorafs_orchestrator_chunk_latency_ms` |直方图| `manifest_id`，`provider_id` | `FetchMetricsCtx` |用于吞吐量/SLO 分析的每块获取延迟分布（毫秒）。 |
| `sorafs_orchestrator_bytes_total` |专柜| `manifest_id`，`provider_id` | `FetchMetricsCtx` |每个清单/提供商交付的字节数；通过 PromQL 中的 `rate()` 获取吞吐量。 |
| `sorafs_orchestrator_stalls_total` |专柜| `manifest_id`、`provider_id` | `FetchMetricsCtx` |计数超过 `ScoreboardConfig::latency_cap_ms` 的块。 |
| `telemetry::sorafs.fetch.lifecycle` |结构化活动| `manifest`、`region`、`job_id`、`event`、`status`、`chunk_count`、`total_bytes`、`provider_candidates`、 `retry_budget`、`global_parallel_limit` | `FetchTelemetryCtx` |使用 Norito JSON 负载镜像作业生命周期（开始/完成）。 |
| `telemetry::sorafs.fetch.retry` |结构化活动| `manifest`、`region`、`job_id`、`provider`、`reason`、`attempts` | `FetchTelemetryCtx` |每个提供商重试次数发出； `attempts` 计算增量重试次数 (≥1)。 |
| `telemetry::sorafs.fetch.provider_failure` |结构化活动| `manifest`、`region`、`job_id`、`provider`、`reason`、`failures` | `FetchTelemetryCtx` |当提供商超过失败阈值时就会出现。 |
| `telemetry::sorafs.fetch.error` |结构化活动| `manifest`、`region`、`job_id`、`reason`、`provider?`、`provider_reason?`、`duration_ms` | `FetchTelemetryCtx` |终端故障记录，对Loki/Splunk摄取友好。 |
| `telemetry::sorafs.fetch.stall` |结构化活动| `manifest`、`region`、`job_id`、`provider`、`latency_ms`、`bytes` | `FetchTelemetryCtx` |当块延迟超出配置的上限（镜像停顿计数器）时引发。 |

### 节点/复制表面

|公制|类型 |标签|笔记|
|--------|------|--------|--------|
| `sorafs_node_capacity_utilisation_pct` |直方图| `provider_id` |存储利用率百分比的 OTEL 直方图（导出为 `_bucket/_sum/_count`）。 |
| `sorafs_node_por_success_total` |专柜| `provider_id` |成功 PoR 样本的单调计数器，源自调度程序快照。 |
| `sorafs_node_por_failure_total` |专柜| `provider_id` |失败 PoR 样本的单调计数器。 |
| `torii_sorafs_storage_bytes_*`、`torii_sorafs_storage_por_*` |仪表| `provider` |现有 Prometheus 计量器用于显示已用字节数、队列深度、PoR 运行计数。 |
| `torii_sorafs_capacity_*`、`torii_sorafs_uptime_bps`、`torii_sorafs_por_bps` |仪表| `provider` |提供商容量/正常运行时间成功数据显示在容量仪表板中。 |
| `torii_sorafs_por_ingest_backlog`、`torii_sorafs_por_ingest_failures_total` |仪表| `provider`、`manifest` |积压深度加上每次轮询 `/v2/sorafs/por/ingestion/{manifest}` 时导出的累积故障计数器，为“PoR Stalls”面板/警报提供数据。 |

### 维修和 SLA

|公制|类型 |标签|笔记|
|--------|------|--------|--------|
| `sorafs_repair_tasks_total` |专柜| `status` |用于修复任务转换的 OTEL 计数器。 |
| `sorafs_repair_latency_minutes` |直方图| `outcome` |维修生命周期延迟的 OTEL 直方图。 |
| `sorafs_repair_queue_depth` |直方图| `provider` |每个提供商的排队任务的 OTEL 直方图（快照式）。 |
| `sorafs_repair_backlog_oldest_age_seconds` |直方图| — |最旧的排队任务寿命（秒）的 OTEL 直方图。 |
| `sorafs_repair_lease_expired_total` |专柜| `outcome` |租约到期的 OTEL 柜台 (`requeued`/`escalated`)。 |
| `sorafs_repair_slash_proposals_total` |专柜| `outcome` |用于斜杠提案转换的 OTEL 计数器。 |
| `torii_sorafs_repair_tasks_total` |专柜| `status` | Prometheus 用于任务转换的计数器。 |
| `torii_sorafs_repair_latency_minutes_bucket` |直方图| `outcome` | Prometheus 修复生命周期延迟的直方图。 |
| `torii_sorafs_repair_queue_depth` |仪表| `provider` | Prometheus 每个提供商的排队任务量表。 |
| `torii_sorafs_repair_backlog_oldest_age_seconds` |仪表| — | Prometheus 最旧的排队任务期限（秒）的计量表。 |
| `torii_sorafs_repair_lease_expired_total` |专柜| `outcome` | Prometheus 租约到期柜台。 |
| `torii_sorafs_slash_proposals_total` |专柜| `outcome` | Prometheus 斜杠提议转换计数器。 |

治理审计 JSON 元数据镜像修复遥测标签（修复事件上的 `status`、`ticket_id`、`manifest`、`provider`；斜杠提案上的 `outcome`），因此指标和审计工件可以确定性地关联。

### 保留和 GC|公制|类型 |标签|笔记|
|--------|------|--------|--------|
| `sorafs_gc_runs_total` |专柜| `result` |用于 GC 扫描的 OTEL 计数器，由嵌入式节点发出。 |
| `sorafs_gc_evictions_total` |专柜| `reason` | OTEL 柜台用于按原因分组的驱逐清单。 |
| `sorafs_gc_bytes_freed_total` |专柜| `reason` | OTEL 计数器按原因分组释放的字节。 |
| `sorafs_gc_blocked_total` |专柜| `reason` | OTEL 计数器用于因主动维修或政策而阻止的驱逐。 |
| `torii_sorafs_gc_runs_total` |专柜| `result` | Prometheus GC 扫描计数器（成功/错误）。 |
| `torii_sorafs_gc_evictions_total` |专柜| `reason` | Prometheus 按原因分组的已驱逐清单计数器。 |
| `torii_sorafs_gc_bytes_freed_total` |专柜| `reason` | Prometheus 按原因分组的释放字节计数器。 |
| `torii_sorafs_gc_blocked_total` |专柜| `reason` | Prometheus 按原因分组的被阻止驱逐的计数器。 |
| `torii_sorafs_gc_expired_manifests` |仪表| — | GC 扫描观察到的过期清单的当前计数。 |
| `torii_sorafs_gc_oldest_expired_age_seconds` |仪表| — |最旧的过期清单的年龄（以秒为单位）（保留宽限期后）。 |

### 和解

|公制|类型 |标签|笔记|
|--------|------|--------|--------|
| `sorafs.reconciliation.runs_total` |专柜| `result` |用于调节快照的 OTEL 计数器。 |
| `sorafs.reconciliation.divergence_total` |专柜| — |每次运行的 OTEL 分歧计数计数器。 |
| `torii_sorafs_reconciliation_runs_total` |专柜| `result` | Prometheus 用于调节运行的计数器。 |
| `torii_sorafs_reconciliation_divergence_count` |仪表| — |调节报告中观察到的最新分歧计数。 |

### 及时检索证明 (PoTR) 和块 SLA

|公制|类型 |标签|制片人|笔记|
|--------|------|--------|----------|--------|
| `sorafs_potr_deadline_ms` |直方图| `tier`、`provider` | PoTR 协调员 |截止时间松弛（以毫秒为单位）（正=满足）。 |
| `sorafs_potr_failures_total` |专柜| `tier`、`provider`、`reason` | PoTR 协调员 |原因：`expired`、`missing_proof`、`corrupt_proof`。 |
| `sorafs_chunk_sla_violation_total` |专柜| `provider`、`manifest_id`、`reason` | SLA 监控 |当块传送未达到 SLO（延迟、成功率）时触发。 |
| `sorafs_chunk_sla_violation_active` |仪表| `provider`、`manifest_id` | SLA 监控 |布尔量表 (0/1) 在活动突破窗口期间切换。 |

## SLO 目标

- 网关免信任可用性：**99.9%**（HTTP 2xx/304 响应）。
- Trustless TTFB P95：热层≤120ms，温层≤300ms。
- 证明成功率：每天≥99.5%。
- Orchestrator 成功率（块完成）：≥99%。

## 仪表板和警报

1. **网关可观察性** (`dashboards/grafana/sorafs_gateway_observability.json`) — 通过 OTEL 指标跟踪无信任可用性、TTFB P95、拒绝故障和 PoR/PoTR 故障。
2. **Orchestrator 运行状况** (`dashboards/grafana/sorafs_fetch_observability.json`) — 涵盖多源负载、重试、提供程序故障和停顿突发。
3. **SoraNet 隐私指标** (`dashboards/grafana/soranet_privacy_metrics.json`) — 通过 `soranet_privacy_last_poll_unixtime`、`soranet_privacy_collector_enabled` 和 `soranet_privacy_poll_errors_total{provider}` 绘制匿名中继存储桶、抑制窗口和收集器运行状况的图表。
4. **容量运行状况** (`dashboards/grafana/sorafs_capacity_health.json`) — 跟踪提供商余量和修复 SLA 升级、提供商的修复队列深度以及 GC 扫描/逐出/字节释放/阻止原因/过期清单期限和协调分歧快照。

警报包：

- `dashboards/alerts/sorafs_gateway_rules.yml` — 网关可用性、TTFB、证明故障峰值。
- `dashboards/alerts/sorafs_fetch_rules.yml` — 协调器故障/重试/停顿；通过 `scripts/telemetry/test_sorafs_fetch_alerts.sh`、`dashboards/alerts/tests/sorafs_fetch_rules.test.yml`、`dashboards/alerts/tests/soranet_privacy_rules.test.yml` 和 `dashboards/alerts/tests/soranet_policy_rules.test.yml` 进行验证。
- `dashboards/alerts/sorafs_capacity_rules.yml` — 容量压力加上修复 SLA/积压/租约到期警报以及用于保留扫描的 GC 停顿/阻塞/错误警报。
- `dashboards/alerts/soranet_privacy_rules.yml` — 隐私降级峰值、抑制警报、收集器空闲检测和禁用收集器警报（`soranet_privacy_last_poll_unixtime`、`soranet_privacy_collector_enabled`）。
- `dashboards/alerts/soranet_policy_rules.yml` — 连接到 `sorafs_orchestrator_brownouts_total` 的匿名掉电警报。
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai 查看器漂移/摄取/CEK 滞后警报以及由 `torii_sorafs_proof_health_*` 提供支持的新 SoraFS 证明健康惩罚/冷却警报。

## 追踪策略

- 采用端到端 OpenTelemetry：
  - 网关发出带有请求 ID、清单摘要和令牌哈希注释的 OTLP 范围 (HTTP)。
  - 协调器使用 `tracing` + `opentelemetry` 导出范围以进行提取尝试。
  - 嵌入式 SoraFS 节点导出跨度以应对 PoR 挑战和存储操作。所有组件共享通过 `x-sorafs-trace` 传播的公共跟踪 ID。
- `SorafsFetchOtel` 将协调器指标桥接到 OTLP 直方图中，而 `telemetry::sorafs.fetch.*` 事件为以日志为中心的后端提供轻量级 JSON 有效负载。
- 收集器：与 Prometheus/Loki/Tempo（首选 Tempo）一起运行 OTEL 收集器。 Jaeger API 导出器仍然是可选的。
- 应对高基数操作进行采样（成功路径为 10%，失败路径为 100%）。

## TLS 遥测协调 (SF-5b)

- 公制对齐：
  - TLS 自动化发布 `sorafs_gateway_tls_cert_expiry_seconds`、`sorafs_gateway_tls_renewal_total{result}` 和 `sorafs_gateway_tls_ech_enabled`。
  - 将这些仪表包含在 TLS/证书面板下的网关概述仪表板中。
- 警报联动：
  - 当 TLS 到期警报触发时（剩余 ≤14 天）与无需信任的可用性 SLO 相关。
  - ECH 禁用会发出引用 TLS 和可用性面板的辅助警报。
- 管道：TLS 自动化作业导出到与网关指标相同的 Prometheus 堆栈；与 SF-5b 的协调可确保重复数据删除。

## 指标命名和标签约定

- 指标名称遵循 Torii 和网关使用的现有 `torii_sorafs_*` 或 `sorafs_*` 前缀。
- 标签集标准化：
  - `result` → HTTP 结果（`success`、`refused`、`failed`）。
  - `reason` → 拒绝/错误代码（`unsupported_chunker`、`timeout` 等）。
  - `status` → 修复任务状态（`queued`、`in_progress`、`completed`、`failed`、`escalated`）。
  - `outcome` → 修复租赁或延迟结果（`requeued`、`escalated`、`completed`、`failed`）。
  - `provider` → 十六进制编码的提供商标识符。
  - `manifest` → 规范清单摘要（高基数时修剪）。
  - `tier` → 声明性层标签（`hot`、`warm`、`archive`）。
- 遥测发射点：
  - 网关指标位于 `torii_sorafs_*` 下，并重用 `crates/iroha_core/src/telemetry.rs` 中的约定。
  - 协调器发出 `sorafs_orchestrator_*` 指标和 `telemetry::sorafs.fetch.*` 事件（生命周期、重试、提供程序故障、错误、停顿），标记有清单摘要、作业 ID、区域和提供程序标识符。
  - 节点表面 `torii_sorafs_storage_*`、`torii_sorafs_capacity_*` 和 `torii_sorafs_por_*`。
- 与 Observability 协调，在共享的 Prometheus 命名文档中注册指标目录，包括标签基数期望（提供者/清单上限）。

## 数据管道

- 收集器与每个组件一起部署，将 OTLP 导出到 Prometheus（指标）和 Loki/Tempo（日志/跟踪）。
- 可选的 eBPF (Tetragon) 丰富了网关/节点的低级跟踪。
- 对 Torii 和嵌入式节点使用 `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}`；协调器继续调用 `install_sorafs_fetch_otlp_exporter`。

## 验证挂钩

- 在 CI 期间运行 `scripts/telemetry/test_sorafs_fetch_alerts.sh`，以确保 Prometheus 警报规则与停顿指标和隐私抑制检查保持同步。
- 将 Grafana 仪表板置于版本控制 (`dashboards/grafana/`) 之下，并在面板更改时更新屏幕截图/链接。
- 混沌演习通过 `scripts/telemetry/log_sorafs_drill.sh` 记录结果；验证利用 `scripts/telemetry/validate_drill_log.sh`（请参阅[操作手册](operations-playbook.md)）。