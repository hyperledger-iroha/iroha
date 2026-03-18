---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-sdk-rust
название: Сниппеты SDK на Rust
Sidebar_label: Фрагменты ржавчины
Описание: Минимальные примеры на Rust для потоков и манифестов, защищенных от потребления.
---

:::note Канонический источник
:::

Ржавые ящики в этом репозитории питаются CLI и могут быть встроены в кастомные
оркестраторы или сервисы. Сниппеты ниже найденных помощников, которые чаще всего
нужным разработчикам.

## Помощник для проверки потока

Используйте существующий анализатор потока доказательств, чтобы агрегировать метрики из
HTTP-ответ:

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
`ProofStreamSummary::to_json()` рендерит тот же JSON-метрик, что и CLI, что влияет
передача данных в серверную часть наблюдаемости или утверждения CI.

## Оценка выборки из нескольких источников

Модуль `sorafs_car::multi_fetch` раскрывает асинхронный планировщик выборки, прогноз
Интерфейс командной строки. Реализуйте `sorafs_car::multi_fetch::ScorePolicy` и передайте его через
`FetchOptions::score_policy`, чтобы настроить порядок провайдеров. Юнит-тест
`multi_fetch::tests::score_policy_can_filter_providers` показывает, как вводить
кастомные выборы.

Другие ручки соответствуют флагам CLI:

- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` для CI
  запусков, которые намеренно ограничивают повторные попытки.
- Скомбинируйте `FetchOptions::global_parallel_limit` с `--max-peers`, чтобы оценить
  количество одновременных провайдеров.
- `OrchestratorConfig::with_telemetry_region("region")` помечает метрики
  `sorafs_orchestrator_*`, а `OrchestratorConfig::with_transport_policy` зеркалит
  флаг CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` идет по умолчанию
  в верхних этажах CLI/SDK; `TransportPolicy::DirectOnly` только при постановке
  понизьте версию или в соответствии с директивой о соответствии и резервируйте `SoranetStrict` для
  PQ-только для пилотов с явным одобрением.
- Установите `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)`, чтобы форсировать загрузку только PQ; помощник автоматически
  повысит политику транспорта/анонимности, если они не отменены явно.
- Используйте `SorafsGatewayFetchOptions::policy_override` для закрепления временного
  транспортный уровень или уровень анонимности для одного запроса; предоставление любых полей пропускает
  понижение напряжения и падение напряжения, если запрошенный уровень не может быть удовлетворительным.
- Python (`sorafs_multi_fetch_local`/`sorafs_gateway_fetch`) и JavaScript
  (`sorafsMultiFetchLocal`) привязки используют тот же планировщик, поэтому задайте
  `return_scoreboard=true` в этих помощниках, чтобы получить рассчитанные веса рядом с
  чаевые квитанции.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` записывает поток OTLP,
  Сформировавший комплект усыновления. Если он не задан, клиент автоматически выводит
  `region:<telemetry_region>` (или `chain:<chain_id>`), чтобы метаданные всегда носили
  описательная этикетка.

## Получить через `iroha::Client`

Rust SDK включает помощника для выборки шлюза; передайте манифест и дескрипторы провайдеров
(включая токены потока) и позволить клиенту управлять выборкой из нескольких источников:

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
```Установите `transport_policy` в `Some(TransportPolicy::SoranetStrict)`, при загрузке
следует отвергать классические реле, или в `Some(TransportPolicy::DirectOnly)`, когда
SoraNet нужно полностью обойти. Укажите `scoreboard.persist_path` по каталогу
релизных документов, опционально зафиксируйте `scoreboard.now_unix_secs` и заполните
`scoreboard.metadata` контекстом захвата (метки фикстуры, цель Torii и т.д.) так,
чтобы `cargo xtask sorafs-adoption-check` потреблял определенный JSON между SDK
с источником blob, который ожидает SF-6c.
`Client::sorafs_fetch_via_gateway` теперь выполняет эти метаданные манифеста идентификатора,
манифест опционального ожидания CID и флагом `gateway_manifest_provided` путем анализа
переданного `GatewayFetchConfig`, для захватов с подписанным конвертом-манифестом
Инструмент доказательства SF-6c без ручного дублирования этих полей.

## Помощники для манифеста

`ManifestBuilder` сохраняет стандартный метод программного сбора полезных данных Norito:

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

Встраивайте застройщика везде, где сервисами необходимо ограничить манифесты на лето; интерфейс командной строки
Остаётся рекомендуемым путём для детерминированных трубопроводов.