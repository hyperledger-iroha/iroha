---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dc6b5cf5cdf902e82ac1e33793558b52fb70dd2f4f9bdbd5c777f116b097b70e
source_last_modified: "2026-01-30T15:37:43+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Rust SDK کوئک اسٹارٹ

Rust کلائنٹ API `iroha` crate میں موجود ہے، جو Torii سے بات کرنے کے لیے `client::Client` ٹائپ فراہم کرتا ہے۔ جب آپ کو ٹرانزیکشنز سبمٹ کرنی ہوں، ایونٹس سبسکرائب کرنے ہوں یا Rust ایپ سے اسٹیٹ کوئری کرنا ہو تو اسے استعمال کریں۔

## 1. crate شامل کریں

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

workspace مثال `client` فیچر کے ذریعے کلائنٹ ماڈیول فعال کرتی ہے۔ اگر آپ پبلشڈ crate استعمال کرتے ہیں تو `path` کو موجودہ ورژن اسٹرنگ سے بدل دیں۔

## 2. کلائنٹ کنفیگر کریں

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

`ClientConfiguration` CLI کنفیگ فائل کی عکاسی کرتا ہے: اس میں Torii اور telemetry URLs، authentication مواد، timeouts اور batching ترجیحات شامل ہیں۔

## 3. ٹرانزیکشن سبمٹ کریں

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

اندرونی طور پر کلائنٹ Norito کے ذریعے payload انکوڈ کرتا ہے پھر Torii کو پوسٹ کرتا ہے۔ اگر سبمیشن کامیاب ہو تو واپس آنے والا ہیش `client.poll_transaction_status(hash)` کے ذریعے اسٹیٹس ٹریک کرنے کے لیے استعمال ہو سکتا ہے۔

## 4. DA blobs سبمٹ کریں

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

جب آپ کو Norito payload کو Torii پر بھیجے بغیر دیکھنا یا محفوظ کرنا ہو تو `client.build_da_ingest_request(...)` کال کریں تاکہ signed request حاصل ہو اور اسے JSON/bytes کے طور پر رینڈر کیا جا سکے، جیسے `iroha app da submit --no-submit`.

## 5. ڈیٹا کوئری کریں

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

کوئریز request/response پیٹرن کی پیروی کرتی ہیں: `iroha_data_model::query` سے کوئری ٹائپ بنائیں، `client.request` کے ذریعے بھیجیں اور نتائج پر iterate کریں۔ جوابات Norito‑backed JSON استعمال کرتے ہیں، اس لیے wire فارمیٹ deterministic رہتا ہے۔

## 6. Explorer QR snapshots

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "<katakana-i105-account-id>",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` `/v1/explorer/accounts/{id}/qr` JSON کی عکاسی کرتا ہے: اس میں canonical Katakana i105 account id، معیاری i105 literal، نیٹ ورک prefix/error-correction میٹاڈیٹا، QR dimensions، اور inline SVG payload شامل ہوتا ہے جسے wallets/explorers براہ راست embed کر سکتے ہیں۔

## 7. ایونٹس سبسکرائب کریں

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

کلائنٹ Torii کے SSE endpoints کے لیے async streams فراہم کرتا ہے، جن میں pipeline events، data events، اور telemetry feeds شامل ہیں۔

## مزید مثالیں

- End-to-end فلو `crates/iroha` میں `tests/` کے تحت موجود ہیں۔ مزید بھرپور منظرناموں کے لیے `transaction_submission.rs` جیسے integration tests تلاش کریں۔
- CLI (`iroha_cli`) اسی کلائنٹ ماڈیول کا استعمال کرتا ہے؛ `crates/iroha_cli/src/` دیکھیں تاکہ authentication، batching، اور retries کی production handling سمجھ آ سکے۔
- Norito ذہن میں رکھیں: کلائنٹ کبھی `serde_json` پر واپس نہیں جاتا۔ SDK کو بڑھاتے وقت JSON endpoints کے لیے `norito::json` اور binary payloads کے لیے `norito::codec` استعمال کریں۔

## متعلقہ Norito مثالیں

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — کمپائل، رن، اور کم سے کم Kotodama scaffold ڈپلائے کریں جو اس quickstart کے setup مرحلے کی عکاسی کرتا ہے۔
- [Register domain and mint assets](../norito/examples/register-and-mint) — اوپر دکھائے گئے `Register` + `Mint` فلو سے ہم آہنگ ہے تاکہ وہی آپریشنز ایک کنٹریکٹ سے دہرائے جا سکیں۔
- [Transfer asset between accounts](../norito/examples/transfer-asset) — وہی account IDs استعمال کرتے ہوئے `transfer_asset` syscall دکھاتا ہے جو SDK quickstarts میں استعمال ہوتے ہیں۔

ان بلڈنگ بلاکس کے ساتھ آپ Torii کو Rust سروسز یا CLIs میں ضم کر سکتے ہیں۔ مکمل ہدایات، کوئریز اور ایونٹس کے لیے generated documentation اور data-model crates سے رجوع کریں۔
