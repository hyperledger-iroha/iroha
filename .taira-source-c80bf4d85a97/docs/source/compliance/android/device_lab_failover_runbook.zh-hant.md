---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2025-12-29T18:16:35.923579+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 設備實驗室故障轉移演練操作手冊 (AND6/AND7)

該操作手冊記錄了程序、證據要求和聯繫矩陣
在執行中引用的**設備實驗室應急計劃**時使用
`roadmap.md`（§“監管人工製品批准和實驗室應急情況”）。它補充了
預訂工作流程 (`device_lab_reservation.md`) 和事件日誌
(`device_lab_contingency.md`) 因此合規審查員、法律顧問和 SRE
對於如何驗證故障轉移準備情況有單一的事實來源。

## 目的和節奏

- 演示Android StrongBox +通用設備池可以故障轉移
  到回退 Pixel 通道、共享池、Firebase 測試實驗室突發隊列，以及
  外部 StrongBox 固定器，不會丟失 AND6/AND7 SLA。
- 製作法律部門可以附加到 ETSI/FISC 提交材料中的證據包
  在二月份的合規審查之前。
- 每季度至少運行一次，以及實驗室硬件名冊發生變化時的任何時間
  （新設備、報廢或維護時間超過 24 小時）。

|鑽頭 ID |日期 |場景 |證據包|狀態 |
|----------|------|----------|------------------|--------|
| DR-2026-02-Q1 | 2026-02-20 |模擬 Pixel8Pro 車道中斷 + AND7 遙測排練證明積壓 | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ 已完成 — 捆綁哈希記錄在 `docs/source/compliance/android/evidence_log.csv` 中。 |
| DR-2026-05-Q2 | 2026-05-22（預定）| StrongBox 維護重疊 + Nexus 排練 | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *（待定）* — `_android-device-lab` 機票 **AND6-DR-202605** 保留預訂；捆綁將在演習後填充。 | 🗓 已安排 — 按照 AND6 節奏將日曆塊添加到“Android 設備實驗室 - 預訂”。 |

## 程序

### 1. 演練前準備

1. 確認 `docs/source/sdk/android/android_strongbox_capture_status.md` 中的基線容量。
2. 通過以下方式導出目標 ISO 週的預訂日曆
   `python3 scripts/android_device_lab_export.py --week <ISO week>`。
3. 歸檔 `_android-device-lab` 票據
   `AND6-DR-<YYYYMM>` 包含範圍（“故障轉移演練”）、計劃插槽和受影響的
   工作負載（證明、CI 煙霧、遙測混亂）。
4. 使用以下內容更新 `device_lab_contingency.md` 中的應急日誌模板
   鑽取日期的佔位符行。

### 2. 模擬故障情況

1. 禁用或取消池化實驗室內的主通道 (`pixel8pro-strongbox-a`)
   調度程序並將預留條目標記為“drill”。
2. 在 PagerDuty 中觸發模擬中斷警報（`AND6-device-lab` 服務）並
   捕獲證據包的通知導出。
3. 註釋通常消耗車道的 Buildkite 作業
   （`android-strongbox-attestation`、`android-ci-e2e`）以及鑽頭 ID。

### 3. 故障轉移執行1. 將後備 Pixel7 通道提升到主要 CI 目標並安排
   針對它的計劃工作負載。
2. 通過 `firebase-burst` 通道觸發 Firebase 測試實驗室突發套件
   零售錢包煙霧測試，而 StrongBox 覆蓋範圍轉移到共享
   車道。捕獲票證中的 CLI 調用（或控制台導出）以供審核
   平價。
3. 使用外部 StrongBox 實驗室固定器進行短暫的認證掃描；
   如下所述記錄聯繫確認。
4. 將所有 Buildkite 運行 ID、Firebase 作業 URL 和保留者記錄記錄在
   `_android-device-lab` 票據和證據包清單。

### 4. 驗證和回滾

1. 將證明/CI 運行時間與基線進行比較；標記增量 >10%
   硬件實驗室負責人。
2. 恢復主通道並更新容量快照和就緒情況
   驗證通過後的矩陣。
3. 將最後一行附加到 `device_lab_contingency.md`，其中包含觸發器、操作、
   和後續行動。
4. 將 `docs/source/compliance/android/evidence_log.csv` 更新為：
   捆綁包路徑、SHA-256 清單、Buildkite 運行 ID、PagerDuty 導出哈希以及
   審稿人簽字。

## 證據包佈局

|文件|描述 |
|------|-------------|
| `README.md` |摘要（演習 ID、範圍、所有者、時間表）。 |
| `bundle-manifest.json` |捆綁包中每個文件的 SHA-256 映射。 |
| `calendar-export.{ics,json}` |來自導出腳本的 ISO 週預留日曆。 |
| `pagerduty/incident_<id>.json` | PagerDuty 事件導出顯示警報 + 確認時間線。 |
| `buildkite/<job>.txt` | Buildkite 受影響作業的運行 URL 和日誌。 |
| `firebase/burst_report.json` | Firebase 測試實驗室突發執行摘要。 |
| `retainer/acknowledgement.eml` |來自外部 StrongBox 實驗室的確認。 |
| `photos/` |如果重新連接硬件，則可以提供實驗室拓撲的可選照片/屏幕截圖。 |

將捆綁包存儲在
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` 並記錄
證據日誌中的清單校驗和加上 AND6 合規性檢查表。

## 聯繫和升級矩陣

|角色 |主要聯繫人 |頻道 |筆記|
|------|-----------------|------------|--------|
|硬件實驗室負責人 |普里亞·拉馬納坦 | `@android-lab` 鬆弛 · +81-3-5550-1234 |擁有現場行動和日曆更新。 |
|設備實驗室運營|馬特奧·克魯茲 | `_android-device-lab` 隊列 |協調預訂門票 + 捆綁上傳。 |
|發布工程|阿列克謝·莫羅佐夫 |發布英語 Slack · `release-eng@iroha.org` |驗證 Buildkite 證據 + 發布哈希值。 |
|外部保險箱實驗室 |櫻花儀器 NOC | `noc@sakura.example` · +81-3-5550-9876 |保持器接觸； 6 小時內確認可用性。 |
| Firebase 突發協調員 |泰莎·賴特 | `@android-ci` 鬆弛 |當需要回退時觸發 Firebase 測試實驗室自動化。 |

如果演練發現阻塞問題，請按以下順序升級：
1. 硬件實驗室負責人
2. Android 基礎 TL
3. 項目負責人/發布工程
4. 合規主管+法律顧問（如果演習揭示監管風險）

## 報告和跟進- 每當引用時，將此操作手冊與預訂程序鏈接起來
  `roadmap.md`、`status.md` 和治理數據包中的故障轉移準備情況。
- 將季度演習回顧連同證據包通過電子郵件發送給合規+法務部
  哈希表並附加 `_android-device-lab` 票據導出。
- 鏡像關鍵指標（故障轉移時間、恢復的工作負載、未完成的操作）
  在 `status.md` 和 AND7 熱門列表跟踪器內，以便審閱者可以跟踪
  依賴於具體的排練。