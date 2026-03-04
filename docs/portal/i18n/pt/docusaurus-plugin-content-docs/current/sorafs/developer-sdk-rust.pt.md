---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-sdk-rust
título: Trechos do SDK Rust
sidebar_label: trechos de ferrugem
description: Exemplos Rust minimos para extrair proof streams e manifests.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas as cópias sincronizadas.
:::

Os crates Rust neste repositório alimentam o CLI e podem ser embutidos em
orquestradores ou serviços customizados. Os trechos abaixo destacam os ajudantes que
mais aparecem nas solicitações dos desenvolvedores.

## Fluxo auxiliar de prova

Reutilize o analisador de fluxo de prova existente para adicionar métricas de uma resposta HTTP:

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

A versão completa (com testes) vive em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza o mesmo JSON de métricas do CLI, facilitando
backends alimentares de observabilidade ou asserções de CI.

## Pontuação de busca de múltiplas fontes

O módulo `sorafs_car::multi_fetch` expoe o agendador de busca sincronizado usado pelo
CLI. Implemente `sorafs_car::multi_fetch::ScorePolicy` e passe via
`FetchOptions::score_policy` para ajustar a ordenação de provedores. O teste unitário
`multi_fetch::tests::score_policy_can_filter_providers` mostra como importar preferências
customizados.

Outros botões espelham flags do CLI:

- `FetchOptions::per_chunk_retry_limit` corresponde ao flag `--retry-budget` para corridas
  de CI que restringem experimentos propositalmente.
- Combine `FetchOptions::global_parallel_limit` com `--max-peers` para limitar o número
  de fornecedores concorrentes.
- `OrchestratorConfig::with_telemetry_region("region")` marca como métrica
  `sorafs_orchestrator_*`, enquanto `OrchestratorConfig::with_transport_policy` espelha
  o sinalizador CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` e o padrão nas
  superfícies CLI/SDK; use `TransportPolicy::DirectOnly` apenas ao preparar um downgrade
  ou seguir uma cláusula de conformidade, e reserve `SoranetStrict` para pilotos somente PQ
  com aprovação explícita.
- Defina `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forçar uploads somente PQ; o ajudante promove
  automaticamente as políticas de transporte/anonimato a menos que sejam explicitamente
  sobrescritas.
- Use `SorafsGatewayFetchOptions::policy_override` para fixar um nível temporário de
  transporte ou anonimato para um pedido único; fornecer qualquer um dos campos pula
  o brownout rebaixamento e falha quando o nível solicitado não pode ser atendido.
- Os enlaces Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) reutilizam o mesmo agendador, então definido
  `return_scoreboard=true` esses helpers para recuperar os pesos calculados junto com
  receitas em pedaços.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registra o stream OTLP que
  produziu um pacote de adoção. Quando omitido, o cliente deriva automaticamente
  `region:<telemetry_region>` (ou `chain:<chain_id>`) para que os metadados sempre
  carregue um rotulo descritivo.

## Buscar via `iroha::Client`

O SDK Rust inclui o helper de gateway fetch; forneca um manifesto mais descritores de
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
```Defina `transport_policy` como `Some(TransportPolicy::SoranetStrict)` quando carrega
precisarem recusar relés clássicos, ou `Some(TransportPolicy::DirectOnly)` quando
SoraNet precisa ser totalmente ignorado. Ponte `scoreboard.persist_path` para o
diretório de artefatos de lançamento, opcionalmente corrigido `scoreboard.now_unix_secs` e formulário de preenchimento
`scoreboard.metadata` com contexto de captura (etiquetas de fixtures, alvo Torii, etc.)
para que `cargo xtask sorafs-adoption-check` consuma JSON determinístico entre SDKs
com o blob de proveniência que SF-6c espera.
`Client::sorafs_fetch_via_gateway` agora aumenta esse metadata com o identificador de
manifest, a expectativa opcional de manifesto CID e o sinalizador `gateway_manifest_provided`
executando o `GatewayFetchConfig` fornecido, para que capturas que incluam um
envelope manifesto assinado atendendo ao requisito de evidência SF-6c sem duplicar esses
campos manualmente.

## Ajudantes de manifesto

`ManifestBuilder` continua sendo a forma canônica de montar payloads Norito de forma
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

Incorporar o construtor sempre que os serviços precisarem gerar manifestos em tempo real; ó
CLI segue sendo o caminho recomendado para pipelines determinísticos.