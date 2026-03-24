---
lang: es
direction: ltr
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3e1cd7a1ec89819f8f3c7916774e07b2c467fcd53381c8629c92ebc86abc6d73
source_last_modified: "2025-11-11T10:23:19.175496+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: رسٹ لیجر فلو ترکیب
description: Rust SDK استعمال کریں تاکہ اثاثہ رجسٹر کیا جا سکے، سپلائی منٹ ہو، ٹرانسفر ہو اور ڈیفالٹ سنگل-پیئر نیٹ ورک پر بیلنس معلوم ہو سکے۔
slug: /sdks/recipes/rust-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

یہ ترکیب [CLI لیجر واک تھرو](../../norito/ledger-walkthrough.md) کی عکاسی کرتی ہے مگر سب کچھ Rust بائنری سے چلاتی ہے۔ یہ ڈیفالٹ ڈیولپمنٹ نیٹ ورک (`docker compose -f defaults/docker-compose.single.yml up --build`) اور `defaults/client.toml` میں موجود ڈیمو کریڈینشلز استعمال کرتی ہے تاکہ آپ SDK اور CLI کے ہیشز کو ایک جیسا موازنہ کر سکیں۔

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="اس Rust سورس فائل کو بیس لائن کے طور پر استعمال کریں تاکہ ساتھ ساتھ چل سکیں یا اپنی تبدیلیوں سے موازنہ کر سکیں۔"
/>

## پیشگی تقاضے

1. Docker Compose کے ذریعے ڈیولپمنٹ پیئر چلائیں ( [Norito quickstart](../../norito/quickstart.md) دیکھیں ).
2. ڈیفالٹ admin/receiver اکاؤنٹس اور admin پرائیویٹ کی برآمد کریں
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="i105..."
   export RECEIVER_ACCOUNT="i105..."
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   پرائیویٹ کی سٹرنگ `[account].private_key` کے تحت محفوظ multihash انکوڈڈ ویلیو ہے۔
3. نیا workspace بائنری بنائیں (یا موجودہ کو استعمال کریں):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. ڈپینڈنسیز شامل کریں (اگر آپ workspace سے باہر ہیں تو crates.io ورژن استعمال کریں):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## مثالی پروگرام

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

## ترکیب چلائیں

```bash
cargo run
```

آپ کو کچھ اس طرح کا لاگ آؤٹ پٹ نظر آنا چاہیے:

```
i105... now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

اگر اثاثے کی تعریف پہلے سے موجود ہو تو رجسٹر کال `ValidationError::Duplicate` واپس کرتی ہے۔ اسے نظر انداز کریں (منٹ پھر بھی کامیاب ہوتا ہے) یا نیا نام منتخب کریں۔

## ہیشز اور برابری کی تصدیق

- `iroha --config defaults/client.toml transaction get --hash <hash>` استعمال کر کے SDK کی بھیجی گئی ٹرانزیکشنز دیکھیں۔
- `iroha --config defaults/client.toml asset list all --table` یا `asset list filter '{"id":"norito:4e52543000000002"}'` سے بیلنس کا موازنہ کریں۔
- CLI walkthrough سے وہی فلو دہرائیں تاکہ تصدیق ہو سکے کہ دونوں سطحیں ایک جیسے Norito payloads اور ٹرانزیکشن اسٹیٹس بناتی ہیں۔
