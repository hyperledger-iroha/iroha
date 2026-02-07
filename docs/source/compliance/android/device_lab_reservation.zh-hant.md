---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2025-12-29T18:16:35.924530+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 設備實驗室預約流程 (AND6/AND7)

本手冊介紹了 Android 團隊如何預訂、確認和審核設備
里程碑 **AND6**（CI 和合規性強化）和 **AND7** 的實驗室時間
（可觀察性準備）。它補充了應急登錄
`docs/source/compliance/android/device_lab_contingency.md` 通過確保容量
首先就避免了不足。

## 1. 目標和範圍

- 將 StrongBox + 通用設備池保持在路線圖規定的 80% 以上
  整個凍結窗口的容量目標。
- 提供確定性日曆，以便清除 CI、證明和混亂
  排練從來不會爭奪相同的硬件。
- 捕獲可審計的跟踪（請求、批准、運行後註釋）
  AND6 合規性檢查表和證據日誌。

此過程涵蓋專用像素通道、共享後備池和
路線圖中引用的外部 StrongBox 實驗室固定器。臨時模擬器
使用超出範圍。

## 2. 預約窗口

|泳池/泳道 |硬件|默認槽長度 |預訂提前期 |業主|
|------------|----------|--------------------------------|--------------------|--------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4小時| 3 個工作日 |硬件實驗室負責人 |
| `pixel8a-ci-b` | Pixel8a（CI 通用）| 2小時| 2 個工作日 | Android 基礎 TL |
| `pixel7-fallback` | Pixel7共享池| 2小時| 1 個工作日 |發布工程|
| `firebase-burst` | Firebase 測試實驗室煙霧隊列 | 1小時| 1 個工作日 | Android 基礎 TL |
| `strongbox-external` |外部 StrongBox 實驗室固定器 | 8小時| 7 個日曆日 |項目負責人 |

時段以 UTC 時間預訂；重疊預訂需要明確批准
來自硬件實驗室負責人。

## 3. 請求工作流程

1. **準備上下文**
   - 更新 `docs/source/sdk/android/android_strongbox_device_matrix.md`
     您計劃鍛煉的設備和準備標籤
     （`attestation`、`ci`、`chaos`、`partner`）。
   - 收集最新的容量快照
     `docs/source/sdk/android/android_strongbox_capture_status.md`。
2. **提交請求**
   - 使用以下模板在 `_android-device-lab` 隊列中提交票證
     `docs/examples/android_device_lab_request.md`（所有者、日期、工作負載、
     後備要求）。
   - 附加任何監管依賴項（例如 AND6 證明掃描、AND7
     遙測演習）並鏈接到相關路線圖條目。
3. **批准**
   - 硬件實驗室負責人在一個工作日內進行審查，確認在
     共享日曆 (`Android Device Lab – Reservations`)，並更新
     `device_lab_capacity_pct` 列
     `docs/source/compliance/android/evidence_log.csv`。
4. **執行**
   - 運行預定的作業；記錄 Buildkite 運行 ID 或工具日誌。
   - 注意任何偏差（硬件交換、超限）。
5. **關閉**
   - 使用文物/鏈接對票證進行評論。
   - 如果運行與合規性相關，則更新
     `docs/source/compliance/android/and6_compliance_checklist.md` 並添加一行
     至 `evidence_log.csv`。

影響合作夥伴演示 (AND8) 的請求必須抄送合作夥伴工程人員。

## 4. 變更與取消- **重新安排：**重新打開原始工單，提出新的時段，並更新
  日曆條目。如果新插槽在 24 小時內，請 ping 硬件實驗室負責人 + SRE
  直接。
- **緊急取消：**遵循應急計劃
  (`device_lab_contingency.md`) 並記錄觸發器/操作/後續行。
- **溢出：** 如果運行超出其時間段 >15 分鐘，則發布更新並確認
  是否可以進行下一次預訂；否則移交給後備
  池或 Firebase 突發通道。

## 5. 證據與審計

