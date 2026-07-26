---
lang: zh-hant
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

:::注意規範來源
此頁面反映了 `docs/source/sorafs_ops_playbook.md` 下維護的運行手冊。保持兩個副本同步，直到 Sphinx 文檔集完全遷移。
:::

## 關鍵參考文獻

- 可觀測性資產：請參閱 `dashboards/grafana/` 下的 Grafana 儀表板和 `dashboards/alerts/` 中的 Prometheus 警報規則。
- 公制目錄：`docs/source/sorafs_observability_plan.md`。
- Orchestrator 遙測表面：`docs/source/sorafs_orchestrator_plan.md`。

## 升級矩陣

|優先|觸發器示例 |主要隨叫隨到|備份|筆記|
|----------|--------------------------------|-----------------|--------|--------|
| P1 |全球網關中斷，PoR 故障率 > 5%（15 分鐘），複製積壓每 10 分鐘翻一番 |存儲 SRE |可觀察性 TL |如果影響超過 30 分鐘，請與治理委員會聯繫。 |
| P2 |區域網關延遲 SLO 違規、編排器重試激增而不影響 SLA |可觀察性 TL |存儲 SRE |繼續推出，但控制新清單。 |
| P3 |非關鍵警報（明顯陳舊，容量 80–90%）|攝入量分類|行動公會 |下一個工作日內的地址。 |

## 網關中斷/可用性下降

**檢測**

- 警報：`SoraFSGatewayAvailabilityDrop`、`SoraFSGatewayLatencySlo`。
- 儀表板：`dashboards/grafana/sorafs_gateway_overview.json`。

**立即行動**

1. 通過請求率面板確認範圍（單一提供商與車隊）。
2. 通過在操作配置 (`docs/source/sorafs_gateway_self_cert.md`) 中切換 `sorafs_gateway_route_weights`，將 Torii 路由切換到健康的提供商（如果是多提供商）。
3. 如果所有提供商都受到影響，請為 CLI/SDK 客戶端啟用“直接獲取”回退 (`docs/source/sorafs_node_client_protocol.md`)。

**分流**

- 根據 `sorafs_gateway_stream_token_limit` 檢查流令牌利用率。
- 檢查網關日誌是否存在 TLS 或准入錯誤。
- 運行 `scripts/telemetry/run_schema_diff.sh` 以確保網關導出的架構與預期版本匹配。

**修復選項**

- 僅重新啟動受影響的網關進程；避免回收整個集群，除非多個提供者出現故障。
- 如果確認飽和，則暫時將流令牌限制增加 10–15%。
- 穩定後重新運行自我認證 (`scripts/sorafs_gateway_self_cert.sh`)。

**事件發生後**

- 使用 `docs/source/sorafs/postmortem_template.md` 提交 P1 事後分析。
- 如果補救措施依賴於手動干預，則安排後續混亂演習。

## 證明失敗峰值 (PoR / PoTR)

**檢測**

- 警報：`SoraFSProofFailureSpike`、`SoraFSPoTRDeadlineMiss`。
- 儀表板：`dashboards/grafana/sorafs_proof_integrity.json`。
- 遙測：`torii_sorafs_proof_stream_events_total` 和 `sorafs.fetch.error` 事件以及 `provider_reason=corrupt_proof`。

**立即行動**

1. 通過標記清單註冊表 (`docs/source/sorafs/manifest_pipeline.md`) 來凍結新的清單准入。
2. 通知治理部門暫停對受影響提供商的激勵措施。

**分流**

- 檢查 PoR 質詢隊列深度與 `sorafs_node_replication_backlog_total`。
- 驗證最近部署的證明驗證管道 (`crates/sorafs_node/src/potr.rs`)。
- 將提供商固件版本與運營商註冊表進行比較。

**修復選項**

- 使用 `sorafs_cli proof stream` 和最新清單觸發 PoR 重放。
- 如果證明始終失敗，請通過更新治理註冊表並強制刷新 Orchestrator 記分板來從活動集中刪除提供者。

**事件發生後**

- 在下一次生產部署之前運行 PoR 混沌演練場景。
- 在事後分析模板中汲取經驗教訓並更新提供商資格清單。

## 複製滯後/積壓增長

**檢測**

- 警報：`SoraFSReplicationBacklogGrowing`、`SoraFSCapacityPressure`。進口
  `dashboards/alerts/sorafs_capacity_rules.yml` 並運行
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  在升級之前，Alertmanager 會反映記錄的閾值。
- 儀表板：`dashboards/grafana/sorafs_capacity_health.json`。
- 指標：`sorafs_node_replication_backlog_total`、`sorafs_node_manifest_refresh_age_seconds`。

**立即行動**

