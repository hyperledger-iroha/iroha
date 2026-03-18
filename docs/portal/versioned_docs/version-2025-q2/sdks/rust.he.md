---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# התחלה מהירה של Rust SDK

ה-API של הלקוח של Rust חי בארגז `iroha`, אשר חושף `client::Client`
הקלד כדי לדבר עם Torii. השתמש בו כאשר אתה צריך לשלוח עסקאות,
הירשם לאירועים או מצב שאילתה מאפליקציית Rust.

## 1. הוסף את הארגז

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

דוגמה של סביבת העבודה פותחת את מודול הלקוח באמצעות תכונת `client`. אם אתה
לצרוך את הארגז שפורסם, להחליף את התכונה `path` בתכונה הנוכחית
מחרוזת גרסה.

## 2. הגדר את הלקוח

```rust title="src/main.rs"
use iroha::client::{Client, ClientConfiguration};

fn main() -> eyre::Result<()> {
    let cfg = ClientConfiguration {
        torii_url: "http://127.0.0.1:8080".parse()?,
        telemetry_url: Some("http://127.0.0.1:8080".parse()?),
        // account_id, key_pair and other options can be populated here or via helper builders
        ..ClientConfiguration::default()
    };

    let client = Client::new(cfg)?;
    println!("Node status: {:?}", client.get_status()?);
    Ok(())
}
```

`ClientConfiguration` משקף את קובץ התצורה של CLI: הוא כולל Torii ו
כתובות URL של טלמטריה, חומרי אימות, פסק זמן והעדפות אצווה.

## 3. שלח עסקה

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::{
    isi::prelude::*,
    prelude::{AccountId, ChainId, DomainId, Name},
};
use iroha_crypto::{KeyPair, PublicKey};

fn submit_example() -> eyre::Result<()> {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let account_id = AccountId::new(
        Name::from_str("alice")?,
        DomainId::from_str("wonderland")?,
    );

    let key_pair = KeyPair::generate_ed25519(); // replace with a persistent key in real apps

    let cfg = ClientConfiguration {
        chain: chain_id.clone(),
        account: account_id.clone(),
        key_pair: key_pair.clone(),
        ..ClientConfiguration::test()
    };

    let client = Client::new(cfg)?;

    let instruction = Register {
        object: Domain::new(Name::from_str("research")?, None),
    };

    let tx = client.build_transaction([instruction]);
    let signed = tx.sign(&key_pair)?;
    let hash = client.submit_transaction(&signed)?;
    println!("Submitted transaction: {hash}");
    Ok(())
}
```

מתחת למכסה המנוע המשתמש משתמש ב-Norito כדי לקודד את מטען העסקה לפני
מפרסם אותו ב-Torii. אם ההגשה תצליח, ניתן להשתמש ב-hash המוחזר
עקוב אחר מצב באמצעות `client.poll_transaction_status(hash)`.

## 4. שלח כתמי DA

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha::da::DaIngestParams;
use iroha_data_model::{da::types::ExtraMetadata, nexus::LaneId};

fn submit_da_blob() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let mut params = DaIngestParams::default();
    params.lane_id = LaneId::new(7);
    params.epoch = 42;
    let payload = std::fs::read("payload.car")?;
    let metadata = ExtraMetadata::default();
    let result = client.submit_da_blob(payload, &params, metadata, None)?;
    println!(
        "status={} duplicate={} bytes={}",
        result.status, result.duplicate, result.payload_len
    );
    Ok(())
}
```

כאשר אתה צריך לבדוק או להתמיד במטען Norito מבלי לשלוח אותו אל
Torii, התקשר ל-`client.build_da_ingest_request(...)` כדי לקבל את הבקשה החתומה
ולעבד אותו כ-JSON/bytes, תוך שיקוף של `iroha app da submit --no-submit`.

## 5. נתוני שאילתה

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::query::prelude::*;

fn list_domains() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let response = client.request(&FindAllDomains::new())?;
    for domain in response {
        println!("{}", domain.name());
    }
    Ok(())
}
```

שאילתות עוקבות אחר דפוס הבקשה/תגובה: בניית סוג שאילתה מ
`iroha_data_model::query`, שלח אותו דרך `client.request`, וחזור על
תוצאות. התגובות משתמשות ב-JSON מגובה Norito, כך שפורמט החוט הוא דטרמיניסטי.

## 6. הירשם לאירועים

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::events::pipeline::PipelineEventFilterBox;
use futures_lite::stream::StreamExt;

async fn listen_for_blocks() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let mut stream = client
        .listen_for_events([PipelineEventFilterBox::any()])
        .await?;

    while let Some(event) = stream.next().await {
        println!("Received event: {:?}", event?);
    }
    Ok(())
}
```

הלקוח חושף זרמים אסינכרוניים עבור נקודות הקצה SSE של Torii, כולל צינור
אירועים, אירועי נתונים והזנות טלמטריה.

## דוגמאות נוספות

- זרימות מקצה לקצה חיות תחת `tests/` ב-`crates/iroha`. חפש אינטגרציה
  בדיקות כגון `transaction_submission.rs` עבור תרחישים עשירים יותר.
- ה-CLI (`iroha_cli`) משתמש באותו מודול לקוח; לדפדף
  `crates/iroha_cli/src/` כדי לראות איך אימות, אצווה וניסיונות חוזרים
  מטופל בכלי ייצור.
- זכור את Norito: הלקוח לעולם לא יחזור ל-`serde_json`. כאשר אתה
  להרחיב את ה-SDK, להסתמך על עוזרי `norito::json` עבור נקודות קצה של JSON ו
  `norito::codec` עבור מטענים בינאריים.

בעזרת אבני הבניין הללו תוכלו לשלב את Torii בשירותי Rust או CLI.
עיין בתיעוד שנוצר ובארגזי מודל הנתונים לקבלת הסט המלא של
הוראות, שאילתות ואירועים.