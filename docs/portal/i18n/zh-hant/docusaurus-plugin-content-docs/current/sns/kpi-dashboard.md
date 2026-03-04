---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS KPI dashboard
description: Live Grafana panels that aggregate registrar, freeze, and revenue metrics for SN-8a.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sora 名稱服務 KPI 儀表板

KPI 儀表板為管理者、監護人和監管者提供了一個統一的位置
在每月附件節奏之前審查採用情況、錯誤和收入信號
(SN-8a)。 Grafana 定義位於存儲庫中
`dashboards/grafana/sns_suffix_analytics.json` 與門戶鏡像相同
通過嵌入式 iframe 進行面板，因此體驗與內部 Grafana 相匹配
實例。

## 過濾器和數據源

- **後綴過濾器** – 驅動 `sns_registrar_status_total{suffix}` 查詢，以便
  `.sora`、`.nexus`和`.dao`可以獨立檢查。
- **批量發布過濾器** – 限制 `sns_bulk_release_payment_*` 指標的範圍，以便
  財務部門可以核對特定註冊商清單。
- **指標** – 取自 Torii (`sns_registrar_status_total`,
  `torii_request_duration_seconds`), 監護人 CLI (`guardian_freeze_active`),
  `sns_governance_activation_total`，以及批量入職幫助程序指標。

## 面板

1. **註冊（過去 24 小時）** – 成功註冊商活動的數量
   選定的後綴。
2. **治理激活（30天）** – 章程/附錄動議記錄
   命令行界面。
3. **註冊商吞吐量** – 每個後綴的註冊商成功操作率。
4. **註冊商錯誤模式** – 5 分鐘錯誤率標記
   `sns_registrar_status_total` 計數器。
5. **Guardian 凍結窗口** – 實時選擇器，其中 `guardian_freeze_active`
   報告未結凍結票證。
6. **按資產劃分的淨支付單位** – 按資產報告的總計
   每項資產 `sns_bulk_release_payment_net_units`。
7. **每個後綴的批量請求** – 每個後綴 ID 的清單卷。
8. **每個請求的淨單位** – 來自發布的 ARPU 樣式計算
   指標。

## 每月 KPI 審核清單

財務負責人在每個月的第一個星期二進行定期審核：

1. 打開門戶的 **分析 → SNS KPI** 頁面（或 Grafana 儀表板 `sns-kpis`）。
2. 捕獲註冊商吞吐量和收入表的 PDF/CSV 導出。
3. 比較 SLA 違規的後綴（錯誤率峰值、凍結選擇器 >72 小時、
   ARPU 增量 >10%）。
4. 日誌摘要 + 相關附件條目中的行動項目
   `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。
5. 將導出的儀表板工件附加到附件提交並將它們鏈接到
   理事會議程。

如果審核發現存在 SLA 違規行為，請為受影響的用戶提交 PagerDuty 事件
所有者（登記員值班經理、值班監護人或管家計劃負責人）以及
在附件日誌中跟踪修復情況。