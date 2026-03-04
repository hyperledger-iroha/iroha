---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-sdk-rust
título: Fragmentos de SDK de Rust
sidebar_label: Fragmentos de óxido
descripción: Ejemplos mínimos en Rust para consumir streams de prueba y manifiestos.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/developer/sdk/rust.md`. Mantén ambas copias sincronizadas.
:::

Los crates de Rust en este repositorio impulsan el CLI y pueden incrustarse dentro de
orquestadores o servicios personalizados. Los fragmentos de abajo resaltan los ayudantes.
que más piden los desarrolladores.

## Flujo de prueba de ayuda

Reutiliza el analizador de flujo de prueba existente para agregar métricas de una respuesta HTTP:

```rust
use std::error::Error;
use std::io::{BufRead, BufReader};

use reqwest::blocking::Response;
use sorafs_car::proof_stream::{ProofStreamItem, ProofStreamMetrics, ProofStreamSummary};

/// Consume an NDJSON proof stream and return aggregated metrics.
pub fn collect_proof_metrics(response: Response) -> Result<ProofStreamSummary, Box<dyn Error>> {
    if !response.status().is_success() {
        return Err(format!("gateway returned {}", response.status()).into());
    }

    let mut reader = BufReader::new(response);
    let mut line = String::new();
    let mut metrics = ProofStreamMetrics::default();
    let mut failures = Vec::new();

    while reader.read_line(&mut line)? != 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }
        let item = ProofStreamItem::from_ndjson(trimmed.as_bytes())?;
        if item.status.is_failure() && failures.len() < 5 {
            failures.push(item.clone());
        }
        metrics.record(&item);
        line.clear();
    }

    Ok(ProofStreamSummary::new(metrics, failures))
}
```

La versión completa (con pruebas) vive en `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza el mismo JSON de métricas que el CLI, lo que
facilitar alimentar backends de observabilidad o aserciones de CI.

## Puntuación de buscar múltiples fuentes

El módulo `sorafs_car::multi_fetch` exponen el planificador de fetch asíncrono usado por el
CLI. Implementa `sorafs_car::multi_fetch::ScorePolicy` y pásalo vía
`FetchOptions::score_policy` para ajustar el orden de los proveedores. El test unitario
`multi_fetch::tests::score_policy_can_filter_providers` muestra cómo forzar
preferencias personalizadas.

Otros mandos que reflejan flags del CLI:- `FetchOptions::per_chunk_retry_limit` coincide con el flag `--retry-budget` para
  ejecuciones de CI que restringen los reintentos a propósito.
- Combina `FetchOptions::global_parallel_limit` con `--max-peers` para limitar la
  cantidad de proveedores concurrentes.
- `OrchestratorConfig::with_telemetry_region("region")` etiqueta las métricas
  `sorafs_orchestrator_*`, mientras que `OrchestratorConfig::with_transport_policy`
  refleja el flag `--transport-policy` del CLI. `TransportPolicy::SoranetPreferred`
  se entrega como valor por defecto en superficie CLI/SDK; estados unidos
  `TransportPolicy::DirectOnly` solo al preparar un downgrade o seguir una directiva de
  Compliance, y reserva `SoranetStrict` para pilotos PQ-only con aprobación específica.
