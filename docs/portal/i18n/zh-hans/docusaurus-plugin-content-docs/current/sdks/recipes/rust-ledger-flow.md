---
slug: /sdks/recipes/rust-ledger-flow
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

从“@site/src/components/SampleDownload”导入 SampleDownload；

此配方反映了 [CLI 账本演练](../../norito/ledger-walkthrough.md)
但从 Rust 二进制文件运行所有内容。它重用默认的开发网络
(`docker compose -f defaults/docker-compose.single.yml up --build`) 和演示
`defaults/client.toml` 中的凭据，因此您可以比较 SDK 和 CLI 哈希值
其一。

<样本下载
  href="/sdk-recipes/rust/src/main.rs"
  文件名=“src/main.rs”
  description="使用此 Rust 源文件作为基线来跟踪或比较您的更改。"
/>

## 先决条件

1. 使用 Docker Compose 运行开发同级（请参阅 [Norito 快速入门](../../norito/quickstart.md)）。
2. 从以下位置导出默认管理员/接收者帐户和管理员私钥
   `defaults/client.toml`：

   ```bash
   export ADMIN_ACCOUNT="ih58..."
   export RECEIVER_ACCOUNT="ih58..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   私钥字符串是存储在 `[account].private_key` 下的多重哈希编码值。
3. 创建一个新的工作区二进制文件（或重复使用现有的工作区二进制文件）：

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. 添加依赖项（如果您不在 crates.io 版本中，请使用 crates.io 版本）
   工作区）：

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## 示例程序

```rust title="src/main.rs"
use std::str::FromStr;

use eyre::Result;
use iroha::client::{Client, ClientConfiguration};
use iroha_crypto::{KeyPair, PrivateKey};
use iroha_data_model::{
    isi::prelude::*,
    prelude::*,
    query::prelude::FindAccountAssets,
};

fn main() -> Result<()> {

    let admin_account = std::env::var("ADMIN_ACCOUNT").expect("export ADMIN_ACCOUNT");
    let receiver_account = std::env::var("RECEIVER_ACCOUNT").expect("export RECEIVER_ACCOUNT");
    let admin_private_key = std::env::var("ADMIN_PRIVATE_KEY").expect("export ADMIN_PRIVATE_KEY");

    let mut cfg = ClientConfiguration::test();
    cfg.torii_url = "http://127.0.0.1:8080".parse()?;
    cfg.chain = ChainId::from("00000000-0000-0000-0000-000000000000");
    cfg.account = AccountId::from_str(&admin_account)?;
    cfg.key_pair = KeyPair::from_private_key(PrivateKey::from_str(&admin_private_key)?)?;

    let client = Client::new(cfg)?;

    // 1) Register coffee#wonderland if it does not exist yet.
    let asset_definition_id = AssetDefinitionId::from_str("coffee#wonderland")?;
    client.submit_blocking(Register::asset_definition(
        AssetDefinition::numeric(asset_definition_id.clone()),
    ))?;

    // 2) Mint 250 units into the admin account.
    let admin_asset = AssetId::new(asset_definition_id.clone(), AccountId::from_str(&admin_account)?);
    client.submit_blocking(Mint::asset_numeric(250_u32, admin_asset.clone()))?;

    // 3) Transfer 50 units to the receiver.
    let receiver_id = AccountId::from_str(&receiver_account)?;
    client.submit_blocking(Transfer::asset_numeric(admin_asset.clone(), 50_u32, receiver_id.clone()))?;

    // 4) Query the receiver balance to confirm the transfer.
    let assets = client.request(&FindAccountAssets::new(receiver_id.clone()))?;
    println!("{} now holds:", receiver_id);
    for asset in assets {
        if asset.id().definition() == &asset_definition_id {
            println!("  {} units of {}", asset.value(), asset.id().definition());
        }
    }

    Ok(())
}
```

## 运行菜谱

```bash
cargo run
```

您应该看到类似于以下内容的日志输出：

```
ih58... now holds:
  50 units of coffee#wonderland
```

如果资产定义已存在，则寄存器调用将返回
`ValidationError::Duplicate`。要么忽略它（铸币厂仍然成功），要么选择
一个新名字。

## 验证哈希值和奇偶校验

- 使用 `iroha --config defaults/client.toml transaction get --hash <hash>` 来
  检查SDK提交的交易。
- 与 `iroha --config defaults/client.toml asset list all --table` 交叉检查余额
  或 `asset list filter '{"id":"norito:4e52543000000002"}'`。
- 重复 CLI 演练中的相同流程，以确认两个表面都产生
  相同的 Norito 有效负载和事务状态。