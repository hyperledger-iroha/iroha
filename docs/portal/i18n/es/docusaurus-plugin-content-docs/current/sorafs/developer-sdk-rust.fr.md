---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-sdk-rust
título: Extraits SDK Rust
sidebar_label: Extraits Rust
descripción: Ejemplos de Rust minimaux para consumir los flujos de prueba y los manifiestos.
---

:::nota Fuente canónica
:::

Les crates Rust de ce dépôt alimentent le CLI et peuvent être embarqués dans des
orquestadores o servicios personalizados. Les extraits ci-dessous mettent en avant
les helpers les plus demandés.

## Flujo de prueba de ayuda

Reutilice el flujo de prueba del analizador existente para agregar mediciones después de una
respuesta HTTP:

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

La versión completa (con pruebas) está en `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` genera el mismo JSON de métricas que la CLI, aquí
Facilita la alimentación de los backends de observabilidad o de las afirmaciones de CI.

## Puntuación de búsqueda de múltiples fuentes

El módulo `sorafs_car::multi_fetch` expone el programador de búsqueda asíncrono utilizado
por CLI. Implemente `sorafs_car::multi_fetch::ScorePolicy` y páselo vía
`FetchOptions::score_policy` para ajustar el orden de los proveedores. La prueba unitaria
`multi_fetch::tests::score_policy_can_filter_providers` montre comentario imponente des
preferencias personalizadas.

Otras perillas alineadas en las banderas CLI:- `FetchOptions::per_chunk_retry_limit` corresponde a la bandera `--retry-budget` para des
  ejecuta CI que contradice voluntariamente los reintentos.
- Combinez `FetchOptions::global_parallel_limit` con `--max-peers` para placa
  nombre de proveedores concurrentes.
- `OrchestratorConfig::with_telemetry_region("region")` etiquetar las métricas
  `sorafs_orchestrator_*`, tandis que `OrchestratorConfig::with_transport_policy`
  Refleje la bandera CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` est
  libro con el valor predeterminado en el código CLI/SDK; utilizado `TransportPolicy::DirectOnly`
  Uniquement lors d'un downgrade ou sur directiva de conformidad, et réservez
  `SoranetStrict` Pilotos auxiliares solo PQ con aprobación explícita.
- Definir `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forzar cargas solo PQ; el ayudante
  Promeut automatiquement les politiques de transport/anonymat sauf anular explícitamente.
- Utilice `SorafsGatewayFetchOptions::policy_override` para bloquear un nivel de
  transporte o transporte anónimo temporal para una solicitud; Fournir l'un des champs
  Contorno de la degradación del apagón y eco si el nivel demandado no puede ser
  satisfacer.
- Los enlaces Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) y
  JavaScript (`sorafsMultiFetchLocal`) utiliza el mismo programador; definir
  `return_scoreboard=true` en estos ayudantes para recuperar los pesos calculados a mí mismo
  Temps que les recibos de trozos.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registrar el flujo OTLP
  qui a produit un paquete de adopción. S'il est omis, el cliente deriva automáticamente`region:<telemetry_region>` (ou `chain:<chain_id>`) según el presagio de los metadonnées
  toujours une étiquette descriptive.

## Obtener a través de `iroha::Client`

El SDK Rust incluye el asistente de recuperación de puerta de enlace; fournissez un manifest plus des
descriptores de proveedores (y que incluyen tokens de flujo) y permitir que el cliente pilote
le buscar múltiples fuentes:

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

Definir `transport_policy` en `Some(TransportPolicy::SoranetStrict)` cuando les
Las cargas no deben rechazar los relés clásicos o en `Some(TransportPolicy::DirectOnly)`.
Cuando SoraNet tiene todo el contorno. Pointez `scoreboard.persist_path` versión le
répertoire d'artefacts de release, fixez éventuellement `scoreboard.now_unix_secs` et
renseignez `scoreboard.metadata` con el contexto de captura (etiquetas de accesorios, cible
Torii, etc.) después de `cargo xtask sorafs-adoption-check` consume un JSON determinado
Entre los SDK con el blob de procedencia incluido en el SF-6c.
`Client::sorafs_fetch_via_gateway` Enriquece el desorden de estos metadonnées con el identificador
manifest, l'attente éventuelle de manifest CID et le flag `gateway_manifest_provided` es
Inspectant le `GatewayFetchConfig` fourni, de sorte que les captures include une sobreppe
manifiesto firmado que satisface la exigencia de preuve SF-6c sin duplicar estos campos en la principal.

## Ayudantes de manifiesto

`ManifestBuilder` reste la façon canonique d'assembler des payloads Norito de façon
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
```Intégrez le builder partout of où les services doivent générer des manifests à la volée;
La CLI resta la vía recomendada para las tuberías determinadas.