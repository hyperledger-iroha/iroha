---
id: observability-plan
lang: zh-hant
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

:::注意規範來源
:::

## 目標
- 為網關、節點和多源協調器定義指標和結構化事件。
- 提供 Grafana 儀表板、警報閾值和驗證掛鉤。
- 制定 SLO 目標以及錯誤預算和混亂演習政策。

## 公制目錄

### 網關表面

|公制|類型 |標籤|筆記|
|--------|------|--------|--------|
| `sorafs_gateway_active` |儀表（UpDownCounter）| `endpoint`、`method`、`variant`、`chunker`、`profile` |通過 `SorafsGatewayOtel` 發出；跟踪每個端點/方法組合的正在進行的 HTTP 操作。 |
| `sorafs_gateway_responses_total` |專櫃| `endpoint`、`method`、`variant`、`chunker`、`profile`、`result`、`status`、`error_code` |每個完成的網關請求都會遞增一次； `result` ∈ {`success`,`error`,`dropped`}。 |
| `sorafs_gateway_ttfb_ms_bucket` |直方圖| `endpoint`、`method`、`variant`、`chunker`、`profile`、`result`、`status`、`error_code` |網關響應的首字節延遲時間；導出為 Prometheus `_bucket/_sum/_count`。 |
| `sorafs_gateway_proof_verifications_total` |專櫃| `profile_version`、`result`、`error_code` |在請求時捕獲的證明驗證結果（`result` ∈ {`success`，`failure`}）。 |
| `sorafs_gateway_proof_duration_ms_bucket` |直方圖| `profile_version`、`result`、`error_code` | PoR 收據的驗證延遲分佈。 |
| `telemetry::sorafs.gateway.request` |結構化活動| `endpoint`、`method`、`variant`、`result`、`status`、`error_code`、`duration_ms` | Loki/Tempo 關聯的每個請求完成時發出的結構化日誌。 |

`telemetry::sorafs.gateway.request` 事件鏡像具有結構化有效負載的 OTEL 計數器，並呈現 `endpoint`、`method`、`variant`、`status`、`error_code` 和 `duration_ms` Loki/Tempo 關聯，而儀表板使用 OTLP 系列進行 SLO 跟踪。

### 健康證明遙測

|公制|類型 |標籤|筆記|
|--------|------|--------|--------|
| `torii_sorafs_proof_health_alerts_total` |專櫃| `provider_id`、`trigger`、`penalty` |每次 `RecordCapacityTelemetry` 發出 `SorafsProofHealthAlert` 時遞增。 `trigger` 區分 PDP/PoTR/兩者失敗，而 `penalty` 捕獲抵押品是否實際上被削減或被冷卻抑制。 |
| `torii_sorafs_proof_health_pdp_failures`，`torii_sorafs_proof_health_potr_breaches` |儀表| `provider_id` |違規遙測窗口內報告的最新 PDP/PoTR 計數，以便團隊可以量化提供商超出政策的程度。 |
| `torii_sorafs_proof_health_penalty_nano` |儀表| `provider_id` | Nano-XOR 數量在最後一次警報時大幅削減（當冷卻時間抑制強制執行時為零）。 |
| `torii_sorafs_proof_health_cooldown` |儀表| `provider_id` |當後續警報暫時靜音時，布爾量表（`1` = 警報被冷卻抑制）浮出水面。 |
| `torii_sorafs_proof_health_window_end_epoch` |儀表| `provider_id` |為與警報相關的遙測窗口記錄的紀元，以便操作員可以將 Norito 偽影關聯起來。 |

這些提要現在為 Taikai 查看器儀表板的健康證明行提供支持
(`dashboards/grafana/taikai_viewer.json`)，為 CDN 運營商提供實時可見性
分為警報量、PDP/PoTR 觸發組合、懲罰和冷卻狀態
提供者。

