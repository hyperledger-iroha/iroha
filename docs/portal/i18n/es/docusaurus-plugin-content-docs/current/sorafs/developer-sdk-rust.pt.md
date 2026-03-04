---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-sdk-rust
título: Fragmentos de SDK Rust
sidebar_label: Fragmentos de Rust
descripción: Ejemplos de Rust mínimos para consumir flujos de prueba y manifiestos.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas como copias sincronizadas.
:::

Las cajas Rust son un repositorio alimentado por CLI y pueden ser embutidos en
orquestadores o servicios personalizados. Os snippets abaixo destacam os ayudantes que
mais aparecem nas solicitacoes dos desenvolvedores.

## Flujo de prueba de ayuda

Reutilice el analizador de flujo de prueba existente para agregar métricas de una respuesta HTTP:

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

A versao completa (con testes) vive en `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza el mismo JSON de métricas de CLI, facilitando
alimentar backends de observabilidade o afirmaciones de CI.

## Puntuación de búsqueda de múltiples fuentes

El módulo `sorafs_car::multi_fetch` expone el programador de búsqueda de sincronización de pelo usado
CLI. Implemente `sorafs_car::multi_fetch::ScorePolicy` y pase a través de
`FetchOptions::score_policy` para ajustar a ordenacao de proveedores. O prueba unitaria
`multi_fetch::tests::score_policy_can_filter_providers` muestra como importar preferencias
personalizados.

Otros botones de banderas de espelham hacen CLI:- `FetchOptions::per_chunk_retry_limit` corresponde a la bandera `--retry-budget` para ejecuciones
  de CI que restringem tentativas propositalmente.
- Combine `FetchOptions::global_parallel_limit` con `--max-peers` para limitar el número
  de proveedores concurrentes.
- `OrchestratorConfig::with_telemetry_region("region")` marca como métricas
  `sorafs_orchestrator_*`, mientras `OrchestratorConfig::with_transport_policy` espelha
  o bandera CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` y nas predeterminado
  superficies CLI/SDK; use `TransportPolicy::DirectOnly` sólo para preparar una degradación
  o seguir una directiva de cumplimiento, y reservar `SoranetStrict` para pilotos PQ-only
  com aprobación explícita.
- Defina `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para cargar cargas solo PQ; o ayudante de promoción
  automaticamente as politicas de transporte/anonymyty a menos que sejam explicitamente
  sobrescritas.
- Utilice `SorafsGatewayFetchOptions::policy_override` para fijar un nivel temporal de
  transporte o anonimato para una única solicitud; fornecer qualquer um dos campos pula
  o degradación por apagón y falha quando o nivel solicitado nao pode ser atendido.
- Enlaces del sistema operativo Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) reutiliza el mismo programador, esto es lo que define
  `return_scoreboard=true` nesses helpers para recuperar los pesos calculados junto com
  recibos en trozos.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registra o transmite la cola OTLP
  Produziu um paquete de adopción. Cuando omitido, el cliente deriva automáticamente`region:<telemetry_region>` (o `chain:<chain_id>`) para que los metadatos siempre
  carregue um rotulo descriptivo.

## Obtener a través de `iroha::Client`

El SDK de Rust incluye el asistente de búsqueda de puerta de enlace; forneca um manifest maisscriptres de
proveedores (incluidos los tokens de transmisión) y el cliente que realiza la búsqueda de fuentes múltiples:

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

Defina `transport_policy` como `Some(TransportPolicy::SoranetStrict)` cuando se cargan
Es necesario recusar relés clásicos o `Some(TransportPolicy::DirectOnly)` cuando
SoraNet necesita ser totalmente bypassada. Aponte `scoreboard.persist_path` para o
directorio de artefatos de liberación, opcionalmente fijo `scoreboard.now_unix_secs` e preencha
`scoreboard.metadata` con contexto de captura (etiquetas de dispositivos, alvo Torii, etc.)
Para que `cargo xtask sorafs-adoption-check` consuma JSON determinístico entre SDK
com o blob de procedencia que SF-6c espera.
`Client::sorafs_fetch_via_gateway` ahora aumenta estos metadatos con el identificador de
manifiesto, una expectativa opcional de manifiesto CID y bandera `gateway_manifest_provided`
inspeccionando el `GatewayFetchConfig` fornecido, para que capturas que incluyan um
manifiesto sobre assinado atendam ao requisito de evidencia SF-6c sem duplicar esses
campos manualmente.

## Ayudantes de manifiesto

`ManifestBuilder` continúa enviando a forma canónica de montar payloads Norito de forma
programática:

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

Incorporar o constructor siempre que los servicios precisarem gerar se manifiesten en tiempo real; oh
CLI sigue el camino recomendado para tuberías determinísticas.