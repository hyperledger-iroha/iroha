---
slug: /norito/ledger-walkthrough
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

本演练通过展示对 [Norito 快速入门](./quickstart.md) 进行了补充
如何使用 `iroha` CLI 改变和检查账本状态。您将注册一个
新的资产定义，将一些单位铸造到默认操作员帐户中，转移
将部分余额转移到另一个账户，并验证由此产生的交易
和持股。每个步骤都反映了 Rust/Python/JavaScript 中涵盖的流程
SDK 快速入门，以便您可以确认 CLI 和 SDK 行为之间的一致性。

## 先决条件

- 按照[快速入门](./quickstart.md)通过以下方式启动单点网络
  `docker compose -f defaults/docker-compose.single.yml up --build`。
- 确保 `iroha`（CLI）已构建或下载，并且您可以访问
  对等体使用 `defaults/client.toml`。
- 可选助手：`jq`（格式化 JSON 响应）和 POSIX shell
  下面使用的环境变量片段。

在整个指南中，将 `$ADMIN_ACCOUNT` 和 `$RECEIVER_ACCOUNT` 替换为
您计划使用的帐户 ID。默认捆绑包已包含两个帐户
从演示密钥派生：

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

通过列出前几个帐户来确认值：

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. 检查创世状态

首先探索 CLI 所针对的账本：

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

这些命令依赖于 Norito 支持的响应，因此过滤和分页是
确定性并与 SDK 收到的内容相匹配。

## 2. 注册资产定义

在 `wonderland` 内创建一个名为 `coffee` 的新的、可无限铸造的资产
域名：

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI 打印提交的交易哈希（例如，
`0x5f…`）。保存下来以便以后查询状态。

## 3. 将铸币单位存入运营商账户

资产数量位于 `(asset definition, account)` 货币对下。薄荷 250
将 `coffee#wonderland` 转换为 `$ADMIN_ACCOUNT` 的单位：

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

再次从 CLI 输出中捕获事务哈希 (`$MINT_HASH`)。至
仔细检查余额，运行：

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

或者，仅针对新资产：

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4.将部分余额转入另一个账户

将 50 个单位从操作员帐户移至 `$RECEIVER_ACCOUNT`：

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

将交易哈希保存为 `$TRANSFER_HASH`。查询双方持股情况
帐户以验证新余额：

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. 验证账本证据

使用保存的哈希值来确认两个事务均已提交：

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

您还可以流式传输最近的块以查看哪个块包含传输：

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

上面的每个命令都使用与 SDK 相同的 Norito 有效负载。如果你复制
通过代码进行此流程（请参阅下面的 SDK 快速入门），哈希值和余额将
只要您的目标网络和默认值相同，就可以排队。

## SDK 比价链接

- [Rust SDK 快速入门](../sdks/rust) — 演示注册指令，
  提交交易，并从 Rust 轮询状态。
- [Python SDK 快速入门](../sdks/python) — 显示相同的寄存器/铸币厂
  使用 Norito 支持的 JSON 帮助程序进行操作。
- [JavaScript SDK 快速入门](../sdks/javascript) — 涵盖 Torii 请求，
  治理助手和类型化查询包装器。

首先运行 CLI 演练，然后使用您首选的 SDK 重复该场景
确保两个表面在交易哈希、余额和查询上达成一致
输出。