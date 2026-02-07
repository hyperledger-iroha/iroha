---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-sdk-rust
título: Fragmentos del SDK de Rust
sidebar_label: fragmentos de óxido
Descripción: Los flujos de prueba y los manifiestos consumen una cantidad mínima de ejemplos de Rust.
---

:::nota مستند ماخذ
:::

اس repositorio کے Rust crates CLI کو potencia کرتے ہیں اور orquestadores personalizados یا servicios میں incrustar کیے جا سکتے ہیں۔
Varios fragmentos y ayudantes y resaltados کرتے ہیں جن کی طلب سب سے زیادہ ہوتی ہے۔

## Ayudante de flujo de prueba

Respuesta HTTP y métricas agregadas para la reutilización del analizador de flujo de prueba existente:

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

Versión completa (pruebas سمیت) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` y métricas de renderizado JSON کرتا ہے جو CLI دیتا ہے، جس سے
backends de observabilidad یا Feed de aserciones de CI کرنا آسان ہوتا ہے۔

## Puntuación de búsqueda de múltiples fuentes

El módulo `sorafs_car::multi_fetch` y el programador de recuperación asíncrono exponen la configuración de CLI استعمال کرتا ہے۔
`sorafs_car::multi_fetch::ScorePolicy` implementar کریں اور `FetchOptions::score_policy` کے ذریعے pasar کریں
تاکہ melodía de pedido del proveedor ہو سکے۔ Prueba unitaria `multi_fetch::tests::score_policy_can_filter_providers`
las preferencias personalizadas imponen کرنے کا طریقہ دکھاتا ہے۔

Otras perillas Banderas CLI کو mirror کرتے ہیں:- `FetchOptions::per_chunk_retry_limit` CI ejecuta کے لیے `--retry-budget` bandera سے coincidencia کرتا ہے
  جو reintentos کو جان بوجھ کر restringir کرتے ہیں۔
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ combinar کریں تاکہ concurrente
  proveedores کی تعداد cap ہو۔
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` métricas etiqueta کرتا ہے،
  جبکہ `OrchestratorConfig::with_transport_policy` CLI کے `--transport-policy` bandera کو espejo کرتا ہے۔
  `TransportPolicy::SoranetPreferred` Superficies CLI/SDK enviadas por defecto ہے؛ `TransportPolicy::DirectOnly`
  صرف etapa de degradación کرنے یا directiva de cumplimiento siga کرنے پر استعمال کریں، اور `SoranetStrict` کو
  Pilotos exclusivos de PQ کے لیے aprobación explícita کے ساتھ reserva کریں۔
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` establece que las cargas solo PQ fuerzan ہوں؛ transporte de ayuda/anonimato
  políticas کو خودکار طور پر promover کرے گا جب تک واضح anular نہ ہو۔
- `SorafsGatewayFetchOptions::policy_override` استعمال کریں تاکہ ایک solicitud کے لیے transporte temporal
  یا pin de nivel de anonimato ہو جائے؛ کسی بھی campo کے دینے سے caída de caída omitir degradación ہوتا ہے اور اگر
  el nivel solicitado satisface نہ ہو سکے تو falla آتا ہے۔
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) y JavaScript (`sorafsMultiFetchLocal`)
  enlaces اسی planificador کو reutilización کرتے ہیں، اس لیے ان ayudantes میں `return_scoreboard=true` set کریں تاکہ
  pesos calculados recibos de fragmentos کے ساتھ مل سکیں۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` Transmisión OTLP کو registro کرتا ہے جس نے paquete de adopciónبنایا تھا۔ اگر omitir ہو تو cliente خودکار طور پر `region:<telemetry_region>` (یا `chain:<chain_id>`) derivar کرتا ہے
  تاکہ metadatos میں ہمیشہ etiqueta descriptiva رہے۔

## Obtener a través de `iroha::Client`

Asistente de búsqueda de puerta de enlace Rust SDK میں شامل ہے؛ manifiesto کے ساتھ descriptores de proveedor (stream tokens سمیت) فراہم کریں
El cliente y la unidad de recuperación de múltiples fuentes son:

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

جب cargas کو relés clásicos سے انکار کرنا ہو تو `transport_policy` کو `Some(TransportPolicy::SoranetStrict)`
پر set کریں، یا جب SoraNet کو مکمل bypass کرنا ہو تو `Some(TransportPolicy::DirectOnly)` پر۔
`scoreboard.persist_path` کو directorio de artefactos de lanzamiento پر point کریں، ضرورت ہو تو `scoreboard.now_unix_secs` fix کریں،
اور `scoreboard.metadata` میں contexto de captura (etiquetas de accesorios, objetivo Torii, وغیرہ) شامل کریں تاکہ
Los SDK `cargo xtask sorafs-adoption-check` de JSON determinista consumen SF-6c y blob de procedencia.
incluir رہے۔ `Client::sorafs_fetch_via_gateway` contiene metadatos y identificador de manifiesto, expectativa de CID de manifiesto opcional,
اور `gateway_manifest_provided` flag کے ساتھ aumentar کرتا ہے a través del `GatewayFetchConfig` suministrado, تاکہ sobre de manifiesto firmado
شامل کرنے والی captura el requisito de evidencia SF-6c پورا کریں بغیر ان campos کو دستی طور پر duplicado کیے۔

## Ayudantes manifiestos

`ManifestBuilder` اب بھی Las cargas útiles Norito ensamblan mediante programación کرنے کا canonical طریقہ ہے:

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
```Varios servicios y manifiestos sobre la marcha generan کرنے ہوں وہاں incrustar کریں؛ canalizaciones deterministas کے لیے CLI ابھی بھی ruta recomendada ہے۔