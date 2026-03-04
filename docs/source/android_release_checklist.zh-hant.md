---
lang: zh-hant
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 發布清單 (AND6)

此清單捕獲了 **AND6 — CI 和合規性強化** 門
`roadmap.md`（§優先級 5）。它將 Android SDK 版本與 Rust 保持一致
通過闡明 CI 作業、合規工件來發布 RFC 期望，
設備實驗室證據以及在 GA 之前必須附加的出處包，
LTS（即修補程序）列車向前推進。

將本文檔與以下內容一起使用：

- `docs/source/android_support_playbook.md` — 發布日曆、SLA 和
  升級樹。
- `docs/source/android_runbook.md` — 日常操作手冊。
- `docs/source/compliance/android/and6_compliance_checklist.md` — 調節器
  文物庫存。
- `docs/source/release_dual_track_runbook.md` — 雙軌發布治理。

## 1. 舞台大門一覽

|舞台|所需的大門 |證據|
|--------|----------------|----------|
| **T−7 天（凍結前）** |每晚 `ci/run_android_tests.sh` 綠色，持續 14 天； `ci/check_android_fixtures.sh`、`ci/check_android_samples.sh` 和 `ci/check_android_docs_i18n.sh` 通過； lint/依賴項掃描已排隊。 | Buildkite 儀表板、夾具差異報告、示例屏幕截圖。 |
| **T−3 天（RC 促銷）** |設備實驗室預訂已確認； StrongBox 認證 CI 運行 (`scripts/android_strongbox_attestation_ci.sh`)；在預定硬件上運行的機器人電動/儀表套件； `./gradlew lintRelease ktlintCheck detekt dependencyGuard` 乾淨。 |設備矩陣 CSV、證明捆綁清單、Gradle 報告存檔在 `artifacts/android/lint/<version>/` 下。 |
| **T−1 天（去/不去）** |遙測編輯狀態包已刷新 (`scripts/telemetry/check_redaction_status.py --write-cache`)；根據 `and6_compliance_checklist.md` 更新合規性工件；來源排練已完成 (`scripts/android_sbom_provenance.sh --dry-run`)。 | `docs/source/compliance/android/evidence_log.csv`，遙測狀態 JSON，來源試運行日誌。 |
| **T0（GA/LTS 切換）** | `scripts/publish_android_sdk.sh --dry-run` 已完成；出處+SBOM 簽名；導出並附加到通過/不通過分鐘的發布清單； `ci/sdk_sorafs_orchestrator.sh` 冒煙工作綠色。 |發布 RFC 附件、Sigstore 捆綁包、`artifacts/android/` 下的採用工件。 |
| **T+1 天（切換後）** |已驗證修補程序準備情況 (`scripts/publish_android_sdk.sh --validate-bundle`)；審查了儀表板差異（`ci/check_android_dashboard_parity.sh`）；證據包已上傳至`status.md`。 |儀表板差異導出，鏈接到 `status.md` 條目，存檔的發布數據包。 |

