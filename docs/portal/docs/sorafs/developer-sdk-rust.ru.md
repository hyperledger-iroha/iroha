---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: be3b8a2a4eaec95f5d4390ef7d143cd25681cce8ca3b5c4733f8036487daac24
source_last_modified: "2025-11-15T11:19:21.711876+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: developer-sdk-rust
title: Сниппеты SDK на Rust
sidebar_label: Rust сниппеты
description: Минимальные примеры на Rust для потребления proof streams и manifests.
---

:::note Канонический источник
:::

Rust crates в этом репозитории питают CLI и могут быть встроены в кастомные
orchestrators или сервисы. Сниппеты ниже выделяют helpers, которые чаще всего
нужны разработчикам.

## Helper для proof stream

Переиспользуйте существующий proof stream parser, чтобы агрегировать метрики из
HTTP-ответа:

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

Полная версия (с тестами) находится в `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` рендерит тот же JSON метрик, что и CLI, что упрощает
подачу данных в observability backend или CI assertions.

## Scoring multi-source fetch

Модуль `sorafs_car::multi_fetch` раскрывает асинхронный fetch scheduler, используемый
CLI. Реализуйте `sorafs_car::multi_fetch::ScorePolicy` и передайте его через
`FetchOptions::score_policy`, чтобы настроить порядок провайдеров. Юнит-тест
`multi_fetch::tests::score_policy_can_filter_providers` показывает, как вводить
кастомные предпочтения.

Другие knobs соответствуют CLI флагам:

- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` для CI
  запусков, которые намеренно ограничивают retries.
- Скомбинируйте `FetchOptions::global_parallel_limit` с `--max-peers`, чтобы ограничить
  количество одновременных провайдеров.
- `OrchestratorConfig::with_telemetry_region("region")` помечает метрики
  `sorafs_orchestrator_*`, а `OrchestratorConfig::with_transport_policy` зеркалит
  флаг CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` идет по умолчанию
  в поверхностях CLI/SDK; используйте `TransportPolicy::DirectOnly` только при staging
  downgrade или в рамках compliance directive, и резервируйте `SoranetStrict` для
  PQ-only пилотов с явным одобрением.
- Установите `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` чтобы форсировать PQ-only uploads; helper автоматически
  повысит transport/anonymity политики, если они не overridden явно.
- Используйте `SorafsGatewayFetchOptions::policy_override` чтобы закрепить временный
  transport или anonymity tier для одного запроса; подача любого поля пропускает
  brownout demotion и падает, если запрошенный tier не может быть удовлетворен.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) и JavaScript
  (`sorafsMultiFetchLocal`) bindings используют тот же scheduler, поэтому задайте
  `return_scoreboard=true` в этих helpers, чтобы получить рассчитанные веса рядом с
  chunk receipts.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` записывает OTLP stream,
  сформировавший adoption bundle. Если он не задан, клиент автоматически выводит
  `region:<telemetry_region>` (или `chain:<chain_id>`) чтобы metadata всегда несла
  описательный label.

## Fetch через `iroha::Client`

Rust SDK включает helper для gateway fetch; передайте manifest и дескрипторы провайдеров
(включая stream tokens) и дайте клиенту управлять multi-source fetch:

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

Установите `transport_policy` в `Some(TransportPolicy::SoranetStrict)`, когда uploads
должны отвергать классические relays, или в `Some(TransportPolicy::DirectOnly)`, когда
SoraNet нужно полностью обходить. Укажите `scoreboard.persist_path` на директорию
релизных артефактов, опционально зафиксируйте `scoreboard.now_unix_secs` и заполните
`scoreboard.metadata` контекстом захвата (labels fixtures, цель Torii и т.д.) так,
чтобы `cargo xtask sorafs-adoption-check` потреблял детерминированный JSON между SDKs
с blob provenance, который ожидает SF-6c.
`Client::sorafs_fetch_via_gateway` теперь дополняет эти metadata идентификатором manifest,
опциональным ожиданием manifest CID и флагом `gateway_manifest_provided` путем анализа
переданного `GatewayFetchConfig`, чтобы захваты с подписанным envelope manifest удовлетворяли
требованию доказательств SF-6c без ручного дублирования этих полей.

## Helpers для manifest

`ManifestBuilder` остается каноничным способом программно собирать Norito payloads:

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

Встраивайте builder везде, где сервисам нужно генерировать manifests на лету; CLI
остается рекомендуемым путем для детерминированных pipelines.
