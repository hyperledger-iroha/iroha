---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2025-12-29T18:16:35.923121+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 設備實驗室應急日誌

在此記錄 Android 設備實驗室應急計劃的每次激活。
包含足夠的詳細信息以供合規性審查和未來的準備審核。

|日期 |觸發|採取的行動|後續行動|業主|
|------|---------|----------------|------------|--------|
| 2026-02-11 | Pixel8 Pro 通道中斷並延遲 Pixel8a 交付後，運力下降至 78%（請參閱 `android_strongbox_device_matrix.md`）。 |將 Pixel7 通道提升為主要 CI 目標，借用共享 Pixel6 車隊，安排 Firebase 測試實驗室對零售錢包樣本進行煙霧測試，並根據 AND6 計劃聘請外部 StrongBox 實驗室。 |更換 Pixel8 Pro 有故障的 USB-C 集線器（截止日期為 2026 年 2 月 15 日）；確認 Pixel8a 抵達並重新設定容量報告基準。 |硬件實驗室負責人 |
| 2026-02-13 | Pixel8 Pro 集線器更換並獲得 GalaxyS24 批准，容量恢復至 85%。 |將 Pixel7 通道返回到輔助通道，重新啟用帶有標籤 `pixel8pro-strongbox-a` 和 `s24-strongbox-a` 的 `android-strongbox-attestation` Buildkite 作業，更新準備矩陣 + 證據日誌。 |監控 Pixel8a 交付預計時間（仍在等待中）；記錄備用輪轂庫存。 |硬件實驗室負責人 |