- Configura `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forzar subidas PQ-only; el ayudante promoverá
  automáticamente las políticas de transporte/anonimato salvo que se sobrescriben
  perfectamente.
- Usa `SorafsGatewayFetchOptions::policy_override` para fijar un transporte o nivel de
  anonimato temporal para una sola solicitud; al proporcionar cualquiera de los campos
  se omite la degradación por apagón y falla si el nivel solicitado no puede
  satisfacerse.
- Los enlaces de Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) y
  JavaScript (`sorafsMultiFetchLocal`) reutiliza el mismo planificador, así que configura
  `return_scoreboard=true` en esos ayudantes para recuperar los pesos calculados junto con
  los recibos de trozos.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registra el stream OTLP queproducido un paquete de adopción. Cuando se omite, el cliente deriva
  `region:<telemetry_region>` (o `chain:<chain_id>`) automáticamente para que el
  metadatos siempre lleve una etiqueta descriptiva.

## Obtener vía `iroha::Client`

El SDK de Rust incorpora el helper de gateway fetch; proporciona un manifiesto más los
descriptores de proveedores (incluidos stream tokens) y deja que el cliente ejecute
el buscar fuente múltiple:

```rust
use eyre::Result;
use iroha::{
    Client,
    client::{SorafsGatewayFetchOptions, SorafsGatewayScoreboardOptions},
};
use sorafs_car::CarBuildPlan;
use sorafs_orchestrator::{
    AnonymityPolicy, PolicyOverride,
    prelude::{GatewayFetchConfig, GatewayProviderInput, TransportPolicy},
};
use std::path::PathBuf;

pub async fn fetch_payload(
    client: &Client,
    plan: &CarBuildPlan,
    gateway: GatewayFetchConfig,
    providers: Vec<GatewayProviderInput>,
) -> Result<Vec<u8>> {
    let options = SorafsGatewayFetchOptions {
        transport_policy: Some(TransportPolicy::SoranetPreferred),
        // Pin Stage C for this fetch; omit `policy_override` to apply staged defaults.
        policy_override: PolicyOverride::new(
            Some(TransportPolicy::SoranetStrict),
            Some(AnonymityPolicy::StrictPq),
        ),
        write_mode_hint: None,
        scoreboard: Some(SorafsGatewayScoreboardOptions {
            persist_path: Some(
                PathBuf::from("artifacts/sorafs_orchestrator/latest/scoreboard.json"),
            ),
            now_unix_secs: None,
            metadata: Some(norito::json!({
                "capture_id": "sdk-smoke-run",
                "fixture": "multi_peer_parity_v1"
            })),
            telemetry_source_label: Some("otel::staging".into()),
        }),
        ..SorafsGatewayFetchOptions::default()
    };
    let outcome = client
        .sorafs_fetch_via_gateway(plan, gateway, providers, options)
        .await?;
    Ok(outcome.assemble_payload())
}
```

Configure `transport_policy` como `Some(TransportPolicy::SoranetStrict)` cuando las
subidas deben rechazar relés clásicos, o `Some(TransportPolicy::DirectOnly)` cuando
SoraNet debería omitirse por completo. Apunta `scoreboard.persist_path` al directorio de
artefactos de liberación, fija opcionalmente `scoreboard.now_unix_secs` y completa
`scoreboard.metadata` con contexto de captura (etiquetas de accesorios, objetivo Torii, etc.)
para que `cargo xtask sorafs-adoption-check` consuma JSON determinista entre SDK con
el blob de procedencia que espera SF-6c.
`Client::sorafs_fetch_via_gateway` ahora complementa esos metadatos con el identificador
 de manifiesto, la expectativa opcional de manifiesto CID y el flag
`gateway_manifest_provided` inspeccionando el `GatewayFetchConfig` suministrado, de modo
que capturas que incluyen un envoltorio de manifiesto firmado cumplan el requisito de
pruebas SF-6c sin duplicar esos campos manualmente.

## Ayudantes de manifiesto

`ManifestBuilder` sigue siendo la forma canónica de ensamblar payloads Norito de forma
programático:

```rust
use sorafs_manifest::{ManifestBuilder, ManifestV1, PinPolicy, StorageClass};

fn build_manifest(bytes: &[u8]) -> Result<ManifestV1, Box<dyn std::error::Error>> {
    let mut builder = ManifestBuilder::new();
    builder.pin_policy(PinPolicy {
        min_streams: 3,
        storage_class: StorageClass::Warm,
        retention_epoch: Some(48),
    });
    builder.payload(bytes)?;
    Ok(builder.build()?)
}
```Incorpora el constructor donde los servicios necesiten generar manifiestos al vuelo; el
CLI sigue siendo la ruta recomendada para ductos deterministas.