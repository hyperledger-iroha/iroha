---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: es
direction: ltr
source: docs/portal/docs/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2151bc561df9599865e2a5aa5d159171c1aa6f4830bfa51bd32d726f0c70a6f
source_last_modified: "2025-11-11T10:23:01.761192+00:00"
translation_last_reviewed: 2026-01-30
---

# Quickstart del SDK de Rust

La API cliente de Rust vive en el crate `iroha`, que expone un tipo `client::Client` para comunicarse con Torii. Úsalo cuando necesites enviar transacciones, suscribirte a eventos o consultar estado desde una aplicación Rust.

## 1. Agrega el crate

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

El ejemplo de workspace habilita el módulo cliente mediante la feature `client`. Si consumes el crate publicado, reemplaza el atributo `path` por la versión actual.

## 2. Configura el cliente

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

`ClientConfiguration` refleja el archivo de configuración de la CLI: incluye URLs de Torii y telemetría, material de autenticación, timeouts y preferencias de batching.

## 3. Envía una transacción

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

Bajo el capó el cliente usa Norito para codificar el payload antes de enviarlo a Torii. Si el envío tiene éxito, el hash devuelto puede usarse para rastrear el estado vía `client.poll_transaction_status(hash)`.

## 4. Envía blobs DA

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

Cuando necesites inspeccionar o persistir el payload Norito sin enviarlo a Torii, llama `client.build_da_ingest_request(...)` para obtener la solicitud firmada y renderizarla como JSON/bytes, replicando `iroha app da submit --no-submit`.

## 5. Consulta datos

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

Las consultas siguen el patrón request/response: construye un tipo de consulta desde `iroha_data_model::query`, envíalo vía `client.request` e itera los resultados. Las respuestas usan JSON respaldado por Norito, por lo que el formato en wire es determinista.

## 6. Snapshots QR del Explorer

```rust
use iroha::client::{
 Client, ClientConfiguration,
};

fn download_qr() -> eyre::Result<()> {
 let client = Client::new(ClientConfiguration::test())?;
 let snapshot = client.get_explorer_account_qr(
 "<katakana-i105-account-id>",
 )?;
 println!("Canonical literal: {}", snapshot.literal);
 println!("SVG payload: {}", snapshot.svg);
 Ok(())
}
```

`ExplorerAccountQrSnapshot` refleja el JSON `/v1/explorer/accounts/{id}/qr`: incluye el account id canónico, el literal canónico i105, metadatos de prefijo/corrección de error, dimensiones del QR y el payload SVG en línea que wallets/explorers pueden incrustar.

## 7. Suscríbete a eventos

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

El cliente expone streams async para los endpoints SSE de Torii, incluidos eventos de pipeline, eventos de datos y feeds de telemetría.

## Más ejemplos

- Los flujos end-to-end viven bajo `tests/` en `crates/iroha`. Busca tests de integración como `transaction_submission.rs` para escenarios más completos.
- La CLI (`iroha_cli`) usa el mismo módulo cliente; revisa `crates/iroha_cli/src/` para ver cómo se manejan autenticación, batching y reintentos en tooling de producción.
- Ten Norito en mente: el cliente nunca recurre a `serde_json`. Al extender el SDK, usa los helpers `norito::json` para endpoints JSON y `norito::codec` para payloads binarios.

## Ejemplos Norito relacionados

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — compila, ejecuta y despliega el andamiaje mínimo de Kotodama que refleja la fase de setup de este quickstart.
- [Register domain and mint assets](../norito/examples/register-and-mint) — se alinea con el flujo `Register` + `Mint` mostrado arriba para que puedas repetir las mismas operaciones desde un contrato.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — demuestra el syscall `transfer_asset` con las mismas cuentas que usan los quickstarts del SDK.

Con estos bloques puedes integrar Torii en servicios o CLIs de Rust. Consulta la documentación generada y los crates del data model para el conjunto completo de instrucciones, consultas y eventos.
