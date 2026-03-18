---
lang: zh-hant
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 賬戶地址合規狀態 (ADDR-2)

狀態：已接受 2026-03-30  
所有者：數據模型團隊/QA Guild  
路線圖參考：ADDR-2 — 雙格式合規套件

### 1. 概述

- 夾具：`fixtures/account/address_vectors.json`（I105（首選）+壓縮（`sora`，第二好）+多重簽名正/負案例）。
- 範圍：確定性 V1 有效負載，涵蓋隱式默認、Local-12、全局註冊表和具有完整錯誤分類的多重簽名控制器。
- 分發：在 Rust 數據模型、Torii、JS/TS、Swift 和 Android SDK 之間共享；如果任何消費者偏離，CI就會失敗。
- 事實來源：生成器位於 `crates/iroha_data_model/src/account/address/compliance_vectors.rs` 中，並通過 `cargo xtask address-vectors` 公開。
### 2. 再生與驗證

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

標誌：

- `--out <path>` — 生成臨時捆綁包時可選覆蓋（默認為 `fixtures/account/address_vectors.json`）。
- `--stdout` — 將 JSON 發送到標準輸出而不是寫入磁盤。
- `--verify` — 將當前文件與新生成的內容進行比較（漂移時快速失敗；不能與 `--stdout` 一起使用）。

### 3. 人工製品矩陣

|表面|執法|筆記|
|--------|-------------|--------|
| Rust 數據模型 | `crates/iroha_data_model/tests/account_address_vectors.rs` |解析 JSON，重建規範有效負載，並檢查 I105（首選）/壓縮（`sora`，第二好）/規範轉換 + 結構化錯誤。 |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` |驗證服務器端編解碼器，以便 Torii 確定性地拒絕格式錯誤的 I105（首選）/壓縮（`sora`，第二好的）有效負載。 |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |鏡像 V1 燈具（I105 首選/壓縮 (`sora`) 第二佳/全角）並為每個負面情況斷言 Norito 樣式錯誤代碼。 |
|斯威夫特 SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |練習 Apple 平台上的 I105（首選）/壓縮（`sora`，第二佳）解碼、多重簽名有效負載和錯誤顯示。 |
|安卓SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |確保 Kotlin/Java 綁定與規範固定裝置保持一致。 |

### 4. 監控和傑出工作- 狀態報告：此文檔與 `status.md` 和路線圖鏈接，因此每週審查可以驗證燈具的運行狀況。
- 開發者門戶摘要：請參閱文檔門戶 (`docs/portal/docs/reference/account-address-status.md`) 中的**參考 → 帳戶地址合規性**，了解面向外部的概要。
- Prometheus 和儀表板：每當您驗證 SDK 副本時，請使用 `--metrics-out`（以及可選的 `--metrics-label`）運行幫助程序，以便 Prometheus 文本文件收集器可以攝取 `account_address_fixture_check_status{target=…}`。 Grafana 儀表板 **帳戶地址夾具狀態** (`dashboards/grafana/account_address_fixture_status.json`) 呈現每個表面的通過/失敗計數，並顯示規範的 SHA-256 摘要以獲取審計證據。當任何目標報告 `0` 時發出警報。
- Torii 指標：`torii_address_domain_total{endpoint,domain_kind}` 現在為每個成功解析的帳戶文字發出，鏡像 `torii_address_invalid_total`/`torii_address_local8_total`。對生產中的任何 `domain_kind="local12"` 流量發出警報，並將計數器鏡像到 SRE `address_ingest` 儀表板，以便 Local-12 退休門擁有可審核的證據。
- 夾具助手：`scripts/account_fixture_helper.py` 下載或驗證規範 JSON，以便 SDK 發布自動化可以獲取/檢查捆綁包，而無需手動複製/粘貼，同時可選擇寫入 Prometheus 指標。示例：

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  當目標匹配時，幫助程序會寫入 `account_address_fixture_check_status{target="android"} 1`，以及公開 SHA-256 摘要的 `account_address_fixture_remote_info` / `account_address_fixture_local_info` 計量器。丟失文件報告 `account_address_fixture_local_missing`。
  自動化包裝器：從 cron/CI 調用 `ci/account_fixture_metrics.sh` 以發出合併的文本文件（默認 `artifacts/account_fixture/address_fixture.prom`）。傳遞重複的 `--target label=path` 條目（可以選擇為每個目標附加 `::https://mirror/...` 以覆蓋源），以便 Prometheus 抓取覆蓋每個 SDK/CLI 副本的一個文件。 GitHub 工作流程 `address-vectors-verify.yml` 已針對規範固定裝置運行此幫助程序，並上傳 `account-address-fixture-metrics` 工件以供 SRE 攝取。