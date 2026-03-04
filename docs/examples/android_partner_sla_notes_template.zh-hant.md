---
lang: zh-hant
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ca51ec624ebbb4b3760d5f2265d31047cd3b6492e21bdb10a3aa61655ccca69
source_last_modified: "2025-12-29T18:16:35.069181+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 合作夥伴 SLA 發現說明 — 模板

將此模板用於每個 AND8 SLA 發現會話。保存填寫好的副本
在 `docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` 下
並附上支持性資料（調查問卷答复、致謝、
附件）在同一目錄中。

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. 議程和背景

- 會議的目的（試點範圍、發布窗口、遙測期望）。
- 在電話會議前共享的參考文檔（支持手冊、發布日曆、
  遙測儀表板）。

## 2. 工作負載概述

|主題 |筆記|
|--------|--------|
|目標工作負載/鏈 | |
|預計交易量 | |
|關鍵業務窗口/停電期 | |
|監管制度（GDPR、MAS、FISC 等）| |
|所需語言/本地化 | |

## 3.SLA 討論

| SLA 等級 |合作夥伴期望 |相對於基線的增量？ |需要採取行動|
|----------|--------------------|----------------------|-----------------|
|關鍵修復（48 小時）| |是/否 | |
|高嚴重性（5 個工作日）| |是/否 | |
|維護（30 天）| |是/否 | |
|割接通知（60 天）| |是/否 | |
|事件通訊節奏 | |是/否 | |

記錄合作夥伴要求的任何附加 SLA 條款（例如專用
電話橋，額外的遙測出口）。

## 4. 遙測和訪問要求

- Grafana / Prometheus 訪問需求：
- 日誌/跟踪導出要求：
- 離線證據或檔案期望：

## 5. 合規性和法律說明

- 管轄區通知要求（法規+時間）。
- 事件更新所需的法律聯繫人。
- 數據駐留限制/存儲要求。

## 6. 決策和行動項目

|項目 |業主|到期 |筆記|
|------|--------|-----|--------|
| | | | |

## 7.致謝

- 合作夥伴承認基線 SLA？ （是/否）
- 後續確認方式（電子郵件/票據/簽名）：
- 關閉前將確認電子郵件或會議紀要附加到此目錄。