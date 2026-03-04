---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2025-12-29T18:16:35.924058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 設備實驗室儀器掛鉤 (AND6)

該參考文獻結束了路線圖行動“暫存剩餘的設備實驗室/
AND6 啟動前儀表掛鉤”。它解釋瞭如何每個保留
device-lab 插槽必須捕獲遙測、隊列和證明工件，以便
AND6 合規性檢查表、證據日誌和治理包共享相同的內容
確定性工作流程。將此註釋與預訂程序配對
(`device_lab_reservation.md`) 和規劃排練時的故障轉移操作手冊。

## 目標和範圍

- **確定性證據** – 所有儀器輸出均位於
  `artifacts/android/device_lab/<slot-id>/` 具有 SHA-256 清單，因此審核員
  可以在不重新運行探針的情況下比較包。
- **腳本優先工作流程** – 重用現有的助手
  （`ci/run_android_telemetry_chaos_prep.sh`，
  `scripts/android_keystore_attestation.sh`、`scripts/android_override_tool.sh`)
  而不是定制的 adb 命令。
- **檢查表保持同步** – 每次運行都會引用此文檔
  AND6 合規性檢查表並將工件附加到
  `docs/source/compliance/android/evidence_log.csv`。

## 工件佈局

1. 選擇與預訂票相匹配的唯一插槽標識符，例如
   `2026-05-12-slot-a`。
2. 播種標準目錄：

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. 將每個命令日誌保存在匹配的文件夾中（例如
   `telemetry/status.ndjson`、`attestation/pixel8pro.log`）。
4. 槽關閉後捕獲 SHA-256 清單：

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## 儀表矩陣

|流量|命令 |輸出位置 |筆記|
|------|------------|-----------------|--------|
|遙測編輯 + 狀態包 | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`、`telemetry/status.log` |在槽的開始和結束處運行；將 CLI 標準輸出附加到 `status.log`。 |
|待處理隊列+混亂準備| `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`、`queue/*.json`、`queue/*.sha256` |鏡像來自 `readiness/labs/telemetry_lab_01.md` 的 ScenarioD；擴展插槽中每個設備的環境變量。 |
|覆蓋分類賬摘要 | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` |即使沒有活動覆蓋也需要；證明零狀態。 |
| StrongBox/TEE認證| `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` |對每個保留設備重複此操作（匹配 `android_strongbox_device_matrix.md` 中的名稱）。 |
| CI 利用證明回歸 | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` |捕獲 CI 上傳的相同證據；包括在手動運行中以實現對稱。 |
| Lint / 依賴基線 | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`、`logs/lint.log` |每個凍結窗口運行一次；引用合規包中的摘要。 |

## 標準槽程序1. **飛行前（T-24h）** – 確認預訂機票引用此
   文檔，更新設備矩陣條目，並播種工件根。
2. **時段期間**
   - 首先運行遙測捆綁+隊列導出命令。通行證
     `--note <ticket>` 到 `ci/run_android_telemetry_chaos_prep.sh` 所以日誌
     引用事件 ID。
   - 觸發每個設備的證明腳本。當線束產生
     `.zip`，將其複製到 artefact 根目錄並記錄打印的 Git SHA
     腳本的結尾。
   - 使用覆蓋的摘要路徑執行 `make android-lint`，即使 CI
     已經跑了；審計員期望每個槽的日誌。
3. **運行後**
   - 在插槽內生成 `sha256sum.txt` 和 `README.md`（自由格式註釋）
     總結已執行命令的文件夾。
   - 將一行添加到 `docs/source/compliance/android/evidence_log.csv`
     插槽 ID、哈希清單路徑、Buildkite 引用（如果有）以及最新版本
     預留日曆導出中的設備實驗室容量百分比。
   - 鏈接 `_android-device-lab` 票證中的插槽文件夾，AND6
     清單和 `docs/source/android_support_playbook.md` 發布報告。

## 故障處理和升級

- 如果任何命令失敗，請捕獲 `logs/` 下的 stderr 輸出並按照
  `device_lab_reservation.md` §6 中的升級階梯。
- 隊列或遙測不足應立即記錄覆蓋狀態
  `docs/source/sdk/android/telemetry_override_log.md` 並引用插槽 ID
  這樣治理就可以追踪演練。
- 證明回歸必須記錄在
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  以及上面記錄的失敗設備序列號和捆綁包路徑。

## 報告清單

在將插槽標記為完成之前，請驗證以下參考是否已更新：

- `docs/source/compliance/android/and6_compliance_checklist.md` — 標記
  完成儀表行並記下插槽 ID。
- `docs/source/compliance/android/evidence_log.csv` — 添加/更新條目
  插槽哈希和容量讀取。
- `_android-device-lab` 票證 — 附加工件鏈接和 Buildkite 作業 ID。
- `status.md` — 在下一個 Android 準備摘要中包含一個簡短的註釋，以便
  路線圖讀者知道哪個插槽產生了最新證據。

遵循此流程可保留 AND6 的“設備實驗室 + 儀器掛鉤”
里程碑可審核，並防止預訂、執行、
和報告。