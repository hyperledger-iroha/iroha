---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2151bc561df9599865e2a5aa5d159171c1aa6f4830bfa51bd32d726f0c70a6f
source_last_modified: "2025-11-11T10:23:01.761192+00:00"
translation_last_reviewed: 2026-01-30
---

# البدء السريع لـ SDK Rust

تعيش واجهة عميل Rust في crate `iroha` التي توفر النوع `client::Client` للتواصل مع Torii. استخدمها عند الحاجة إلى إرسال معاملات، الاشتراك في الأحداث، أو استعلام الحالة من تطبيق Rust.

## 1. أضف crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

مثال الـ workspace يفعّل وحدة العميل عبر خاصية `client`. إذا كنت تستخدم crate المنشور، استبدل سمة `path` بسلسلة الإصدار الحالية.

## 2. اضبط العميل

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

`ClientConfiguration` يعكس ملف إعدادات CLI: يتضمن عناوين Torii والقياس عن بعد، بيانات المصادقة، المهلات وتفضيلات التجميع.

## 3. أرسل معاملة

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

تحت الغطاء يستخدم العميل Norito لترميز الـ payload قبل الإرسال إلى Torii. إذا نجح الإرسال، يمكن استخدام الهاش المعاد لتتبع الحالة عبر `client.poll_transaction_status(hash)`.

## 4. أرسل DA blobs

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

عندما تحتاج إلى فحص أو حفظ payload Norito دون إرساله إلى Torii، استدعِ `client.build_da_ingest_request(...)` للحصول على الطلب الموقّع وعرضه كـ JSON/bytes، مماثلًا لـ `iroha app da submit --no-submit`.

## 5. استعلم عن البيانات

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

تتبع الاستعلامات نمط request/response: أنشئ نوع الاستعلام من `iroha_data_model::query`، أرسله عبر `client.request` وكرر النتائج. تستخدم الردود JSON مدعومًا بـ Norito، لذا يكون تنسيق السلك حتميًا.

## 6. لقطات QR للمستكشف

```rust
use iroha::client::{
    AddressFormat, Client, ClientConfiguration, ExplorerAccountQrOptions,
};

fn download_qr() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let snapshot = client.get_explorer_account_qr(
        "ih58...",
        Some(ExplorerAccountQrOptions {
            address_format: Some(AddressFormat::Compressed),
        }),
    )?;
    println!("Canonical literal: {}", snapshot.literal);
    println!("SVG payload: {}", snapshot.svg);
    Ok(())
}
```

`ExplorerAccountQrSnapshot` يعكس JSON `/v1/explorer/accounts/{id}/qr`: يشمل معرف الحساب القياسي، الحرفية حسب التنسيق المطلوب، بيانات بادئة/تصحيح خطأ، أبعاد QR، وSVG داخلية يمكن تضمينها مباشرة. اترك `ExplorerAccountQrOptions` فارغًا لاستخدام مخرجات IH58 المفضلة أو اضبط `address_format: Some(AddressFormat::Compressed)` للحصول على متغير `sora…` المستخدم في ADDR-6b.

## 7. اشترك في الأحداث

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

يوفر العميل تدفقات غير متزامنة لنقاط SSE في Torii، بما في ذلك أحداث pipeline وأحداث البيانات وتغذيات القياس عن بعد.

## المزيد من الأمثلة

- توجد التدفقات الشاملة تحت `tests/` في `crates/iroha`. ابحث عن اختبارات تكامل مثل `transaction_submission.rs` لسيناريوهات أغنى.
- تستخدم CLI (`iroha_cli`) نفس وحدة العميل؛ راجع `crates/iroha_cli/src/` لمعرفة كيف تُدار المصادقة والتجميع وإعادات المحاولة في أدوات الإنتاج.
- ضع Norito في الاعتبار: العميل لا يعود إلى `serde_json` أبدًا. عند توسيع SDK، استخدم مساعدات `norito::json` لنهايات JSON و`norito::codec` للحمولات الثنائية.

## أمثلة Norito ذات صلة

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — يبني ويشغّل وينشر هيكل Kotodama الأدنى الذي يعكس مرحلة الإعداد في هذا الـ quickstart.
- [Register domain and mint assets](../norito/examples/register-and-mint) — يتوافق مع تدفق `Register` + `Mint` أعلاه لإعادة نفس العمليات من عقد ذكي.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — يعرض syscall `transfer_asset` بنفس معرّفات الحسابات المستخدمة في quickstarts الخاصة بالـ SDK.

بهذه اللبنات يمكنك دمج Torii في خدمات Rust أو أدوات CLI. راجع التوثيق المُولد وحِزم نموذج البيانات للحصول على مجموعة التعليمات والاستعلامات والأحداث الكاملة.
