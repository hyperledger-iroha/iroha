<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: be3b8a2a4eaec95f5d4390ef7d143cd25681cce8ca3b5c4733f8036487daac24
source_last_modified: "2025-11-15T11:19:21.711876+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: developer-sdk-rust
title: Extraits SDK Rust
sidebar_label: Extraits Rust
description: Exemples Rust minimaux pour consommer les proof streams et les manifests.
---

:::note Source canonique
:::

Les crates Rust de ce dépôt alimentent le CLI et peuvent être embarqués dans des
orchestrateurs ou services personnalisés. Les extraits ci-dessous mettent en avant
les helpers les plus demandés.

## Helper proof stream

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
facilite l'alimentation des backends d'observabilité ou des assertions CI.

## Scoring multi-source fetch

Le module `sorafs_car::multi_fetch` expose le scheduler de fetch asynchrone utilisé
par le CLI. Implémentez `sorafs_car::multi_fetch::ScorePolicy` et passez-le via
`FetchOptions::score_policy` pour ajuster l'ordre des providers. Le test unitaire
`multi_fetch::tests::score_policy_can_filter_providers` montre comment imposer des
préférences personnalisées.

Autres knobs alignés sur les flags CLI :

- `FetchOptions::per_chunk_retry_limit` correspond au flag `--retry-budget` pour des
  runs CI qui contraignent volontairement les retries.
- Combinez `FetchOptions::global_parallel_limit` avec `--max-peers` pour plafonner le
  nombre de providers concurrents.
- `OrchestratorConfig::with_telemetry_region("region")` tague les métriques
  `sorafs_orchestrator_*`, tandis que `OrchestratorConfig::with_transport_policy`
  reflète le flag CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` est
  livré comme valeur par défaut côté CLI/SDK ; utilisez `TransportPolicy::DirectOnly`
  uniquement lors d'un downgrade ou sur directive de conformité, et réservez
  `SoranetStrict` aux pilotes PQ-only avec approbation explicite.
- Définissez `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` pour forcer les uploads PQ-only ; le helper
  promeut automatiquement les politiques de transport/anonymat sauf override explicite.
- Utilisez `SorafsGatewayFetchOptions::policy_override` pour verrouiller un tier de
  transport ou d'anonymat temporaire pour une requête ; fournir l'un des champs
  contourne la dégradation brownout et échoue si le tier demandé ne peut pas être
  satisfait.
- Les bindings Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) et
  JavaScript (`sorafsMultiFetchLocal`) réutilisent le même scheduler ; définissez
  `return_scoreboard=true` dans ces helpers pour récupérer les poids calculés en même
  temps que les receipts de chunk.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` enregistre le flux OTLP
  qui a produit un bundle d'adoption. S'il est omis, le client dérive automatiquement
  `region:<telemetry_region>` (ou `chain:<chain_id>`) afin que les métadonnées portent
  toujours une étiquette descriptive.

## Fetch via `iroha::Client`

Le SDK Rust embarque le helper de gateway fetch ; fournissez un manifest plus des
descripteurs de providers (y compris des stream tokens) et laissez le client piloter
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
uploads doivent refuser les relays classiques, ou sur `Some(TransportPolicy::DirectOnly)`
quand SoraNet doit être entièrement contourné. Pointez `scoreboard.persist_path` vers le
répertoire d'artefacts de release, fixez éventuellement `scoreboard.now_unix_secs` et
renseignez `scoreboard.metadata` avec le contexte de capture (labels de fixtures, cible
Torii, etc.) afin que `cargo xtask sorafs-adoption-check` consomme un JSON déterministe
entre SDKs avec le blob de provenance attendu par SF-6c.
`Client::sorafs_fetch_via_gateway` enrichit désormais ces métadonnées avec l'identifiant
manifest, l'attente éventuelle de manifest CID et le flag `gateway_manifest_provided` en
inspectant le `GatewayFetchConfig` fourni, de sorte que les captures incluant une enveloppe
manifest signée satisfont l'exigence de preuve SF-6c sans dupliquer ces champs à la main.

## Helpers de manifest

`ManifestBuilder` reste la façon canonique d'assembler des payloads Norito de façon
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
```

Intégrez le builder partout où les services doivent générer des manifests à la volée ;
le CLI reste la voie recommandée pour les pipelines déterministes.
