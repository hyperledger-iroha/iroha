---
lang: he
direction: rtl
source: docs/portal/docs/sdks/recipes/rust-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3e1cd7a1ec89819f8f3c7916774e07b2c467fcd53381c8629c92ebc86abc6d73
source_last_modified: "2025-11-11T10:23:19.175496+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: מתכון זרימת לדג'ר ב-Rust
description: השתמשו ב-SDK של Rust כדי לרשום נכס, להטביע היצע, להעביר אותו ולשאול יתרות מול רשת ברירת המחדל עם עמית אחד.
slug: /sdks/recipes/rust-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

המתכון הזה משקף את [סיור הלדג'ר ב-CLI](../../norito/ledger-walkthrough.md) אבל מריץ הכל מבינארי Rust. הוא עושה שימוש חוזר ברשת הפיתוח ברירת המחדל (`docker compose -f defaults/docker-compose.single.yml up --build`) ובפרטי הדמו ב-`defaults/client.toml`, כדי שתוכלו להשוות האשים בין SDK ל-CLI אחד לאחד.

<SampleDownload
  href="/sdk-recipes/rust/src/main.rs"
  filename="src/main.rs"
  description="השתמשו בקובץ המקור הזה של Rust כבסיס למעקב או להשוואה מול השינויים שלכם."
/>

## דרישות מקדימות

1. הפעילו את עמית הפיתוח באמצעות Docker Compose (ראו [ה-quickstart של Norito](../../norito/quickstart.md)).
2. ייצאו את חשבונות ה-admin/receiver ברירת המחדל ואת המפתח הפרטי של admin מתוך
   `defaults/client.toml`:

   ```bash
   export ADMIN_ACCOUNT="<katakana-i105-account-id>"
   export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
   export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
   ```

   מחרוזת המפתח הפרטי היא הערך המקודד ב-multihash המאוחסן תחת `[account].private_key`.
3. צרו בינארי workspace חדש (או השתמשו בקיים):

   ```bash
   cargo new --bin rust-ledger-recipe
   cd rust-ledger-recipe
   ```

4. הוסיפו את התלויות (השתמשו בגרסת crates.io אם אתם מחוץ ל-workspace):

   ```toml title="Cargo.toml"
   [dependencies]
   eyre = "0.6"
   iroha = { path = "../../crates/iroha", features = ["client"] }
   iroha_crypto = { path = "../../crates/iroha_crypto" }
   iroha_data_model = { path = "../../crates/iroha_data_model", features = ["transparent_api", "json"] }
   ```

## תוכנית לדוגמה

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

## הרצת המתכון

```bash
cargo run
```

אתם אמורים לראות פלט דומה ל:

```
<katakana-i105-account-id> now holds:
  50 units of 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

אם הגדרת הנכס כבר קיימת, קריאת הרישום מחזירה `ValidationError::Duplicate`. התעלמו מכך (ההטבעה עדיין מצליחה) או בחרו שם חדש.

## אימות האשים ופריטי

- השתמשו ב-`iroha --config defaults/client.toml transaction get --hash <hash>` כדי לבדוק את העסקאות שה-SDK שלח.
- השוו יתרות עם `iroha --config defaults/client.toml asset list all --table` או `asset list filter '{"id":"norito:4e52543000000002"}'`.
- חזרו על אותו זרם מה-walkthrough של ה-CLI כדי לאשר ששתי הדרכים מפיקות את אותם payloads Norito ואת אותם סטטוסי טרנזקציה.

