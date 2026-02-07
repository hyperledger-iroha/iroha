---
lang: kk
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rust SDK жылдам іске қосу

Rust клиентінің API интерфейсі `iroha` жәшігінде тұрады, ол `client::Client` ашады.
Torii телефонымен сөйлесу үшін теріңіз. Оны транзакцияларды жіберу қажет болғанда пайдаланыңыз,
оқиғаларға жазылу немесе Rust қолданбасынан күй сұрау.

## 1. Қорапты қосыңыз

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Жұмыс кеңістігінің мысалы клиент модулін `client` мүмкіндігі арқылы ашады. Егер сіз
жарияланған жәшікті тұтыну, `path` төлсипатын ағымдағымен ауыстырыңыз
нұсқа жолы.

## 2. Клиентті конфигурациялаңыз

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

`ClientConfiguration` CLI конфигурация файлын көрсетеді: оған Torii және
телеметрия URL мекенжайлары, аутентификация материалы, күту уақыттары және пакеттік теңшелімдер.

## 3. Транзакция жіберіңіз

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

Сорғыштың астында клиент транзакцияның пайдалы жүктемесін кодтау үшін Norito пайдаланады.
оны Torii мекенжайына жариялау. Жіберу сәтті болса, қайтарылған хэшті қолдануға болады
`client.poll_transaction_status(hash)` арқылы күйді қадағалау.

## 4. DA блоктарын жіберіңіз

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

Norito пайдалы жүктемесін жібермей тексеру немесе сақтау қажет болғанда
Torii, қол қойылған сұрауды алу үшін `client.build_da_ingest_request(...)` нөміріне қоңырау шалыңыз
және оны `iroha app da submit --no-submit` шағылысатын JSON/байт ретінде көрсетіңіз.

## 5. Деректерді сұрау

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

Сұраулар сұрау/жауап үлгісіне сәйкес келеді: сұрау түрін құрастырыңыз
`iroha_data_model::query`, оны `client.request` арқылы жіберіңіз және қайталаңыз
нәтижелер. Жауаптар Norito қолдайтын JSON пайдаланады, сондықтан сым пішімі детерминирленген.

## 6. Explorer QR суреттері

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

`ExplorerAccountQrSnapshot` `/v1/explorer/accounts/{id}/qr` JSON көрсетеді
беті: ол канондық тіркелгі идентификаторын, әріппен көрсетілген литералды қамтиды
сұралған пішім, желі префиксі/қателерді түзету метадеректері, QR өлшемдері және
әмияндар/зерттеушілер тікелей ендіре алатын кірістірілген SVG пайдалы жүктемесі. Өткізіп тастаңыз
`ExplorerAccountQrOptions` таңдаулы IH58 шығысына әдепкіге немесе орнатуға
Екінші ең жақсысын алу үшін `address_format: Some(AddressFormat::Compressed)`
ADDR-6b пайдаланатын `sora…` нұсқасы.

## 7. Оқиғаларға жазылыңыз

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

Клиент Torii SSE соңғы нүктелері үшін асинхронды ағындарды, соның ішінде құбыр желісін көрсетеді
оқиғалар, деректер оқиғалары және телеметрия арналары.

## Қосымша мысалдар

- `crates/iroha` ішіндегі `tests/` астында ұшып-соңғы ағындар өмір сүреді. Интеграцияны іздеу
  бай сценарийлер үшін `transaction_submission.rs` сияқты сынақтар.
- CLI (`iroha_cli`) бірдей клиент модулін пайдаланады; шолу
  Аутентификация, топтастыру және қайталау әрекеттерінің қалай орындалатынын көру үшін `crates/iroha_cli/src/`
  өндірістік құрал-саймандарда өңделеді.
- Norito есте сақтаңыз: клиент ешқашан `serde_json` нұсқасына қайта оралмайды. Сіз кезде
  SDK кеңейтіңіз, JSON соңғы нүктелері үшін `norito::json` көмекшілеріне сеніңіз және
  `norito::codec` екілік пайдалы жүктемелер үшін.

## Қатысты Norito мысалдары

- [Hajimari кіру нүктесінің қаңқасы](../norito/examples/hajimari-entrypoint) — құрастыру, іске қосу және орналастыру
  осы жылдам іске қосудағы орнату фазасын көрсететін минималды Kotodama тірегі.
- [Домен мен ақша активтерін тіркеу](../norito/examples/register-and-mint) — келесімен тураланады
  `Register` + `Mint` ағыны жоғарыда көрсетілген, осылайша келісім-шарттағы бірдей әрекеттерді қайталай аласыз.
- [Активтерді шоттар арасында тасымалдау](../norito/examples/transfer-asset) — көрсетеді
  SDK жылдам бастаулары пайдаланатын бірдей тіркелгі идентификаторлары бар `transfer_asset` жүйесін шақыру.

Осы құрылыс блоктарымен Torii жүйесін Rust қызметтеріне немесе CLI қызметтеріне біріктіруге болады.
Толық жиынтығы үшін жасалған құжаттаманы және деректер үлгісі жәшіктерін қараңыз
нұсқаулар, сұраулар және оқиғалар.