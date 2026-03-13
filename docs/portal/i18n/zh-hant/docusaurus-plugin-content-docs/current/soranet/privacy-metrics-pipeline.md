---
id: privacy-metrics-pipeline
lang: zh-hant
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

:::注意規範來源
:::

# SoraNet 隱私指標管道

SNNet-8 為中繼運行時引入了隱私感知遙測表面。的
中繼現在將握手和電路事件聚合到分鐘大小的桶中，
僅導出粗 Prometheus 計數器，保留單獨的電路
不可鏈接，同時為操作員提供可操作的可見性。

## 聚合器概述

- 運行時實現位於 `tools/soranet-relay/src/privacy.rs` 中，如下所示
  `PrivacyAggregator`。
- 存儲桶由掛鐘分鐘（`bucket_secs`，默認 60 秒）和
  存儲在有界環中（`max_completed_buckets`，默認 120）。收藏家
  股票保留自己的有限積壓（`max_share_lag_buckets`，默認 12）
  陳舊的 Prio 窗戶會像抑制水桶一樣被沖走，而不是漏水
  內存或屏蔽卡住的收集器。
- `RelayConfig::privacy` 直接映射到 `PrivacyConfig`，暴露調優
  旋鈕（`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、
  `force_flush_buckets`、`max_completed_buckets`、`max_share_lag_buckets`、
  `expected_shares`）。生產運行時保留默認值，而 SNNet-8a
  引入安全聚合閾值。
- 運行時模塊通過類型化助手記錄事件：
  `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、
  `record_throttle_cooldown`、`record_capacity_reject`、`record_active_sample`、
  `record_verified_bytes` 和 `record_gar_category`。

## 中繼管理端點

操作員可以通過以下方式輪詢中繼的管理監聽器以獲取原始觀察結果
`GET /privacy/events`。端點返回以換行符分隔的 JSON
(`application/x-ndjson`) 包含鏡像的 `SoranetPrivacyEventV1` 有效負載
來自內部 `PrivacyEventBuffer`。緩衝區保留最新的事件
到 `privacy.event_buffer_capacity` 條目（默認 4096）並耗盡
閱讀，因此抓取工具應該足夠頻繁地輪詢以避免出現間隙。活動涵蓋
相同的握手、油門、驗證帶寬、有源電路和 GAR 信號
為 Prometheus 計數器供電，允許下游收集器歸檔
隱私安全的麵包屑或提供安全的聚合工作流程。

## 繼電器配置

操作員通過以下方式調整中繼配置文件中的隱私遙測節奏
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

字段默認值符合 SNNet-8 規範，並在加載時進行驗證：

|領域|描述 |默認|
|--------|-------------|---------|
| `bucket_secs` |每個聚合窗口的寬度（秒）。 | `60` |
| `min_handshakes` |存儲桶可以發出計數器之前的最小貢獻者計數。 | `12` |
| `flush_delay_buckets` |在嘗試刷新之前要等待已完成的存儲桶。 | `1` |
| `force_flush_buckets` |我們發射抑製桶之前的最大年齡。 | `6` |
| `max_completed_buckets` |保留存儲桶積壓（防止內存無界）。 | `120` |
| `max_share_lag_buckets` |抑制前收集股的保留窗口。 | `12` |
| `expected_shares` |合併前需要 Prio 收集者股份。 | `2` |
| `event_buffer_capacity` |管理流的 NDJSON 事件積壓。 | `4096` |

將 `force_flush_buckets` 設置為低於 `flush_delay_buckets`，將
閾值，或禁用保留防護現在無法通過驗證來避免
會洩漏每個中繼遙測數據的部署。

`event_buffer_capacity` 限制還限制了 `/admin/privacy/events`，確保
爬蟲不可能無限期地落後。

## Prio 收藏家分享

SNNet-8a 部署雙收集器，發出秘密共享的 Prio 存儲桶。的
Orchestrator 現在解析 `/privacy/events` NDJSON 流
`SoranetPrivacyEventV1` 條目和 `SoranetPrivacyPrioShareV1` 共享，
將它們轉發到 `SoranetSecureAggregator::ingest_prio_share`。桶排放
一旦 `PrivacyBucketConfig::expected_shares` 貢獻到達，鏡像
中繼行為。份額經過桶對齊和直方圖形狀驗證
在合併為 `SoranetPrivacyBucketMetricsV1` 之前。如果結合起來
握手計數低於 `min_contributors`，存儲桶導出為
`suppressed`，鏡像中繼內聚合器的行為。壓抑
Windows 現在發出 `suppression_reason` 標籤，以便操作員可以區分
`insufficient_contributors`、`collector_suppressed` 之間，
`collector_window_elapsed` 和 `forced_flush_window_elapsed` 場景
診斷遙測差距。 `collector_window_elapsed` 原因也引發
當 Prio 股價徘徊在 `max_share_lag_buckets` 以上時，讓收藏家陷入困境
可見，而不會在內存中留下陳舊的累加器。

