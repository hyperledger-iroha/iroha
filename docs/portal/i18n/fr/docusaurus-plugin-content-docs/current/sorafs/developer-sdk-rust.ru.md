---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-rust
titre : Captures d'écran du SDK pour Rust
sidebar_label : extraits de Rust
description : Exemples minimaux de Rust pour la diffusion de flux de preuve et de manifestes.
---

:::note Канонический источник
:::

Rust crates dans ce dépôt CLI et vous pouvez utiliser les outils de stockage
orchestrateurs ou services. Les extraits de code pour les assistants, qui sont pour vous
нужны разработчикам.

## Aide pour le flux de preuves

Utilisez l'analyseur de flux de preuve approprié pour obtenir des mesures
Réponse HTTP :

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

La version (avec les tests) est disponible dans `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` rend la mesure JSON, qui et CLI, mise à jour
Cela concerne le backend d'observabilité ou les assertions CI.

## Notation de la récupération multi-sources

Le module `sorafs_car::multi_fetch` utilise un planificateur de récupération automatique
CLI. Réalisez `sorafs_car::multi_fetch::ScorePolicy` et attendez-vous à cela
`FetchOptions::score_policy`, que devez-vous choisir pour les fournisseurs. Юнит-test
`multi_fetch::tests::score_policy_can_filter_providers` est prêt à vous aider
кастомные предпочтения.

Les différents boutons répondent au signal CLI :- `FetchOptions::per_chunk_retry_limit` signale le drapeau `--retry-budget` pour CI
  запусков, которые намеренно ограничивают tentatives.
- Combinez `FetchOptions::global_parallel_limit` avec `--max-peers` pour obtenir
  количество одновременных провайдеров.
- `OrchestratorConfig::with_telemetry_region("region")` mesures de mesure
  `sorafs_orchestrator_*`, et `OrchestratorConfig::with_transport_policy` sont disponibles
  drapeau CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` pour l'installation
  dans le CLI/SDK le plus performant ; utilisez `TransportPolicy::DirectOnly` uniquement pour la mise en scène
  rétrograder ou respecter la directive de conformité et réserver `SoranetStrict` pour
  Les pilotes PQ uniquement sont pour vous.
- Installez `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` permet de télécharger des téléchargements PQ uniquement ; assistant automatique
  Il s'agit d'une politique de transport/anonymat, sauf si cela n'est pas annulé.
- Utilisez `SorafsGatewayFetchOptions::policy_override` pour désactiver la valeur actuelle
  transport ou niveau d'anonymat pour les demandes de transport ; подача любого поля пропускает
  la rétrogradation en cas de baisse de tension et la suppression du niveau ne peuvent pas être prises en compte.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) et JavaScript
  (`sorafsMultiFetchLocal`) les liaisons utilisent ce planificateur, puis le planifier
  `return_scoreboard=true` dans ces assistants, qui peuvent travailler ensemble avec vous
  reçus en morceaux.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` supprime le flux OTLP,
  paquet d'adoption сформировавший. Si ce n'est pas le cas, le client vous contactera automatiquement
  `region:<telemetry_region>` (ou `chain:<chain_id>`) que les métadonnées sont disponibles iciÉtiquette описательный.

## Récupérer ici `iroha::Client`

Le SDK Rust offre une aide pour la récupération de la passerelle ; передайте manifest и дескрипторы провайдеров
(avec les jetons de flux) et votre client peut désormais utiliser la récupération multi-source :

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

Installez `transport_policy` dans `Some(TransportPolicy::SoranetStrict)`, ainsi que les téléchargements
Vous devez activer les relais classiques, ou dans `Some(TransportPolicy::DirectOnly)`, alors
SoraNet doit être mis à jour. Téléchargez `scoreboard.persist_path` pour le directeur
Опционально зафиксируйте `scoreboard.now_unix_secs` и заполните
`scoreboard.metadata` контекстом захвата (étiquettes, цель Torii и т.д.) так,
que `cargo xtask sorafs-adoption-check` permet de déterminer les SDK JSON
Avec la provenance du blob, celui-ci indique SF-6c.
`Client::sorafs_fetch_via_gateway` теперь дополняет эти métadonnées d'identification du manifeste,
Analyse optionnelle du manifeste CID et du drapeau `gateway_manifest_provided`
Le `GatewayFetchConfig` est destiné à recevoir le manifeste d'enveloppe enregistré
требованию доказательств SF-6c без ручного дублирования этих полей.

## Aides pour le manifeste

`ManifestBuilder` permet de programmer des charges utiles Norito :

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

Si vous créez un constructeur maintenant, votre service doit générer des manifestes pour vous ; CLI
Nous proposons des installations recommandées pour la détection des pipelines.