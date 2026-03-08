---
slug: /sdks/recipes/rust-ledger-flow
lang: ba
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

импорт SapleDownload '@site/src/компоненттар/SampleDownload';

Был рецепт көҙгө [CLI леджер проходка] (../../norito/ledger-walkthrough.md X)
әммә барыһын да йүгерә, тип, руст бинар. Ул ҡабаттан ҡулланыла default de de de dev .
(`docker compose -f defaults/docker-compose.single.yml up --build`) һәм демо
ышаныс ҡағыҙҙары `defaults/client.toml`, шуға күрә һеҙ SDK һәм CLI хеш сағыштырырға мөмкин бер
берәүһе өсөн.

<СэмплДау-лог
  href="/sdk-рецепттар/көсһөҙ/src/main.rs".
  файл исеме="src/main.rs".
  Һеҙҙең үҙгәрештәргә ҡаршы йәки айырмалы рәүештә, был Rust сығанаҡ файлын ҡулланыу.
/>

## Алдан шарттар

1. Йүгерергә dev менән I18NT000000002X Композа (ҡара: [I18NT000000000X faverstart](../../norito/quickstart.md)).
2. Экспорт ғәҙәттәге админ/ҡабул итеүсе иҫәп-хисап һәм админ шәхси асҡыс .
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="ih58..."
   export RECEIVER_ACCOUNT="ih58..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   Шәхси асҡыс ептәре булып тора, күп хеш-кодланған ҡиммәт һаҡланған I18NI000000014X.
3. Яңы эш урыны бинар төҙөү (йәки ғәмәлдәгеһен ҡабаттан ҡулланырға):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Өҫтәү менән бәйлелек (ҡулланыу crate.io версияһы, әгәр һеҙ ситтә .
   эш урыны):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Миҫал программаһы

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

## Рецепты йүгерегеҙ

```bash
cargo run
```

Һеҙ күрергә тейеш лог сығыш оҡшаш:

```
ih58... now holds:
  50 units of coffee#wonderland
```

Әгәр актив билдәләмәһе инде бар, регистр шылтыратыу ҡайтарыу а .
`ValidationError::Duplicate`. Әллә уны иғтибарға алмай (метка һаман да уңышҡа өлгәшә) йәки һайлап алыу
яңы исем.

## Хеш һәм паритет раҫлау

- I18NI000000016X тиклем ҡулланыу.
  тикшерергә операциялар, тип SDK тапшырҙы.
- I18NI000000017X менән крест-тикшерергә
  йәки `asset list filter '{"id":"norito:4e52543000000002"}'`.
- CLI проходкаһынан бер үк ағымды ҡабатлағыҙ, ике ер өҫтөн дә етештереүҙе раҫлау өсөн
  шул уҡ I18NT000000001X файҙалы йөктәр һәм транзакция статусы.