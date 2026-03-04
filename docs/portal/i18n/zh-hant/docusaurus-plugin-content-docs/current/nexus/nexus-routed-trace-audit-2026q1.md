---
id: nexus-routed-trace-audit-2026q1
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 2026 Q1 routed-trace audit report (B1)
description: Mirror of `docs/source/nexus_routed_trace_audit_report_2026q1.md`, covering the quarterly telemetry rehearsal outcomes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::注意規範來源
此頁面鏡像 `docs/source/nexus_routed_trace_audit_report_2026q1.md`。保持兩個副本對齊，直到剩餘的翻譯落地。
:::

# 2026 年第一季度路由跟踪審計報告 (B1)

路線圖項目 **B1 — 路由跟踪審核和遙測基線** 需要
Nexus 路由跟踪計劃的季度審查。該報告記錄了
Q12026 審計窗口（一月至三月），以便治理委員會可以簽署
Q2發射前的遙測姿態排練。

## 範圍和時間表

|跟踪 ID |窗口 (UTC) |目標 |
|----------|--------------|------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 |在啟用多通道之前驗證通道准入直方圖、隊列八卦和警報流。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 |在 AND4/AND7 里程碑之前驗證 OTLP 重播、diff 機器人奇偶校驗和 SDK 遙測攝取。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 |在 RC1 削減之前確認治理批准的 `iroha_config` 增量和回滾準備情況。 |

每次排練都在類似生產的拓撲上運行，並帶有路由跟踪
啟用儀器（`nexus.audit.outcome` 遙測 + Prometheus 計數器），
已加載 Alertmanager 規則，並將證據導出到 `docs/examples/`。

## 方法論

1. **遙測收集。 ** 所有節點都發出結構化的
   `nexus.audit.outcome` 事件和隨附指標
   （`nexus_audit_outcome_total*`）。幫手
   `scripts/telemetry/check_nexus_audit_outcome.py` 追踪 JSON 日誌，
   驗證事件狀態，並將有效負載存檔在
   `docs/examples/nexus_audit_outcomes/`.【腳本/遙測/check_nexus_audit_outcome.py:1】
2. **警報驗證。 ** `dashboards/alerts/nexus_audit_rules.yml` 及其測試
   線束確保警報噪音閾值和有效負載模板保持不變
   一致。 CI 運行 `dashboards/alerts/tests/nexus_audit_rules.test.yml`
   每一次改變；在每個窗口期間手動執行相同的規則。
3. **儀表板捕獲。 ** 操作員從
   `dashboards/grafana/soranet_sn16_handshake.json`（握手健康）和
   遙測概覽儀表板將隊列運行狀況與審計結果關聯起來。
4. **審稿人註釋。 ** 治理秘書記錄了審稿人姓名縮寫，
   決定，以及 [Nexus 過渡說明](./nexus-transition-notes) 中的任何緩解票據
   和配置增量跟踪器 (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## 調查結果

|跟踪 ID |結果|證據|筆記|
|----------|---------|----------|--------|
| `TRACE-LANE-ROUTING` |通行證 |警報火災/恢復截圖（內部鏈接）+ `dashboards/alerts/tests/soranet_lane_rules.test.yml` 重播；遙測差異記錄在 [Nexus 轉換註釋](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) 中。 |隊列准入 P95 保持 612ms（目標 ≤750ms）。無需後續行動。 |
| `TRACE-TELEMETRY-BRIDGE` |通行證 |存檔的結果有效負載 `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` 加上 `status.md` 中記錄的 OTLP 重播哈希。 | SDK 修訂鹽與 Rust 基線相匹配； diff 機器人報告零增量。 |
| `TRACE-CONFIG-DELTA` |通過（緩解已關閉）|治理跟踪器條目 (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS 配置文件清單 (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + 遙測包清單 (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)。 |第二季度重新運行對批准的 TLS 配置文件進行哈希處理並確認零落後者；遙測清單記錄插槽範圍 912–936 和工作負載種子 `NEXUS-REH-2026Q2`。 |

所有跟踪在其內部至少產生一個 `nexus.audit.outcome` 事件
windows，滿足Alertmanager護欄（`NexusAuditOutcomeFailure`
本季度保持綠色）。

## 後續行動

- 使用 TLS 哈希 `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` 更新路由跟踪附錄；
  緩解措施 `NEXUS-421` 在過渡說明中關閉。
- 繼續將原始 OTLP 回放和 Torii diff 工件附加到存檔中
  為 Android AND4/AND7 評論提供同等證據。
- 確認即將進行的 `TRACE-MULTILANE-CANARY` 排練會重複使用相同的內容
  遙測助手，因此第二季度簽核受益於經過驗證的工作流程。

## 文物索引

|資產|地點 |
|--------|----------|
|遙測驗證器 | `scripts/telemetry/check_nexus_audit_outcome.py` |
|警報規則和測試 | `dashboards/alerts/nexus_audit_rules.yml`，`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|結果負載示例 | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
|配置增量跟踪器 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|路由跟踪時間表和註釋| [Nexus 過渡說明](./nexus-transition-notes) |

該報告、上述工件以及警報/遙測導出應該是
附在治理決策日誌中以結束本季度的 B1。