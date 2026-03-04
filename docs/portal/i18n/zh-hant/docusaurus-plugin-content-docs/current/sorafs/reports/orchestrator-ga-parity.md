---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Orchestrator GA 奇偶校驗報告

現在每個 SDK 都會跟踪確定性多重獲取奇偶校驗，因此發布工程師可以確認
有效負載字節、塊收據、提供商報告和記分板結果保持一致
實施。每個線束都會消耗規範的多提供商捆綁包
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`，包含 SF1 計劃、提供商
元數據、遙測快照和協調器選項。

## Rust 基線

- **命令：** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **範圍：** 通過進程內編排器運行 `MultiPeerFixture` 計劃兩次，驗證
  組裝有效負載字節、塊收據、提供商報告和記分板結果。儀器儀表
  還跟踪峰值並發和有效工作集大小 (`max_parallel × max_chunk_length`)。
- **性能防護：** 每次運行必須在 CI 硬件上在 2 秒內完成。
- **工作設置上限：** 通過 SF1 型材，線束強制執行 `max_parallel = 3`，產生
  ≤196608字節窗口。

日誌輸出示例：

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK 工具

- **命令：** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **範圍：** 通過 `iroha_js_host::sorafsMultiFetchLocal` 重播相同的裝置，比較有效負載，
  連續運行的收據、提供商報告和記分板快照。
- **性能守衛：**每次執行必須在2秒內完成；線束打印測量值
  持續時間和保留字節上限（`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`）。

摘要行示例：

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK 線束

- **命令：** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **範圍：** 運行 `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` 中定義的奇偶校驗套件，
  通過 Norito 橋 (`sorafsLocalFetch`) 重播 SF1 賽程兩次。線束驗證
  有效負載字節、塊收據、提供商報告和記分板條目使用相同的確定性
  作為 Rust/JS 套件的提供者元數據和遙測快照。
- **Bridge bootstrap：** 線束按需解包 `dist/NoritoBridge.xcframework.zip` 並加載
  通過 `dlopen` 的 macOS 切片。當 xcframework 缺失或缺少 SoraFS 綁定時，它
  回退到 `cargo build -p connect_norito_bridge --release` 並鏈接到
  `target/release/libconnect_norito_bridge.dylib`，所以CI中不需要手動設置。
- **性能守衛：** CI 硬件上每次執行必須在 2 秒內完成；線束打印
  測量的持續時間和保留字節上限（`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`）。

摘要行示例：

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python 綁定線束

- **命令：** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **範圍：** 練習高級 `iroha_python.sorafs.multi_fetch_local` 包裝器及其類型
  數據類，因此規範裝置流經輪消費者調用的相同 API。測試
  從 `providers.json` 重建提供者元數據，注入遙測快照並驗證
  有效負載字節、塊收據、提供商報告和記分板內容，就像 Rust/JS/Swift 一樣
  套房。
- **先決條件：** 運行 `maturin develop --release` （或安裝輪子），以便 `_crypto` 公開
  `sorafs_multi_fetch_local` 調用 pytest 之前綁定；綁定時線束會自動跳過
  不可用。
- **性能保障：** 與 Rust 套件相同 ≤2s 預算； pytest 記錄組裝的字節數
  以及發布工件的提供商參與摘要。

發布門控應該捕獲每個工具（Rust、Python、JS、Swift）的摘要輸出，以便
歸檔報告可以在推廣構建之前統一區分有效負載收據和指標。運行
`ci/sdk_sorafs_orchestrator.sh` 執行每個奇偶校驗套件（Rust、Python 綁定、JS、Swift）
一趟； CI 工件應附加該助手的日誌摘錄以及生成的
`matrix.md`（SDK/狀態/持續時間表）到發布票證，以便審核者可以審核奇偶校驗
矩陣而無需在本地重新運行套件。