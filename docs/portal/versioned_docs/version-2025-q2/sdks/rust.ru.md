---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Быстрый старт Rust SDK

Клиентский API Rust находится в крейте `iroha`, который предоставляет файл `client::Client`.
введите для разговора с Torii. Используйте его, когда вам нужно отправить транзакции,
подписывайтесь на события или запрашивайте состояние из приложения Rust.

## 1. Добавьте ящик

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Пример рабочей области разблокирует клиентский модуль с помощью функции `client`. Если ты
использовать опубликованный крейт, замените атрибут `path` текущим
строка версии.

## 2. Настройте клиент

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

`ClientConfiguration` отражает файл конфигурации CLI: он включает Torii и
URL-адреса телеметрии, материалы аутентификации, тайм-ауты и настройки пакетной обработки.

## 3. Отправьте транзакцию

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

Под капотом клиент использует Norito для кодирования полезных данных транзакции перед
разместив это на Torii. Если отправка прошла успешно, возвращенный хеш можно использовать для
отслеживать статус через `client.poll_transaction_status(hash)`.

## 4. Отправьте объекты DA

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

Когда вам нужно проверить или сохранить полезную нагрузку Norito, не отправляя ее в
Torii, позвоните `client.build_da_ingest_request(...)`, чтобы получить подписанный запрос.
и визуализировать его как JSON/bytes, зеркально отображая `iroha app da submit --no-submit`.

## 5. Запрос данных

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

Запросы следуют шаблону запрос/ответ: создайте тип запроса из
`iroha_data_model::query`, отправьте его через `client.request` и выполните итерацию по
результаты. В ответах используется JSON с поддержкой Norito, поэтому формат передачи является детерминированным.

## 6. Подпишитесь на события

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

Клиент предоставляет асинхронные потоки для конечных точек SSE Torii, включая конвейер.
события, события данных и каналы телеметрии.

## Еще примеры

- Сквозные потоки живут под `tests/` в `crates/iroha`. Поиск интеграции
  тесты, такие как `transaction_submission.rs`, для более сложных сценариев.
- CLI (`iroha_cli`) использует тот же клиентский модуль; просматривать
  `crates/iroha_cli/src/`, чтобы узнать, как выполняются аутентификация, пакетная обработка и повторные попытки.
  обрабатывается на производственной оснастке.
- Имейте в виду Norito: клиент никогда не возвращается к `serde_json`. Когда ты
  расширить SDK, использовать помощники `norito::json` для конечных точек JSON и
  `norito::codec` для двоичных полезных данных.

С помощью этих строительных блоков вы можете интегрировать Torii в службы Rust или интерфейсы командной строки.
Обратитесь к сгенерированной документации и крейтам моделей данных для получения полного набора
инструкции, запросы и события.