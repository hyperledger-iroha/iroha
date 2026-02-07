---
lang: zh-hant
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-12-29T18:16:35.076964+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
# SNS 仲裁透明度報告 — <YYYY 月>

- **後綴：** `<.sora / .nexus / .dao>`
- **報告窗口：** `<ISO start>` → `<ISO end>`
- **編制者：** `<Council liaison>`
- **源工件：** `cases.ndjson` SHA256 `<hash>`，儀表板導出 `<filename>.json`

## 1. 執行摘要

- 新病例總數：`<count>`
- 本期結案案件：`<count>`
- SLA 合規性：`<ack %>` 確認/`<resolution %>` 決定
- 監護人覆蓋已發出：`<count>`
- 轉賬/退款執行：`<count>`

## 2. 案例組合

|爭議類型|新案例 |已結案件 |中值分辨率（天）|
|--------------|------------|--------------|----------------------------|
|所有權| 0 | 0 | 0 |
|違反政策 | 0 | 0 | 0 |
|濫用 | 0 | 0 | 0 |
|計費| 0 | 0 | 0 |
|其他| 0 | 0 | 0 |

## 3.SLA 性能

|優先|確認 SLA |已達成 | SLA | 決議已達成 |違規|
|----------|-----------------|----------|----------------|----------|----------|
|緊急| ≤ 2 小時 | 0% | ≤ 72 小時 | 0% | 0 |
|高| ≤ 8 小時 | 0% | ≤10天| 0% | 0 |
|標準| ≤ 24 小時 | 0% | ≤ 21 天 | 0% | 0 |
|信息 | ≤ 3d | 0% | ≤ 30 天 | 0% | 0 |

描述任何違規行為的根本原因並鏈接到補救通知單。

## 4. 案例登記

|案例編號 |選擇器|優先|狀態 |結果|筆記|
|--------|----------|----------|--------|---------|--------|
| SNS-YYYY-NNNNN | `label.suffix` |標準|關閉 |維持| `<summary>` |

提供引用匿名事實或公眾投票鏈接的一行註釋。密封件
在需要時並提及應用的修訂。

## 5. 行動與補救措施

- **凍結/釋放：** `<counts + case ids>`
- **轉賬：** `<counts + assets moved>`
- **計費調整：** `<credits/debits>`
- **政策跟進：** `<tickets or RFCs opened>`

## 6. 上訴和監護人優先

總結所有升級到監護委員會的上訴，包括時間戳和
決定（批准/拒絕）。鏈接至 `sns governance appeal` 記錄或理事會
投票。

## 7. 未完成項目

- `<Action item>` — 所有者 `<name>`，預計到達時間 `<date>`
- `<Action item>` — 所有者 `<name>`，預計到達時間 `<date>`

附上本報告中引用的 NDJSON、Grafana 導出和 CLI 日誌。