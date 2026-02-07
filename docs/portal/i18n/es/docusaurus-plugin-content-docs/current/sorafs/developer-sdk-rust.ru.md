---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-sdk-rust
título: Сниппеты SDK en Rust
sidebar_label: Elementos de Rust
descripción: Primeros pasos mínimos en Rust para mostrar flujos de prueba y manifiestos.
---

:::nota Канонический источник
:::

Rust crates en estos repositorios requiere CLI y puede instalar archivos en cajas
orquestadores или сервисы. Сниппеты ниже выделяют ayudantes, которые чаще всего
нужны разработчикам.

## Ayudante para el flujo de prueba

Utilice un analizador de flujo de prueba sofisticado para agregar métricas
HTTP-ответа:

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

La versión rusa (con pruebas) está disponible en `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza la métrica JSON, la CLI y la actualización
подачу данных в observabilidad backend o aserciones de CI.

## Puntuación de búsqueda de múltiples fuentes

El módulo `sorafs_car::multi_fetch` elimina el programador de búsqueda asincrono, implementado
CLI. Realice `sorafs_car::multi_fetch::ScorePolicy` y pierda su tiempo
`FetchOptions::score_policy`, чтобы настроить порядок провайдеров. Юнит-тест
`multi_fetch::tests::score_policy_can_filter_providers` показывает, как вводить
кастомные предпочтения.

Los botones de control incluidos en la bandera CLI:- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` para CI
  запусков, которые намеренно ограничивают reintentos.
- Combinación de `FetchOptions::global_parallel_limit` con `--max-peers`, que están diseñadas
  количество одновременных провайдеров.
- `OrchestratorConfig::with_telemetry_region("region")` medidores de potencia
  `sorafs_orchestrator_*`, а `OrchestratorConfig::with_transport_policy` cierre
  Bandera CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` идет по умолчанию
  en CLI/SDK integrado; используйте `TransportPolicy::DirectOnly` только при puesta en escena
  degradar o reducir la directiva de cumplimiento y restaurar `SoranetStrict` para
  Pilotos solo PQ con явным одобрением.
- Instalar `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` permite cargas solo de PQ; ayudante automático
  повысит transporte/anonimato политики, если они не anulado явно.
- Utilice `SorafsGatewayFetchOptions::policy_override` para guardar archivos antiguos
  transporte или nivel de anonimato для одного запроса; подача любого поля пропускает
  La degradación por apagón y el aumento de nivel, o el nivel de bajada, no pueden reducirse.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) y JavaScript
  (`sorafsMultiFetchLocal`) Los enlaces se aplican al programador, pero no se pueden utilizar
  `return_scoreboard=true` en estos ayudantes, чтобы получить рассчитанные веса рядом с
  recibos en trozos.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` captura la secuencia OTLP,
  paquete de adopción formal. Если он не задан, клиент автоматически выводит
  `region:<telemetry_region>` (или `chain:<chain_id>`) чтобы metadatos всегда неслаописательный etiqueta.

## Recuperar por `iroha::Client`

Rust SDK proporciona ayuda para la búsqueda de puerta de enlace; передайте manifiesto y descriptores de провайдеров
(incluidos tokens de transmisión) y el cliente actual puede actualizar la recuperación de múltiples fuentes:

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

Instale `transport_policy` en `Some(TransportPolicy::SoranetStrict)`, cuántas cargas
должны отвергать классические relés, o en `Some(TransportPolicy::DirectOnly)`, когда
SoraNet no está disponible. Inserte `scoreboard.persist_path` en el directorio
релизных артефактов, опционально зафиксируйте `scoreboard.now_unix_secs` y заполните
`scoreboard.metadata` контекстом захвата (accesorios de etiquetas, цель Torii и т.д.) так,
чтобы `cargo xtask sorafs-adoption-check` потреблял детерминированный JSON между SDK
с procedencia de la mancha, который ожидает SF-6c.
`Client::sorafs_fetch_via_gateway` contiene este manifiesto de identificación de metadatos,
Opciones de análisis del manifiesto CID y el indicador `gateway_manifest_provided`
переданного `GatewayFetchConfig`, чтобы захваты с подписанным sobre manifiesto удовлетворяли
требованию доказательств SF-6c без ручного дублирования этих полей.

## Ayudantes del manifiesto

`ManifestBuilder` muestra un código de programa que vincula las cargas útiles de Norito:

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
```

Встраивайте builder везде, где сервисам нужно генерировать manifests на лету; CLI
Se recomienda utilizar tuberías para determinar las tuberías.