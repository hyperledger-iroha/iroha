---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-rust
titre : Extraits du SDK Rust
sidebar_label : extraits de rouille
description : Les flux de preuves et les manifestes consomment des exemples minimaux de Rust
---

:::note مستند ماخذ
:::

Le référentiel et Rust crates CLI ainsi que l'alimentation et les orchestrateurs personnalisés ainsi que les services et l'intégration sont également disponibles.
Il y a des extraits de code et des aides pour mettre en évidence les détails de votre projet.

## Assistant de flux de preuve

Réponse HTTP et agrégat de métriques pour la réutilisation de l'analyseur de flux de preuves existant :

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

Version complète (tests en cours) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` et les métriques de rendu JSON sont disponibles et CLI est disponible
backends d'observabilité et flux d'assertions CI

## Score de récupération multi-sources

Le module `sorafs_car::multi_fetch` et le planificateur de récupération asynchrone exposent les fonctionnalités de la CLI
`sorafs_car::multi_fetch::ScorePolicy` implémente le pass et `FetchOptions::score_policy` passe le pass
تاکہ fournisseur de mélodie de commande ہو سکے۔ Test unitaire `multi_fetch::tests::score_policy_can_filter_providers`
les préférences personnalisées sont appliquées

Autres boutons CLI drapeaux et miroirs:- `FetchOptions::per_chunk_retry_limit` CI exécute l'indicateur `--retry-budget` et correspond à l'indicateur `FetchOptions::per_chunk_retry_limit`.
  جو retries کو جان بوجھ کر contrainte کرتے ہیں۔
- `FetchOptions::global_parallel_limit` et `--max-peers` combinent deux éléments simultanément
  fournisseurs کی تعداد cap ہو۔
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` métriques et balise de sécurité
  `OrchestratorConfig::with_transport_policy` CLI et `--transport-policy` drapeau et miroir blanc
  Surfaces CLI/SDK `TransportPolicy::SoranetPreferred` livrées par défaut ہے؛ `TransportPolicy::DirectOnly`
  L'étape de déclassement et la directive de conformité suivent la procédure de déclassement et la directive `SoranetStrict`.
  Pilotes PQ uniquement avec approbation explicite et réserve
- `SorafsGatewayFetchOptions ::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` set pour forcer les téléchargements PQ uniquement aide au transport/anonymat
  politiques کو خودکار طور پر promouvoir کرے گا جب تک واضح override نہ ہو۔
- `SorafsGatewayFetchOptions::policy_override` demande de transport temporaire
  یا épinglette de niveau d'anonymat ہو جائے؛ کسی بھی field کے دینے سے baisse de tension rétrogradation skip ہوتا ہے اور اگر
  niveau demandé satisfaire نہ ہو سکے تو échec آتا ہے۔
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) et JavaScript (`sorafsMultiFetchLocal`)
  liaisons pour planificateur et réutilisation pour les assistants comme `return_scoreboard=true` set pour les tâches
  poids calculés reçus de morceaux کے ساتھ مل سکیں۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` Flux OTLP et enregistrement pour un bundle d'adoptionبنایا تھا۔ Omettre le client et dériver `region:<telemetry_region>` (`chain:<chain_id>`)
  تاکہ métadonnées میں ہمیشہ étiquette descriptive رہے۔

## Récupérer via `iroha::Client`

Rust SDK est un outil d'aide à la récupération de passerelle. manifeste et descripteurs de fournisseur (jetons de flux)
Le client et le lecteur de récupération multi-sources sont les suivants :

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

Les téléchargements de relais classiques sont effectués par `transport_policy` et `Some(TransportPolicy::SoranetStrict)`
Pour définir le contournement de SoraNet, utilisez `Some(TransportPolicy::DirectOnly)`.
`scoreboard.persist_path` et le répertoire des artefacts de version au point de départ pour le correctif `scoreboard.now_unix_secs`.
Pour `scoreboard.metadata`, le contexte de capture (étiquettes de luminaire, cible Torii, et) est configuré pour
Les SDK `cargo xtask sorafs-adoption-check` pour JSON déterministe consomment du blob de provenance SF-6c et
inclure رہے۔ `Client::sorafs_fetch_via_gateway` est une métadonnée et un identifiant de manifeste, une attente facultative du CID du manifeste.
Le drapeau `gateway_manifest_provided` est augmenté par l'intermédiaire de l'enveloppe manifeste signée `GatewayFetchConfig` fournie.
Le système capture les exigences en matière de preuves SF-6c et les champs sont en double.

## Aides au manifeste

`ManifestBuilder` pour les charges utiles Norito assemblent par programme des fichiers canoniques :

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
```Les services et les manifestes à la volée génèrent des fichiers et des constructeurs intégrés. pipelines déterministes et CLI et chemin recommandé