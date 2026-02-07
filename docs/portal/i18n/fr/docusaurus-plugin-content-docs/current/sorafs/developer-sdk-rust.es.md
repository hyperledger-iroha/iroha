---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-rust
titre : Fragments du SDK de Rust
sidebar_label : Fragments de Rust
description : Exemples minimes en Rust pour consommer des flux et des manifestes de preuve.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/sdk/rust.md`. Mantén est une copie synchronisée.
:::

Les caisses de Rust dans ce dépôt sont activées par la CLI et peuvent être incrustées à l'intérieur de
orquestadores o servicios personalizados. Los fragmentos de abajo resaltan los helpers
que plus piden los desarrolladores.

## Helper de preuve de flux

Réutilisez l'analyseur de flux de preuve existant pour collecter des données d'une réponse HTTP :

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

La version complète (avec tests) est disponible en `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` rend le JSON similaire à celui de la CLI, comme ça
faciliter l'alimentation des backends d'observabilité ou des applications de CI.

## Point de récupération multi-source

Le module `sorafs_car::multi_fetch` expose le planificateur de récupération comme un chronomètre utilisé par le
CLI. Implémentez le `sorafs_car::multi_fetch::ScorePolicy` et passalo via
`FetchOptions::score_policy` pour ajuster l'ordre des fournisseurs. Le test unitaire
`multi_fetch::tests::score_policy_can_filter_providers` doit être utilisé pour forcer
préférences personnalisées.

D'autres boutons reflètent les drapeaux de la CLI :- `FetchOptions::per_chunk_retry_limit` coïncide avec le drapeau `--retry-budget` pour
  ejecuciones de CI qui restringen los reintentos a propósito.
- Combiner `FetchOptions::global_parallel_limit` avec `--max-peers` pour limiter la
  cantidad de provenedores concurrentes.
- `OrchestratorConfig::with_telemetry_region("region")` étiquette des valeurs métriques
  `sorafs_orchestrator_*`, mets que `OrchestratorConfig::with_transport_policy`
  reflète le drapeau `--transport-policy` de la CLI. `TransportPolicy::SoranetPreferred`
  il entre comme valeur par défaut dans la surface CLI/SDK ; Etats-Unis
  `TransportPolicy::DirectOnly` juste pour préparer un downgrade ou suivre une directive de
  conformité, et réserve `SoranetStrict` pour les pilotes PQ uniquement avec approbation explicite.
- Configurer `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` pour remplacer les PQ uniquement ; l'assistant promouvra
  automatiquement la politique de transport/anonimato salvo qui se surscriban
  explicitement.
- Usa `SorafsGatewayFetchOptions::policy_override` pour fixer un transport ou un niveau de
  anonimato temporal para una sola sollicitud ; en proposant n'importe quelle personne des champs
  se omettre la dégradation par baisse de tension et tomber si le niveau sollicité ne peut pas
  satisfaire.
- Les liaisons de Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) et
  JavaScript (`sorafsMultiFetchLocal`) réutilise le planificateur mismo, ainsi que le configure
  `return_scoreboard=true` et esos helpers pour récupérer les pesos calculés ensemble avec
  los recibos de chunk.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` enregistre le flux OTLP queprodujo un bundle d’adoption. Quand le client est omite
  `region:<telemetry_region>` (ou `chain:<chain_id>`) automatiquement pour que le
  les métadonnées donnent toujours une étiquette descriptive.

## Récupérer via `iroha::Client`

Le SDK de Rust intègre l'assistant de récupération de passerelle ; proposer un manifeste plus
descripteurs des fournisseurs (y compris les jetons de flux) et déjà que le client exécute
el récupérer multi-source :

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

Configurer `transport_policy` comme `Some(TransportPolicy::SoranetStrict)` lorsque la
subidas deban rechazar relais classiques, ou `Some(TransportPolicy::DirectOnly)` quand
SoraNet doit être omis complètement. Apunta `scoreboard.persist_path` au directeur de
artefactos de release, fija facultativemente `scoreboard.now_unix_secs` et completa
`scoreboard.metadata` avec le contexte de capture (étiquettes des appareils, cible Torii, etc.)
pour que `cargo xtask sorafs-adoption-check` utilise JSON pour déterminer les SDK avec
le blob de procédure qui attend SF-6c.
`Client::sorafs_fetch_via_gateway` vient maintenant compléter ces métadonnées avec l'identifiant
 du manifeste, l'attente facultative du manifeste CID et le drapeau
`gateway_manifest_provided` inspectant le `GatewayFetchConfig` conjointement, de façon
que captures qui incluent un envoltorio de manifeste firmado cumplan el requisito de
les tests SF-6c ne doivent pas être dupliqués manuellement.

## Helpers de manifeste

`ManifestBuilder` suivre la forme canonique d'assemblage des charges utiles Norito de forme
programmatique :

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
```Incorporez le constructeur aux services nécessaires pour générer des manifestes à vuelo ; el
CLI suit l'itinéraire recommandé pour les pipelines déterministes.