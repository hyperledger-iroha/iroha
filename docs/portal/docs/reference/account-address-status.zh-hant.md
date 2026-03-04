---
lang: zh-hant
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b92bdfc323a4bc031ca7f2237f238d5d515f7238791a6ec9c50b55e361c85560
source_last_modified: "2026-01-28T17:11:30.639071+00:00"
translation_last_reviewed: 2026-02-07
id: account-address-status
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
---

規範的 ADDR-2 捆綁包 (`fixtures/account/address_vectors.json`) 捕獲
IH58（首選）、壓縮（`sora`，第二好；半角/全角）、多重簽名和負固定裝置。
每個 SDK + Torii 表面都依賴於相同的 JSON，因此我們可以檢測任何編解碼器
在投入生產之前就發生了漂移。此頁面反映了內部狀態簡介
（根存儲庫中的`docs/source/account_address_status.md`）所以門戶
讀者可以參考工作流程，而無需深入研究 mono-repo。

## 重新生成或驗證包

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

標誌：

- `--stdout` — 將 JSON 發送到標準輸出以進行臨時檢查。
- `--out <path>` — 寫入不同的路徑（例如，當本地差異更改時）。
- `--verify` — 將工作副本與新生成的內容進行比較（不能
  與 `--stdout` 組合）。

CI 工作流程**地址向量漂移**運行 `cargo xtask address-vectors --verify`
每當裝置、生成器或文檔發生變化時，都會立即提醒審閱者。

## 誰消耗了燈具？

|表面|驗證 |
|---------|------------|
| Rust 數據模型 | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii（服務器）| `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
|斯威夫特 SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
|安卓SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

每個線束往返規範字節 + IH58 + 壓縮（`sora`，第二好的）編碼和
檢查 Norito 類型的錯誤代碼是否與負面情況下的夾具一致。

## 需要自動化嗎？

發布工具可以使用助手來編寫夾具刷新腳本
`scripts/account_fixture_helper.py`，獲取或驗證規範
捆綁無需複制/粘貼步驟：

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

幫助程序接受 `--source` 覆蓋或 `IROHA_ACCOUNT_FIXTURE_URL`
環境變量，以便 SDK CI 作業可以指向其首選鏡像。
當提供 `--metrics-out` 時，幫助程序寫入
`account_address_fixture_check_status{target=\"…\"}` 以及規範
SHA-256 摘要 (`account_address_fixture_remote_info`) 所以 Prometheus 文本文件
收集器和 Grafana 儀表板 `account_address_fixture_status` 可以證明
每個表面都保持同步。每當目標報告 `0` 時發出警報。對於
多表面自動化使用包裝器 `ci/account_fixture_metrics.sh`
（接受重複的 `--target label=path[::source]`）以便待命團隊可以發布
一個用於節點導出器文本文件收集器的合併 `.prom` 文件。

## 需要完整的簡介嗎？

完整的 ADDR-2 合規狀態（所有者、監控計劃、未決行動項目）
位於存儲庫中的 `docs/source/account_address_status.md` 中
與地址結構 RFC (`docs/account_structure.md`)。使用此頁面作為
快捷的操作提醒；請參閱回購文檔以獲取深入指導。