相同的指標現在支持兩個 Taikai 查看器警報規則：
`SorafsProofHealthPenalty` 每當
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` 增加
最後 15 分鐘，而 `SorafsProofHealthCooldown` 會在以下情況下發出警告：
提供者仍處於冷卻狀態五分鐘。兩個警報都位於
`dashboards/alerts/taikai_viewer_rules.yml` 因此 SRE 可以立即接收上下文
每當 PoR/PoTR 執法升級時。

### Orchestrator 表面

|指標/事件 |類型 |標籤|製片人|筆記|
|----------------|------|--------|----------|--------|
| `sorafs_orchestrator_active_fetches` |儀表| `manifest_id`，`region` | `FetchMetricsCtx` |會議目前正在進行中。 |
| `sorafs_orchestrator_fetch_duration_ms` |直方圖| `manifest_id`，`region` | `FetchMetricsCtx` |持續時間直方圖（以毫秒為單位）； 1ms→30s桶。 |
| `sorafs_orchestrator_fetch_failures_total` |專櫃| `manifest_id`、`region`、`reason` | `FetchMetricsCtx` |原因：`no_providers`、`no_healthy_providers`、`no_compatible_providers`、`exhausted_retries`、`observer_failed`、`internal_invariant`。 |
| `sorafs_orchestrator_retries_total` |專櫃| `manifest_id`、`provider_id`、`reason` | `FetchMetricsCtx` |區分重試原因（`retry`、`digest_mismatch`、`length_mismatch`、`provider_error`）。 |
| `sorafs_orchestrator_provider_failures_total` |專櫃| `manifest_id`、`provider_id`、`reason` | `FetchMetricsCtx` |捕獲會話級禁用/故障計數。 |
| `sorafs_orchestrator_chunk_latency_ms` |直方圖| `manifest_id`，`provider_id` | `FetchMetricsCtx` |用於吞吐量/SLO 分析的每塊獲取延遲分佈（毫秒）。 |
| `sorafs_orchestrator_bytes_total` |專櫃| `manifest_id`，`provider_id` | `FetchMetricsCtx` |每個清單/提供商交付的字節數；通過 PromQL 中的 `rate()` 獲取吞吐量。 |
| `sorafs_orchestrator_stalls_total` |專櫃| `manifest_id`、`provider_id` | `FetchMetricsCtx` |計數超過 `ScoreboardConfig::latency_cap_ms` 的塊。 |
| `telemetry::sorafs.fetch.lifecycle` |結構化活動| `manifest`、`region`、`job_id`、`event`、`status`、`chunk_count`、`total_bytes`、`provider_candidates`、 `retry_budget`、`global_parallel_limit` | `FetchTelemetryCtx` |使用 Norito JSON 負載鏡像作業生命週期（開始/完成）。 |
| `telemetry::sorafs.fetch.retry` |結構化活動| `manifest`、`region`、`job_id`、`provider`、`reason`、`attempts` | `FetchTelemetryCtx` |每個提供商重試次數發出； `attempts` 計算增量重試次數 (≥1)。 |
| `telemetry::sorafs.fetch.provider_failure` |結構化活動| `manifest`、`region`、`job_id`、`provider`、`reason`、`failures` | `FetchTelemetryCtx` |當提供商超過失敗閾值時就會出現。 |
| `telemetry::sorafs.fetch.error` |結構化活動| `manifest`、`region`、`job_id`、`reason`、`provider?`、`provider_reason?`、`duration_ms` | `FetchTelemetryCtx` |終端故障記錄，對Loki/Splunk攝取友好。 |
| `telemetry::sorafs.fetch.stall` |結構化活動| `manifest`、`region`、`job_id`、`provider`、`latency_ms`、`bytes` | `FetchTelemetryCtx` |當塊延遲超出配置的上限（鏡像停頓計數器）時引發。 |

### 節點/複製表面

|公制|類型 |標籤|筆記|
|--------|------|--------|--------|
| `sorafs_node_capacity_utilisation_pct` |直方圖| `provider_id` |存儲利用率百分比的 OTEL 直方圖（導出為 `_bucket/_sum/_count`）。 |
| `sorafs_node_por_success_total` |專櫃| `provider_id` |成功 PoR 樣本的單調計數器，源自調度程序快照。 |
| `sorafs_node_por_failure_total` |專櫃| `provider_id` |失敗 PoR 樣本的單調計數器。 |
| `torii_sorafs_storage_bytes_*`、`torii_sorafs_storage_por_*` |儀表| `provider` |現有 Prometheus 計量器用於顯示已用字節數、隊列深度、PoR 運行計數。 |
| `torii_sorafs_capacity_*`、`torii_sorafs_uptime_bps`、`torii_sorafs_por_bps` |儀表| `provider` |提供商容量/正常運行時間成功數據顯示在容量儀表板中。 |
| `torii_sorafs_por_ingest_backlog`、`torii_sorafs_por_ingest_failures_total` |儀表| `provider`、`manifest` |積壓深度加上每次輪詢 `/v1/sorafs/por/ingestion/{manifest}` 時導出的累積故障計數器，為“PoR Stalls”面板/警報提供數據。 |

### 維修和 SLA

|公制|類型 |標籤|筆記|
|--------|------|--------|--------|
| `sorafs_repair_tasks_total` |專櫃| `status` |用於修復任務轉換的 OTEL 計數器。 |
| `sorafs_repair_latency_minutes` |直方圖| `outcome` |維修生命週期延遲的 OTEL 直方圖。 |
| `sorafs_repair_queue_depth` |直方圖| `provider` |每個提供商的排隊任務的 OTEL 直方圖（快照式）。 |
| `sorafs_repair_backlog_oldest_age_seconds` |直方圖| — |最舊的排隊任務壽命（秒）的 OTEL 直方圖。 |
| `sorafs_repair_lease_expired_total` |專櫃| `outcome` |租約到期的 OTEL 櫃檯 (`requeued`/`escalated`)。 |
| `sorafs_repair_slash_proposals_total` |專櫃| `outcome` |用於斜杠提案轉換的 OTEL 計數器。 |
| `torii_sorafs_repair_tasks_total` |專櫃| `status` | Prometheus 用於任務轉換的計數器。 |
| `torii_sorafs_repair_latency_minutes_bucket` |直方圖| `outcome` | Prometheus 修復生命週期延遲的直方圖。 |
| `torii_sorafs_repair_queue_depth` |儀表| `provider` | Prometheus 每個提供商的排隊任務量表。 |
| `torii_sorafs_repair_backlog_oldest_age_seconds` |儀表| — | Prometheus 最舊的排隊任務期限（秒）的計量表。 |
| `torii_sorafs_repair_lease_expired_total` |專櫃| `outcome` | Prometheus 租約到期櫃檯。 |
| `torii_sorafs_slash_proposals_total` |專櫃| `outcome` | Prometheus 斜杠提議轉換計數器。 |

治理審計 JSON 元數據鏡像修復遙測標籤（修復事件上的 `status`、`ticket_id`、`manifest`、`provider`；斜杠提案上的 `outcome`），因此指標和審計工件可以確定性地關聯。

### 保留和 GC|公制|類型 |標籤|筆記|
|--------|------|--------|--------|
| `sorafs_gc_runs_total` |專櫃| `result` |用於 GC 掃描的 OTEL 計數器，由嵌入式節點發出。 |
| `sorafs_gc_evictions_total` |專櫃| `reason` | OTEL 櫃檯用於按原因分組的驅逐清單。 |
| `sorafs_gc_bytes_freed_total` |專櫃| `reason` | OTEL 計數器按原因分組釋放的字節。 |
| `sorafs_gc_blocked_total` |專櫃| `reason` | OTEL 計數器用於因主動維修或政策而阻止的驅逐。 |
| `torii_sorafs_gc_runs_total` |專櫃| `result` | Prometheus GC 掃描計數器（成功/錯誤）。 |
| `torii_sorafs_gc_evictions_total` |專櫃| `reason` | Prometheus 按原因分組的已驅逐清單計數器。 |
| `torii_sorafs_gc_bytes_freed_total` |專櫃| `reason` | Prometheus 按原因分組的釋放字節計數器。 |
| `torii_sorafs_gc_blocked_total` |專櫃| `reason` | Prometheus 按原因分組的被阻止驅逐的計數器。 |
| `torii_sorafs_gc_expired_manifests` |儀表| — | GC 掃描觀察到的過期清單的當前計數。 |
| `torii_sorafs_gc_oldest_expired_age_seconds` |儀表| — |最舊的過期清單的年齡（以秒為單位）（保留寬限期後）。 |

### 和解

|公制|類型 |標籤|筆記|
|--------|------|--------|--------|
| `sorafs.reconciliation.runs_total` |專櫃| `result` |用於調節快照的 OTEL 計數器。 |
| `sorafs.reconciliation.divergence_total` |專櫃| — |每次運行的 OTEL 分歧計數計數器。 |
| `torii_sorafs_reconciliation_runs_total` |專櫃| `result` | Prometheus 用於調節運行的計數器。 |
| `torii_sorafs_reconciliation_divergence_count` |儀表| — |調節報告中觀察到的最新分歧計數。 |

### 及時檢索證明 (PoTR) 和塊 SLA

|公制|類型 |標籤|製片人|筆記|
|--------|------|--------|----------|--------|
| `sorafs_potr_deadline_ms` |直方圖| `tier`、`provider` | PoTR 協調員 |截止時間鬆弛（以毫秒為單位）（正=滿足）。 |
| `sorafs_potr_failures_total` |專櫃| `tier`、`provider`、`reason` | PoTR 協調員 |原因：`expired`、`missing_proof`、`corrupt_proof`。 |
| `sorafs_chunk_sla_violation_total` |專櫃| `provider`、`manifest_id`、`reason` | SLA 監控 |當塊傳送未達到 SLO（延遲、成功率）時觸發。 |
| `sorafs_chunk_sla_violation_active` |儀表| `provider`、`manifest_id` | SLA 監控 |布爾量表 (0/1) 在活動突破窗口期間切換。 |

## SLO 目標

- 網關免信任可用性：**99.9%**（HTTP 2xx/304 響應）。
- Trustless TTFB P95：熱層≤120ms，溫層≤300ms。
- 證明成功率：每天≥99.5%。
- Orchestrator 成功率（塊完成）：≥99%。

## 儀表板和警報

1. **網關可觀察性** (`dashboards/grafana/sorafs_gateway_observability.json`) — 通過 OTEL 指標跟踪無信任可用性、TTFB P95、拒絕故障和 PoR/PoTR 故障。
2. **Orchestrator 運行狀況** (`dashboards/grafana/sorafs_fetch_observability.json`) — 涵蓋多源負載、重試、提供程序故障和停頓突發。
3. **SoraNet 隱私指標** (`dashboards/grafana/soranet_privacy_metrics.json`) — 通過 `soranet_privacy_last_poll_unixtime`、`soranet_privacy_collector_enabled` 和 `soranet_privacy_poll_errors_total{provider}` 繪製匿名中繼存儲桶、抑制窗口和收集器運行狀況的圖表。
4. **容量運行狀況** (`dashboards/grafana/sorafs_capacity_health.json`) — 跟踪提供商餘量和修復 SLA 升級、提供商的修復隊列深度以及 GC 掃描/逐出/字節釋放/阻止原因/過期清單期限和協調分歧快照。

警報包：

- `dashboards/alerts/sorafs_gateway_rules.yml` — 網關可用性、TTFB、證明故障峰值。
- `dashboards/alerts/sorafs_fetch_rules.yml` — 協調器故障/重試/停頓；通過 `scripts/telemetry/test_sorafs_fetch_alerts.sh`、`dashboards/alerts/tests/sorafs_fetch_rules.test.yml`、`dashboards/alerts/tests/soranet_privacy_rules.test.yml` 和 `dashboards/alerts/tests/soranet_policy_rules.test.yml` 進行驗證。
- `dashboards/alerts/sorafs_capacity_rules.yml` — 容量壓力加上修復 SLA/積壓/租約到期警報以及用於保留掃描的 GC 停頓/阻塞/錯誤警報。
- `dashboards/alerts/soranet_privacy_rules.yml` — 隱私降級峰值、抑制警報、收集器空閒檢測和禁用收集器警報（`soranet_privacy_last_poll_unixtime`、`soranet_privacy_collector_enabled`）。
- `dashboards/alerts/soranet_policy_rules.yml` — 連接到 `sorafs_orchestrator_brownouts_total` 的匿名掉電警報。
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai 查看器漂移/攝取/CEK 滯後警報以及由 `torii_sorafs_proof_health_*` 提供支持的新 SoraFS 證明健康懲罰/冷卻警報。

## 追踪策略

- 採用端到端 OpenTelemetry：
  - 網關發出帶有請求 ID、清單摘要和令牌哈希註釋的 OTLP 範圍 (HTTP)。
  - 協調器使​​用 `tracing` + `opentelemetry` 導出範圍以進行提取嘗試。
  - 嵌入式 SoraFS 節點導出跨度以應對 PoR 挑戰和存儲操作。所有組件共享通過 `x-sorafs-trace` 傳播的公共跟踪 ID。
- `SorafsFetchOtel` 將協調器指標橋接到 OTLP 直方圖中，而 `telemetry::sorafs.fetch.*` 事件為以日誌為中心的後端提供輕量級 JSON 有效負載。
- 收集器：與 Prometheus/Loki/Tempo（首選 Tempo）一起運行 OTEL 收集器。 Jaeger API 導出器仍然是可選的。
- 應對高基數操作進行採樣（成功路徑為 10%，失敗路徑為 100%）。

## TLS 遙測協調 (SF-5b)

- 公制對齊：
  - TLS 自動化發布 `sorafs_gateway_tls_cert_expiry_seconds`、`sorafs_gateway_tls_renewal_total{result}` 和 `sorafs_gateway_tls_ech_enabled`。
  - 將這些儀表包含在 TLS/證書面板下的網關概述儀表板中。
- 警報聯動：
  - 當 TLS 到期警報觸發時（剩餘 ≤14 天）與無需信任的可用性 SLO 相關。
  - ECH 禁用會發出引用 TLS 和可用性面板的輔助警報。
- 管道：TLS 自動化作業導出到與網關指標相同的 Prometheus 堆棧；與 SF-5b 的協調可確保重複數據刪除。

## 指標命名和標籤約定

- 指標名稱遵循 Torii 和網關使用的現有 `torii_sorafs_*` 或 `sorafs_*` 前綴。
- 標籤集標準化：
  - `result` → HTTP 結果（`success`、`refused`、`failed`）。
  - `reason` → 拒絕/錯誤代碼（`unsupported_chunker`、`timeout` 等）。
  - `status` → 修復任務狀態（`queued`、`in_progress`、`completed`、`failed`、`escalated`）。
  - `outcome` → 修復租賃或延遲結果（`requeued`、`escalated`、`completed`、`failed`）。
  - `provider` → 十六進制編碼的提供商標識符。
  - `manifest` → 規范清單摘要（高基數時修剪）。
  - `tier` → 聲明性層標籤（`hot`、`warm`、`archive`）。
- 遙測發射點：
  - 網關指標位於 `torii_sorafs_*` 下，並重用 `crates/iroha_core/src/telemetry.rs` 中的約定。
  - 協調器發出 `sorafs_orchestrator_*` 指標和 `telemetry::sorafs.fetch.*` 事件（生命週期、重試、提供程序故障、錯誤、停頓），標記有清單摘要、作業 ID、區域和提供程序標識符。
  - 節點表面 `torii_sorafs_storage_*`、`torii_sorafs_capacity_*` 和 `torii_sorafs_por_*`。
- 與 Observability 協調，在共享的 Prometheus 命名文檔中註冊指標目錄，包括標籤基數期望（提供者/清單上限）。

## 數據管道

- 收集器與每個組件一起部署，將 OTLP 導出到 Prometheus（指標）和 Loki/Tempo（日誌/跟踪）。
- 可選的 eBPF (Tetragon) 豐富了網關/節點的低級跟踪。
- 對 Torii 和嵌入式節點使用 `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}`；協調器繼續調用 `install_sorafs_fetch_otlp_exporter`。

## 驗證掛鉤

- 在 CI 期間運行 `scripts/telemetry/test_sorafs_fetch_alerts.sh`，以確保 Prometheus 警報規則與停頓指標和隱私抑制檢查保持同步。
- 將 Grafana 儀表板置於版本控制 (`dashboards/grafana/`) 之下，並在面板更改時更新屏幕截圖/鏈接。
- 混沌演習通過 `scripts/telemetry/log_sorafs_drill.sh` 記錄結果；驗證利用 `scripts/telemetry/validate_drill_log.sh`（請參閱[操作手冊](operations-playbook.md)）。