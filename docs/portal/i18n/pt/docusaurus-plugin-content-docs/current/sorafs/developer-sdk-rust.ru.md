---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-sdk-rust
título: Сниппеты SDK em Rust
sidebar_label: Rust trechos
description: Exemplos mínimos de Rust para gerar fluxos de prova e manifestos.
---

:::nota História Canônica
:::

Rust crates neste repositório питают CLI e pode ser usado em caixas
orquestradores ou serviços. Сниппеты ниже выделяют ajudantes, которые чаще всего
нужны разработчикам.

## Helper para stream de prova

Переиспользуйте существующий analisador de fluxo de prova, чтобы агрегировать метрики из
Suporte HTTP:

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

A versão (com teste) foi inserida em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` gera uma métrica JSON, isso e CLI, que é usado
pode ser usado no back-end de observabilidade ou nas asserções de CI.

## Pontuação de busca de múltiplas fontes

Módulo `sorafs_car::multi_fetch` раскрывает асинхронный agendador de busca, используемый
CLI. Realize `sorafs_car::multi_fetch::ScorePolicy` e instale-o
`FetchOptions::score_policy`, este é o resultado do teste. Юнит-teste
`multi_fetch::tests::score_policy_can_filter_providers` показывает, как вводить
кастомные предпочтения.

Outros botões usam a bandeira CLI:

- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` para CI
  запусков, которые намеренно ограничивают tenta novamente.
- Combinar `FetchOptions::global_parallel_limit` com `--max-peers`, isso é feito
  количество одновременных провайдеров.
- `OrchestratorConfig::with_telemetry_region("region")` métrica de medição
  `sorafs_orchestrator_*`, e `OrchestratorConfig::with_transport_policy` bloqueado
  sinalizador CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` é uma ideia para uso
  no CLI/SDK; usar `TransportPolicy::DirectOnly` para encenação
  fazer downgrade ou para a diretiva de conformidade e restaurar `SoranetStrict` para
  Pilotos somente PQ com явным одобрением.
- Escolha `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` чтобы форсировать uploads somente PQ; ajudante automático
  повысит transporte/anonimato политики, если они не явно substituído.
- Используйте `SorafsGatewayFetchOptions::policy_override` чтобы закрепить временный
  nível de transporte ou anonimato para proteção de dados; подача любого поля пропускает
  rebaixamento de brownout e падает, если запрошенный nível não pode ser melhorado.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e JavaScript
  (`sorafsMultiFetchLocal`) ligações são usadas para o agendador, que são usadas
  `return_scoreboard=true` em seus ajudantes, você pode usá-los com mais eficiência
  receitas em pedaços.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` transmite fluxo OTLP,
  pacote de adoção сформировавший. Se não for assim, o cliente será automaticamente atendido
  `region:<telemetry_region>` (ou `chain:<chain_id>`) esses metadados estão faltando
  rótulo описательный.

## Buscar chave `iroha::Client`

Rust SDK usa auxiliar para busca de gateway; передайте manifest e дескрипторы провайдеров
(você usa tokens de fluxo) e o cliente de dados atualiza a busca de várias fontes:

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
```Instalar `transport_policy` em `Some(TransportPolicy::SoranetStrict)`, fazer uploads
relés de classe padrão, ou em `Some(TransportPolicy::DirectOnly)`, como
SoraNet não está disponível para download. Use `scoreboard.persist_path` no diretório
artefatos reutilizáveis, opcionais de proteção `scoreboard.now_unix_secs` e de proteção
`scoreboard.metadata` контекстом захвата (fixadores de etiquetas, цель Torii e т.д.) так,
`cargo xtask sorafs-adoption-check` permite definir SDKs de formato JSON
com proveniência do blob, который ожидает SF-6c.
`Client::sorafs_fetch_via_gateway` теперь дополняет эти metadata идентификатором manifesto,
опциональным ожиданием manifesto CID e флагом `gateway_manifest_provided` путем анализа
переданного `GatewayFetchConfig`, чтобы захваты с подписанным envelope manifest удовлетворяли
A versão completa do SF-6c não é totalmente compatível com o modelo.

## Ajudantes para o manifesto

`ManifestBuilder` остается каноничным способом программно собирать Cargas úteis Norito:

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

Встраивайте construtor везде, где сервисам нужно генерировать manifestos на лету; CLI
остается рекомендуемым путем para determinar pipelines.