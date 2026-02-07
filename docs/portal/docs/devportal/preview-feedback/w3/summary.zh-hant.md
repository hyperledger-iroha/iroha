---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-12-29T18:16:35.110857+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w3-summary
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
---

|項目 |詳情 |
| ---| ---|
|波| W3 — Beta 群組（財務 + 運營 + SDK 合作夥伴 + 生態系統倡導者）|
|邀請窗口 | 2026-02-18 → 2026-02-28 |
|文物標籤| `preview-20260218` |
|追踪器問題 | `DOCS-SORA-Preview-W3` |
|參與者 |財務-beta-01、可觀察性-ops-02、合作夥伴-sdk-03、生態系統-倡導者-04 |

## 亮點

1. **端到端證據管道。 ** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` 生成每波摘要 (`artifacts/docs_portal_preview/preview-20260218-summary.json`)、摘要 (`preview-20260218-digest.md`) 並刷新 `docs/portal/src/data/previewFeedbackSummary.json`，以便治理審核人員可以依賴單個命令。
2. **遙測+治理覆蓋範圍。 ** 所有四位審閱者均承認校驗和門控訪問，提交反饋，並按時撤銷；該摘要引用了反饋問題（`docs-preview/20260218` 集 + `DOCS-SORA-Preview-20260218`）以及在 Wave 期間收集的 Grafana 運行。
3. **門戶呈現。 ** 刷新後的門戶表現在顯示已關閉的 W3 波以及延遲和響應率指標，下面的新日誌頁面反映了不提取原始 JSON 日誌的審核員的事件時間線。

## 行動項目

|身份證 |描述 |業主|狀態 |
| ---| ---| ---| ---|
| W3-A1 |捕獲預覽摘要並附加到跟踪器。 |文檔/DevRel 領導 | ✅ 2026-02-28 完成 |
| W3-A2 |將邀請/摘要證據鏡像到門戶+路線圖/狀態中。 |文檔/DevRel 領導 | ✅ 2026-02-28 完成 |

## 退出總結 (2026-02-28)

- 2026 年 2 月 18 日發出邀請，並在幾分鐘後記錄確認；最終遙測檢查通過後，預覽訪問權限於 2026 年 2 月 28 日被撤銷。
- 在 `artifacts/docs_portal_preview/` 下捕獲的摘要 + 摘要，原始日誌由 `artifacts/docs_portal_preview/feedback_log.json` 錨定以實現可重玩性。
- 向治理跟踪器 `DOCS-SORA-Preview-20260218` 提交 `docs-preview/20260218` 下的問題跟進； CSP/Try it 註釋路由至可觀察性/財務所有者並從摘要鏈接。
- 跟踪器行更新為🈴已完成，門戶反饋表反映了已關閉的浪潮，完成了剩餘的 DOCS-SORA beta 準備任務。