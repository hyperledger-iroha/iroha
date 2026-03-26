---
lang: ba
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rust SDK Quickstart

тип һөйләшкән өсөн . Уны ҡулланыу, ҡасан һеҙгә кәрәк, операциялар тапшырырға,
ваҡиғаларға яҙылырға, йәки эҙләү дәүләте Rust ғариза.

## 1. Йәшник өҫтәгеҙ

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

версия еп.

## 2. Клиент конфигурациялау

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

`ClientConfiguration` CLI конфигурацияһы файлын көҙгөләй: ул Torii һәм
телеметрия URL-адрестар, аутентификация материалы, тайм-ауттар, һәм партия өҫтөнлөктәре.

## 3. Транзакция тапшырыу

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

Капот аҫтында клиент ҡуллана, транзакция файҙалы йөкләмәһен кодлау өсөн .
уны Torii-ға урынлаштырыу. Әгәр ҙә тапшырыу уңышлы булһа, ҡайтарылған хеш ҡулланырға мөмкин

## 4. DA блобтарын тапшырыу

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

Ҡасан һеҙгә тикшерергә йәки һаҡланырға кәрәк файҙалы йөк ебәрмәйенсә, уны
һәм уны JSON/байте тип күрһәтә, көҙгө `iroha app da submit --no-submit`.

## 5. Һорау мәғлүмәттәре

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

Һорауҙар үтенес/яуап өлгөһө буйынса үтә: эҙләү тибы төҙөү.
һөҙөмтәләр. Яуаптар ҡулланыу -ярҙам JSON, шуға күрә сым форматы детерминистик.

## 6. Эксплорер QR снимок

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

ер өҫтө: ул канонлы иҫәп id инә, туранан-тура күрһәтелгән менән
канон i105 literal, селтәр префикс/хата-коррекция метамағлүмәттәре, QR үлсәмдәре һәм
рәтле SVG файҙалы йөк, тип янсыҡтар/тикшерелгән туранан-тура встраиваемый ала. Ҡотолоу

## 7. Ваҡиғаларға яҙылығыҙ



Клиент ’s SSE ос нөктәләре өсөн асинк ағымдарын фашлай, шул иҫәптән торба .
ваҡиғалар, мәғлүмәт ваҡиғалары һәм телеметрия каналдары.

## Күберәк миҫалдар

- CLI (`iroha_cli`) шул уҡ клиент модулен ҡуллана; браузер
 етештереү инструменттары менән эш итеү.
- -ты күҙ уңында тотоп һаҡлағыҙ: клиент бер ҡасан да `serde_json`-ға ҡайтмай. Ҡасан һеҙ
 `norito::codec` бинар файҙалы йөктәр өсөн.

## миҫалдары

- [Хажимари инеү нөктәһе скелеты] () — компиляция, йүгерә һәм таратыу
 был тиҙ стартта ҡуйыу фазаһын көҙгөләгән минималь.
- [Регистр домен һәм мәтрүшкә активтары]() — тура килә.
- [Иҫәптәр араһында күсерергә] (../norito/examples/transfer-asset) — күрһәтә

Был төҙөлөш блоктары менән һеҙ интеграциялай аласыз, йәки CLIs Rust хеҙмәттәре.
Һылтанма генерацияланған документация һәм мәғлүмәт-модель йәшниктәр өсөн тулы комплект .
күрһәтмәләр, эҙләүҙәр һәм ваҡиғалар.