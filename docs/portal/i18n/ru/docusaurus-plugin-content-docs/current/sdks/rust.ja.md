---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 52b0da2dabad34ba605c67355e5f32ae8043eceb0061e8cbdcc0d5b0cf842260
source_last_modified: "2026-01-30T15:37:43+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Быстрый старт SDK Rust

Клиентский API Rust живет в crate `iroha`, который предоставляет тип `client::Client` для работы с Torii. Используйте его для отправки транзакций, подписки на события или запроса состояния из Rust‑приложения.

## 1. Добавьте crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

Пример workspace включает модуль клиента через feature `client`. Если используете опубликованный crate, замените атрибут `path` на актуальную версию.

## 2. Настройте клиента

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

`ClientConfiguration` отражает конфигурацию CLI: включает URL Torii и телеметрии, материалы аутентификации, тайм‑ауты и настройки батчинга.

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

Под капотом клиент использует Norito для кодирования payload перед отправкой в Torii. При успехе возвращенный хэш можно использовать для отслеживания статуса через `client.poll_transaction_status(hash)`.

## 4. Отправьте DA‑blobs

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

Если нужно просмотреть или сохранить Norito payload без отправки в Torii, вызовите `client.build_da_ingest_request(...)`, чтобы получить подписанный запрос и отрендерить его в JSON/bytes, аналогично `iroha app da submit --no-submit`.

## 5. Запрашивайте данные

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

Запросы следуют шаблону request/response: создайте тип запроса из `iroha_data_model::query`, отправьте через `client.request` и переберите результаты. Ответы используют JSON на Norito, поэтому формат по проводу детерминирован.

## 6. QR‑снапшоты Explorer

```rust
use iroha::client::{
    Client, ClientConfiguration, ExplorerAccountQrOptions,
};

fn download_qr() -> eyre::Result<()> {
    let client = Client::new(ClientConfiguration::test())?;
    let snapshot = client.get_explorer_account_qr(
        "i105...",
        Some(ExplorerAccountQrOptions {
        }),
    )?;
    println!("Canonical literal: {}", snapshot.literal);
    println!("SVG payload: {}", snapshot.svg);
    Ok(())
}
```

`ExplorerAccountQrSnapshot` отражает JSON `/v1/explorer/accounts/{id}/qr`: включает канонический account id, literal в выбранном формате, метаданные префикса/коррекции ошибок, размеры QR и inline SVG payload. Оставьте `ExplorerAccountQrOptions` пустым, чтобы получить предпочитаемый I105, или задайте canonical I105 output для канонического I105, используемого ADDR-6b.

## 7. Подпишитесь на события

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

Клиент предоставляет async‑стримы для SSE‑эндпоинтов Torii, включая pipeline events, data events и телеметрию.

## Другие примеры

- Сценарии end‑to‑end находятся в `tests/` внутри `crates/iroha`. Ищите интеграционные тесты вроде `transaction_submission.rs` для более богатых сценариев.
- CLI (`iroha_cli`) использует тот же модуль клиента; просмотрите `crates/iroha_cli/src/`, чтобы понять аутентификацию, batching и retries в прод‑инструментах.
- Помните про Norito: клиент никогда не использует `serde_json`. При расширении SDK используйте `norito::json` для JSON‑эндпоинтов и `norito::codec` для бинарных payloads.

## Связанные примеры Norito

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — компилирует, запускает и деплоит минимальный каркас Kotodama, отражающий фазу setup этого quickstart.
- [Register domain and mint assets](../norito/examples/register-and-mint) — соответствует потоку `Register` + `Mint` выше, чтобы повторить те же операции из контракта.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — демонстрирует syscall `transfer_asset` с теми же account IDs, что и в quickstart SDK.

Эти блоки позволяют интегрировать Torii в Rust‑сервисы или CLI. Смотрите сгенерированную документацию и crates data model для полного набора инструкций, запросов и событий.
