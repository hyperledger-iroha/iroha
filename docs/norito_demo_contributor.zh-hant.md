---
lang: zh-hant
direction: ltr
source: docs/norito_demo_contributor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-12-29T18:16:35.099277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito SwiftUI 演示貢獻者指南

本文檔記錄了針對 SwiftUI 演示運行所需的手動設置步驟
本地 Torii 節點和模擬賬本。它通過以下方式補充 `docs/norito_bridge_release.md`
專注於日常開發任務。有關集成的更深入演練
Norito 將堆棧橋接/連接到 Xcode 項目，請參閱 `docs/connect_swift_integration.md`。

## 環境設置

1. 安裝 `rust-toolchain.toml` 中定義的 Rust 工具鏈。
2. 在 macOS 上安裝 Swift 5.7+ 和 Xcode 命令行工具。
3. （可選）安裝 [SwiftLint](https://github.com/realm/SwiftLint) 以進行 linting。
4. 運行 `cargo build -p irohad` 以確保節點在您的主機上編譯。
5. 將 `examples/ios/NoritoDemoXcode/Configs/demo.env.example` 複製到 `.env` 並調整
   與您的環境相匹配的價值觀。應用程序在啟動時讀取這些變量：
   - `TORII_NODE_URL` — 基本 REST URL（WebSocket URL 源自它）。
   - `CONNECT_SESSION_ID` — 32 字節會話標識符 (base64/base64url)。
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — `/v2/connect/session` 返回的令牌。
   - `CONNECT_CHAIN_ID` — 控制握手期間宣布的鏈標識符。
   - `CONNECT_ROLE` — 在 UI 中預先選擇的默認角色（`app` 或 `wallet`）。
   - 用於手動測試的可選助手：`CONNECT_PEER_PUB_B64`、`CONNECT_SHARED_KEY_B64`、
     `CONNECT_APPROVE_ACCOUNT_ID`, `CONNECT_APPROVE_PRIVATE_KEY_B64`,
     `CONNECT_APPROVE_SIGNATURE_B64`。

## 引導 Torii + 模擬賬本

該存儲庫提供了幫助程序腳本，這些腳本啟動帶有內存中分類帳預置的 Torii 節點。
加載模擬賬戶：

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

該腳本發出：

- Torii 節點記錄到 `artifacts/torii.log`。
- 分類帳指標（Prometheus 格式）至 `artifacts/metrics.prom`。
- `artifacts/torii.jwt` 的客戶端訪問令牌。

`start.sh` 使演示對等點保持運行，直到您按 `Ctrl+C`。它寫了一個就緒狀態
`artifacts/ios_demo_state.json` 的快照（其他工件的真實來源），
複製活動的 Torii 標準輸出日誌，輪詢 `/metrics` 直到 Prometheus 刮取
可用，並將配置的帳戶渲染為 `torii.jwt`（包括私鑰
當配置提供它們時）。該腳本接受 `--artifacts` 來覆蓋輸出
目錄，`--telemetry-profile` 匹配自定義 Torii 配置，以及
`--exit-after-ready` 用於非交互式 CI 作業。

`SampleAccounts.json` 中的每個條目都支持以下字段：

- `name`（字符串，可選）— 存儲為帳戶元數據 `alias`。
- `public_key`（多重哈希字符串，必需）— 用作帳戶簽名人。
- `private_key`（可選）— 包含在 `torii.jwt` 中，用於生成客戶端憑證。
- `domain`（可選）— 如果省略，則默認為資產域。
- `asset_id`（字符串，必需）— 為賬戶鑄造的資產定義。
- `initial_balance`（字符串，必需）— 鑄造到帳戶中的數字金額。

## 運行 SwiftUI 演示

1. 按照 `docs/norito_bridge_release.md` 中所述構建 XCFramework 並將其捆綁
   進入演示項目（項目中的引用預計為 `NoritoBridge.xcframework`
   根）。
2. 在 Xcode 中打開 `NoritoDemoXcode` 項目。
3. 選擇 `NoritoDemo` 方案並定位 iOS 模擬器或設備。
4. 確保通過方案的環境變量引用 `.env` 文件。
   填充 `/v2/connect/session` 導出的 `CONNECT_*` 值，以便 UI 為
   應用程序啟動時預先填充。
5. 驗證硬件加速默認值：`App.swift` 調用
   `DemoAccelerationConfig.load().apply()` 因此演示會選擇
   `NORITO_ACCEL_CONFIG_PATH` 環境覆蓋或捆綁
   `acceleration.{json,toml}`/`client.{json,toml}` 文件。刪除/調整這些輸入，如果您
   想要在運行之前強制 CPU 回退。
6. 構建並啟動應用程序。如果沒有，主屏幕會提示輸入 Torii URL/令牌
   已通過 `.env` 設置。
7. 啟動“連接”會話以訂閱帳戶更新或批准請求。
8. 提交 IRH 傳輸並檢查屏幕上的日誌輸出以及 Torii 日誌。

### 硬件加速切換（金屬/霓虹燈）

`DemoAccelerationConfig` 鏡像 Rust 節點配置，以便開發人員可以練習
沒有硬編碼閾值的金屬/NEON 路徑。加載器搜索以下內容
發射地點：

1. `NORITO_ACCEL_CONFIG_PATH`（在 `.env`/scheme 參數中定義）— 絕對路徑或
   `tilde` 擴展的指向 `iroha_config` JSON/TOML 文件的指針。
2. 名為 `acceleration.{json,toml}` 或 `client.{json,toml}` 的捆綁配置文件。
3. 如果兩個源都不可用，則保留默認設置 (`AccelerationSettings()`)。

示例 `acceleration.toml` 片段：

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

保留字段 `nil` 會繼承工作區默認值。負數被忽略，
缺少 `[accel]` 部分則回退到確定性 CPU 行為。當運行於
沒有金屬支撐的模擬器，橋會默默地保持標量路徑，即使
配置請求金屬。

## 集成測試

- 集成測試駐留在 `Tests/NoritoDemoTests` 中（待 macOS CI 啟動後添加）
  可用）。
- 使用上面的腳本測試啟動 Torii 並執行 WebSocket 訂閱、令牌
  餘額，並通過 Swift 包轉移流量。
- 測試運行的日誌與指標一起存儲在 `artifacts/tests/<timestamp>/` 中
  樣本分類賬轉儲。

## CI 奇偶校驗

- 在發送涉及演示或共享裝置的 PR 之前運行 `make swift-ci`。的
  目標執行夾具奇偶校驗，驗證儀表板提要，並呈現
  本地總結。在 CI 中，相同的工作流程取決於 Buildkite 元數據
  (`ci/xcframework-smoke:<lane>:device_tag`) 因此儀表板可以將結果歸因於
  正確的模擬器或 StrongBox 通道 - 如果您調整，請驗證元數據是否存在
  管道或代理標籤。
- 當 `make swift-ci` 失敗時，請按照 `docs/source/swift_parity_triage.md` 中的步驟操作
  並查看渲染的 `mobile_ci` 輸出以確定需要哪個通道
  再生或事件後續。

## 故障排除

- 如果演示無法連接到 Torii，請驗證節點 URL 和 TLS 設置。
- 確保 JWT 令牌（如果需要）有效且未過期。
- 檢查 `artifacts/torii.log` 是否有服務器端錯誤。
- 對於 WebSocket 問題，請檢查客戶端日誌窗口或 Xcode 控制台輸出。