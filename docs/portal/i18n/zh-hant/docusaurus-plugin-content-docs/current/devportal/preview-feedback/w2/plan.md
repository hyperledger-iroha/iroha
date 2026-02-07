---
id: preview-feedback-w2-plan
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W2 community intake plan
sidebar_label: W2 plan
description: Intake, approvals, and evidence checklist for the community preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

|項目 |詳情 |
| ---| ---|
|波| W2 — 社區審閱者 |
|目標窗口| 2025 年第 3 季度第 1 週（暫定）|
|人工製品標籤（計劃中）| `preview-2025-06-15` |
|追踪器問題 | `DOCS-SORA-Preview-W2` |

## 目標

1. 定義社區接納標準和審查工作流程。
2. 獲得治理部門對擬議名冊和可接受用途附錄的批准。
3. 在新窗口中刷新經過校驗和驗證的預覽工件和遙測包。
4. 在邀請發送之前暫存 Try it 代理 + 儀表板。

## 任務分解

|身份證 |任務|業主|到期 |狀態 |筆記|
| ---| ---| ---| ---| ---| ---|
| W2-P1 |起草社區接納標準（資格、最大名額、CoC 要求）並分發給治理 |文檔/DevRel 領導 | 2025-05-15 | ✅ 已完成 |接收政策合併到 `DOCS-SORA-Preview-W2` 中，並在 2025 年 5 月 20 日理事會會議上批准。 |
| W2-P2 |使用社區特定問題（動機、可用性、本地化需求）更新請求模板 |文檔核心-01 | 2025-05-18 | ✅ 已完成 | `docs/examples/docs_preview_request_template.md` 現在包括社區部分，在接收表格中引用。 |
| W2-P3 |確保接收計劃得到治理批准（會議投票 + 記錄會議記錄）|治理聯絡| 2025-05-22 | ✅ 已完成 |投票於2025年5月20日一致通過；分鐘 + 點名鏈接在 `DOCS-SORA-Preview-W2` 中。 |
| W2-P4 |計劃嘗試 W2 窗口的代理暫存 + 遙測捕獲 (`preview-2025-06-15`) |文檔/開發版本 + 操作 | 2025-06-05 | ✅ 已完成 |變更票 `OPS-TRYIT-188` 批准並執行 2025-06-09 02:00–04:00UTC； Grafana 屏幕截圖與票證一起存檔。 |
| W2-P5 |構建/驗證新的預覽工件標籤 (`preview-2025-06-15`) 和存檔描述符/校驗和/探測日誌 |門戶網站 TL | 2025-06-07 | ✅ 已完成 | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 運行於 2025-06-10；輸出存儲在 `artifacts/docs_preview/W2/preview-2025-06-15/` 下。 |
| W2-P6 |組建社區邀請名冊（≤25 名審稿人，分批），並提供經政府批准的聯繫信息 |社區經理 | 2025-06-10 | ✅ 已完成 |第一批 8 名社區評審員獲得批准；跟踪器中記錄的請求 ID `DOCS-SORA-Preview-REQ-C01…C08`。 |

## 證據清單

- [x] 治理批准記錄（會議記錄 + 投票鏈接）附於 `DOCS-SORA-Preview-W2`。
- [x] 更新了在 `docs/examples/` 下提交的請求模板。
- [x] `preview-2025-06-15` 描述符、校驗和日誌、探測輸出、鏈接報告以及存儲在 `artifacts/docs_preview/W2/` 下的 Try it 代理記錄。
- [x] Grafana 為 W2 預檢窗口捕獲的屏幕截圖（`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`）。
- [x] 邀請名冊表，其中包含審閱者 ID、請求票證和在發送前填充的批准時間戳（請參閱跟踪器 W2 部分）。

保持該計劃的更新；跟踪器會引用它，以便 DOCS-SORA 路線圖可以準確地看到 W2 邀請發出之前剩餘的內容。