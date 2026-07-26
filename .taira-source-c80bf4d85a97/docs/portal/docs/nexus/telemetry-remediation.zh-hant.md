---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 35fe9abd10cb1454b72042b5b9dfbc35d45cc1cd91e2a4d0af4909032189df22
source_last_modified: "2025-12-29T18:16:35.147058+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-telemetry-remediation
title: Nexus telemetry remediation plan (B2)
description: Mirror of `docs/source/nexus_telemetry_remediation_plan.md`, documenting the telemetry gap matrix and operational workflow.
translator: machine-google-reviewed
---

# 概述

路線圖項目 **B2 — 遙測差距所有權** 需要發布的捆綁計劃
每個突出的 Nexus 遙測間隙到信號、警報護欄、業主、
2026 年第一季度審核窗口開始之前的截止日期和驗證工件。
本頁鏡像 `docs/source/nexus_telemetry_remediation_plan.md` 所以發布
工程、遙測操作和 SDK 所有者可以在發布之前確認覆蓋範圍
路由跟踪和 `TRACE-TELEMETRY-BRIDGE` 排練。

# 間隙矩陣

|間隙 ID |信號及警報護欄 |所有者/升級|截止日期（UTC）|證據與驗證 |
|--------|------------------------------------|--------------------|------------------------|------------------------|
| `GAP-TELEM-001` |直方圖 `torii_lane_admission_latency_seconds{lane_id,endpoint}`，帶有警報 **`SoranetLaneAdmissionLatencyDegraded`**，當 `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 持續 5 分鐘 (`dashboards/alerts/soranet_lane_rules.yml`) 時觸發。 | `@torii-sdk`（信號）+ `@telemetry-ops`（警報）；通過 Nexus 路由跟踪 on-call 升級。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` 下的警報測試加上 `TRACE-LANE-ROUTING` 排練捕獲顯示已觸發/恢復的警報和 Torii `/metrics` 抓取，存檔於 [Nexus 過渡說明](./nexus-transition-notes)。 |
| `GAP-TELEM-002` |櫃檯 `nexus_config_diff_total{knob,profile}` 帶護欄 `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` 門控部署 (`docs/source/telemetry.md`)。 | `@nexus-core`（儀表）→ `@telemetry-ops`（警報）；當計數器意外增加時，治理值班人員會收到傳呼。 | 2026-02-26 |治理試運行輸出存儲在 `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 旁邊；發布清單包括 Prometheus 查詢屏幕截圖以及證明 `StateTelemetry::record_nexus_config_diff` 發出差異的日誌摘錄。 |
| `GAP-TELEM-003` |當故障或丟失結果持續超過 30 分鐘 (`dashboards/alerts/nexus_audit_rules.yml`) 時，事件 `TelemetryEvent::AuditOutcome`（指標 `nexus.audit.outcome`），並發出警報 **`NexusAuditOutcomeFailure`**。 | `@telemetry-ops`（管道）升級到 `@sec-observability`。 | 2026-02-27 | CI 門 `scripts/telemetry/check_nexus_audit_outcome.py` 歸檔 NDJSON 有效負載，並在 TRACE 窗口缺少成功事件時失敗；警報屏幕截圖附加到路由跟踪報告。 |
| `GAP-TELEM-004` |帶護欄 `nexus_lane_configured_total != EXPECTED_LANE_COUNT` 的儀表 `nexus_lane_configured_total` 為 SRE 值班檢查表提供數據。 |當節點報告目錄大小不一致時，`@telemetry-ops`（計量/導出）升級為 `@nexus-core`。 | 2026-02-28 |調度程序遙測測試 `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` 證明發射；操作員將 Prometheus diff + `StateTelemetry::set_nexus_catalogs` 日誌摘錄附加到 TRACE 排練包中。 |

# 操作流程

1. **每週分類。 ** 業主報告 Nexus 準備電話的進展情況；
   攔截器和警報測試工件記錄在 `status.md` 中。
2. **警報試運行。 ** 每個警報規則都附帶一個
   `dashboards/alerts/tests/*.test.yml` 條目，以便 CI 執行 `promtool test
   每當護欄發生變化時都要遵守規則。
3. **審計證據。 ** 在 `TRACE-LANE-ROUTING` 期間和
   `TRACE-TELEMETRY-BRIDGE` 排練 on-call 捕獲 Prometheus 查詢
   結果、警報歷史記錄和相關腳本輸出
   （`scripts/telemetry/check_nexus_audit_outcome.py`，
   `scripts/telemetry/check_redaction_status.py` 用於相關信號）和
   將它們與路由跟踪工件一起存儲。
4. **升級。 ** 如果任何護欄在經過排練的窗口外起火，業主
   團隊提交了引用此計劃的 Nexus 事件通知單，包括
   恢復審計之前的指標快照和緩解步驟。

隨著這個矩陣的發布——並引用自 `roadmap.md` 和
`status.md` — 路線圖項目 **B2** 現在滿足“責任、截止日期、
警報、驗證”驗收標準。