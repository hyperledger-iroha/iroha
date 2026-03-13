---
lang: uz
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rust SDK tezkor ishga tushirish

Rust mijoz API'si `iroha` qutisida joylashgan bo'lib, u `client::Client`-ni ochadi.
Torii bilan gaplashish uchun yozing. Tranzaktsiyalarni topshirish kerak bo'lganda foydalaning,
hodisalarga obuna bo'ling yoki Rust ilovasidan holatini so'rang.

## 1. Sandiqni qo'shing

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Ish maydoni misoli mijoz modulini `client` xususiyati orqali ochadi. Agar siz
chop etilgan qutini iste'mol qiling, `path` atributini joriy bilan almashtiring
versiya qatori.

## 2. Mijozni sozlang

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

`ClientConfiguration` CLI konfiguratsiya faylini aks ettiradi: u Torii va
telemetriya URL-manzillari, autentifikatsiya materiali, kutish vaqti va toʻplam afzalligi.

## 3. Tranzaksiyani yuboring

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

Kaput ostida mijoz avval tranzaksiya yukini kodlash uchun Norito dan foydalanadi.
uni Torii ga joylashtirish. Agar topshirish muvaffaqiyatli bo'lsa, qaytarilgan xeshdan foydalanish mumkin
holatini `client.poll_transaction_status(hash)` orqali kuzatib boring.

## 4. DA bloblarini yuboring

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

Norito foydali yukini jo'natmasdan tekshirish yoki davom ettirish kerak bo'lganda
Torii, imzolangan so'rovni olish uchun `client.build_da_ingest_request(...)` raqamiga qo'ng'iroq qiling
va uni `iroha app da submit --no-submit` aks ettirgan holda JSON/bayt sifatida ko'rsating.

## 5. So'rov ma'lumotlari

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

So'rovlar so'rov/javob namunasiga amal qiladi: so'rov turini yarating
`iroha_data_model::query`, uni `client.request` orqali yuboring va uni takrorlang
natijalar. Javoblar Norito tomonidan qo'llab-quvvatlanadigan JSON-dan foydalanadi, shuning uchun sim formati deterministikdir.

## 6. Explorer QR suratlari

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

`ExplorerAccountQrSnapshot` `/v2/explorer/accounts/{id}/qr` JSON-ni aks ettiradi
sirt: u kanonik hisob identifikatorini o'z ichiga oladi, literal bilan ko'rsatilgan
kanonik I105 literal, tarmoq prefiksi/xatoni tuzatish metama'lumotlari, QR o'lchamlari va
hamyonlar/tadqiqchilar to'g'ridan-to'g'ri joylashtirishi mumkin bo'lgan inline SVG foydali yuk.

## 7. Tadbirlarga obuna bo'ling

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

Mijoz Torii ning SSE so'nggi nuqtalari, shu jumladan quvur liniyasi uchun asinxron oqimlarni ko'rsatadi.
voqealar, ma'lumotlar hodisalari va telemetriya tasmasi.

## Yana misollar

- `crates/iroha` da `tests/` ostida oxirigacha oqimlar mavjud. Integratsiyani qidirish
 boyroq stsenariylar uchun `transaction_submission.rs` kabi testlar.
- CLI (`iroha_cli`) bir xil mijoz modulidan foydalanadi; ko'rib chiqish
 `crates/iroha_cli/src/` autentifikatsiya, paketlash va qayta urinishlar qanday ekanligini koʻrish uchun
 ishlab chiqarish asboblarida ishlanadi.
- Norito ni yodda tuting: mijoz hech qachon `serde_json` ga qaytmaydi. Qachon siz
 SDK-ni kengaytiring, JSON so'nggi nuqtalari uchun `norito::json` yordamchilariga tayaning va
 Ikkilik foydali yuklar uchun `norito::codec`.

## Tegishli Norito misollari

- [Hajimari kirish nuqtasi skeleti](../norito/examples/hajimari-entrypoint) — kompilyatsiya qilish, ishga tushirish va joylashtirish
 ushbu tezkor ishga tushirishda sozlash bosqichini aks ettiruvchi minimal Kotodama iskala.
- [Domen va zarb aktivlarini ro'yxatdan o'tkazish](../norito/examples/register-and-mint) - bilan mos keladi
 `Register` + `Mint` oqimi yuqorida ko'rsatilgan, shuning uchun siz shartnomadan bir xil operatsiyalarni takrorlashingiz mumkin.
- [Aktivni hisoblar o'rtasida o'tkazish](../norito/examples/transfer-asset) - ko'rsatadi
 `transfer_asset` tizimi qo'ng'iroqlari SDK tezkor ishga tushirishlari foydalanadigan bir xil hisob identifikatorlari bilan.

Ushbu qurilish bloklari bilan siz Torii ni Rust xizmatlari yoki CLI-larga integratsiya qilishingiz mumkin.
To'liq to'plam uchun yaratilgan hujjatlar va ma'lumotlar modeli qutilariga qarang
ko'rsatmalar, so'rovlar va voqealar.