---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e39dc94f52395bd9323177df1a7feeb7bbd4f9a3cdea07b02f9d60e7826e199e
source_last_modified: "2026-01-22T16:26:46.506936+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
slug: /norito/quickstart
translator: machine-google-reviewed
---

本演練反映了我們希望開發人員在學習時遵循的工作流程
首次Norito和Kotodama：啟動確定性單點網絡，
編譯合約，在本地試運行，然後通過 Torii 發送
參考 CLI。

該示例合約將鍵/值對寫入調用者的帳戶，以便您可以
立即使用 `iroha_cli` 驗證副作用。

## 先決條件

- [Docker](https://docs.docker.com/engine/install/) 啟用 Compose V2（使用
  啟動 `defaults/docker-compose.single.yml` 中定義的示例對等點）。
- Rust 工具鏈（1.76+），用於構建輔助二進製文件（如果您不下載）
  已發表的。
- `koto_compile`、`ivm_run` 和 `iroha_cli` 二進製文件。您可以從以下位置構建它們
  工作區結帳如下所示或下載匹配的發布工件：

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> 上面的二進製文件可以安全地與工作區的其餘部分一起安裝。
> 它們從不鏈接到 `serde`/`serde_json`； Norito 編解碼器是端到端強制執行的。

## 1. 啟動單點開發網絡

該存儲庫包含由 `kagami swarm` 生成的 Docker Compose 捆綁包
（`defaults/docker-compose.single.yml`）。它連接默認的創世、客戶端
配置和運行狀況探測，以便可以在 `http://127.0.0.1:8080` 處訪問 Torii。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

讓容器保持運行（在前台或分離）。全部
後續 CLI 調用通過 `defaults/client.toml` 定位該對等點。

## 2. 編寫合同

創建一個工作目錄並保存最小的 Kotodama 示例：

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> 更喜歡將 Kotodama 源代碼保留在版本控制中。門戶託管的示例是
> 如果您也可以在 [Norito 示例庫](./examples/) 下找到
> 想要一個更豐富的起點。

## 3. 使用 IVM 編譯並試運行

將合約編譯為 IVM/Norito 字節碼（`.to`）並在本地執行
在接觸網絡之前確認主機系統調用成功：

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

運行程序打印 `info("Hello from Kotodama")` 日誌並執行
`SET_ACCOUNT_DETAIL` 針對模擬主機的系統調用。如果可選`ivm_tool`
二進制可用，`ivm_tool inspect target/quickstart/hello.to` 顯示
ABI 標頭、功能位和導出的入口點。

## 4.通過Torii提交字節碼

在節點仍在運行的情況下，使用 CLI 將編譯後的字節碼發送到 Torii。
默認的開發身份來自於公鑰
`defaults/client.toml`，所以賬戶ID為
```
ih58...
```

使用配置文件提供 Torii URL、鏈 ID 和簽名密鑰：

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI 使用 Norito 對交易進行編碼，使用開發密鑰對其進行簽名，然後
將其提交給正在運行的對等點。觀看 `set_account_detail` 的 Docker 日誌
系統調用或監視 CLI 輸出以獲取已提交的事務哈希。

## 5.驗證狀態變化

使用相同的 CLI 配置文件來獲取合約寫入的帳戶詳細信息：

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

您應該看到 Norito 支持的 JSON 有效負載：

```json
{
  "hello": "world"
}
```

如果該值缺失，請確認 Docker 撰寫服務仍然存在
正在運行並且 `iroha` 報告的交易哈希達到了 `Committed`
狀態。

## 後續步驟

- 探索自動生成的[示例庫](./examples/) 以查看
  更高級的 Kotodama 片段如何映射到 Norito 系統調用。
- 閱讀 [Norito 入門指南](./getting-started) 了解更深入的信息
  編譯器/運行器工具、清單部署和 IVM 的說明
  元數據。
- 迭代您自己的合約時，請在
  用於重新生成可下載片段的工作區，以便保留門戶文檔和工件
  與 `crates/ivm/docs/examples/` 下的來源同步。