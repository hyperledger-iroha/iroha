---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3d02a831f0e098972835b0b124fb8880dc825783fe28aedeaf41217620400456
source_last_modified: "2025-12-29T18:16:35.109910+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w2-summary
title: W2 community feedback & status
sidebar_label: W2 summary
description: Live digest for the community preview wave (W2).
translator: machine-google-reviewed
---

|項目 |詳情 |
| ---| ---|
|波| W2 — 社區審閱者 |
|邀請窗口 | 2025-06-15 → 2025-06-29 |
|文物標籤| `preview-2025-06-15` |
|追踪器問題 | `DOCS-SORA-Preview-W2` |
|參與者 |通訊卷 01 … 通訊卷 08 |

## 亮點

1. **治理和工具** — 社區接納政策於 2025 年 5 月 20 日一致批准；帶有動機/時區字段的更新請求模板位於 `docs/examples/docs_preview_request_template.md` 下。
2. **預檢證據** — 嘗試代理更改 `OPS-TRYIT-188` 在 2025-06-09 運行，捕獲 Grafana 儀表板，並在 `artifacts/docs_preview/W2/` 下存檔 `preview-2025-06-15` 描述符/校驗和/探針輸出。
3. **邀請波** — 2025 年 6 月 15 日邀請了八位社區審閱者，並在跟踪者邀請表中記錄了致謝信息；瀏覽前全部完成校驗和驗證。
4. **反饋** — `docs-preview/w2 #1`（工具提示措辭）和 `#2`（本地化側邊欄順序）於 2025-06-18 提交，並於 2025-06-21 解決（Docs-core-04/05）；浪潮期間沒有發生任何事件。

## 行動項目

|身份證 |描述 |業主|狀態 |
| ---| ---| ---| ---|
| W2-A1 |地址 `docs-preview/w2 #1`（工具提示措辭）。 |文檔核心-04 | ✅ 2025-06-21 完成 |
| W2-A2 |地址 `docs-preview/w2 #2`（本地化側邊欄）。 |文檔核心-05 | ✅ 2025-06-21 完成 |
| W2-A3 |存檔退出證據+更新路線圖/狀態。 |文檔/DevRel 領導 | ✅ 2025-06-29 完成 |

## 退出總結 (2025-06-29)

- 所有八位社區審閱者均確認完成並取消了預覽訪問權限；跟踪器邀請日誌中記錄的確認。
- 最終遙測快照（`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`）保持綠色；日誌以及附加到 `DOCS-SORA-Preview-W2` 的 Try it 代理記錄。
- 證據包（描述符、校驗和日誌、探測輸出、鏈接報告、Grafana 屏幕截圖、邀請確認）存檔在 `artifacts/docs_preview/W2/preview-2025-06-15/` 下。
- 跟踪器 W2 檢查點日誌通過退出進行更新，確保路線圖在 W3 規劃開始之前保留可審核的記錄。