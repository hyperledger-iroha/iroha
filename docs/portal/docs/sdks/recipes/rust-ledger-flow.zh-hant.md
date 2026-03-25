---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0865789ddc73146a5a6065c3326d7c6bde40d9136d51cfe32d244022df6b5cf
source_last_modified: "2026-01-22T16:26:46.513514+00:00"
translation_last_reviewed: 2026-02-07
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
slug: /sdks/recipes/rust-ledger-flow
translator: machine-google-reviewed
---

從“@site/src/components/SampleDownload”導入 SampleDownload；

此配方反映了 [CLI 賬本演練](../../norito/ledger-walkthrough.md)
但從 Rust 二進製文件運行所有內容。它重用默認的開發網絡
(`docker compose -f defaults/docker-compose.single.yml up --build`) 和演示
`defaults/client.toml` 中的憑據，因此您可以比較 SDK 和 CLI 哈希值
其一。

<樣本下載
  href="/sdk-recipes/rust/src/main.rs"
  文件名=“src/main.rs”
  description="使用此 Rust 源文件作為基線來跟踪或比較您的更改。"
/>

## 先決條件

1. 使用 Docker Compose 運行開發同級（請參閱 [Norito 快速入門](../../norito/quickstart.md)）。
2. 從以下位置導出默認管理員/接收者帳戶和管理員私鑰
   `defaults/client.toml`：

   ```bash
   export ADMIN_ACCOUNT="i105..."
   export RECEIVER_ACCOUNT="i105..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   私鑰字符串是存儲在 `[account].private_key` 下的多重哈希編碼值。
3. 創建一個新的工作區二進製文件（或重複使用現有的工作區二進製文件）：

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. 添加依賴項（如果您不在 crates.io 版本中，請使用 crates.io 版本）
   工作區）：

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

    // 1) Register 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ if it does not exist yet.
    let asset_definition_id = AssetDefinitionId::from_str("7Sp2j6zDvJFnMoscAiMaWbWHRDBZ")?;
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

## 運行菜譜

```bash
cargo run
```

您應該看到類似於以下內容的日誌輸出：

```
i105... now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

如果資產定義已存在，則寄存器調用將返回
`ValidationError::Duplicate`。要么忽略它（鑄幣廠仍然成功），要么選擇
一個新名字。

## 驗證哈希值和奇偶校驗

- 使用 `iroha --config defaults/client.toml transaction get --hash <hash>` 來
  檢查SDK提交的交易。
- 與 `iroha --config defaults/client.toml asset list all --table` 交叉檢查餘額
  或 `asset list filter '{"id":"norito:4e52543000000002"}'`。
- 重複 CLI 演練中的相同流程，以確認兩個表面都產生
  相同的 Norito 有效負載和事務狀態。