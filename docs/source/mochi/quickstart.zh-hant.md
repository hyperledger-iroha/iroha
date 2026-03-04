---
lang: zh-hant
direction: ltr
source: docs/source/mochi/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44faf6c98d141959cf8cf40b1df7d3d82c3448e6f2b1bc4fa54cdeceb97994b0
source_last_modified: "2025-12-29T18:16:35.985408+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI 快速入門

**MOCHI** 是本地 Hyperledger Iroha 網絡的桌面管理程序。本指南將介紹
安裝先決條件，構建應用程序，啟動 egui shell，並使用
用於日常開發的運行時工具（設置、快照、擦除）。

## 先決條件

- Rust 工具鏈：`rustup default stable`（工作區目標版本 2024 / Rust 1.82+）。
- 平台工具鏈：
  - macOS：Xcode 命令行工具 (`xcode-select --install`)。
  - Linux：GCC、pkg-config、OpenSSL 標頭 (`sudo apt install build-essential pkg-config libssl-dev`)。
- Iroha 工作區依賴項：
  - `cargo xtask mochi-bundle` 需要內置 `irohad`、`kagami` 和 `iroha_cli`。通過構建一次
    `cargo build -p irohad -p kagami -p iroha_cli`。
- 可選：`direnv` 或 `cargo binstall` 用於管理本地貨物二進製文件。

MOCHI shell 到 CLI 二進製文件。確保可以通過環境變量發現它們
下面或在 PATH 上可用：

|二進制 |環境覆蓋|筆記|
|----------|------------------------------------|--------------------------------------------------------|
| `irohad` | `MOCHI_IROHAD` |監督同行|
| `kagami` | `MOCHI_KAGAMI` |生成創世清單/快照 |
| `iroha_cli` | `MOCHI_IROHA_CLI` |即將推出的輔助功能可選 |

## 構建 MOCHI

從存儲庫根目錄：

```bash
cargo build -p mochi-ui-egui
```

此命令構建 `mochi-core` 和 egui 前端。要生成可分發的包，請運行：

```bash
cargo xtask mochi-bundle
```

捆綁任務在 `target/mochi-bundle` 下組裝二進製文件、清單和配置存根。

## 啟動 egui shell

直接從 Cargo 運行 UI：

```bash
cargo run -p mochi-ui-egui
```

默認情況下，MOCHI 在臨時數據目錄中創建單點預設：

- 數據根：`$TMPDIR/mochi`。
- Torii 基本端口：`8080`。
- P2P基本端口：`1337`。

啟動時使用 CLI 標誌覆蓋默認值：

```bash
cargo run -p mochi-ui-egui -- \
  --data-root /path/to/workspace \
  --profile four-peer-bft \
  --torii-start 12000 \
  --p2p-start 13000 \
  --kagami /path/to/kagami \
  --irohad /path/to/irohad
```

當省略 CLI 標誌時，環境變量會鏡像相同的覆蓋：設置 `MOCHI_DATA_ROOT`，
`MOCHI_PROFILE`、`MOCHI_CHAIN_ID`、`MOCHI_TORII_START`、`MOCHI_P2P_START`、`MOCHI_RESTART_MODE`、
`MOCHI_RESTART_MAX` 或 `MOCHI_RESTART_BACKOFF_MS` 來預置 Supervisor 構建器；二進制路徑
繼續尊重 `MOCHI_IROHAD`/`MOCHI_KAGAMI`/`MOCHI_IROHA_CLI`，並且 `MOCHI_CONFIG` 指向
顯式 `config/local.toml`。

## 設置和持久化

從儀表板工具欄打開 **設置** 對話框以調整主管配置：

- **數據根** — 對等配置、存儲、日誌和快照的基本目錄。
- **Torii / P2P 基本端口** — 用於確定性分配的起始端口。
- **日誌可見性** — 在日誌查看器中切換 stdout/stderr/system 通道。

高級旋鈕，例如主管重啟策略
`config/local.toml`。將 `[supervisor.restart] mode = "never"` 設置為禁用
事件調試時自動重啟，或者調整
`max_restarts`/`backoff_ms`（通過配置文件或 CLI 標誌
`--restart-mode`、`--restart-max`、`--restart-backoff-ms`) 控制重試
行為。應用更改會重建主管，重新啟動任何正在運行的對等點，並將覆蓋寫入
`config/local.toml`。配置合併保留不相關的密鑰，以便高級用戶可以保留
手動調整以及 MOCHI 管理的值。

