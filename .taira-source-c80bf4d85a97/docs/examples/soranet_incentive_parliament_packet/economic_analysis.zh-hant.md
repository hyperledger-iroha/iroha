---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-12-29T18:16:35.087502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 經濟分析 - 2025-10 -> 2025-11 Shadow Run

來源文物：`docs/examples/soranet_incentive_shadow_run.json`（簽名+
公鑰在同一目錄中）。每個中繼重放 60 個 epoch 的模擬
獎勵引擎固定到 `RewardConfig` 記錄在
`reward_config.json`。

## 分佈總結

- **總支出：** 5,160 XOR 超過 360 個獎勵時期。
- **公平包絡線：**基尼係數0.121；頂級繼電器份額 23.26%
  （遠低於 30% 的治理護欄）。
- **可用性：**機隊平均96.97%，所有中繼保持在94%以上。
- **帶寬：** 機群平均 91.20%，表現最低的為 87.23%
  在計劃維護期間；處罰是自動實施的。
- **合規噪音：** 觀察到 9 次警告時期和 3 次暫停
  轉化為支出減少；沒有繼電器超過 12 次警告上限。
- **操作衛生：**沒有由於丟失而跳過指標快照
  配置、債券或重複項；沒有發出任何計算器錯誤。

## 觀察結果

- 暫停對應於繼電器進入維護模式的時期。的
  支付引擎在這些時期發出零支付，同時保留
  影子運行 JSON 中的審計跟踪。
- 警告處罰使受影響的支出減少 2%；由此產生的
  由於正常運行時間/帶寬權重（650/350
  每千）。
- 帶寬方差跟踪匿名防護熱圖。表現最差的人
  (`6666...6666`) 在窗口上保留 620 XOR，高於 0.6x 下限。
- 延遲敏感警報 (`SoranetRelayLatencySpike`) 仍低於警告
  整個窗口的閾值；相關儀表板被捕獲在
  `dashboards/grafana/soranet_incentives.json`。

## 正式發布前的建議操作

1. 繼續運行每月的影子重播並更新工件集和此
   分析機隊構成是否發生變化。
2. 在路線圖中引用的 Grafana 警報套件上進行自動支付
   （`dashboards/alerts/soranet_incentives_rules.yml`）；將屏幕截圖複製到
   尋求更新時的治理會議紀要。
3. 如果基本獎勵、正常運行時間/帶寬權重或
   合規處罰變化 >=10%。