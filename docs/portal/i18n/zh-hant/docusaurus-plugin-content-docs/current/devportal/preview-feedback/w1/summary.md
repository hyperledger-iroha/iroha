---
id: preview-feedback-w1-summary
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W1 partner feedback & exit summary
sidebar_label: W1 summary
description: Findings, actions, and exit evidence for the partner/Torii integrator preview wave.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

|項目 |詳情 |
| ---| ---|
|波| W1 — 合作夥伴和 Torii 集成商 |
|邀請窗口 | 2025-04-12 → 2025-04-26 |
|文物標籤| `preview-2025-04-12` |
|追踪器問題 | `DOCS-SORA-Preview-W1` |
|參與者 | sorafs-op-01…03、torii-int-01…02、sdk-partner-01…02、gateway-ops-01 |

## 亮點

1. **校驗和工作流程** — 所有審閱者都通過 `scripts/preview_verify.sh` 驗證描述符/存檔；日誌與邀請確認一起存儲。
2. **遙測** — `docs.preview.integrity`、`TryItProxyErrors` 和 `DocsPortal/GatewayRefusals` 儀表板在整個波次中保持綠色；沒有觸發任何事件或警報頁面。
3. **文檔反饋 (`docs-preview/w1`)** — 提交了兩個小問題：
   - `docs-preview/w1 #1`：澄清“嘗試”部分中的導航措辭（已解決）。
   - `docs-preview/w1 #2`：更新嘗試屏幕截圖（已解決）。
4. **Runbook 奇偶校驗** — SoraFS 運營商確認 `orchestrator-ops` 和 `multi-source-rollout` 之間的新交叉鏈接解決了他們的 W0 問題。

## 行動項目

|身份證 |描述 |業主|狀態 |
| ---| ---| ---| ---|
| W1-A1 |根據 `docs-preview/w1 #1` 更新嘗試導航措辭。 |文檔核心-02 | ✅ 已完成（2025-04-18）。 |
| W1-A2|刷新試試 `docs-preview/w1 #2` 的屏幕截圖。 |文檔核心-03 | ✅ 已完成（2025-04-19）。 |
| W1-A3 |在路線圖/狀態中總結合作夥伴的發現+遙測證據。 |文檔/DevRel 領導 | ✅ 已完成（參見 tracker + status.md）。 |

## 退出總結 (2025-04-26)

- 所有八位審閱者均在最後辦公時間內確認完成，清除了當地文物，並撤銷了他們的訪問權限。
- 退出時遙測保持綠色；最終快照附於 `DOCS-SORA-Preview-W1`。
- 使用退出確認更新邀請日誌；跟踪器將 W1 翻轉到 🈴 並添加檢查點條目。
- 證據包（描述符、校驗和日誌、探測輸出、Try it 代理記錄、遙測屏幕截圖、反饋摘要）存檔於 `artifacts/docs_preview/W1/` 下。

## 後續步驟

- 準備 W2 社區接納計劃（治理批准 + 請求模板調整）。
- 刷新 W2 波的預覽工件標籤，並在日期確定後重新運行預檢腳本。
- 將適用的 W1 調查結果移植到路線圖/狀態中，以便社區 Wave 擁有最新的指導。