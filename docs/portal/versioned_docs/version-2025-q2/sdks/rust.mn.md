---
lang: mn
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T14:35:36.896251+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK хурдан эхлүүлэх

Rust клиентийн API нь `iroha` хайрцагт амьдардаг бөгөөд энэ нь `client::Client`-г ил гаргадаг.
Torii-тэй ярих гэж бичнэ үү. Гүйлгээ хийх шаардлагатай үед үүнийг ашиглах,
үйл явдалд бүртгүүлэх эсвэл Rust програмаас төлөвийг асуух.

## 1. Хайрцаг нэмнэ

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Ажлын талбарын жишээ нь `client` функцээр дамжуулан үйлчлүүлэгчийн модулийг нээдэг. Хэрэв та
нийтлэгдсэн хайрцгийг ашиглах, `path` шинж чанарыг одоогийнхоор солих
хувилбарын мөр.

## 2. Үйлчлүүлэгчийн тохиргоог хийнэ үү

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

`ClientConfiguration` CLI тохиргооны файлыг тусгадаг: үүнд Torii болон
телеметрийн URL, баталгаажуулалтын материал, завсарлага, багцын сонголт.

## 3. Гүйлгээ илгээх

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

Бүрээсний доор үйлчлүүлэгч Norito-г ашиглан гүйлгээний ачааллыг кодчилдог.
Torii дээр нийтэлж байна. Илгээлт амжилттай бол буцаасан хэшийг ашиглаж болно
`client.poll_transaction_status(hash)`-ээр дамжуулан статусыг хянах.

## 4. DA blob илгээнэ үү

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

Та Norito ачааг илгээхгүйгээр шалгах эсвэл үргэлжлүүлэх шаардлагатай үед
Torii, гарын үсэг зурсан хүсэлтийг авахын тулд `client.build_da_ingest_request(...)` руу залгана уу
мөн `iroha app da submit --no-submit` толин тусгал болгон JSON/байт хэлбэрээр үзүүлээрэй.

## 5. Дата асуулга

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

Асуултууд нь хүсэлт/хариултын загварыг дагадаг: -аас асуулгын төрлийг үүсгэнэ
`iroha_data_model::query`, `client.request`-ээр илгээж, давталт хийнэ үү.
үр дүн. Хариултууд нь Norito-д тулгуурласан JSON ашигладаг тул утасны формат нь тодорхойлогддог.

## 6. Үйл явдалд бүртгүүлэх

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

Үйлчлүүлэгч нь дамжуулах хоолой зэрэг Torii-ийн SSE төгсгөлийн цэгүүдэд зориулсан асинхронгүй урсгалуудыг үзүүлдэг.
үйл явдал, өгөгдлийн үйл явдал, телеметрийн хангамж.

## Илүү олон жишээ

- Төгсгөл хоорондын урсгал нь `crates/iroha`-д `tests/` дор амьдардаг. Интеграцийг хайх
  илүү баялаг хувилбаруудад зориулсан `transaction_submission.rs` зэрэг тестүүд.
- CLI (`iroha_cli`) нь ижил клиент модулийг ашигладаг; үзэх
  `crates/iroha_cli/src/` баталгаажуулалт, багцлах болон дахин оролдлого хэрхэн явагдаж байгааг харах
  үйлдвэрлэлийн багаж хэрэгсэлд зохицуулдаг.
- Norito гэдгийг санаарай: үйлчлүүлэгч хэзээ ч `serde_json` рүү буцдаггүй. Чи хэзээ
  SDK-г өргөтгөж, JSON төгсгөлийн цэгүүдэд `norito::json` туслахуудад найдаж,
  Хоёртын ачааллын хувьд `norito::codec`.

Эдгээр барилгын блокуудын тусламжтайгаар та Torii-г Rust үйлчилгээ эсвэл CLI-д нэгтгэх боломжтой.
Бүрэн иж бүрдлийг авахын тулд үүсгэсэн баримт бичиг болон өгөгдлийн загварын хайрцагнаас харна уу
заавар, асуулга, үйл явдал.