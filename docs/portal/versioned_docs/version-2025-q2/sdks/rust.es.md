---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 926ec1446b2ed51270a59a2842ba668cc442cf47f6c7bb0bd8b3189f7d16e738
source_last_modified: "2026-01-22T15:38:30.655816+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Inicio rápido del SDK de Rust

La API del cliente Rust se encuentra en la caja `iroha`, que expone un `client::Client`.
escriba para hablar con Torii. Úselo cuando necesite enviar transacciones,
suscribirse a eventos o consultar el estado desde una aplicación Rust.

## 1. Agrega la caja

```toml title="Cargo.toml"
[dependencies]
iroha = { path = "../../crates/iroha", features = ["client"] }
```

El ejemplo del espacio de trabajo desbloquea el módulo del cliente mediante la función `client`. si tu
consumir la caja publicada, reemplace el atributo `path` con el actual
cadena de versión.

## 2. Configurar el cliente

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

`ClientConfiguration` refleja el archivo de configuración CLI: incluye Torii y
URL de telemetría, material de autenticación, tiempos de espera y preferencias de procesamiento por lotes.

## 3. Enviar una transacción

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

Debajo del capó, el cliente usa Norito para codificar la carga útil de la transacción antes
publicarlo en Torii. Si el envío tiene éxito, el hash devuelto se puede utilizar para
Seguimiento del estado a través de `client.poll_transaction_status(hash)`.

## 4. Enviar blobs DA

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

Cuando necesita inspeccionar o conservar la carga útil Norito sin enviarla a
Torii, llame a `client.build_da_ingest_request(...)` para obtener la solicitud firmada
y renderícelo como JSON/bytes, reflejando `iroha app da submit --no-submit`.

## 5. Consultar datos

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

Las consultas siguen el patrón de solicitud/respuesta: construya un tipo de consulta a partir de
`iroha_data_model::query`, enviarlo a través de `client.request` e iterar sobre el
resultados. Las respuestas utilizan JSON respaldado por Norito, por lo que el formato del cable es determinista.

## 6. Suscríbete a eventos

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

El cliente expone flujos asíncronos para los puntos finales SSE de Torii, incluida la canalización
eventos, eventos de datos y fuentes de telemetría.

## Más ejemplos

- Los flujos de un extremo a otro se encuentran bajo `tests/` en `crates/iroha`. Buscar integración
  pruebas como `transaction_submission.rs` para escenarios más ricos.
- La CLI (`iroha_cli`) utiliza el mismo módulo de cliente; navegar
  `crates/iroha_cli/src/` para ver cómo son la autenticación, el procesamiento por lotes y los reintentos
  manipulados en herramientas de producción.
- Tenga en cuenta Norito: el cliente nunca vuelve a recurrir a `serde_json`. cuando tu
  amplíe el SDK, confíe en los asistentes `norito::json` para puntos finales JSON y
  `norito::codec` para cargas útiles binarias.

Con estos bloques de construcción puede integrar Torii en servicios o CLI de Rust.
Consulte la documentación generada y las cajas del modelo de datos para obtener el conjunto completo de
instrucciones, consultas y eventos.