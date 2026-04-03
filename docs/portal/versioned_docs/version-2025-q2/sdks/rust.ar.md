---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# الصدأ SDK البدء السريع

توجد واجهة برمجة تطبيقات عميل Rust في صندوق `iroha`، مما يعرض `client::Client`
اكتب للتحدث إلى Torii. استخدامه عندما تحتاج إلى تقديم المعاملات،
الاشتراك في الأحداث، أو الاستعلام عن الحالة من تطبيق Rust.

## 1. أضف الصندوق

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

يفتح مثال مساحة العمل وحدة العميل عبر ميزة `client`. إذا كنت
استهلك الصندوق المنشور، واستبدل السمة `path` بالسمة الحالية
سلسلة الإصدار.

## 2. تكوين العميل

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

يعكس `ClientConfiguration` ملف تكوين CLI: فهو يتضمن Torii و
عناوين URL للقياس عن بعد، ومواد المصادقة، والمهلة، وتفضيلات التجميع.

## 3. أرسل المعاملة

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

تحت الغطاء، يستخدم العميل Norito لتشفير حمولة المعاملة قبل
نشره على Torii. إذا نجح الإرسال، فيمكن استخدام التجزئة التي تم إرجاعها
تتبع الحالة عبر `client.poll_transaction_status(hash)`.

## 4. إرسال النقط DA

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

عندما تحتاج إلى فحص الحمولة Norito أو استمرارها دون إرسالها إلى
Torii، اتصل بـ `client.build_da_ingest_request(...)` للحصول على الطلب الموقع
وجعله بتنسيق JSON/بايت، مما يعكس `iroha app da submit --no-submit`.

## 5. بيانات الاستعلام

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

تتبع الاستعلامات نمط الطلب/الاستجابة: أنشئ نوع استعلام من
`iroha_data_model::query`، وأرسله عبر `client.request`، ثم كرره عبر
النتائج. تستخدم الاستجابات JSON المدعومة بـ Norito، لذا فإن تنسيق السلك محدد.

## 6. اشترك في الأحداث

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

يكشف العميل عن التدفقات غير المتزامنة لنقاط نهاية SSE الخاصة بـ Torii، بما في ذلك خط الأنابيب
الأحداث وأحداث البيانات وخلاصات القياس عن بعد.

## المزيد من الأمثلة

- التدفقات من النهاية إلى النهاية تقع تحت `tests/` في `crates/iroha`. البحث عن التكامل
  اختبارات مثل `transaction_submission.rs` لسيناريوهات أكثر ثراءً.
- يستخدم CLI (`iroha_cli`) نفس وحدة العميل؛ تصفح
  `crates/iroha_cli/src/` لمعرفة كيفية المصادقة والتجميع وإعادة المحاولة
  التعامل معها في أدوات الإنتاج.
- ضع Norito في الاعتبار: لن يعود العميل أبدًا إلى `serde_json`. عندما
  توسيع SDK، والاعتماد على مساعدي `norito::json` لنقاط نهاية JSON و
  `norito::codec` للحمولات الثنائية.

باستخدام هذه العناصر الأساسية، يمكنك دمج Torii في خدمات Rust أو واجهات سطر الأوامر (CLI).
راجع الوثائق التي تم إنشاؤها وصناديق نماذج البيانات للحصول على المجموعة الكاملة من
التعليمات والاستفسارات والأحداث.