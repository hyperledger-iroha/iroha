---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI 審核校準報告 - 2026 年 2 月

本報告打包了 **MINFO-1** 的首次校準工件。的
數據集、清單和記分板於 2026 年 2 月 5 日製作，並由
2026年2月10日召開部委理事會，並紮根於治理DAG的高度
`912044`。

## 數據集清單

- **數據集參考：** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **子彈：** `ai-moderation-calibration-202602`
- **條目：** 清單 480、塊 12,800、元數據 920、音頻 160
- **標籤組合：**安全 68%，可疑 19%，升級 13%
- **文物摘要：** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **分佈：** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

完整清單位於 `docs/examples/ai_moderation_calibration_manifest_202602.json` 中
並包含治理簽名以及發佈時捕獲的運行者哈希值
時間。

## 記分板摘要

校準使用 opset 17 和確定性種子管道運行。的
完整記分板 JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
記錄哈希值和遙測摘要；下表突出顯示了最
重要指標。

|模特（家庭）|荊棘|歐洲經委會 |奧羅克 |精準@檢疫|召回@升級 |
| -------------- | -----| ---| -----| -------------------- | ---------------- |
| ViT-H/14 安全（視覺）| 0.141 | 0.141 0.031 | 0.031 0.987 | 0.987 0.964 | 0.964 0.912 | 0.912
| LLaVA-1.6 34B 安全（多式聯運）| 0.118 | 0.118 0.028 | 0.028 0.978 | 0.978 0.942 | 0.942 0.904 | 0.904
|知覺整體（知覺） | 0.162 | 0.162 0.047 | 0.047 0.953 | 0.953 0.883 | 0.883 0.861 | 0.861

組合指標：`Brier = 0.126`、`ECE = 0.034`、`AUROC = 0.982`。判決結果
整個校準窗口的分佈通過率為 91.2%，隔離率為 6.8%，
升級2.0%，符合清單中記錄的政策預期
總結。假陽性積壓保持為零，漂移分數 (7.1%)
遠低於 20% 的警報閾值。

## 閾值和簽核

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- 治理動議：`MINFO-2026-02-07`
- 由 `ministry-council-seat-03` 在 `2026-02-10T11:33:12Z` 簽名

CI 將簽名包存儲在 `artifacts/ministry/ai_moderation/2026-02/` 中
與審核運行程序二進製文件一起。清單摘要和記分板
在審核和上訴期間必須引用上述哈希值。

## 儀表板和警報

審核 SRE 應導入 Grafana 儀表板：
`dashboards/grafana/ministry_moderation_overview.json` 及其匹配
`dashboards/alerts/ministry_moderation_rules.yml` 中的 Prometheus 警報規則
（測試覆蓋範圍位於 `dashboards/alerts/tests/ministry_moderation_rules.test.yml` 下）。
這些工件會針對攝取停滯、漂移尖峰和隔離發出警報
隊列增長，滿足中提出的監控要求
[AI 審核運行器規範](../../ministry/ai-moderation-runner.md)。