1. 驗證積壓範圍（單個提供商或隊列）並暫停非必要的複制任務。
2. 如果積壓訂單被隔離，則通過複製調度程序臨時將新訂單重新分配給備用提供商。

**分流**

- 檢查 Orchestrator 遙測是否存在可能級聯積壓的重試突發。
- 確認存儲目標有足夠的空間 (`sorafs_node_capacity_utilisation_percent`)。
- 查看最近的配置更改（塊配置文件更新、證明節奏）。

**修復選項**

- 使用 `--rebalance` 選項運行 `sorafs_cli` 以重新分發內容。
- 為受影響的提供商水平擴展複製工作人員。
- 觸發清單刷新以重新對齊 TTL 窗口。

**事件發生後**

- 安排一次容量演習，重點關注提供商飽和故障。
- 更新 `docs/source/sorafs_node_client_protocol.md` 中的複制 SLA 文檔。

## 修復積壓和 SLA 違規

**檢測**

- 警報：
  - `SoraFSRepairBacklogHigh`（隊列深度 > 50 或最舊的排隊年齡 > 4h，持續 10m）。
  - `SoraFSRepairEscalations`（> 3 次升級/小時）。
  - `SoraFSRepairLeaseExpirySpike`（> 5 個租約到期/小時）。
  - `SoraFSRetentionBlockedEvictions`（在過去 15m 中由於主動修復而導致保留被阻止）。
- 儀表板：`dashboards/grafana/sorafs_capacity_health.json`。

**立即行動**

1. 識別受影響的提供商（隊列深度峰值）並暫停它們的新 pin/複製訂單。
2. 驗證修復工作人員的活躍度並在安全的情況下增加工作人員的並發性。

**分流**

- 將 `torii_sorafs_repair_backlog_oldest_age_seconds` 與 4 小時 SLA 窗口進行比較。
- 檢查 `torii_sorafs_repair_lease_expired_total{outcome=...}` 是否有崩潰/時鐘偏差模式。
- 查看重複清單/提供商對的升級票證並驗證證據包。

**修復選項**

- 重新指派或重新啟動停滯的維修人員；通過正常索賠流程清除孤立租賃。
- 修復排水管時限制新銷，以防止額外的 SLA 壓力。
- 如果升級持續存在，則升級至治理並附上修復審核工件。

## 保留/GC 檢查（只讀）

**檢測**

- 警報：`SoraFSCapacityPressure` 或持續 `torii_sorafs_storage_bytes_used` > 90%。
- 儀表板：`dashboards/grafana/sorafs_capacity_health.json`。

**立即行動**

1. 運行本地保留快照：
   ```bash
   iroha app sorafs gc inspect --data-dir /var/lib/sorafs
   ```
2. 捕獲僅過期視圖以進行分類：
   ```bash
   iroha app sorafs gc dry-run --data-dir /var/lib/sorafs
   ```
3. 將 JSON 輸出附加到事件單以進行審計。

**分流**

- 確認艙單報告 `retention_epoch=0`（無到期日）與有截止日期的艙單報告。
- 在GC JSON輸出中使用`retention_sources`來查看哪個約束設置有效
  保留（`deal_end`、`governance_cap`、`pin_policy` 或 `unbounded`）。交易和治理上限
  通過清單元數據鍵 `sorafs.retention.deal_end_epoch` 提供
  `sorafs.retention.governance_cap_epoch`。
- 如果 `dry-run` 報告清單已過期但容量仍處於固定狀態，請驗證否
  主動修復或保留策略會覆蓋塊驅逐。
  容量觸發的掃描按最近最少使用的順序逐出過期清單
  `manifest_id` 決勝局。

**修復選項**

- GC CLI 是只讀的。不要在生產中手動刪除清單或塊。
- 升級為保留政策調整或容量擴展的治理
  當過期數據累積而沒有自動驅逐時。

## 混沌練習節奏

- **季度**：組合網關中斷 + Orchestrator 重試風暴模擬。
- **一年兩次**：跨兩個提供商進行 PoR/PoTR 故障注入並進行恢復。
- **每月抽查**：使用暫存清單的複制滯後場景。
- 通過以下方式在共享運行手冊日誌 (`ops/drill-log.md`) 中跟踪演練：

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

- 在提交之前驗證日誌：

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- 對於即將進行的演習使用 `--status scheduled`，對於已完成的運行使用 `pass`/`fail`，當行動項保持開放狀態時使用 `follow-up`。
- 使用 `--log` 覆蓋目的地以進行試運行或自動驗證；如果沒有它，腳本將繼續更新 `ops/drill-log.md`。

## 事後分析模板

對每個 P1/P2 事件和混沌演習回顧使用 `docs/source/sorafs/postmortem_template.md`。該模板涵蓋時間表、影響量化、影響因素、糾正措施和後續驗證任務。