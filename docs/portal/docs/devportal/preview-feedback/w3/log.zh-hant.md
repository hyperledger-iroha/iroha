---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3863fc4d8f6b1358d34e413af6a6be984bcd6b9b9df8e2d8acf4fac32abae52e
source_last_modified: "2025-12-29T18:16:35.110447+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w3-log
title: W3 beta invite log
sidebar_label: W3 log
description: Timeline for the 2026-02-18 preview invite wave.
translator: machine-google-reviewed
---

下面記錄的事件鏡像在 `artifacts/docs_portal_preview/feedback_log.json` 中
並總結在 `preview-20260218-summary.json` / `preview-20260218-digest.md` 中。

|時間戳 (UTC) |活動 |收件人 |筆記|
| ---| ---| ---| ---|
| 2026-02-18 14:00 |發送邀請 |財務-beta-01 |金融試點隊列|
| 2026-02-18 14:08 |承認|財務-beta-01 |  |
| 2026-02-21 10:22 |反饋已提交 |財務-beta-01 |文檔預覽/20260218#1 |
| 2026-02-28 17:00 |訪問權限被撤銷 |財務-beta-01 |  |
| 2026-02-18 14:05 |發送邀請 |可觀察性-ops-02 |可觀察性準備 |
| 2026-02-18 14:20 |承認|可觀察性-ops-02 |  |
| 2026-02-23 09:45 |反饋已提交 |可觀察性-ops-02 |文檔預覽/20260218#2 |
| 2026-02-23 11:15 |問題已打開 |可觀察性-ops-02 | DOCS-SORA-預覽-20260218 |
| 2026-02-28 17:05 |訪問權限被撤銷 |可觀察性-ops-02 |  |
| 2026-02-18 14:10 |發送邀請 |合作夥伴-sdk-03 | SDK合作夥伴浪潮|
| 2026-02-19 08:30 |承認|合作夥伴-sdk-03 |  |
| 2026-02-24 16:10 |反饋已提交 |合作夥伴-sdk-03 |文檔預覽/20260218#3 |
| 2026-02-28 17:10 |訪問權限被撤銷 |合作夥伴-sdk-03 |  |
| 2026-02-18 14:15 |發送邀請 |生態系統倡導者-04 |生態系統倡導者|
| 2026-02-18 14:50 |承認|生態系統倡導者-04 |  |
| 2026-02-26 12:35 |反饋已提交 |生態系統倡導者-04 |文檔預覽/20260218#4 |
| 2026-02-28 17:15 |訪問權限被撤銷 |生態系統倡導者-04 |  |

使用 `npm run --prefix docs/portal preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01`
更新此日誌時重新生成摘要和門戶數據。