## 2. CI 和質量門矩陣|門 |命令/腳本 |筆記|
|------|--------------------|--------|
|單元+集成測試| `ci/run_android_tests.sh`（包裹 `ci/run_android_tests.sh`）|發出 `artifacts/android/tests/test-summary.json` + 測試日誌。包括 Norito 編解碼器、隊列、StrongBox 回退和 Torii 客戶端利用測試。每晚和標記之前需要。 |
|燈具奇偶校驗| `ci/check_android_fixtures.sh`（包裹 `scripts/check_android_fixtures.py`）|確保重新生成的 Norito 裝置與 Rust 規範集相匹配；當門失敗時附加 JSON diff。 |
|示例應用程序 | `ci/check_android_samples.sh` |構建 `examples/android/{operator-console,retail-wallet}` 並通過 `scripts/android_sample_localization.py` 驗證本地化屏幕截圖。 |
|文檔/I18N | `ci/check_android_docs_i18n.sh` | Guards README + 本地化快速入門。在 doc 編輯到發布分支後再次運行。 |
|儀表板奇偶校驗 | `ci/check_android_dashboard_parity.sh` |確認 CI/導出的指標與 Rust 的對應指標一致； T+1驗證時需要。 |
| SDK採用煙霧| `ci/sdk_sorafs_orchestrator.sh` |使用當前 SDK 練習多源 Sorafs Orchestrator 綁定。上傳舞台製品之前需要。 |
|鑑證驗證 | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` |聚合 `artifacts/android/attestation/**` 下的 StrongBox/TEE 證明包；將摘要附加到 GA 數據包中。 |
|設備實驗室插槽驗證 | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` |在將證據附加到發布包之前驗證儀器包； CI 針對 `fixtures/android/device_lab/slot-sample` 中的示例槽運行（遙測/證明/隊列/日誌 + `sha256sum.txt`）。 |

> **提示：** 將這些作業添加到 `android-release` Buildkite 管道中，以便
> 凍結週會使用釋放分支提示自動重新運行每個門。

整合的 `.github/workflows/android-and6.yml` 作業運行 lint，
每個 PR/推送的測試套件、證明摘要和設備實驗室插槽檢查
觸及 Android 源，在 `artifacts/android/{lint,tests,attestation,device_lab}/` 下上傳證據。

## 3. Lint 和依賴項掃描

從存儲庫根目錄運行 `scripts/android_lint_checks.sh --version <semver>`。的
腳本執行：

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- 報告和依賴性保護輸出存檔於
  用於發布的 `artifacts/android/lint/<label>/` 和 `latest/` 符號鏈接
  管道。
- 失敗的 lint 發現需要修復或在版本中記錄
  RFC 記錄可接受的風險（由發布工程 + 計劃批准）
  鉛）。
- `dependencyGuardBaseline` 重新生成依賴鎖；附上差異
  到通過/不通過數據包。

## 4. 設備實驗室和 StrongBox 覆蓋範圍

1. 使用中引用的容量跟踪器保留 Pixel + Galaxy 設備
   `docs/source/compliance/android/device_lab_contingency.md`。阻止發布
   如果可用性 ` 刷新證明報告。
3. 運行儀器矩陣（記錄設備中的套件/ABI 列表）
   跟踪器）。即使重試成功，也會在事件日誌中捕獲失敗。
4. 如果需要回退到 Firebase 測試實驗室，請提交票證；鏈接票證
   在下面的清單中。

## 5. 合規性和遙測工件- 歐盟遵循 `docs/source/compliance/android/and6_compliance_checklist.md`
  和 JP 提交的材料。更新 `docs/source/compliance/android/evidence_log.csv`
  帶有哈希值 + Buildkite 作業 URL。
- 通過以下方式刷新遙測編輯證據
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`。
  將生成的 JSON 存儲在
  `artifacts/android/telemetry/<version>/status.json`。
- 記錄架構差異輸出
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  證明與 Rust 出口商的平等。

## 6. 出處、SBOM 和出版

1. 試運行發布管道：

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. 生成 SBOM + Sigstore 出處：

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3.附上`artifacts/android/provenance/<semver>/manifest.json`並簽名
   `checksums.sha256` 到發布 RFC。
4.升級到真正的Maven倉庫時，重新運行
   `scripts/publish_android_sdk.sh` 沒有 `--dry-run`，捕獲控制台
   記錄，並將生成的工件上傳到 `artifacts/android/maven/<semver>`。

## 7. 提交包模板

每個 GA/LTS/修補程序版本應包括：

1. **已完成的清單** — 複製此文件的表格，勾選每個項目，然後鏈接
   支持工件（Buildkite 運行、日誌、文檔差異）。
2. **設備實驗室證據** — 認證報告摘要、預訂日誌和
   任何應急激活。
3. **遙測數據包** — 編輯狀態 JSON、架構差異、鏈接
   `docs/source/sdk/android/telemetry_redaction.md` 更新（如果有）。
4. **合規性工件** — 在合規性文件夾中添加/更新的條目
   加上刷新的證據日誌 CSV。
5. **出處捆綁** — SBOM、Sigstore 簽名和 `checksums.sha256`。
6. **發布摘要** — `status.md` 總結中附有一頁概述
   以上（日期、版本、任何被放棄的門的亮點）。

將數據包存儲在 `artifacts/android/releases/<version>/` 下並引用它
在 `status.md` 和發布 RFC 中。

- 自動`scripts/run_release_pipeline.py --publish-android-sdk ...`
  複製最新的 lint 存檔 (`artifacts/android/lint/latest`) 和
  合規證據登錄 `artifacts/android/releases/<version>/`，以便
  提交數據包始終具有規範位置。

---

**提醒：**每當有新的 CI 作業、合規工件、
或添加遙測要求。路線圖項目 AND6 保持開放狀態，直到
檢查表和相關自動化在連續兩個版本中證明是穩定的
火車。