|文物|地點 |筆記|
|----------|----------|--------|
|訂票 | `_android-device-lab` 隊列 (Jira) |導出每週匯總；證據日誌中的鏈接票證 ID。 |
|日曆導出 | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` |每週五運行 `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>`；幫助程序保存過濾後的 `.ics` 文件以及 ISO 週的 JSON 摘要，以便審核可以附加這兩個工件，而無需手動下載。 |
|容量快照 | `docs/source/compliance/android/evidence_log.csv` |每次預訂/關閉後更新。 |
|運行後筆記 | `docs/source/compliance/android/device_lab_contingency.md`（如果有意外情況）或票證評論 |審核所需。 |

在季度合規審查期間，附上日曆導出、票據摘要、
AND6 清單提交的證據日誌摘錄。

### 日曆導出自動化

1. 獲取“Android Device Lab – Reservations”的 ICS feed URL（或下載 `.ics` 文件）。
2. 執行

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   該腳本寫入 `artifacts/android/device_lab/<YYYY-WW>-calendar.ics`
   和 `...-calendar.json`，捕獲選定的 ISO 週。
3. 上傳生成的文件和每週證據包並引用
   JSON摘要在`docs/source/compliance/android/evidence_log.csv`時
   記錄設備實驗室容量。

## 6. 升級階梯

1. 硬件實驗室負責人（初級）
2. Android 基礎 TL
3. 項目主管/發布工程（用於凍結窗口）
4. 外部 StrongBox 實驗室聯繫人（調用保留器時）

升級必須記錄在票證中並反映在每週的 Android 中
狀態郵件。

## 7. 相關文檔

- `docs/source/compliance/android/device_lab_contingency.md` — 事件日誌
  能力不足。
- `docs/source/compliance/android/and6_compliance_checklist.md` — 主控
  可交付成果清單。
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — 硬件
  覆蓋跟踪器。
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  AND6/AND7 引用的 StrongBox 證明證據。

維持此預留程序滿足路線圖行動項目“定義
設備實驗室預留程序”並保留面向合作夥伴的合規工件
與 Android 準備計劃的其餘部分同步。

## 8. 故障轉移演練程序和聯繫方式

路線圖項目 AND6 還需要每季度進行一次故障轉移演練。完整的、
分步說明位於
`docs/source/compliance/android/device_lab_failover_runbook.md`，但是高
級別工作流程總結如下，以便請求者可以同時計劃演練
常規預訂。1. **安排演習：** 封鎖受影響的車道（`pixel8pro-strongbox-a`，
   共享中的後備池、`firebase-burst`、外部 StrongBox 保留器）
   日曆和 `_android-device-lab` 至少在演習前 7 天排隊。
2. **模擬中斷：** Depool主通道，觸發PagerDuty
   (`AND6-device-lab`) 事件，並註釋依賴的 Buildkite 作業
   操作手冊中註明的演練 ID。
3. **故障轉移：** 提升 Pixel7 後備通道，啟動 Firebase 突發
   套件，並在 6 小時內與外部 StrongBox 合作夥伴合作。捕捉
   Buildkite 運行 URL、Firebase 導出和保留確認。
4. **驗證和恢復：**驗證證明+ CI運行時，恢復
   原始車道，並更新 `device_lab_contingency.md` 加上證據日誌
   與捆綁路徑+校驗和。

### 聯繫和升級參考

|角色 |主要聯繫人 |頻道 |升級命令 |
|------|-----------------|------------------------|--------------------|
|硬件實驗室負責人 |普里亞·拉馬納坦 | `@android-lab` 鬆弛 · +81-3-5550-1234 | 1 |
|設備實驗室運營|馬特奧·克魯茲 | `_android-device-lab` 隊列 | 2 |
| Android 基礎 TL |埃琳娜·沃羅貝娃 | `@android-foundations` 鬆弛 | 3 |
|發布工程|阿列克謝·莫羅佐夫 | `release-eng@iroha.org` | 4 |
|外部保險箱實驗室 |櫻花儀器 NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

如果演練發現阻塞問題或有任何後備措施，則按順序升級
Lane無法在30分鐘內上線。始終記錄升級情況
在 `_android-device-lab` 票證中註明並將其鏡像到應急日誌中。