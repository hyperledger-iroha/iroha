---
lang: zh-hant
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2025-12-29T18:16:35.984945+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI 包裝指南

本指南解釋瞭如何構建 MOCHI 桌面管理程序包、檢查
生成的工件，並調整隨
捆綁。它通過關注可複制的包裝來補充快速入門
和 CI 的使用。

## 先決條件

- Rust 工具鏈（2024 版/Rust 1.82+）與工作區依賴項
  已經建成了。
- 針對所需目標編譯的 `irohad`、`iroha_cli` 和 `kagami`。的
  捆綁器重用 `target/<profile>/` 中的二進製文件。
- `target/` 或自定義下的捆綁輸出有足夠的磁盤空間
  目的地。

在運行捆綁器之前構建一次依賴項：

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## 構建捆綁包

從存儲庫根調用專用 `xtask` 命令：

```bash
cargo xtask mochi-bundle
```

默認情況下，這會在 `target/mochi-bundle/` 下生成一個發行包，其名稱為
從主機操作系統和體系結構派生的文件名（例如，
`mochi-macos-aarch64-release.tar.gz`）。使用以下標誌進行自定義
構建：

- `--profile <name>` – 選擇貨物配置文件（`release`、`debug` 或
  自定義配置文件）。
- `--no-archive` – 保留擴展目錄而不創建 `.tar.gz`
  存檔（對於本地測試有用）。
- `--out <path>` – 將捆綁包寫入自定義目錄而不是
  `target/mochi-bundle/`。
- `--kagami <path>` – 提供預構建的 `kagami` 可執行文件以包含在
  存檔。當省略時，捆綁器會重用（或構建）來自
  選定的配置文件。
- `--matrix <path>` – 將捆綁包元數據附加到 JSON 矩陣文件（如果創建
  缺失），因此 CI 管道可以記錄在一個進程中生成的每個主機/配置文件工件
  跑。條目包括捆綁目錄、清單路徑和 SHA-256（可選）
  存檔位置以及最新的冒煙測試結果。
- `--smoke` – 將打包的 `mochi --help` 作為輕量級煙門執行
  捆綁後；在發布之前失敗會導致缺少依賴項
  人工製品。
- `--stage <path>` – 將完成的捆綁包（以及生成時的存檔）複製到
  暫存目錄，以便多平台構建可以將工件存放在一個目錄中
  無需額外腳本的位置。

該命令複製 `mochi-ui-egui`、`kagami`、`LICENSE`、示例
配置，並將 `mochi/BUNDLE_README.md` 放入捆綁包中。確定性的
`manifest.json` 與二進製文件一起生成，以便 CI 作業可以跟踪文件
哈希值和大小。

## 捆綁包佈局和驗證

擴展包遵循 `BUNDLE_README.md` 中記錄的佈局：

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

`manifest.json` 文件列出了每個工件及其 SHA-256 哈希值。驗證
將捆綁包複製到另一個系統後：

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI 管道可以緩存擴展目錄、對存檔進行簽名或發布
清單和發行說明。清單包含生成器
配置文件、目標三元組和創建時間戳以幫助來源跟踪。

## 運行時覆蓋

MOCHI 通過 CLI 標誌或發現幫助程序二進製文件和運行時位置
環境變量：- `--data-root` / `MOCHI_DATA_ROOT` – 覆蓋用於對等的工作空間
  配置、存儲和日誌。
- `--profile` – 在拓撲預設之間切換（`single-peer`、
  `four-peer-bft`）。
- `--torii-start`、`--p2p-start` – 更改分配時使用的基本端口
  服務。
- `--irohad` / `MOCHI_IROHAD` – 指向特定的 `irohad` 二進製文件。
- `--kagami` / `MOCHI_KAGAMI` – 覆蓋捆綁的 `kagami`。
- `--iroha-cli` / `MOCHI_IROHA_CLI` – 覆蓋可選的 CLI 幫助程序。
- `--restart-mode <never|on-failure>` – 禁用自動重啟或強制
  指數退避策略。
- `--restart-max <attempts>` – 覆蓋重新啟動嘗試的次數
  在 `on-failure` 模式下運行。
- `--restart-backoff-ms <millis>` – 設置自動重啟的基本退避。
- `MOCHI_CONFIG` – 提供自定義 `config/local.toml` 路徑。

CLI 幫助 (`mochi --help`) 打印完整的標誌列表。環境覆蓋
啟動時生效，可以與里面的設置對話框結合使用
用戶界面。

## CI 使用提示

- 運行`cargo xtask mochi-bundle --no-archive`生成一個目錄，可以
  使用特定於平台的工具進行壓縮（Windows 為 ZIP，Windows 為 tarball）
  Unix）。
- 使用 `cargo xtask mochi-bundle --matrix dist/matrix.json` 捕獲捆綁包元數據
  因此發布作業可以發布列出每個主機/配置文件的單個 JSON 索引
  管道中產生的人工製品。
- 在每個上使用 `cargo xtask mochi-bundle --stage /mnt/staging/mochi` （或類似的）
  構建代理將捆綁包和存檔上傳到共享目錄
  發布作業可以消耗。
- 發布存檔和 `manifest.json` 以便操作員可以驗證捆綁包
  誠信。
- 將生成的目錄存儲為構建工件，以種子煙霧測試
  使用確定性打包的二進製文件來鍛煉主管。
- 在發行說明或 `status.md` 日誌中記錄捆綁包哈希以供將來使用
  出處檢查。