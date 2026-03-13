---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-29T18:16:35.107308+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-log
title: W1 feedback & telemetry log
sidebar_label: W1 feedback log
description: Aggregate roster, telemetry checkpoints, and reviewer notes for the first partner preview wave.
translator: machine-google-reviewed
---

此日誌保留邀請名冊、遙測檢查點和審閱者反饋
**W1 合作夥伴預覽**，伴隨接受任務
[`preview-feedback/w1/plan.md`](./plan.md) 和波形跟踪器條目
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。每當收到邀請時更新它
發送、記錄遙測快照或對反饋項目進行分類，以便治理審核者可以重播
無需追尋外部票據即可獲得證據。

## 隊列名單

|合作夥伴 ID |索取門票 |收到 NDA |邀請已發送 (UTC) |確認/首次登錄 (UTC) |狀態 |筆記|
| ---| ---| ---| ---| ---| ---| ---|
|合作夥伴-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ 2025-04-26 完成 |索拉夫-op-01；專注於協調器文檔平價證據。 |
|合作夥伴-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ 2025-04-26 完成 |索拉夫-op-02；已驗證 Norito/遙測交叉鏈接。 |
|合作夥伴-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ 2025-04-26 完成 |索拉夫-op-03；進行多源故障轉移演練。 |
|合作夥伴-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ 2025-04-26 完成 |鳥居-int-01； Torii `/v2/pipeline` + 嘗試食譜評論。 |
|合作夥伴-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ 2025-04-26 完成 |鳥居-int-02；配對嘗試屏幕截圖更新 (docs-preview/w1 #2)。 |
|合作夥伴-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ 2025-04-26 完成 | SDK-合作夥伴-01； JS/Swift 食譜反饋 + ISO 橋完整性檢查。 |
|合作夥伴-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ 2025-04-26 完成 | SDK-合作夥伴-02；合規性於 2025 年 4 月 11 日通過，重點關注連接/遙測註釋。 |
|合作夥伴-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ 2025-04-26 完成 |網關-ops-01；審核網關操作指南 + 匿名嘗試代理流程。 |

發出出站電子郵件後，立即填充 **發送邀請** 和 **確認** 時間戳。
將時間錨定到 W1 計劃中定義的 UTC 時間表。

## 遙測檢查點

|時間戳 (UTC) |儀表板/探頭|業主|結果 |文物|
| ---| ---| ---| ---| ---|
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |文檔/開發版本 + 操作 | ✅ 全綠色 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` 成績單 |行動| ✅ 上演 | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`、`probe:portal` |文檔/開發版本 + 操作 | ✅ 預邀請快照，無回歸 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |上面的儀表板 + 嘗試代理延遲差異 |文檔/DevRel 領導 | ✅ 中點檢查已通過（0 個警報；嘗試一下延遲 p95=410ms）| `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |上面的儀表板 + 出口探針 |文檔/DevRel + 治理聯絡 | ✅ 退出快照，零未完成警報 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

每日辦公時間樣本 (2025-04-13 → 2025-04-25) 捆綁為 NDJSON + PNG 導出，位於
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` 與文件名
`docs-preview-integrity-<date>.json` 和相應的屏幕截圖。

## 反饋和問題日誌

使用此表總結審稿人提交的發現。將每個條目鏈接到 GitHub/討論
票證加上通過捕獲的結構化表格
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|參考|嚴重性 |業主|狀態 |筆記|
| ---| ---| ---| ---| ---|
| `docs-preview/w1 #1` |低|文檔核心-02 | ✅ 2025-04-18 解決 |澄清“嘗試一下”導航措辭 + 側邊欄錨點（`docs/source/sorafs/tryit.md` 使用新標籤更新）。 |
| `docs-preview/w1 #2` |低|文檔核心-03 | ✅ 2025-04-19 解決 |根據審閱者請求刷新嘗試屏幕截圖 + 標題；人工製品 `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| — |信息 |文檔/DevRel 領導 | 🟢 已關閉 |其餘評論僅限問答；在 `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` 下每個合作夥伴的反饋表中獲取。 |

## 知識檢查和調查跟踪

1.記錄每位審稿人的測驗分數（目標≥90%）；將導出的 CSV 附加到
   邀請文物。
2. 收集通過反饋表模板捕獲的定性調查答案並進行鏡像
   在 `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` 下。
3. 為得分低於閾值的任何人安排補救電話，並將其記錄在此文件中。

所有八位審稿人在知識檢查中得分均≥94%（CSV：
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`）。沒有接到任何補救電話
被要求；調查每個合作夥伴的出口情況
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## 文物清單

- 預覽描述符/校驗和包：`artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- 探測 + 鏈路檢查摘要：`artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- 嘗試代理更改日誌：`artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- 遙測導出：`artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- 每日辦公時間遙測捆綁包：`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- 反饋 + 調查導出：將審閱者特定的文件夾放置在
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- 知識檢查 CSV 和摘要：`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

保持庫存與跟踪器問題同步。將工件複製到時附加哈希值
治理票證，以便審計員無需 shell 訪問即可驗證文件。