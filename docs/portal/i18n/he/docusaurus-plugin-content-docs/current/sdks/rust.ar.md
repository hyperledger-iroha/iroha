---
lang: ar
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 898147204572ac59f1a0e592e73ef12ec4f714979528f3aa8cdd29fe84847954
source_last_modified: "2026-01-30T15:37:43+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# מדריך התחלה מהירה ל‑SDK Rust

ממשק הלקוח ב‑Rust נמצא ב‑crate `iroha`, שמספק את הטיפוס `client::Client` לתקשורת עם Torii. השתמשו בו כשצריך לשלוח טרנזקציות, להירשם לאירועים או לשאול מצב מאפליקציית Rust.

## 1. הוסיפו את ה‑crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

דוגמת ה‑workspace מפעילה את מודול הלקוח באמצעות feature `client`. אם אתם משתמשים ב‑crate שפורסם, החליפו את מאפיין `path` במחרוזת הגרסה העדכנית.

## 2. הגדירו את הלקוח

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

`ClientConfiguration` משקף את קובץ התצורה של ה‑CLI: הוא כולל כתובות Torii וטלמטריה, חומרי אימות, timeouts והעדפות batching.

## 3. שלחו טרנזקציה

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

מתחת למכסה המנוע הלקוח משתמש ב‑Norito לקידוד ה‑payload לפני שליחה ל‑Torii. אם השליחה מצליחה, ניתן להשתמש בהאש שהוחזר כדי לעקוב אחר סטטוס דרך `client.poll_transaction_status(hash)`.

## 4. שלחו DA blobs

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

כאשר צריך לבדוק או לשמור payload של Norito בלי לשלוח ל‑Torii, קראו ל‑`client.build_da_ingest_request(...)` כדי לקבל בקשה חתומה ולהציג אותה כ‑JSON/bytes, בדומה ל‑`iroha app da submit --no-submit`.

## 5. שאילת נתונים

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

שאילתות פועלות לפי request/response: בנו סוג שאילתה מ‑`iroha_data_model::query`, שלחו דרך `client.request` ועברו על התוצאות. התגובות משתמשות ב‑JSON מבוסס Norito, כך שהפורמט על החוט דטרמיניסטי.

## 6. Snapshots QR ל‑Explorer

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "soraカタカナ...",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` משקף את ה‑JSON `/v1/explorer/accounts/{id}/qr`: כולל מזהה חשבון קנוני, literal קנוני I105, מטא‑נתוני prefix/תיקון שגיאות, ממדי QR ו‑SVG inline שניתן להטמיע ישירות.

## 7. הירשמו לאירועים

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

הלקוח חושף זרמים async ל‑SSE endpoints של Torii, כולל pipeline events, data events ו‑telemetry feeds.

## דוגמאות נוספות

- זרימות end‑to‑end נמצאות תחת `tests/` ב‑`crates/iroha`. חפשו בדיקות אינטגרציה כגון `transaction_submission.rs` לתרחישים עשירים יותר.
- ה‑CLI (`iroha_cli`) משתמש באותו מודול לקוח; עיינו ב‑`crates/iroha_cli/src/` כדי לראות כיצד מתבצעים אימות, batching ו‑retries בכלים פרודקשן.
- זכרו את Norito: הלקוח לעולם לא נופל חזרה ל‑`serde_json`. בעת הרחבת ה‑SDK, השתמשו ב‑`norito::json` עבור endpoints JSON וב‑`norito::codec` עבור payloads בינאריים.

## דוגמאות Norito קשורות

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — קומפל, הריץ ופרוס את שלד Kotodama המינימלי שמשקף את שלב ההקמה של מדריך זה.
- [Register domain and mint assets](../norito/examples/register-and-mint) — מתיישר עם הזרימה `Register` + `Mint` לעיל כדי לשחזר את אותן פעולות מתוך חוזה.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — מדגים את syscall `transfer_asset` עם אותם מזהי חשבון שבהם משתמשים מדריכי ה‑SDK.

בעזרת אבני הבניין האלה ניתן לשלב את Torii בשירותי Rust או CLI. עיינו בתיעוד שנוצר וב‑crates של מודל הנתונים עבור סט מלא של הוראות, שאילתות ואירועים.
