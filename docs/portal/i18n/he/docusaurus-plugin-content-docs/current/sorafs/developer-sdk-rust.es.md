---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-rust
כותרת: Fragmentos de SDK de Rust
sidebar_label: Fragmentos de Rust
תיאור: Ejemplos mínimos en Rust para consumir proof streams y manifests.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/developer/sdk/rust.md`. Mantén ambas copias sincronizadas.
:::

Los crates de Rust en este repositorio impulsan el CLI y pueden incrustarse dentro de
אופקיסטדורים או שירותים אישיים. Los fragmentos de abajo resaltan los helpers
que más piden los desarrolladores.

## זרם עוזר הוכחה

Reutiliza el parser de proof stream existente para agregar métricas de una respuesta HTTP:

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

La version completa (con tests) vive en `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza el mismo JSON de métricas que el CLI, lo que
facilita alimentar backends de observabilidad o aserciones de CI.

## אחזור מקורות מרובים

El módulo `sorafs_car::multi_fetch` expone el scheduler de fetch asíncrono usado por el
CLI. Implementa `sorafs_car::multi_fetch::ScorePolicy` y pásalo vía
`FetchOptions::score_policy` para ajustar el orden de proveedores. El test unitario
`multi_fetch::tests::score_policy_can_filter_providers` muestra cómo forzar
העדפות אישיות.

אוטרוס מכוון את דגלי ה-CLI:

- `FetchOptions::per_chunk_retry_limit` עולה בקנה אחד עם דגל `--retry-budget` para
  ejecuciones de CI que restringen los reintentos a propósito.
- Combina `FetchOptions::global_parallel_limit` con `--max-peers` para limitar la
  cantidad de proveedores concurrentes.
- `OrchestratorConfig::with_telemetry_region("region")` כללי כללי התנהגות
  `sorafs_orchestrator_*`, mientras que `OrchestratorConfig::with_transport_policy`
  refleja el flag `--transport-policy` del CLI. `TransportPolicy::SoranetPreferred`
  se entrega como valor por defecto en superficies CLI/SDK; ארה"ב
  `TransportPolicy::DirectOnly` סולו על הכנה ושיפור שדרוג לאחור או סגירת הנחיות
  תאימות, y reserva `SoranetStrict` ל-PQ-only pilotos with aprobación explícita.
- הגדרות `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forzar subidas PQ-only; el helper promoverá
  automáticamente las políticas de transporte/anonimato salvo que se sobrescriban
  explícitamente.
- Usa `SorafsGatewayFetchOptions::policy_override` para fijar un transporte o tier de
  anonimato temporal para una sola solicitud; al proporcionar cualquiera de los campos
  se omite la degradación por brownout y falla si el tier solicitado no puede
  סיפוק.
- Los bindings de Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) y
  JavaScript (`sorafsMultiFetchLocal`) מחדש את מתזמן Mismo, כמו הגדרה
  `return_scoreboard=true` en esos helpers para recuperar los pesos calculados junto con
  los recibos de chunk.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` רישום אל זרם OTLP que
  הפקה un bundle de adopción. Cuando se omite, el cliente deriva
  `region:<telemetry_region>` (o `chain:<chain_id>`) אוטומטית
  metadata siempre lleve una ethiqueta descriptiva.

## אחזר דרך `iroha::Client`

El SDK de Rust כולל את העוזר לשער הבא; proporciona un manifest más los
descriptores de proveedores (כולל אסימוני זרם) y deja que el cliente ejecute
el fetch multi-source:

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
```Configura `transport_policy` como `Some(TransportPolicy::SoranetStrict)` cuando las
subidas deban rechazar relays clasicos, o `Some(TransportPolicy::DirectOnly)` cuando
SoraNet deba omitirse por completo. Apunta `scoreboard.persist_path` אל המדריך
חפצי שחרור, פייה אופציונליים `scoreboard.now_unix_secs` y completa
`scoreboard.metadata` con contexto de captura (גינונים של גופי, יעד Torii וכו')
para que `cargo xtask sorafs-adoption-check` צרכן JSON קובע SDKs עם
el blob de procedencia que espera SF-6c.
`Client::sorafs_fetch_via_gateway` זה משלים את המטא נתונים עם מזהה
 de manifest, la expectativa optional de manifest CID y el flag
`gateway_manifest_provided` inspectionando el `GatewayFetchConfig` suministrado, de modo
que capturas que incluyen un envoltorio de manifest firmado cumplan el requisito de
pruebas SF-6c sin duplicar esos campos manualmente.

## עוזרי המניפסט

`ManifestBuilder` segue siendo la forma canónica de ensamblar מטענים Norito de forma
תוכנה:

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

אינcorpora el Builder donde los servicios necesiten generar manifests al vuelo; אל
CLI סיוגו סינדו לה רוטה המלצות עבור צינורות דטרמיניסטים.