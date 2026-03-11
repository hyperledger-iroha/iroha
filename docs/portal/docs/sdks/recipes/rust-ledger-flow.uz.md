---
lang: uz
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

SampleDownloadni '@site/src/components/SampleDownload'dan import qilish;

Ushbu retsept [CLI ledger walkthrough](../../norito/ledger-walkthrough.md)ni aks ettiradi
lekin hamma narsani Rust ikkilik faylidan boshqaradi. U standart ishlab chiqaruvchi tarmog'idan qayta foydalanadi
(`docker compose -f defaults/docker-compose.single.yml up --build`) va demo
`defaults/client.toml` da hisob ma'lumotlari, shuning uchun siz SDK va CLI xeshlarini solishtirishingiz mumkin
biri uchun.

<Namunani yuklab olish
  href="/sdk-recipes/rust/src/main.rs"
  fayl nomi = "src/main.rs"
  description="Ushbu Rust manba faylini kuzatish yoki oʻzgartirishlaringizdan farq qilish uchun asos sifatida foydalaning."
/>

## Old shartlar

1. Docker Compose bilan ishlab peerni ishga tushiring (qarang: [Norito tezkor boshlash](../../norito/quickstart.md)).
2. Standart administrator/qabul qiluvchi hisoblarini va administrator shaxsiy kalitini eksport qiling
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="i105..."
   export RECEIVER_ACCOUNT="i105..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   Shaxsiy kalit qatori `[account].private_key` ostida saqlanadigan multixesh-kodlangan qiymatdir.
3. Yangi ish maydoni ikkilik faylini yarating (yoki mavjudidan qayta foydalaning):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Bog'liqlarni qo'shing (agar siz tashqarida bo'lsangiz crates.io versiyasidan foydalaning
   ish maydoni):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Namuna dastur

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

## Retseptni ishga tushiring

```bash
cargo run
```

Quyidagiga o'xshash jurnal chiqishini ko'rishingiz kerak:

```
i105... now holds:
  50 units of coffee#wonderland
```

Agar aktiv ta'rifi allaqachon mavjud bo'lsa, ro'yxatga olish chaqiruvi a ni qaytaradi
`ValidationError::Duplicate`. Yoki buni e'tiborsiz qoldiring (yalpiz hali ham muvaffaqiyatli) yoki tanlang
yangi nom.

## Xesh va paritetni tekshiring

- `iroha --config defaults/client.toml transaction get --hash <hash>` dan foydalaning
  SDK taqdim etgan tranzaktsiyalarni tekshiring.
- `iroha --config defaults/client.toml asset list all --table` bilan balanslarni o'zaro tekshirish
  yoki `asset list filter '{"id":"norito:4e52543000000002"}'`.
- Ikkala sirt ishlab chiqarishni tasdiqlash uchun CLI-dan bir xil oqimni takrorlang
  bir xil Norito foydali yuklar va tranzaksiya holatlari.