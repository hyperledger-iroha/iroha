---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM 和來源證明 — Android SDK

|領域|價值|
|--------|--------|
|範圍 | Android SDK (`java/iroha_android`) + 示例應用程序 (`examples/android/*`) |
|工作流程所有者 |發布工程（Alexei Morozov）|
|最後驗證 | 2026-02-11（Buildkite `android-sdk-release#4821`）|

## 1. 生成工作流程

運行幫助程序腳本（為 AND6 自動化添加）：

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

該腳本執行以下操作：

1. 執行 `ci/run_android_tests.sh` 和 `scripts/check_android_samples.sh`。
2. 調用 `examples/android/` 下的 Gradle 包裝器來構建 CycloneDX SBOM
   `:android-sdk`、`:operator-console` 和 `:retail-wallet` 與隨附的
   `-PversionName`。
3. 使用規範名稱將每個 SBOM 複製到 `artifacts/android/sbom/<sdk-version>/` 中
   （`iroha-android.cyclonedx.json` 等）。

## 2. 出處和簽名

相同的腳本使用 `cosign sign-blob --bundle <file>.sigstore --yes` 簽署每個 SBOM
並在目標目錄中發出 `checksums.txt` (SHA-256)。設置 `COSIGN`
如果二進製文件位於 `$PATH` 之外，則為環境變量。腳本完成後，
記錄包/校驗和路徑以及 Buildkite 運行 ID
`docs/source/compliance/android/evidence_log.csv`。

## 3.驗證

要驗證已發布的 SBOM：

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

將輸出 SHA 與 `checksums.txt` 中列出的值進行比較。審閱者還將 SBOM 與之前的版本進行比較，以確保依賴性增量是有意的。

## 4. 證據快照 (2026-02-11)

|組件| SBOM | SHA-256 | Sigstore 捆綁包 |
|------------|------|----------|------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` 捆綁包存儲在 SBOM 旁邊 |
|操作員控制台樣本 | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
|零售錢包樣本| `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*（從 Buildkite 運行 `android-sdk-release#4821` 捕獲的哈希值；通過上面的驗證命令重現。）*

## 5. 出色的工作

- 在 GA 之前自動執行發布管道內的 SBOM + 聯合簽名步驟。
- 一旦 AND6 標記清單完成，將 SBOM 鏡像到公共工件存儲桶。
- 與文檔協調，從面向合作夥伴的發行說明中鏈接 SBOM 下載位置。