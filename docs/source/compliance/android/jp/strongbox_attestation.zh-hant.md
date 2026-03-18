---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox 證明證據 — 日本部署

|領域|價值|
|--------|--------|
|評估窗口| 2026-02-10 – 2026-02-12 |
|文物地點 | `artifacts/android/attestation/<device-tag>/<date>/`（每個 `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` 的捆綁格式）|
|捕獲工具| `scripts/android_keystore_attestation.sh`、`scripts/android_strongbox_attestation_ci.sh`、`scripts/android_strongbox_attestation_report.py` |
|審稿人|合規與法律硬件實驗室主管（日本）|

## 1. 捕獲過程

1. 在 StrongBox 矩陣中列出的每個設備上，生成質詢並捕獲證明包：
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. 將捆綁元數據（`result.json`、`chain.pem`、`challenge.hex`、`alias.txt`）提交到證據樹。
3. 運行 CI 幫助程序以重新離線驗證所有捆綁包：
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. 設備概要 (2026-02-12)

|設備標籤 |模型/保險箱|捆綁路徑 |結果 |筆記|
|------------|--------------------|-------------|--------|--------|
| `pixel6-strongbox-a` | Pixel 6 / 張量 G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ 通過（硬件支持）|挑戰限制，操作系統補丁 2025 年 3 月 5 日。 |
| `pixel7-strongbox-a` | Pixel 7 / 張量 G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ 通過 |主要 CI 泳道候選者；溫度在規格範圍內|
| `pixel8pro-strongbox-a` | Pixel 8 Pro / 張量 G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ 通過（複試）|更換 USB-C 集線器； Buildkite `android-strongbox-attestation#221` 捕獲了傳遞的包。 |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ 通過 | Knox 認證配置文件於 2026 年 2 月 9 日導入。 |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ 通過 |導入 Knox 認證配置文件； CI 車道現已綠色。 |

設備標籤映射到 `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`。

## 3. 審稿人清單

- [x] 驗證 `result.json` 顯示 `strongbox_attestation: true` 和到受信任根的證書鏈。
- [x] 確認挑戰字節匹配 Buildkite 運行 `android-strongbox-attestation#219`（初始掃描）和 `#221`（Pixel 8 Pro 重新測試 + S24 捕獲）。
- [x] 硬件修復後重新運行 Pixel 8 Pro 捕獲（所有者：硬件實驗室負責人，於 2026 年 2 月 13 日完成）。
- [x] 一旦 Knox 配置文件批准到達，即可完成 Galaxy S24 捕獲（所有者：Device Lab Ops，於 2026 年 2 月 13 日完成）。

## 4. 分配

- 將此摘要以及最新報告文本文件附加到合作夥伴合規性數據包中（FISC 清單§數據駐留）。
- 響應監管機構審核時參考捆綁路徑；不要在加密通道之外傳輸原始證書。

## 5. 變更日誌

|日期 |改變 |作者 |
|------|--------|--------|
| 2026-02-12 |初始 JP 捆綁捕獲 + 報告。 |設備實驗室運營|