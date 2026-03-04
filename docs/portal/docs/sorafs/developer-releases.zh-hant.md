---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a59639046a496f35c3cf80006a3330a25407b8143213f1b02b5ce766a70b4f0
source_last_modified: "2026-01-05T09:28:11.867590+00:00"
translation_last_reviewed: 2026-02-07
title: Release Process
summary: Run the CLI/SDK release gate, apply the shared versioning policy, and publish canonical release notes.
translator: machine-google-reviewed
---

# 發布流程

SoraFS 二進製文件（`sorafs_cli`、`sorafs_fetch`、幫助程序）和 SDK 包
（`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`）一起發貨。發布
管道使 CLI 和庫保持一致，確保 lint/測試覆蓋率，以及
為下游消費者捕獲人工製品。為每個項目運行下面的清單
候選標籤。

## 0. 確認安全審查簽核

在執行技術發布門之前，捕獲最新的安全審查
文物：

- 下載最新的 SF-6 安全審查備忘錄 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  並在發布票據中記錄其 SHA256 哈希值。
- 附上補救票鏈接（例如 `governance/tickets/SF6-SR-2026.md`）並記下簽字
  來自安全工程和工具工作組的批准者。
- 驗證備忘錄中的補救清單是否已關閉；未解決的項目會阻止發布。
- 準備上傳奇偶校驗線束日誌 (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  與清單包一起。
- 確認您計劃運行的簽名命令包括 `--identity-token-provider` 和顯式
  `--identity-token-audience=<aud>` 因此 Fulcio 範圍在發布證據中被捕獲。

在通知治理和發布版本時包括這些工件。

## 1. 執行發布/測試門

`ci/check_sorafs_cli_release.sh` 幫助程序運行格式化、Clippy 和測試
跨 CLI 和 SDK 包，帶有工作區本地目標目錄 (`.target`)
以避免在 CI 容器內執行時發生權限衝突。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

該腳本執行以下斷言：

- `cargo fmt --all -- --check`（工作區）
- `cargo clippy --locked --all-targets` 用於 `sorafs_car`（具有 `cli` 功能），
  `sorafs_manifest` 和 `sorafs_chunker`
- `cargo test --locked --all-targets` 對於那些相同的板條箱

如果任何步驟失敗，請在標記之前修復回歸。發布版本必須是
與主線連續；不要將修復挑選到發布分支中。大門
還檢查無密鑰簽名標誌（`--identity-token-issuer`、`--identity-token-audience`）
在適用的情況下提供；缺少參數導致運行失敗。

## 2. 應用版本控制策略

所有 SoraFS CLI/SDK 包都使用 SemVer：

- `MAJOR`：在第一個 1.0 版本中引入。 1.0 之前，`0.y` 略有不同
  **表示 CLI 表面或 Norito 架構中的重大更改**。
  可選策略、遙測添加後面的字段）。
- `PATCH`：錯誤修復、僅文檔版本以及依賴項更新
  不改變可觀察到的行為。

始終將 `sorafs_car`、`sorafs_manifest` 和 `sorafs_chunker` 保持在同一位置
版本，以便下游 SDK 消費者可以依賴於單個對齊版本
字符串。當碰撞版本時：

1. 更新每個 crate 的 `Cargo.toml` 中的 `version =` 字段。
2. 通過 `cargo update -p <crate>@<new-version>` 重新生成 `Cargo.lock`（
   工作區強制執行顯式版本）。
3. 再次運行釋放門以確保沒有陳舊的工件殘留。

## 3. 準備發行說明

每個版本都必鬚髮布一個 markdown 變更日誌，重點介紹 CLI、SDK 和
影響治理的變革。使用中的模板
`docs/examples/sorafs_release_notes.md`（將其複製到您的發布工件中
目錄並填寫具體細節的部分）。

最低內容：

- **亮點**：CLI 和 SDK 消費者的功能標題。
  要求。
- **升級步驟**：TL;DR命令用於碰撞貨物依賴性並重新運行
  確定性的固定裝置。
- **驗證**：命令輸出哈希值或信封以及確切的
  已執行 `ci/check_sorafs_cli_release.sh` 修訂版。

將填寫的發行說明附加到標籤（例如 GitHub 發行正文）並存儲
它們與確定性生成的人工製品一起。

## 4. 執行釋放鉤子

運行 `scripts/release_sorafs_cli.sh` 生成簽名包並
每個版本附帶的驗證摘要。包裝器構建 CLI
必要時，調用`sorafs_cli manifest sign`，並立即重放
`manifest verify-signature` 因此故障在標記之前就會出現。示例：

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

溫馨提示：

- 跟踪您的發布輸入（有效負載、計劃、摘要、預期令牌哈希）
  存儲庫或部署配置，以便腳本保持可重現。 CI 夾具
  `fixtures/sorafs_manifest/ci_sample/` 下的捆綁包顯示了規範佈局。
- 基於 `.github/workflows/sorafs-cli-release.yml` 的 CI 自動化；它運行
  發布門，調用上面的腳本，並將捆綁包/簽名存檔為
  工作流程工件。鏡像相同的命令順序（釋放門→標誌→
  在其他 CI 系統中驗證），以便審核日誌與生成的哈希值保持一致。
- 保留生成的`manifest.bundle.json`、`manifest.sig`，
  `manifest.sign.summary.json` 和 `manifest.verify.summary.json` 在一起——它們
  形成治理通知中引用的數據包。
- 當版本更新規範固定裝置時，複製刷新的清單，
  塊計劃，並將摘要寫入 `fixtures/sorafs_manifest/ci_sample/`（並更新
  `docs/examples/sorafs_ci_sample/manifest.template.json`) 在標記之前。
  下游運營商依賴於已提交的固定裝置來重現發布
  捆綁。
