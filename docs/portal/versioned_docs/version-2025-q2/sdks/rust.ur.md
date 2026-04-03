---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# مورچا SDK کوئیک اسٹارٹ

مورچا کلائنٹ API `iroha` کریٹ میں رہتا ہے ، جو `client::Client` کو بے نقاب کرتا ہے
Torii سے بات کرنے کے لئے ٹائپ کریں۔ جب آپ کو لین دین پیش کرنے کی ضرورت ہو تو اسے استعمال کریں ،
واقعات کو سبسکرائب کریں ، یا زنگ آلود درخواست سے ریاست سے استفسار کریں۔

## 1. کریٹ شامل کریں

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

ورک اسپیس کی مثال `client` خصوصیت کے ذریعے کلائنٹ کے ماڈیول کو کھولتی ہے۔ اگر آپ
شائع شدہ کریٹ کھائیں ، `path` وصف کو موجودہ کے ساتھ تبدیل کریں
ورژن کی تار.

## 2. مؤکل کو تشکیل دیں

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

`ClientConfiguration` CLI کنفیگریشن فائل کی آئینہ دار ہے: اس میں Torii اور شامل ہے
ٹیلی میٹری یو آر ایل ، توثیق کا مواد ، ٹائم آؤٹ ، اور بیچنگ ترجیحات۔

## 3. ٹرانزیکشن جمع کروائیں

```rust
use iroha::client::{Client, ClientConfiguration};
use iroha_data_model::{
    isi::prelude::*,
    prelude::{AccountId, ChainId, Domain, DomainId},
};
use iroha_crypto::KeyPair;

fn submit_example() -> eyre::Result<()> {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let key_pair = KeyPair::generate_ed25519(); // replace with a persistent key in real apps
    let account_id = AccountId::new(key_pair.public_key().clone());

    let cfg = ClientConfiguration {
        chain: chain_id.clone(),
        account: account_id.clone(),
        key_pair: key_pair.clone(),
        ..ClientConfiguration::test()
    };

    let client = Client::new(cfg)?;

    let instruction = Register::domain(Domain::new(DomainId::try_new("research", "universal")?));

    let tx = client.build_transaction([instruction]);
    let signed = tx.sign(&key_pair)?;
    let hash = client.submit_transaction(&signed)?;
    println!("Submitted transaction: {hash}");
    Ok(())
}
```

ہڈ کے تحت کلائنٹ اس سے پہلے ٹرانزیکشن پے لوڈ کو انکوڈ کرنے کے لئے Norito استعمال کرتا ہے
اسے Torii پر پوسٹ کرنا۔ اگر جمع کرانے میں کامیاب ہوجاتا ہے تو ، واپسی ہیش کا استعمال کیا جاسکتا ہے
`client.poll_transaction_status(hash)` کے ذریعے حیثیت کو ٹریک کریں۔

## 4. ڈی اے بلبس جمع کروائیں

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

جب آپ کو Norito پے لوڈ کو بھیجے بغیر معائنہ کرنے یا برقرار رکھنے کی ضرورت ہے
Torii ، دستخط شدہ درخواست حاصل کرنے کے لئے `client.build_da_ingest_request(...)` پر کال کریں
اور اسے JSON/بائٹس کے طور پر پیش کریں ، `iroha app da submit --no-submit` کی آئینہ دار۔

## 5۔ استفسار کا ڈیٹا

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

سوالات درخواست/رسپانس کے نمونہ کی پیروی کریں: استفسار کی قسم بنائیں
`iroha_data_model::query` ، اسے `client.request` کے ذریعے بھیجیں ، اور اس پر تکرار کریں
نتائج جوابات Norito کی حمایت یافتہ JSON استعمال کرتے ہیں ، لہذا تار کی شکل عین مطابق ہے۔

## 6. واقعات کو سبسکرائب کریں

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

کلائنٹ Torii کے SSE اختتامی مقامات کے لئے Async اسٹریمز کو بے نقاب کرتا ہے ، جس میں پائپ لائن بھی شامل ہے
واقعات ، ڈیٹا کے واقعات ، اور ٹیلی میٹری فیڈز۔

## مزید مثالیں

-`crates/iroha` میں `tests/` کے تحت اختتام سے آخر میں بہاؤ۔ انضمام کے لئے تلاش کریں
  امیر منظرناموں کے لئے `transaction_submission.rs` جیسے ٹیسٹ۔
- CLI (`iroha_cli`) ایک ہی کلائنٹ ماڈیول کا استعمال کرتا ہے۔ براؤز کریں
  `crates/iroha_cli/src/` یہ دیکھنے کے لئے کہ توثیق ، ​​بیچنگ ، ​​اور دوبارہ کوششیں کس طرح ہیں
  پروڈکشن ٹولنگ میں سنبھالا۔
- Norito کو ذہن میں رکھیں: مؤکل کبھی بھی `serde_json` پر واپس نہیں آتا ہے۔ جب آپ
  ایس ڈی کے میں توسیع کریں ، JSON اختتامی مقامات کے لئے `norito::json` مددگاروں پر انحصار کریں اور
  بائنری پے لوڈ کے لئے `norito::codec`۔

ان بلڈنگ بلاکس کے ذریعہ آپ Torii کو مورچا خدمات یا CLIs میں ضم کرسکتے ہیں۔
مکمل سیٹ کے لئے تیار کردہ دستاویزات اور ڈیٹا ماڈل کریٹس کا حوالہ دیں
ہدایات ، سوالات اور واقعات۔