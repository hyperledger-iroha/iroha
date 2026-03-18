---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-rust
titre : Extraits SDK Rust
sidebar_label : Extraits Rust
description : Exemples Rust minimaux pour consommer les proof streams et les manifestes.
---

:::note Source canonique
:::

Les caisses Rust de ce dépôt alimentent le CLI et peuvent être embarquées dans des
orchestrateurs ou services personnalisés. Les extraits ci-dessous mettent en avant
les helpers les plus demandés.

## Flux de preuve d'assistance

Réutilisez le parser proof stream existant pour agréger des métriques depuis une
réponse HTTP :

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

La version complète (avec tests) est dans `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` rend le même JSON de métriques que le CLI, ce qui
faciliter l'alimentation des backends d'observabilité ou des assertions CI.

## Notation de la récupération multi-sources

Le module `sorafs_car::multi_fetch` expose le planificateur de récupération asynchrone utilisé
par CLI. Implémentez `sorafs_car::multi_fetch::ScorePolicy` et passez-le via
`FetchOptions::score_policy` pour ajuster l'ordre des fournisseurs. Le test unitaire
`multi_fetch::tests::score_policy_can_filter_providers` montre comment imposer des
préférences personnalisées.

Autres boutons alignés sur les flags CLI :- `FetchOptions::per_chunk_retry_limit` correspond au drapeau `--retry-budget` pour des
  run CI qui contraigne volontairement les tentatives.
- Combinez `FetchOptions::global_parallel_limit` avec `--max-peers` pour plafonner le
  nombre de fournisseurs concurrents.
- `OrchestratorConfig::with_telemetry_region("region")` tague les métriques
  `sorafs_orchestrator_*`, tandis que `OrchestratorConfig::with_transport_policy`
  reflète le drapeau CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` est
  livré comme valeur par défaut côté CLI/SDK ; utiliser `TransportPolicy::DirectOnly`
  uniquement lors d'un downgrade ou sur directive de conformité, et réserver
  `SoranetStrict` aux pilotes PQ-only avec approbation explicite.
- Définissez `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` pour forcer les uploads PQ-only ; l'assistant
  promet automatiquement les politiques de transport/anonymat sauf override explicite.
- Utilisez `SorafsGatewayFetchOptions::policy_override` pour verrouiller un niveau de
  transport ou d'anonymat temporaire pour une requête ; fournir l'un des champs
  contourne la dégradation brownout et échoue si le niveau demandé ne peut pas être
  satisfait.
- Les liaisons Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) et
  JavaScript (`sorafsMultiFetchLocal`) réutilise le même planificateur ; défini
  `return_scoreboard=true` dans ces helpers pour récupérer les poids calculés en même
  temps que les recettes de chunk.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` enregistre le flux OTLP
  qui a produit un bundle d'adoption. S'il est omis, le client dérive automatiquement`region:<telemetry_region>` (ou `chain:<chain_id>`) afin que les métadonnées portent
  toujours une étiquette descriptive.

## Récupérer via `iroha::Client`

Le SDK Rust embarque l'assistant de gateway fetch ; fournissez un manifeste plus des
descripteurs de fournisseurs (y compris des stream tokens) et laissez le client piloter
le fetch multi-source :

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

Définissez `transport_policy` sur `Some(TransportPolicy::SoranetStrict)` lorsque les
les téléchargements doivent refuser les relais classiques, ou sur `Some(TransportPolicy::DirectOnly)`
quand SoraNet doit être entièrement profilé. Pointez `scoreboard.persist_path` vers le
répertoire d'artefacts de release, fixez éventuellement `scoreboard.now_unix_secs` et
renseignez `scoreboard.metadata` avec le contexte de capture (labels de luminaires, cible
Torii, etc.) afin que `cargo xtask sorafs-adoption-check` consomme un JSON déterministe
entre SDK avec le blob de provenance attendu par SF-6c.
`Client::sorafs_fetch_via_gateway` enrichit désormais ces métadonnées avec l'identifiant
manifest, l'attente éventuelle de manifest CID et le flag `gateway_manifest_provided` fr
inspectant le `GatewayFetchConfig` fourni, de sorte que les captures incluant une enveloppe
manifeste signé satisfaisant l'exigence de preuve SF-6c sans dupliquer ces champs à la main.

## Helpers de manifeste

`ManifestBuilder` reste la façon canonique d'assembler des charges utiles Norito de façon
programmatique :

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
```Intégrez le constructeur partout où les services doivent générer des manifestes à la volée ;
le CLI reste la voie recommandée pour les pipelines déterministes.