- 捕獲 `sorafs_cli proof stream` 有界通道驗證的運行日誌並將其附加到
  發布數據包以證明流媒體防護措施仍然有效。
- 在發行說明中記錄簽名期間使用的確切 `--identity-token-audience`；治理
  在批准出版之前，根據 Fulcio 政策對受眾進行交叉檢查。

當版本還帶有
網關推出。將其指向同一個清單包以證明證明
匹配候選工件：

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. 標記並發布

檢查通過並掛鉤完成後：

1. 運行 `sorafs_cli --version` 和 `sorafs_fetch --version` 來確認二進製文件
   報告新版本。
2.在簽入的`sorafs_release.toml`中準備發布配置
   （首選）或部署存儲庫跟踪的另一個配置文件。避免
   依賴臨時環境變量；將路徑傳遞給 CLI
   `--config`（或等效），因此釋放輸入是明確的並且
   可重現。
3. 創建簽名標籤（首選）或註釋標籤：
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. 上傳工件（CAR 包、清單、證明摘要、發行說明、
   認證輸出）到項目註冊處進行治理
   [部署指南](./developer-deployment.md) 中的清單。如果釋放
   創建新的固定裝置，將它們推送到共享固定裝置存儲庫或對象存儲中，以便
   審計自動化可以將已發布的捆綁包與源代碼控制進行區分。
5. 通知治理渠道，提供簽名標籤、發行說明、
   清單包/簽名哈希值、存檔的 `manifest.sign/verify` 摘要、
   以及任何證明信封。包括 CI 作業 URL（或日誌存檔）
   運行 `ci/check_sorafs_cli_release.sh` 和 `scripts/release_sorafs_cli.sh`。更新
   治理票證，以便審計員可以追踪對人工製品的批准；當
   `.github/workflows/sorafs-cli-release.yml` 職位發布通知，鏈接
   記錄哈希輸出而不是粘貼臨時摘要。

## 6. 發布後跟進

- 確保文檔指向新版本（快速入門、CI 模板）
  已更新或確認無需更改。
- 後續工作的文件路線圖條目（例如，遷移標誌、棄用
- 為審計員存檔發布門輸出日誌——將它們存儲在簽名的旁邊
  文物。

遵循此管道可以將 CLI、SDK 包和治理抵押品保留在
每個發布週期的鎖步。