---
lang: zh-hant
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-12-29T18:16:35.060432+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 文檔自動化基線

該目錄捕獲路線圖項目的自動化表面，例如
AND5/AND6（Android 開發者體驗 + 發布準備）和 DA-1
（數據可用性威脅模型自動化）指的是當他們要求
可審計的文件證據。暫存命令參考和預期
artefacts in-tree 甚至可以保留合規審查的先決條件
當 CI 管道或儀表板離線時。

## 目錄佈局

|路徑|目的|
|------|---------|
| `docs/automation/android/` | Android 文檔和本地化自動化基線 (AND5)，包括 AND6 簽署之前所需的 i18n 存根同步日誌、奇偶校驗摘要和 SDK 發布證據。 |
| `docs/automation/da/` | `cargo xtask da-threat-model-report` 和夜間文檔刷新引用的數據可用性威脅模型自動化輸出。 |

每個子目錄都記錄了產生證據的命令以及
我們希望簽入的文件佈局（通常是 JSON 摘要、運行日誌或
體現）。每當
自動化運行實質上更改了已發布的文檔，然後鏈接到提交
來自相關狀態/路線圖條目。

## 用法

1. **使用子目錄中描述的命令運行自動化**
   自述文件（例如，`ci/check_android_fixtures.sh` 或
   `cargo xtask da-threat-model-report`）。
2. **將生成的 JSON/日誌工件**從 `artifacts/…` 複製到
   將 `docs/automation/<program>/…` 文件夾與 ISO-8601 時間戳相匹配
   文件名，以便審計員可以將證據與治理會議記錄關聯起來。
3. **關閉路線圖時參考 `status.md`/`roadmap.md` 中的提交**
   門，以便審核者可以確認用於該決策的自動化基線。
4. **保持文件輕量級**。期望是結構化元數據，
   清單或摘要，而不是批量二進制 blob。較大的垃圾場應保留在
   帶有此處記錄的簽名引用的對象存儲。

通過集中這些自動化註釋，我們解鎖了“文檔/自動化基線”
AND6 提出並發出 DA 威脅的先決條件“可供審計”
模型流是夜間報告和手動抽查的確定性主頁。