---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T14:35:36.896251+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
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

## 6. Оқиғаларға жазылыңыз

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

- `crates/iroha` ішіндегі `tests/` астында ұштастыра ағындар өмір сүреді. Интеграцияны іздеу
  бай сценарийлер үшін `transaction_submission.rs` сияқты сынақтар.
- CLI (`iroha_cli`) бірдей клиент модулін пайдаланады; шолу
  Аутентификация, топтастыру және қайталау әрекеттерінің қалай орындалатынын көру үшін `crates/iroha_cli/src/`
  өндірістік құрал-саймандарда өңделеді.
- Norito есте сақтаңыз: клиент ешқашан `serde_json` нұсқасына қайта оралмайды. Сіз кезде
  SDK кеңейтіңіз, JSON соңғы нүктелері үшін `norito::json` көмекшілеріне сеніңіз және
  `norito::codec` екілік пайдалы жүктемелер үшін.

Осы құрылыс блоктарымен Torii жүйесін Rust қызметтеріне немесе CLI қызметтеріне біріктіруге болады.
Толық жиынтығы үшін жасалған құжаттаманы және деректер үлгісі жәшіктерін қараңыз
нұсқаулар, сұраулар және оқиғалар.