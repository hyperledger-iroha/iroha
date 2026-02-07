---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T14:35:36.896251+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Rust SDK Quickstart

Rust клиент API йәшәй `iroha` йәшник, был фашлай `client::Client` .
тип һөйләшкән өсөн Torii. Уны ҡулланыу, ҡасан һеҙгә кәрәк, операциялар тапшырырға,
ваҡиғаларға яҙылырға, йәки эҙләү дәүләте Rust ғариза.

## 1. Йәшник өҫтәгеҙ

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Эш урыны миҫалы клиент модулен `client` функцияһы аша аса. Әгәр һеҙ
баҫылған йәшникте ҡулланыу, `path` атрибутын ток менән алмаштырыу
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

Капот аҫтында клиент Norito ҡуллана, транзакция файҙалы йөкләмәһен кодлау өсөн .
уны Torii-ға урынлаштырыу. Әгәр ҙә тапшырыу уңышлы булһа, ҡайтарылған хеш ҡулланырға мөмкин
трасса статусы аша `client.poll_transaction_status(hash)`.

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

Ҡасан һеҙгә тикшерергә йәки һаҡланырға кәрәк Norito файҙалы йөк ебәрмәйенсә, уны
Torii, шылтыратыу `client.build_da_ingest_request(...)` ҡул ҡуйылған үтенес алыу өсөн
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
`iroha_data_model::query`, уны `client.request` аша ебәрергә, һәм итерационный өҫтөндә .
һөҙөмтәләр. Яуаптар ҡулланыу Norito-ярҙам JSON, шуға күрә сым форматы детерминистик.

## 6. Ваҡиғаларға яҙылығыҙ

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

Клиент Torii’s SSE ос нөктәләре өсөн асинк ағымдарын фашлай, шул иҫәптән торба .
ваҡиғалар, мәғлүмәт ваҡиғалары һәм телеметрия каналдары.

## Күберәк миҫалдар

- `tests/` XX осонда тура эфирҙа осона тиклемге ағымдар Torii. Интеграцияны эҙләү
  һынауҙар, мәҫәлән, `transaction_submission.rs` байыраҡ сценарийҙар өсөн.
- CLI (`iroha_cli`) шул уҡ клиент модулен ҡуллана; браузер
  `crates/iroha_cli/src/`, нисек аутентификация, партия, һәм ретиялар күрергә
  етештереү инструменттары менән эш итеү.
- Norito-ты күҙ уңында тотоп һаҡлағыҙ: клиент бер ҡасан да `serde_json`-ҡа ҡайтмай. Ҡасан һеҙ
  SDK оҙайтыу, `norito::json` ярҙамсылары өсөн JSON ос нөктәләре һәм
  `norito::codec` бинар файҙалы йөктәр өсөн.

Был төҙөлөш блоктары менән һеҙ Torii интеграциялай аласыз, йәки CLIs Rust хеҙмәттәре.
Һылтанма генерацияланған документация һәм мәғлүмәт-модель йәшниктәр өсөн тулы комплект .
күрһәтмәләр, эҙләүҙәр һәм ваҡиғалар.