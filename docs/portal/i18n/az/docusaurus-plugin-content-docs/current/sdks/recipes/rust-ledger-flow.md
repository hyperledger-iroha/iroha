---
slug: /sdks/recipes/rust-ledger-flow
lang: az
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust ledger flow recipe
description: Use the Rust SDK to register an asset, mint supply, transfer it, and query balances against the default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-ı '@site/src/components/SampleDownload'dan idxal edin;

Bu resept [CLI ledger walkthrough](../../norito/ledger-walkthrough.md) əks etdirir
lakin Rust binar sistemindən hər şeyi idarə edir. Defolt inkişaf şəbəkəsini yenidən istifadə edir
(`docker compose -f defaults/docker-compose.single.yml up --build`) və demo
`defaults/client.toml`-də etimadnamələr, beləliklə siz SDK və CLI hash-lərini müqayisə edə bilərsiniz
biri üçün.

<Nümunə Yüklə
  href="/sdk-recipes/rust/src/main.rs"
  fayl adı = "src/main.rs"
  description="Dəyişikliklərinizi izləmək və ya onlara qarşı çıxmaq üçün bu Rust mənbə faylını əsas kimi istifadə edin."
/>

## İlkin şərtlər

1. Docker Compose ilə dev peer-i işə salın (bax [Norito sürətli başlanğıc](../../norito/quickstart.md)).
2. Defolt admin/qəbuledici hesablarını və admin şəxsi açarını ixrac edin
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="ih58..."
   export RECEIVER_ACCOUNT="ih58..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   Şəxsi açar sətri `[account].private_key` altında saxlanılan multihash kodlu dəyərdir.
3. Yeni ikili iş sahəsi yaradın (və ya mövcud olanı yenidən istifadə edin):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Asılılıqları əlavə edin (əgər xaricindəsinizsə crates.io versiyasından istifadə edin
   iş sahəsi):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Nümunə proqram

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

## Resepti işlədin

```bash
cargo run
```

Siz aşağıdakılara bənzər jurnal çıxışını görməlisiniz:

```
ih58... now holds:
  50 units of coffee#wonderland
```

Aktiv tərifi artıq mövcuddursa, qeydiyyat çağırışı a qaytarır
`ValidationError::Duplicate`. Ya buna məhəl qoymayın (nanə hələ də uğur qazanır) və ya seçin
yeni ad.

## Haşları və pariteti yoxlayın

- `iroha --config defaults/client.toml transaction get --hash <hash>` istifadə edin
  SDK-nın təqdim etdiyi əməliyyatları yoxlayın.
- `iroha --config defaults/client.toml asset list all --table` ilə balansları çarpaz yoxlayın
  və ya `asset list filter '{"id":"coffee#wonderland##<account>"}'`.
- Hər iki səthin istehsalını təsdiqləmək üçün CLI gedişindən eyni axını təkrarlayın
  eyni Norito faydalı yüklər və əməliyyat statusları.