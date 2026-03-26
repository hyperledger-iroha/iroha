---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T16:26:46.562262+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 入門

本快速指南展示了編譯 Kotodama 合約的最小工作流程，
檢查生成的 Norito 字節碼，在本地運行並部署它
到 Iroha 節點。

## 先決條件

1. 安裝 Rust 工具鏈（1.76 或更高版本）並查看此存儲庫。
2. 構建或下載支持的二進製文件：
   - `koto_compile` – 發出 IVM/Norito 字節碼的 Kotodama 編譯器
   - `ivm_run` 和 `ivm_tool` – 本地執行和檢查實用程序
   - `iroha_cli` – 用於通過 Torii 進行合約部署

   存儲庫 Makefile 需要 `PATH` 上的這些二進製文件。你可以
   下載預構建的工件或從源代碼構建它們。如果你編譯
   本地工具鏈，將 Makefile 助手指向二進製文件：

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. 確保到達部署步驟時 Iroha 節點正在運行。的
   下面的示例假設 Torii 可通過您中配置的 URL 訪問
   `iroha_cli` 配置文件 (`~/.config/iroha/cli.toml`)。

## 1.編譯Kotodama合約

該存儲庫提供了一個最小的“hello world”合約
`examples/hello/hello.ko`。將其編譯為 Norito/IVM 字節碼（`.to`）：

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

關鍵標誌：

- `--abi 1` 將合約鎖定為 ABI 版本 1（唯一受支持的版本）
  寫作時）。
- `--max-cycles 0` 請求無限執行；設置一個正數來綁定
  零知識證明的循環填充。

## 2. 檢查 Norito 工件（可選）

使用 `ivm_tool` 驗證標頭和嵌入元數據：

```sh
ivm_tool inspect target/examples/hello.to
```

您應該看到 ABI 版本、啟用的功能標誌和導出的條目
點。這是部署前的快速健全性檢查。

## 3.本地運行合約

使用 `ivm_run` 執行字節碼以確認行為，而無需觸摸
節點：

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` 示例記錄問候語並發出 `SET_ACCOUNT_DETAIL` 系統調用。
在發布之前迭代合約邏輯時，在本地運行非常有用
它在鏈上。

## 4. 通過 `iroha_cli` 部署

當您對合同感到滿意時，請使用 CLI 將其部署到節點。
提供授權帳戶、其簽名密鑰以及 `.to` 文件或
Base64 有效負載：

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

該命令通過 Torii 提交 Norito 清單 + 字節碼包並打印
由此產生的交易狀態。事務提交後，代碼
響應中顯示的哈希可用於檢索清單或列出實例：

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. 運行 Torii

註冊字節碼後，您可以通過提交指令來調用它
引用存儲的代碼（例如，通過 `iroha_cli ledger transaction submit`
或您的應用程序客戶端）。確保帳戶權限允許所需的
系統調用（`set_account_detail`、`transfer_asset` 等）。

## 提示和故障排除

- 使用 `make examples-run` 編譯並執行所提供的示例
  射擊。如果二進製文件未打開，則覆蓋 `KOTO`/`IVM` 環境變量
  `PATH`。
- 如果 `koto_compile` 拒絕 ABI 版本，請驗證編譯器和節點
  兩者都以 ABI v1 為目標（運行 `koto_compile --abi`，不帶參數列出
  支持）。
- CLI 接受十六進製或 Base64 簽名密鑰。為了進行測試，您可以使用
  由 `iroha_cli tools crypto keypair` 發出的密鑰。
- 調試 Norito 有效負載時，`ivm_tool disassemble` 子命令有幫助
  將指令與 Kotodama 源關聯起來。

此流程反映了 CI 和集成測試中使用的步驟。為了更深入
深入了解 Kotodama 語法、系統調用映射和 Norito 內部結構，請參閱：

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`