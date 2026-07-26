---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-12-29T18:16:35.092552+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 操作員驗證報告（T0 階段）

- 操作員姓名：______________________
- 中繼描述符 ID：______________________
- 提交日期（UTC）：___________________
- 聯繫電子郵件/矩陣：___________________

### 清單摘要

|項目 |已完成（是/否）|筆記|
|------|-----------------|--------|
|硬件和網絡驗證| | |
|應用合規塊 | | |
|錄取信封已驗證 | | |
|防護輪旋轉煙霧測試| | |
|遙測數據抓取和儀表板實時顯示 | | |
|執行限電演習 | | |
| PoW 票證在目標範圍內成功 | | |

### 指標快照

- PQ 比率 (`sorafs_orchestrator_pq_ratio`)：________
- 過去 24 小時降級次數：________
- 平均電路 RTT (p95): ________ ms
- PoW 中值解決時間：________ 毫秒

### 附件

請附上：

1. 中繼支持捆綁散列 (`sha256`)： __________________________
2. 儀表板屏幕截圖（PQ 比率、電路成功、PoW 直方圖）。
3. 簽名的鑽取包（`drills-signed.json` + 簽名者公鑰十六進制和附件）。
4. SNNet-10 指標報告 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`)。

### 操作員簽名

我保證上述信息準確無誤，並且所有必需的步驟均已完成
完成。

簽名：_________________________ 日期：_________________