## Torii 攝取端點

Torii 現在公開兩個遙測門控 HTTP 端點，以便中繼和收集器
可以在不嵌入定制傳輸的情況下轉發觀察結果：

- `POST /v2/soranet/privacy/event` 接受
  `RecordSoranetPrivacyEventDto` 有效負載。身體包裹著一個
  `SoranetPrivacyEventV1` 加上可選的 `source` 標籤。 Torii 驗證
  針對活動遙測配置文件的請求、記錄事件並響應
  帶有 HTTP `202 Accepted` 以及包含以下內容的 Norito JSON 信封
  計算的桶窗口（`bucket_start_unix`、`bucket_duration_secs`）和
  中繼模式。
- `POST /v2/soranet/privacy/share` 接受 `RecordSoranetPrivacyShareDto`
  有效負載。機身帶有 `SoranetPrivacyPrioShareV1` 和可選的
  `forwarded_by` 提示操作員可以審核收集器流量。成功
  提交返回 HTTP `202 Accepted` 和 Norito JSON 信封總結
  收集器、存儲桶窗口和抑制提示；驗證失敗映射到
  遙測 `Conversion` 響應以保留確定性錯誤處理
  跨收藏家。編排器的事件循環現在會發出這些共享
  輪詢中繼，使 Torii 的 Prio 累加器與中繼桶保持同步。

兩個端點均遵循遙測配置文件：它們發出“503 服務”
當指標被禁用時不可用。客戶端可以發送 Norito 二進製文件
(`application/x.norito`) 或 Norito JSON (`application/x.norito+json`) 機構；
服務器通過標準 Torii 自動協商格式
提取器。

## Prometheus 指標

每個導出的桶攜帶 `mode` (`entry`, `middle`, `exit`) 和
`bucket_start` 標籤。發出以下度量標準系列：

|公制|描述 |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` |握手分類為 `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`。 |
| `soranet_privacy_throttles_total{scope}` |油門計數器為 `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |由限制握手造成的聚合冷卻持續時間。 |
| `soranet_privacy_verified_bytes_total` |通過盲法測量證明驗證帶寬。 |
| `soranet_privacy_active_circuits_{avg,max}` |每個桶的平均和峰值有源電路。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT 百分位估計（`p50`、`p90`、`p99`）。 |
| `soranet_privacy_gar_reports_total{category_hash}` |按類別摘要鍵入的哈希治理行動報告計數器。 |
| `soranet_privacy_bucket_suppressed` |由於未達到貢獻者閾值，存儲桶被扣留。 |
| `soranet_privacy_pending_collectors{mode}` |收集器共享累加器待組合，按中繼模式分組。 |
| `soranet_privacy_suppression_total{reason}` |使用 `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` 抑制存儲桶計數器，以便儀表板可以歸因隱私差距。 |
| `soranet_privacy_snapshot_suppression_ratio` |最後一次消耗的抑制/消耗比率 (0-1)，對於警報預算很有用。 |
| `soranet_privacy_last_poll_unixtime` |最近成功輪詢的 UNIX 時間戳（驅動收集器空閒警報）。 |
| `soranet_privacy_collector_enabled` |當隱私收集器被禁用或無法啟動時，儀表會翻轉為 `0`（驅動收集器禁用警報）。 |
| `soranet_privacy_poll_errors_total{provider}` |按中繼別名分組的輪詢失敗（解碼錯誤、HTTP 失敗或意外狀態代碼的增量）。 |

沒有觀察的桶保持沉默，保持儀表板整潔，無需觀察
製作零填充窗口。

## 操作指導

1. **儀表板** – 將上述指標按 `mode` 和 `window_start` 分組繪製圖表。
   突出顯示缺失的窗口以暴露收集器或繼電器問題。使用
   `soranet_privacy_suppression_total{reason}` 區分貢獻者
   對間隙進行分類時收集器驅動的抑製造成的不足。 Grafana
   資產現在運送一個專用的 **“抑制原因 (5m)”** 面板，由這些人員提供
   計數器加上 **“Suppressed Bucket %”** 計算統計數據
   `sum(soranet_privacy_bucket_suppressed) / count(...)` 每個選擇所以
   運營商可以一目了然地發現預算違規情況。 **收藏家分享
   積壓**系列 (`soranet_privacy_pending_collectors`) 和**快照
   抑制率**統計數據突出了收藏家和預算漂移
   自動運行。
2. **警報** – 從隱私安全計數器發出警報：PoW 拒絕峰值，
   冷卻頻率、RTT 漂移和容量拒絕。因為計數器是
   每個存儲桶內都是單調的，基於率的簡單規則效果很好。
