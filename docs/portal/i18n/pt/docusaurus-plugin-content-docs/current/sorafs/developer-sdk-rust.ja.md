---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd72de1f7eeb931cc52ed3afe0f587c382f3ed4a4962b6207c4710c13bb5f95
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: developer-sdk-rust
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas as copias sincronizadas.
:::

Os crates Rust neste repositorio alimentam o CLI e podem ser embutidos em
orquestradores ou servicos customizados. Os snippets abaixo destacam os helpers que
mais aparecem nas solicitacoes dos desenvolvedores.

## Helper de proof stream

Reutilize o parser de proof stream existente para agregar metricas de uma resposta HTTP:

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

A versao completa (com testes) vive em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza o mesmo JSON de metricas do CLI, facilitando
alimentar backends de observabilidade ou assertions de CI.

## Scoring de fetch multi-source

O modulo `sorafs_car::multi_fetch` expoe o scheduler de fetch assincrono usado pelo
CLI. Implemente `sorafs_car::multi_fetch::ScorePolicy` e passe via
`FetchOptions::score_policy` para ajustar a ordenacao de provedores. O teste unitario
`multi_fetch::tests::score_policy_can_filter_providers` mostra como impor preferencias
customizadas.

Outros knobs espelham flags do CLI:

- `FetchOptions::per_chunk_retry_limit` corresponde ao flag `--retry-budget` para runs
  de CI que restringem tentativas propositalmente.
- Combine `FetchOptions::global_parallel_limit` com `--max-peers` para limitar o numero
  de provedores concorrentes.
- `OrchestratorConfig::with_telemetry_region("region")` marca as metricas
  `sorafs_orchestrator_*`, enquanto `OrchestratorConfig::with_transport_policy` espelha
  o flag CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` e o default nas
  superficies CLI/SDK; use `TransportPolicy::DirectOnly` apenas ao preparar um downgrade
  ou seguir uma diretiva de compliance, e reserve `SoranetStrict` para pilotos PQ-only
  com aprovacao explicita.
- Defina `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forcar uploads PQ-only; o helper promove
  automaticamente as politicas de transporte/anonymity a menos que sejam explicitamente
  sobrescritas.
- Use `SorafsGatewayFetchOptions::policy_override` para fixar um tier temporario de
  transporte ou anonimato para um unico request; fornecer qualquer um dos campos pula
  o brownout demotion e falha quando o tier solicitado nao pode ser atendido.
- Os bindings Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) reutilizam o mesmo scheduler, entao defina
  `return_scoreboard=true` nesses helpers para recuperar os pesos calculados junto com
  chunk receipts.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registra o stream OTLP que
  produziu um adoption bundle. Quando omitido, o cliente deriva automaticamente
  `region:<telemetry_region>` (ou `chain:<chain_id>`) para que o metadata sempre
  carregue um rotulo descritivo.

## Fetch via `iroha::Client`

O SDK Rust inclui o helper de gateway fetch; forneca um manifest mais descritores de
provedores (incluindo stream tokens) e deixe o cliente conduzir o fetch multi-source:

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

Defina `transport_policy` como `Some(TransportPolicy::SoranetStrict)` quando uploads
precisarem recusar relays classicos, ou `Some(TransportPolicy::DirectOnly)` quando
SoraNet precisar ser totalmente bypassada. Aponte `scoreboard.persist_path` para o
diretorio de artefatos de release, opcionalmente fixe `scoreboard.now_unix_secs` e preencha
`scoreboard.metadata` com contexto de captura (labels de fixtures, alvo Torii, etc.)
para que `cargo xtask sorafs-adoption-check` consuma JSON deterministico entre SDKs
com o blob de provenance que SF-6c espera.
`Client::sorafs_fetch_via_gateway` agora aumenta esse metadata com o identificador de
manifest, a expectativa opcional de manifest CID e o flag `gateway_manifest_provided`
inspecionando o `GatewayFetchConfig` fornecido, para que capturas que incluem um
manifest envelope assinado atendam ao requisito de evidencia SF-6c sem duplicar esses
campos manualmente.

## Helpers de manifest

`ManifestBuilder` continua sendo a forma canonica de montar payloads Norito de forma
programatica:

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

Incorpore o builder sempre que servicos precisarem gerar manifests em tempo real; o
CLI segue sendo o caminho recomendado para pipelines deterministicos.
