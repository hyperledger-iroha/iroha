---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-rust
titre : Extraits du SDK Rust
sidebar_label : Extraits de Rust
description : exemples de Rust réduits pour consommer des flux et des manifestes de preuve.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas comme copies synchronisées.
:::

Os crates Rust est un dépôt d'aliments ou CLI et peut être embutidos em
orquestradores ou servicos customizados. Les extraits de code abaixo destacam os helpers que
mais aparecem nas sollicitacoes dos desenvolvedores.

## Helper de preuve de flux

Réutilisez l'analyseur de flux de preuve existant pour collecter des mesures d'une réponse HTTP :

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

La versao complète (avec testicules) vive em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` rend le fichier JSON de métriques de la CLI, facilitant
Alimenter les backends d'observabilité ou les assertions de CI.

## Scoring de récupération multi-source

Le module `sorafs_car::multi_fetch` expose le planificateur pour récupérer un ordinateur utilisé par lui
CLI. Implémentez `sorafs_car::multi_fetch::ScorePolicy` et passez via
`FetchOptions::score_policy` pour ajuster l'ordre des fournisseurs. Le test unitaire
`multi_fetch::tests::score_policy_can_filter_providers` montre les préférences importantes
personnalisées.

Les autres boutons Espelham Flags font CLI :- `FetchOptions::per_chunk_retry_limit` correspond au drapeau `--retry-budget` pour les courses
  de CI que restringem tentativas propositionitalmente.
- Combiner `FetchOptions::global_parallel_limit` avec `--max-peers` pour limiter le numéro
  de proveneurs concorrentes.
- Marque `OrchestratorConfig::with_telemetry_region("region")` comme métriques
  `sorafs_orchestrator_*`, quant à `OrchestratorConfig::with_transport_policy` Espelha
  o indicateur CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` et le NAS par défaut
  CLI/SDK superficiel ; utilisez `TransportPolicy::DirectOnly` pour préparer un déclassement
  ou suivez une directive de conformité, et réservez `SoranetStrict` pour les pilotes PQ uniquement
  com aprovacao explicita.
- Définissez `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` pour télécharger des fichiers PQ uniquement ; o aide à la promotion
  automatiquement en tant que politique de transport/anonymat à moins que cela se fasse explicitement
  sobrescritas.
- Utilisez `SorafsGatewayFetchOptions::policy_override` pour fixer un niveau temporaire de
  transporte ou anonimato pour une demande unique ; fornecer qualquer um dos campos pula
  La rétrogradation en cas de baisse de tension est fausse lorsque le niveau sollicité n'est pas disponible.
- Liaisons du système d'exploitation Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) réutilise le planificateur de messages, puis définit
  `return_scoreboard=true` aides nécessaires pour récupérer les pesos calculés avec
  reçus en morceaux.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` enregistre le flux OTLP que
  produire un pack d'adoption. Lorsqu'il est omis, le client dérive automatiquement`region:<telemetry_region>` (ou `chain:<chain_id>`) pour que les métadonnées soient toujours
  carregue um rotulo descritivo.

## Récupérer via `iroha::Client`

Le SDK Rust inclut l'assistant de récupération de passerelle ; forneca um manifest plus descripteurs de
fournisseurs (y compris les jetons de flux) et le client qui permet de récupérer plusieurs sources :

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

Définir `transport_policy` comme `Some(TransportPolicy::SoranetStrict)` lors des téléchargements
precisarem recusar relays classicos, ou `Some(TransportPolicy::DirectOnly)` quando
SoraNet précise être totalement contourné. Pont `scoreboard.persist_path` pour o
répertoire des articles de version, optionnellement corrigé `scoreboard.now_unix_secs` et preencha
`scoreboard.metadata` avec le contexte de capture (étiquettes de luminaires, alvo Torii, etc.)
pour que `cargo xtask sorafs-adoption-check` consomme du JSON déterminant entre les SDK
com o blob de provenance que SF-6c espère.
`Client::sorafs_fetch_via_gateway` vient d'augmenter ces métadonnées avec l'identifiant de
manifeste, une attente facultative du manifeste CID et du drapeau `gateway_manifest_provided`
inspecionando o `GatewayFetchConfig` fornecido, para que capturas qui incluem um
enveloppe manifeste assinado atendam ao requisito de evidencia SF-6c sem duplicar esses
campez manuellement.

## Helpers de manifeste

`ManifestBuilder` continue d'envoyer des charges utiles de montage canoniques de forme Norito de forme
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
```

Incorporer le constructeur toujours que les services précis se manifestent à un rythme réel ; o
La CLI suit l'envoi du chemin recommandé pour la détermination des pipelines.