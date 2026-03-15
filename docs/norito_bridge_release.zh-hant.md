---
lang: zh-hant
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge 發布包裝

本指南概述了將 `NoritoBridge` Swift 綁定發佈為
可以從 Swift Package Manager 和 CocoaPods 使用的 XCFramework。的
工作流程使 Swift 工件與發布的 Rust 包版本保持同步
Iroha 的 Norito 編解碼器。有關使用已發布的內容的端到端說明
應用程序內的工件（Xcode 項目接線、ChaChaPoly 用法等），請參閱
`docs/connect_swift_integration.md`。

> **注意：** 一旦 macOS 構建者俱備所需的能力，此流程的 CI 自動化就會落地
> Apple 工具上線（在發布工程 macOS 構建器待辦事項中進行跟踪）。
> 在此之前，必須在開發 Mac 上手動執行以下步驟。

## 先決條件

- 安裝了最新穩定的 Xcode 命令行工具的 macOS 主機。
- 與工作區 `rust-toolchain.toml` 匹配的 Rust 工具鏈。
- Swift 工具鏈 5.7 或更高版本。
- CocoaPods（通過 Ruby gems）如果發佈到中央規格存儲庫。
- 訪問 Hyperledger Iroha 版本簽名密鑰以標記 Swift 工件。

## 版本控制模型

1. 確定 Norito 編解碼器 (`crates/norito/Cargo.toml`) 的 Rust 箱版本。
2. 使用版本標識符標記工作區（例如 `v2.1.0`）。
3. 對 Swift 包和 CocoaPods podspec 使用相同的語義版本。
4. 當 Rust 箱增加其版本時，重複該過程並發布匹配的版本
   迅捷神器。測試時版本可能包含元數據後綴（例如 `-alpha.1`）。

## 構建步驟

1. 從存儲庫根目錄調用幫助程序腳本來組裝 XCFramework：

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   該腳本為 iOS 和 macOS 目標編譯 Rust 橋接庫並捆綁
   在單個 XCFramework 目錄下生成靜態庫。
   它還發出 `dist/NoritoBridge.artifacts.json`，捕獲橋接版本並
   每個平台的 SHA-256 哈希值（如果存在，則使用 `--bridge-version <version>` 覆蓋版本
   需要）。

2. 壓縮 XCFramework 進行分發：

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. 更新 Swift 包清單 (`IrohaSwift/Package.swift`) 以指向新的
   版本和校驗和：

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   定義二進制目標時，將校驗和記錄在 `Package.swift` 中。

4. 使用新版本、校驗和和存檔更新 `IrohaSwift/IrohaSwift.podspec`
   網址。

5. **如果橋獲得新的導出，則重新生成標頭。 ** Swift 橋現在公開
   `connect_norito_set_acceleration_config` 所以 `AccelerationSettings` 可以切換金屬/
   GPU 後端。確保 `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   壓縮前匹配 `crates/connect_norito_bridge/include/connect_norito_bridge.h`。

6. 在標記之前運行 Swift 驗證套件：

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   第一個命令確保 Swift 包（包括 `AccelerationSettings`）保持不變
   綠色；第二個驗證夾具奇偶校驗，呈現奇偶校驗/CI 儀表板，以及
   執行與 Buildkite 中強制執行的相同遙測檢查（包括
   `ci/xcframework-smoke:<lane>:device_tag` 元數據要求）。

7. 在發布分支中提交生成的工件並標記提交。

## 出版

### Swift 包管理器

- 將標籤推送到公共 Git 存儲庫。
- 確保標籤可通過包索引（Apple 或社區鏡像）訪問。
- 消費者現在可以信賴 `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`。

### CocoaPods

1. 在本地驗證 pod：

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. 推送更新後的 podspec：

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. 確認新版本出現在 CocoaPods 索引中。

## CI注意事項

- 創建一個運行打包腳本、歸檔工件並上傳的 macOS 作業
  生成的校驗和作為工作流程輸出。
- Gate 在針對新生成的框架構建的 Swift 演示應用程序上發布。
- 存儲構建日誌以幫助診斷故障。

## 其他自動化想法

- 一旦所有需要的目標都暴露出來，就直接使用 `xcodebuild -create-xcframework`。
- 集成簽名/公證以在開發人員機器之外分發。
- 通過固定 SPM 使集成測試與打包版本保持同步
  對發布標籤的依賴。