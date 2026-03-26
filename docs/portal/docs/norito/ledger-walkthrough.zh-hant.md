---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c61035c0e4b0fd478f08beeef34d7ae41415f55b09dc93dfda9490efe94fb91
source_last_modified: "2026-01-22T16:26:46.505734+00:00"
translation_last_reviewed: 2026-02-07
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
slug: /norito/ledger-walkthrough
translator: machine-google-reviewed
---

本演練通過展示對 [Norito 快速入門](./quickstart.md) 進行了補充
如何使用 `iroha` CLI 改變和檢查賬本狀態。您將註冊一個
新的資產定義，將一些單位鑄造到默認操作員帳戶中，轉移
將部分餘額轉移到另一個賬戶，並驗證由此產生的交易
和持股。每個步驟都反映了 Rust/Python/JavaScript 中涵蓋的流程
SDK 快速入門，以便您可以確認 CLI 和 SDK 行為之間的一致性。

## 先決條件

- 按照[快速入門](./quickstart.md)通過以下方式啟動單點網絡
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- 確保 `iroha`（CLI）已構建或下載，並且您可以訪問
  對等體使用 `defaults/client.toml`。
- 可選助手：`jq`（格式化 JSON 響應）和 POSIX shell
  下面使用的環境變量片段。

在整個指南中，將 `$ADMIN_ACCOUNT` 和 `$RECEIVER_ACCOUNT` 替換為
您計劃使用的帳戶 ID。默認捆綁包已包含兩個帳戶
從演示密鑰派生：

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

通過列出前幾個帳戶來確認值：

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. 檢查創世狀態

首先探索 CLI 所針對的賬本：

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

這些命令依賴於 Norito 支持的響應，因此過濾和分頁是
確定性並與 SDK 收到的內容相匹配。

## 2. 註冊資產定義

在 `wonderland` 內創建一個名為 `coffee` 的新的、可無限鑄造的資產
域名：

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI 打印提交的交易哈希（例如，
`0x5f…`）。保存下來以便以後查詢狀態。

## 3. 將鑄幣單位存入運營商賬戶

資產數量位於 `(asset definition, account)` 貨幣對下。薄荷 250
將 `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` 轉換為 `$ADMIN_ACCOUNT` 的單位：

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

再次從 CLI 輸出中捕獲事務哈希 (`$MINT_HASH`)。至
仔細檢查餘額，運行：

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

或者，僅針對新資產：

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4.將部分餘額轉入另一個賬戶

將 50 個單位從操作員帳戶移至 `$RECEIVER_ACCOUNT`：

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

將交易哈希保存為 `$TRANSFER_HASH`。查詢雙方持股情況
帳戶以驗證新余額：

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. 驗證賬本證據

使用保存的哈希值來確認兩個事務均已提交：

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

您還可以流式傳輸最近的塊以查看哪個塊包含傳輸：

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

上面的每個命令都使用與 SDK 相同的 Norito 有效負載。如果你複製
通過代碼進行此流程（請參閱下面的 SDK 快速入門），哈希值和余額將
只要您的目標網絡和默認值相同，就可以排隊。

## SDK 比價鏈接

- [Rust SDK 快速入門](../sdks/rust) — 演示註冊指令，
  提交交易，並從 Rust 輪詢狀態。
- [Python SDK 快速入門](../sdks/python) — 顯示相同的寄存器/鑄幣廠
  使用 Norito 支持的 JSON 幫助程序進行操作。
- [JavaScript SDK 快速入門](../sdks/javascript) — 涵蓋 Torii 請求，
  治理助手和類型化查詢包裝器。

首先運行 CLI 演練，然後使用您首選的 SDK 重複該場景
確保兩個表面在交易哈希、餘額和查詢上達成一致
輸出。