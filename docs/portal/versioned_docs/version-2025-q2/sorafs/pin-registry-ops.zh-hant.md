---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops-zh-hant
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
slug: /sorafs/pin-registry-ops-zh-hant
---

:::注意規範來源
鏡子 `docs/source/sorafs/runbooks/pin_registry_ops.md`。保持兩個版本在各個版本之間保持一致。
:::

## 概述

此操作手冊記錄瞭如何監控和分類 SoraFS pin 註冊表及其複制服務級別協議 (SLA)。這些指標源自 `iroha_torii`，並通過 `torii_sorafs_*` 命名空間下的 Prometheus 導出。 Torii 在後台以 30 秒的間隔對註冊表狀態進行採樣，因此即使沒有操作員輪詢 `/v2/sorafs/pin/*` 端點，儀表板仍保持最新狀態。導入精心策劃的儀表板 (`docs/source/grafana_sorafs_pin_registry.json`)，以獲得直接映射到以下部分的即用型 Grafana 佈局。

## 指標參考

|公制|標籤|描述 |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) |按生命週期狀態列出的鏈上清單清單。 |
| `torii_sorafs_registry_aliases_total` | — |註冊表中記錄的活動清單別名的計數。 |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) |複製訂單積壓按狀態分段。 |
| `torii_sorafs_replication_backlog_total` | — |方便儀錶鏡像 `pending` 訂單。 |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA 核算：`met` 統計截止日期內已完成的訂單，`missed` 匯總延遲完成+到期的訂單，`pending` 鏡像未完成的訂單。 |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) |聚合完成延遲（發布和完成之間的時期）。 |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) |掛單鬆弛窗口（截止日期減去發行紀元）。 |

所有儀表在每次快照拉取時都會重置，因此儀表板應以 `1m` 節奏或更快的速度進行採樣。

## Grafana 儀表板

儀表板 JSON 附帶七個面板，涵蓋操作員工作流程。如果您喜歡構建定製圖表，下面列出了查詢以供快速參考。

1. **清單生命週期** – `torii_sorafs_registry_manifests_total`（按 `status` 分組）。
2. **別名目錄趨勢** – `torii_sorafs_registry_aliases_total`。
3. **按狀態排序隊列** – `torii_sorafs_registry_orders_total`（按 `status` 分組）。
4. **積壓訂單與過期訂單** – 將 `torii_sorafs_replication_backlog_total` 和 `torii_sorafs_registry_orders_total{status="expired"}` 結合到表面飽和。
5. **SLA 成功率** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **延遲與截止時間鬆弛** – 疊加 `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` 和 `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`。當您需要絕對鬆弛地板時，使用 Grafana 轉換添加 `min_over_time` 視圖，例如：

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **錯過訂單（1 小時率）** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## 警報閾值- **SLA 成功  0**
  - 閾值：`increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - 行動：檢查治理清單以確認提供商流失情況。
- **完成第95頁>截止日期鬆弛平均值**
  - 閾值：`torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - 行動：驗證提供商是否在截止日期前做出承諾；考慮發布重新分配。

### 示例 Prometheus 規則

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

## 分類工作流程

1. **查明原因**
   - 如果 SLA 錯過了峰值，而積壓仍然很低，則應關注提供商的性能（PoR 失敗、延遲完成）。
   - 如果積壓隨著穩定的錯過而增加，請檢查入場 (`/v2/sorafs/pin/*`) 以確認等待理事會批准的清單。
2. **驗證提供商狀態**
   - 運行 `iroha app sorafs providers list` 並驗證公佈的功能是否符合複製要求。
   - 檢查 `torii_sorafs_capacity_*` 儀表以確認配置的 GiB 和 PoR 成功。
3. **重新分配複製**
   - 當積壓訂單 (`stat="avg"`) 低於 5 個週期時，通過 `sorafs_manifest_stub capacity replication-order` 發出新訂單（艙單/CAR 包裝使用 `iroha app sorafs toolkit pack`）。
   - 如果別名缺少活動清單綁定（`torii_sorafs_registry_aliases_total` 意外下降），則通知治理。
4. **記錄結果**
   - 在 SoraFS 操作日誌中記錄事件註釋以及時間戳和受影響的清單摘要。
   - 如果引入新的故障模式或儀表板，請更新此運行手冊。

## 推出計劃

在生產中啟用或收緊別名緩存策略時，請遵循此分階段過程：1. **準備配置**
   - 使用商定的 TTL 和寬限窗口更新 `iroha_config` 中的 `torii.sorafs_alias_cache`（用戶 → 實際）：`positive_ttl`、`refresh_window`、`hard_expiry`、`negative_ttl`、`revocation_ttl`、 `rotation_max_age`、`successor_grace` 和 `governance_grace`。默認值與 `docs/source/sorafs_alias_policy.md` 中的策略匹配。
   - 對於 SDK，通過其配置層（Rust / NAPI / Python 綁定中的 `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`）分發相同的值，以便客戶端執行與網關匹配。
2. **分期試運行**
   - 將配置更改部署到鏡像生產拓撲的臨時集群。
   - 運行 `cargo xtask sorafs-pin-fixtures` 以確認規範別名裝置仍在解碼和往返；任何不匹配都意味著必須首先解決上游明顯的漂移。
   - 使用涵蓋新鮮、刷新窗口、過期和硬過期情況的綜合證明來練習 `/v2/sorafs/pin/{digest}` 和 `/v2/sorafs/aliases` 端點。根據此 Runbook 驗證 HTTP 狀態代碼、標頭（`Sora-Proof-Status`、`Retry-After`、`Warning`）和 JSON 正文字段。
3. **在生產中啟用**
   - 通過標準更改窗口推出新配置。首先將其應用到 Torii，然後在節點在日誌中確認新策略後重新啟動網關/SDK 服務。
   - 將 `docs/source/grafana_sorafs_pin_registry.json` 導入 Grafana（或更新現有儀表板）並將別名緩存刷新面板固定到 NOC 工作區。
4. **部署後驗證**
   - 監視 `torii_sorafs_alias_cache_refresh_total` 和 `torii_sorafs_alias_cache_age_seconds` 30 分鐘。 `error`/`expired` 曲線中的峰值應與策略刷新窗口相關；意外的增長意味著運營商必須在繼續之前檢查別名證明和提供商的健康狀況。
   - 確認客戶端日誌顯示相同的策略決策（當證明過時或過期時，SDK 將顯示錯誤）。沒有客戶端警告表明配置錯誤。
5. **後備**
   - 如果別名發放落後並且刷新窗口頻繁跳閘，請通過在配置中增加 `refresh_window` 和 `positive_ttl` 來暫時放寬策略，然後重新部署。保持 `hard_expiry` 完整，這樣真正過時的證明仍然會被拒絕。
   - 如果遙測繼續顯示 `error` 計數升高，請通過恢復先前的 `iroha_config` 快照來恢復到先前的配置，然後打開事件以跟踪別名生成延遲。

## 相關材料

- `docs/source/sorafs/pin_registry_plan.md` — 實施路線圖和治理背景。
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — 存儲工作人員操作，補充了此註冊表手冊。