3. **事件響應** – 首先依賴聚合數據。當進行更深層次的調試時
   必要時，請求中繼重放存儲桶快照或進行盲檢查
   測量證明而不是收集原始流量日誌。
4. **保留** – 經常刮擦以避免超過
   `max_completed_buckets`。出口商應將 Prometheus 輸出視為
   規范源並在轉發後刪除本地存儲桶。

## 抑制分析和自動運行SNNet-8 的接受取決於證明自動收集器的存在
健康且抑制保持在政策範圍內（每個桶的 ≤10%）
任何 30 分鐘窗口內的中繼）。現在滿足該要求所需的工具
隨樹而船；操作員必須將其納入每週的例行公事中。新的
Grafana 抑制面板鏡像下面的 PromQL 片段，提供 on-call
團隊在需要回退到手動查詢之前實時了解情況。

### 用於抑制審查的 PromQL 配方

操作員應隨身攜帶以下 PromQL 助手；兩者都被引用
在共享 Grafana 儀表板 (`dashboards/grafana/soranet_privacy_metrics.json`) 中
和警報管理器規則：

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

使用比率輸出確認 **“Suppressed Bucket %”** 統計數據仍低於
政策預算；將尖峰檢測器連接到 Alertmanager 以獲得快速反饋
當貢獻者數量意外下降時。

### 離線存儲桶報告 CLI

工作區公開 `cargo xtask soranet-privacy-report` 用於一次性 NDJSON
捕獲。將其指向一個或多個中繼管理導出：

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

幫助程序通過 `SoranetSecureAggregator` 流式傳輸捕獲，打印
抑制摘要到標準輸出，並可選擇寫入結構化 JSON 報告
通過 `--json-out <path|->`。它具有與現場收集器相同的旋鈕
（`--bucket-secs`、`--min-contributors`、`--expected-shares` 等），讓
操作員在分類時重播不同閾值下的歷史捕獲
一個問題。將 JSON 與 Grafana 屏幕截圖一起附加，以便 SNNet-8
抑制分析門仍然可審計。

### 第一個自動運行清單

治理仍然需要證明第一次自動化運行滿足
抑制預算。助手現在接受 `--max-suppression-ratio <0-1>` 所以
當抑制的存儲桶超過允許的範圍時，CI 或操作員可能會快速失敗
窗口（默認 10%）或尚不存在存儲桶時。推薦流程：

1. 從中繼管理端點以及協調器的端點導出 NDJSON
   `/v2/soranet/privacy/event|share` 流進
   `artifacts/sorafs_privacy/<relay>.ndjson`。
2. 使用策略預算運行助手：

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   該命令打印觀察到的比率，並在預算達到時以非零值退出
   超過**或**，當沒有桶準備好時，表明遙測尚未準備好
   尚未生產用於運行。實時指標應該顯示
   `soranet_privacy_pending_collectors` 向零耗盡並且
   `soranet_privacy_snapshot_suppression_ratio` 保持在相同的預算範圍內
   當運行執行時。
3. 之前使用 SNNet-8 證據包歸檔 JSON 輸出和 CLI 日誌
   翻轉傳輸默認值，以便審閱者可以重放確切的工件。

## 後續步驟 (SNNet-8a)

- 集成雙 Prio 收集器，將其共享攝取連接到
  運行時，以便繼電器和收集器發出一致的 `SoranetPrivacyBucketMetricsV1`
  有效負載。 *（完成 — 請參閱 `ingest_privacy_payload`
  `crates/sorafs_orchestrator/src/lib.rs` 和隨附的測試。）*
- 發布共享的 Prometheus 儀表板 JSON 和警報規則，涵蓋
  抑制差距、收集器健康狀況和匿名限制。 *（完成 - 參見
  `dashboards/grafana/soranet_privacy_metrics.json`，
  `dashboards/alerts/soranet_privacy_rules.yml`，
  `dashboards/alerts/soranet_policy_rules.yml`，和驗證夾具。）*
- 產生差分隱私校準工件中描述的
  `privacy_metrics_dp.md`，包括可複制的筆記本和治理
  消化。 *（完成——筆記本+由以下人員生成的文物
  `scripts/telemetry/run_privacy_dp.py`； CI 包裝器
  `scripts/telemetry/run_privacy_dp_notebook.sh` 通過以下命令執行筆記本
  `.github/workflows/release-pipeline.yml` 工作流程；治理摘要提交於
  `docs/source/status/soranet_privacy_dp_digest.md`。）*

當前版本提供了 SNNet-8 基礎：確定性、
隱私安全的遙測技術，可直接插入現有的 Prometheus 抓取器
和儀表板。差分隱私校準工件已就位，
發布管道工作流程使筆記本輸出保持新鮮，剩餘的
工作重點是監控第一次自動運行以及擴展抑制
警報分析。