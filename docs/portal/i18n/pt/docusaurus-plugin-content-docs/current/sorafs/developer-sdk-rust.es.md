---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-sdk-rust
título: Fragmentos de SDK de Rust
sidebar_label: Fragmentos de Ferrugem
description: Exemplos mínimos em Rust para consumir prova streams e manifestos.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas as cópias sincronizadas.
:::

As caixas de Rust neste repositório impulsionam a CLI e podem incrustar-se dentro dele
orquestradores ou serviços personalizados. Os fragmentos de baixo realçam os ajudantes
que mais piden os desarrolladores.

## Fluxo auxiliar de prova

Reutiliza o analisador de fluxo de prova existente para adicionar atributos de uma resposta HTTP:

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

A versão completa (com testes) está em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza o mesmo JSON de métricas que a CLI, o que
facilitar backends alimentares de observabilidade ou aserções de CI.

## Pontuação de busca de múltiplas fontes

O módulo `sorafs_car::multi_fetch` expõe o agendador de busca asíncrono usado pelo
CLI. Implemente `sorafs_car::multi_fetch::ScorePolicy` e passe via
`FetchOptions::score_policy` para ajustar a ordem dos provedores. O teste unitário
`multi_fetch::tests::score_policy_can_filter_providers` mostra como forçar
preferências personalizadas.

Outros botões refletem sinalizadores da CLI:

- `FetchOptions::per_chunk_retry_limit` coincide com a bandeira `--retry-budget` para
  execuções de CI que restringem as reintenções propositalmente.
- Combina `FetchOptions::global_parallel_limit` com `--max-peers` para limitar a
  cantidad de provadores simultâneos.
- `OrchestratorConfig::with_telemetry_region("region")` etiqueta as métricas
  `sorafs_orchestrator_*`, enquanto `OrchestratorConfig::with_transport_policy`
  reflita o sinalizador `--transport-policy` da CLI. `TransportPolicy::SoranetPreferred`
  se entrega como valor por defeito em superfícies CLI/SDK; EUA
  `TransportPolicy::DirectOnly` apenas para preparar um downgrade ou seguir uma diretriz de
  conformidade, e reserva `SoranetStrict` para pilotos somente PQ com aprovação explícita.
- Configurar `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forçar subidas PQ-only; o ajudante promoverá
  automaticamente as políticas de transporte/anonimato salvo que se sobrescrevem
  explicitamente.
- Usa `SorafsGatewayFetchOptions::policy_override` para fixar um transporte ou nível de
  anonimato temporal para uma solicitud; para fornecer qualquer quiera dos campos
  se omite a degradação por queda de energia e falha se o nível solicitado não puder
  satisfeito.
- As ligações do Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) reutiliza o mesmo agendador, assim como configura
  `return_scoreboard=true` en esos helpers para recuperar os pesos calculados junto com
  os recibos de pedaço.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registra o stream OTLP que
  produziu um pacote de adoção. Quando se omite, o cliente deriva
  `region:<telemetry_region>` (ou `chain:<chain_id>`) automaticamente para que o
  metadados sempre levam uma etiqueta descritiva.

## Buscar via `iroha::Client`

O SDK de Rust incorpora o auxiliar de busca de gateway; fornece um manifesto mais
descritores de provedores (incluindo tokens de fluxo) e deixe que o cliente execute
el buscar multi-fonte:

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
```Configure `transport_policy` como `Some(TransportPolicy::SoranetStrict)` quando
subidas deban rechazar relés clássicos, o `Some(TransportPolicy::DirectOnly)` quando
SoraNet deve ser omitido por completo. Aponta `scoreboard.persist_path` no diretório de
artefatos de lançamento, fixados opcionalmente `scoreboard.now_unix_secs` e completos
`scoreboard.metadata` com contexto de captura (etiquetas de fixtures, alvo Torii, etc.)
para que `cargo xtask sorafs-adoption-check` consuma JSON determinista entre SDKs com
o blob de procedência que espera SF-6c.
`Client::sorafs_fetch_via_gateway` agora complementa esses metadados com o identificador
 de manifesto, a expectativa opcional de manifesto CID e a bandeira
`gateway_manifest_provided` inspecionando o `GatewayFetchConfig` fornecido, de modo
que capturas que incluam um envoltório de manifesto firmado cumprem o requisito de
teste o SF-6c sem duplicar esses campos manualmente.

## Ajudantes de manifesto

`ManifestBuilder` segue a forma canônica de agrupar payloads Norito de forma
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
```

Incorpora o construtor onde os serviços necessários geram manifestos ao voo; el
CLI segue seguindo a rota recomendada para deterministas de pipelines.