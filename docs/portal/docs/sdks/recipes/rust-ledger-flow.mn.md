---
lang: mn
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

SampleDownload-г '@site/src/components/SampleDownload'-аас импортлох;

Энэ жор нь [CLI дэвтэрийн дэлгэрэнгүй танилцуулга](../../norito/ledger-walkthrough.md)
гэхдээ Rust хоёртын файлаас бүгдийг ажиллуулдаг. Энэ нь анхдагч хөгжүүлэлтийн сүлжээг дахин ашигладаг
(`docker compose -f defaults/docker-compose.single.yml up --build`) болон демо
`defaults/client.toml` дахь итгэмжлэлүүд, ингэснээр та SDK болон CLI хэшүүдийг нэгээр нь харьцуулах боломжтой.
нэг нь.

<Жишээ татаж авах
  href="/sdk-recipes/rust/src/main.rs"
  файлын нэр = "src/main.rs"
  description="Өөрчлөлтүүдийг дагаж мөрдөх эсвэл өөрчлөхийн тулд энэ Rust эх файлыг үндсэн шугам болгон ашиглаарай."
/>

## Урьдчилсан нөхцөл

1. Docker Compose ашиглан dev peer-ийг ажиллуул ([Norito хурдан эхлүүлэх](../../norito/quickstart.md)-г үзнэ үү).
2. Анхдагч админ/хүлээн авагчийн бүртгэл болон админ хувийн түлхүүрийг эндээс экспортлох
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="ih58..."
   export RECEIVER_ACCOUNT="ih58..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   Хувийн түлхүүр мөр нь `[account].private_key` доор хадгалагдсан олон хэш кодлогдсон утга юм.
3. Ажлын талбарын шинэ хоёртын файл үүсгэх (эсвэл байгаа нэгийг нь дахин ашиглах):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. Хамааралуудыг нэмнэ үү (хэрэв та гаднаас байгаа бол crates.io хувилбарыг ашиглаарай
   ажлын талбар):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## Жишээ програм

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

## Жорыг ажиллуул

```bash
cargo run
```

Та дараахтай төстэй бүртгэлийн гаралтыг харах ёстой:

```
ih58... now holds:
  50 units of coffee#wonderland
```

Хэрэв хөрөнгийн тодорхойлолт аль хэдийн байгаа бол бүртгэлийн дуудлага нь a буцаана
`ValidationError::Duplicate`. Үүнийг үл тоомсорлох (гаа амжилттай хэвээр байна) эсвэл сонгох
шинэ нэр.

## Хэш болон паритетыг шалгана уу

- `iroha --config defaults/client.toml transaction get --hash <hash>` ашиглана уу
  SDK-ийн ирүүлсэн гүйлгээг шалгах.
- Үлдэгдлийг `iroha --config defaults/client.toml asset list all --table` ашиглан шалгана уу
  эсвэл `asset list filter '{"id":"coffee#wonderland##<account>"}'`.
- Хоёр гадаргуугийн үйлдвэрлэлийг баталгаажуулахын тулд CLI-ийн алхмаас ижил урсгалыг давт
  ижил Norito ачаалал болон гүйлгээний төлөв.