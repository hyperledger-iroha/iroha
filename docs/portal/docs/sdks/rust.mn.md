---
lang: mn
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:11:30.697433+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK хурдан эхлүүлэх

Rust клиент API нь `iroha` хайрцагт амьдардаг бөгөөд энэ нь `client::Client`-г ил гаргадаг.
Torii-тэй ярих гэж бичнэ үү. Гүйлгээ хийх шаардлагатай үед үүнийг ашиглах,
үйл явдалд бүртгүүлэх эсвэл Rust програмаас төлөвийг асуух.

## 1. Хайрцаг нэмнэ

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Ажлын талбарын жишээ нь `client` функцээр дамжуулан клиент модулийн түгжээг тайлдаг. Хэрэв та
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
мөн `iroha app da submit --no-submit` толин тусгал болгон JSON/байт хэлбэрээр үзүүлнэ.

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

## 6. Explorer QR агшин зуурын зургууд

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "i105...",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` `/v2/explorer/accounts/{id}/qr` JSON-г тусгадаг
гадаргуу: энэ нь каноник дансны id, кодоор илэрхийлэгдсэн үг хэллэгийг агуулдаг
канон I105 literal, сүлжээний угтвар/алдаа засах мета өгөгдөл, QR хэмжээсүүд болон
түрийвч/судлаачдын шууд оруулах боломжтой SVG ачаалал.

## 7. Үйл явдалд бүртгүүлэх

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

- Төгсгөл хоорондын урсгал нь `crates/iroha`-д `tests/`-ийн дагуу амьдардаг. Интеграцийг хайх
 илүү баялаг хувилбаруудад зориулсан `transaction_submission.rs` зэрэг тестүүд.
- CLI (`iroha_cli`) нь ижил клиент модулийг ашигладаг; үзэх
 `crates/iroha_cli/src/` нэвтрэлт баталгаажуулалт, багцлах болон дахин оролдлого хэрхэн явагдаж байгааг харахын тулд
 үйлдвэрлэлийн багаж хэрэгсэлд зохицуулдаг.
- Norito-г санаарай: үйлчлүүлэгч хэзээ ч `serde_json` руу буцдаггүй. Чи хэзээ
 SDK-г өргөтгөж, JSON төгсгөлийн цэгүүдэд `norito::json` туслахуудад найдаж,
 `norito::codec` хоёртын ачааллын хувьд.

## Холбогдох Norito жишээнүүд

- [Хажимари нэвтрэх цэгийн араг яс](../norito/examples/hajimari-entrypoint) — эмхэтгэх, ажиллуулах, байрлуулах
 Энэ хурдан эхлүүлэхэд тохируулах үе шатыг тусгасан хамгийн бага Kotodama шат.
- [Домэйн болон гаалийн хөрөнгийг бүртгүүлэх](../norito/examples/register-and-mint) — дараахтай нийцдэг
 `Register` + `Mint` урсгалыг дээр харуулснаар та гэрээнээс ижил үйлдлүүдийг дахин тоглуулах боломжтой.
- [Дансны хооронд хөрөнгө шилжүүлэх](../norito/examples/transfer-asset) — харуулж байна
 `transfer_asset` систем нь SDK-ийн хурдан эхлүүлэхийн ашигладаг ижил дансны ID-тай.

Эдгээр барилгын блокуудын тусламжтайгаар та Torii-г Rust үйлчилгээ эсвэл CLI-д нэгтгэх боломжтой.
Бүрэн иж бүрдлийг авахын тулд үүсгэсэн баримт бичиг болон өгөгдлийн загварын хайрцагнаас харна уу
заавар, асуулга, үйл явдал.