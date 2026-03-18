---
lang: zh-hant
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 文檔自動化基線 (AND5)

路線圖項目 AND5 需要文檔、本地化和發布
在 AND6（CI 與合規性）開始之前，自動化可進行審核。這個文件夾
記錄 AND5/AND6 引用的命令、工件和證據佈局，
反映捕獲的計劃
`docs/source/sdk/android/developer_experience_plan.md` 和
`docs/source/sdk/android/parity_dashboard_plan.md`。

## 管道和命令

|任務|命令 |預期的文物|筆記|
|------|------------|--------------------|--------|
|本地化存根同步 | `python3 scripts/sync_docs_i18n.py`（每次運行可選擇傳遞 `--lang <code>`）|存儲在 `docs/automation/android/i18n/<timestamp>-sync.log` 下的日誌文件以及翻譯的存根提交 |使 `docs/i18n/manifest.json` 與翻譯的存根保持同步；日誌記錄觸及的語言代碼以及基線中捕獲的 git 提交。 |
| Norito 夾具+奇偶校驗| `ci/check_android_fixtures.sh`（包裹 `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`）|將生成的摘要 JSON 複製到 `docs/automation/android/parity/<stamp>-summary.json` |驗證 `java/iroha_android/src/test/resources` 有效負載、清單哈希和簽名的固定長度。將摘要附在 `artifacts/android/fixture_runs/` 下的節奏證據旁邊。 |
|清單示例和發布證明 | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]`（運行測試 + SBOM + 來源）|出處捆綁元數據加上存儲在 `docs/automation/android/samples/<version>/` 下的 `docs/source/sdk/android/samples/` 生成的 `sample_manifest.json` |將 AND5 示例應用程序和發布自動化聯繫在一起 - 捕獲生成的清單、SBOM 哈希和出處日誌以進行 Beta 審核。 |
| Parity 儀表板提要 | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` 後跟 `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` |將 `metrics.prom` 快照或 Grafana 導出 JSON 複製到 `docs/automation/android/parity/<stamp>-metrics.prom` |提供儀表板計劃，以便 AND5/AND7 治理可以驗證無效的提交計數器和遙測採用情況。 |

## 證據捕獲

1. **為所有內容添加時間戳。 ** 使用 UTC 時間戳命名文件
   (`YYYYMMDDTHHMMSSZ`) 因此奇偶校驗儀表板、治理會議記錄並發布
   文檔可以確定性地引用相同的運行。
2. **參考提交。 ** 每個日誌應包含運行的 git commit 哈希值
   加上任何相關配置（例如，`ANDROID_PARITY_PIPELINE_METADATA`）。
   當隱私需要編輯時，請添加註釋並鏈接到安全保管庫。
3. **存檔最小上下文。 ** 我們只檢查結構化摘要（JSON、
   `.prom`、`.log`）。重要的文物（APK 包、屏幕截圖）應保留在
   `artifacts/` 或在日誌中記錄有簽名哈希的對象存儲。
4. **更新狀態條目。 ** 當 AND5 里程碑在 `status.md` 中取得進展時，引用
   相應的文件（例如，`docs/automation/android/parity/20260324T010203Z-summary.json`）
   因此審計人員可以跟踪基線而無需抓取 CI 日誌。

遵循此佈局滿足“可用於的文檔/自動化基線”
AND6 引用並保存 Android 文檔程序的“審核”先決條件
與已公佈的計劃保持同步。