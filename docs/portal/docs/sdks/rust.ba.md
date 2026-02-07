---
lang: ba
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 035600f179f4dd225778fae57c927b2a6c9a0f1c45ca949e3536b99283c2dde3
source_last_modified: "2026-01-28T17:11:30.697433+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK Quickstart

Rust клиент API йәшәй I18NI000000022X йәшник, был фашлай `client::Client` .
тип һөйләшкән өсөн I18NT0000000006X. Уны ҡулланыу, ҡасан һеҙгә кәрәк, операциялар тапшырырға,
ваҡиғаларға яҙылырға, йәки эҙләү дәүләте Rust ғариза.

## 1. Йәшник өҫтәгеҙ

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Эш урыны миҫалы клиент модулен I18NI000000024X функцияһы аша аса. Әгәр һеҙ
баҫылған йәшникте ҡулланыу, I18NI000000025X атрибутын ток менән алмаштырыу
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

Капот аҫтында клиент I18NT000000001X ҡуллана, транзакция файҙалы йөкләмәһен кодлау өсөн .
уны Torii-ға урынлаштырыу. Әгәр ҙә тапшырыу уңышлы булһа, ҡайтарылған хеш ҡулланырға мөмкин
трасса статусы аша I18NI0000000027X.

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

Ҡасан һеҙгә тикшерергә йәки һаҡланырға кәрәк I18NT0000000002X файҙалы йөк ебәрмәйенсә, уны
I18NT000000009X, шылтыратыу I18NI0000000028X ҡул ҡуйылған запрос алыу өсөн
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
I18NI0000000030X, уны I18NI0000000031X аша ебәрергә, һәм итерационный өҫтөндә .
һөҙөмтәләр. Яуаптар ҡулланыу I18NT0000000003X-ярҙам JSON, шуға күрә сым форматы детерминистик.

## 6. Эксплорер QR снимок

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

I18NI000000032Х көҙгө I18NI0000000333X JSON
ер өҫтө: ул канонлы иҫәп id инә, туранан-тура күрһәтелгән менән
форматында, селтәр префикс/хата-коррекция метамағлүмәттәре, QR үлсәмдәре һәм
рәтле SVG файҙалы йөк, тип янсыҡтар/тикшерелгән туранан-тура встраиваемый ала. Ҡотолоу
I18NI000000034X өҫтөнлөк IH58 сығыш йәки комплект өсөн ғәҙәттәгесә ғәҙәттәгесә
I18NI0000000035X икенсе иң яҡшыһын алыу өсөн
ADDR-6б ҡулланған `sora…` варианты.

## 7. Ваҡиғаларға яҙылығыҙ

I18NF000000018X

Клиент I18NT000000010X’s SSE ос нөктәләре өсөн асинк ағымдарын фашлай, шул иҫәптән торба .
ваҡиғалар, мәғлүмәт ваҡиғалары һәм телеметрия каналдары.

## Күберәк миҫалдар

- `tests/` буйынса тура эфирҙа осонда-осона тура килә I18NI000000038X. Интеграцияны эҙләү
  һынауҙар, мәҫәлән, I18NI000000039X байыраҡ сценарийҙар өсөн.
- CLI (`iroha_cli`) шул уҡ клиент модулен ҡуллана; браузер
  I18NI0000000041X нисек аутентификация, партиялы һәм ретиялар күрергә
  етештереү инструменттары менән эш итеү.
- I18NT000000004X-ты күҙ уңында тотоп һаҡлағыҙ: клиент бер ҡасан да `serde_json`-ға ҡайтмай. Ҡасан һеҙ
  SDK оҙайтыу, I18NI000000043X ярҙамсылары өсөн JSON ос нөктәләре һәм
  `norito::codec` бинар файҙалы йөктәр өсөн.

## I18NT0000000005X миҫалдары

- [Хажимари инеү нөктәһе скелеты] (I18NU000000019X) — компиляция, йүгерә һәм таратыу
  был тиҙ стартта ҡуйыу фазаһын көҙгөләгән I18NT0000000000000000000 минималь.
- [Регистр домен һәм мәтрүшкә активтары](I18NU000000020X) — тура килә.
  I18NI000000045X + I18NI0000000000046X ағымы өҫтә күрһәтелгән, шулай итеп, һеҙ контракттан шул уҡ операцияларҙы ҡабатлай алаһығыҙ.
- [Иҫәптәр араһында күсерергә] (../norito/examples/transfer-asset) — күрһәтә
  I18NI0000000047X syscall менән шул уҡ иҫәп идентификаторҙары SDK quickstarts ҡулланыу.

Был төҙөлөш блоктары менән һеҙ I18NT000000011X интеграциялай аласыз, йәки CLIs Rust хеҙмәттәре.
Һылтанма генерацияланған документация һәм мәғлүмәт-модель йәшниктәр өсөн тулы комплект .
күрһәтмәләр, эҙләүҙәр һәм ваҡиғалар.