## 快照和擦除/重新生成

**維護**對話框公開了兩個安全操作：

- **導出快照** — 將對等存儲/配置/日誌和當前創世清單複製到
  活動數據根下的 `snapshots/<label>`。標籤會自動清理。
- **恢復快照** - 重新水化對等存儲、快照根、配置、日誌和起源
  來自現有包的清單。 `Supervisor::restore_snapshot` 接受絕對路徑或
  清理後的 `snapshots/<label>` 文件夾名稱； UI 反映了此流程，因此維護 → 恢復
  可以重播證據包而無需手動觸摸文件。
- **擦除和重新創世** — 停止運行對等點，刪除存儲目錄，通過重新生成創世
  Kagami，並在擦除完成後重新啟動對等點。

回歸測試涵蓋了這兩個流程（`export_snapshot_captures_storage_and_metadata`，
`wipe_and_regenerate_resets_storage_and_genesis`）以保證確定性輸出。

## 日誌和流

儀表板一目了然地顯示數據/指標：

- **日誌** — 遵循 `irohad` stdout/stderr/系統生命週期消息。在“設置”中切換頻道。
- **塊/事件** — 託管流通過指數退避和註釋幀自動重新連接
  帶有 Norito 解碼的摘要。
- **狀態** — 輪詢 `/status` 並呈現隊列深度、吞吐量和延遲的迷你圖。
- **啟動準備** - 按**開始**（單個對等點或所有對等點）後，MOCHI 探測
  `/status` 有界退避；橫幅報告每個對等點何時準備好（以及觀察到的
  隊列深度）或在準備超時時顯示 Torii 錯誤。

狀態瀏覽器和編輯器的選項卡提供對帳戶、資產、對等點和常見內容的快速訪問
無需離開 UI 即可執行指令。 Peers 視圖鏡像 `FindPeers` 查詢，以便您可以確認
在運行集成測試之前，當前在驗證器集中註冊了哪些公鑰。

使用編寫器工具欄的 **管理簽名庫** 按鈕導入或編輯簽名權限。的
對話框將條目寫入活動網絡根 (`<data_root>/<profile>/signers.json`)，並保存
保管庫密鑰可立即用於交易預覽和提交。當金庫是
清空作曲家後退到捆綁的開發密鑰，以便本地工作流程繼續工作。
表單現在涵蓋鑄幣/銷毀/轉移（包括隱式接收）、域/帳戶/資產定義
註冊、帳戶准入政策、多重簽名提案、空間目錄清單 (AXT/AMX)、
SoraFS pin 清單以及授予或撤銷角色等治理操作非常常見
無需手寫 Norito 有效負載即可演練路線圖創作任務。

## 清理和故障排除- 停止應用程序以終止受監督的對等點。
- 刪除數據根（`rm -rf <data_root>`）以重置所有狀態。
- 如果 Kagami 或 irohad 位置發生變化，請更新環境變量或使用以下命令重新運行 MOCHI
  適當的 CLI 標誌；設置對話框將在下次應用時保留新路徑。

對於額外的自動化檢查 `mochi/mochi-core/tests`（主管生命週期測試）和
`mochi/mochi-integration` 用於模擬 Torii 場景。運送捆綁或接線
桌面到 CI 管道中，請參閱 {doc}`mochi/packaging` 指南。

## 本地測試門

在發送補丁之前運行 `ci/check_mochi.sh`，以便共享 CI 門執行所有三個 MOCHI
板條箱：

```bash
./ci/check_mochi.sh
```

幫助程序針對 `mochi-core`、`mochi-ui-egui` 執行 `cargo check`/`cargo test`，以及
`mochi-integration`，捕獲夾具漂移（規範塊/事件捕獲）和 egui 線束
一次性回歸。如果腳本報告陳舊的裝置，請重新運行忽略的再生測試，
例如：

```bash
cargo test -p mochi-core regenerate_block_wire_fixture -- --ignored
```

重新生成後重新運行門可確保更新的字節在推送之前保持一致。