---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-sdk-rust
título: Extraits SDK Rust
sidebar_label: Extrai ferrugem
description: Exemplos Rust mínimos para consumir streams de prova e manifestos.
---

:::nota Fonte canônica
:::

As caixas enferrujam neste depósito, alimentam o CLI e podem ser embarcadas nele
orquestradores ou serviços personalizados. Les extraits ci-dessous mettent en avant
os ajudantes, os mais exigidos.

## Fluxo de prova auxiliar

Reutilize o fluxo de prova do analisador existente para ajustar as métricas a partir de um
resposta HTTP:

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
`ProofStreamSummary::to_json()` gera o mesmo JSON de métricas que o CLI, este aqui
facilitar a alimentação de back-ends de observabilidade ou de afirmações CI.

## Pontuação de busca de múltiplas fontes

O módulo `sorafs_car::multi_fetch` expõe o agendador de busca assíncrona utilizado
pela CLI. Implemente `sorafs_car::multi_fetch::ScorePolicy` e passe-o via
`FetchOptions::score_policy` para ajustar a ordem dos provedores. O teste unitário
`multi_fetch::tests::score_policy_can_filter_providers` mostrar comentário impostor des
preferências personalizadas.

Outros botões alinhados nas bandeiras CLI:

- `FetchOptions::per_chunk_retry_limit` corresponde à bandeira `--retry-budget` para des
  executa CI qui contraignent voluntariamente as novas tentativas.
- Combinez `FetchOptions::global_parallel_limit` com `--max-peers` para plafonner le
  nome de provedores simultâneos.
- `OrchestratorConfig::with_telemetry_region("region")` etiqueta as métricas
  `sorafs_orchestrator_*`, assim como `OrchestratorConfig::with_transport_policy`
  reflita o sinalizador CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` est
  liberado como valor padrão no CLI/SDK ; usar `TransportPolicy::DirectOnly`
  somente em caso de downgrade ou diretiva de conformidade, e reserva
  `SoranetStrict` aux pilotos PQ somente com aprovação explícita.
- Defina `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forçar uploads PQ-only ; o ajudante
  promete automaticamente as políticas de transporte/anônimas sem substituir explicitamente.
- Utilize `SorafsGatewayFetchOptions::policy_override` para remover uma camada de
  transporte ou anonimato temporário para uma demanda; fournir l'un des champs
  contorne a degradação e o eco se o nível exigido não puder ocorrer
  satisfeito.
- As ligações Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) utiliza o mesmo agendador ; definição
  `return_scoreboard=true` nestes auxiliares para recuperar pesos calculados ao mesmo tempo
  tempo que os recibos do pedaço.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` registra o fluxo OTLP
  aqui produz um pacote de adoção. Se for omitido, o cliente derivará automaticamente
  `region:<telemetry_region>` (ou `chain:<chain_id>`) para que os metadonées sejam um presságio
  toujours une étiquette descritivo.

## Buscar via `iroha::Client`

O SDK Rust embarca o ajudante do gateway fetch ; fornecer um manifesto mais um
descritores de provedores (e compreendem tokens de fluxo) e libera o piloto do cliente
vamos buscar várias fontes:

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
```Defina `transport_policy` em `Some(TransportPolicy::SoranetStrict)` quando
uploads devem recusar os relés clássicos, ou em `Some(TransportPolicy::DirectOnly)`
quando o SoraNet deve ter contornos completos. Pointez `scoreboard.persist_path` para baixo
repertório de artefatos de lançamento, correção de eventos `scoreboard.now_unix_secs` e
Renseignez `scoreboard.metadata` com o contexto de captura (etiquetas de fixtures, cible
Torii, etc.) para que `cargo xtask sorafs-adoption-check` use um JSON determinado
entre SDKs com o blob de proveniência atendido por SF-6c.
`Client::sorafs_fetch_via_gateway` enriquece a desorção dessas metadonas com o identificador
manifesto, o evento de manifesto CID e o sinalizador `gateway_manifest_provided` en
inspecione o `GatewayFetchConfig`, verifique se as capturas incluem um envelope
manifesto assinado satisfont l'exigence de preuve SF-6c sans dupliquer ces champs à la main.

## Ajudantes de manifesto

`ManifestBuilder` resta a forma canônica de montagem de cargas úteis Norito de forma
programático:

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

Integre o construtor em parte dos serviços que devem gerar manifestos à vontade;
a CLI mantém a voz recomendada para os pipelines determinados.