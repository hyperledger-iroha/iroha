---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-sdk-rust
заголовок: Фрагменты Rust SDK
Sidebar_label: Фрагменты ржавчины
описание: Потоки доказательства и манифесты потребляют минимальное количество примеров на Rust.
---

:::примечание
:::

Репозиторий, ящики Rust, интерфейс командной строки, мощность, пользовательские оркестраторы, сервисы, встраивание, возможность встраивания.
Здесь вы найдете фрагменты и помощники, а также выделите нужные фрагменты и разделы с информацией.

## Помощник потока проверки

HTTP-ответ и совокупность метрик, а также повторное использование существующего анализатора потока подтверждения:

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

Полная версия (тесты سمیت) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` и метрики JSON-рендеринга и CLI-интерфейса
Серверные части наблюдаемости. Подача утверждений CI.

## Оценка выборки из нескольких источников

Модуль `sorafs_car::multi_fetch` и планировщик асинхронной выборки предоставляют возможность использования CLI
`sorafs_car::multi_fetch::ScorePolicy` реализует пропуск `FetchOptions::score_policy` позволяет выполнить пропуск
تاکہ провайдер заказывает мелодию ہو سکے۔ Модульный тест `multi_fetch::tests::score_policy_can_filter_providers`
пользовательские настройки обеспечивают принудительное выполнение

Другие флажки CLI и зеркало:

- `FetchOptions::per_chunk_retry_limit` CI запускает کے لیے `--retry-budget` флаг سے соответствует کرتا ہے
  Если вы повторите попытку, вы можете ограничить ограничение или ограничить его.
- `FetchOptions::global_parallel_limit` или `--max-peers` можно объединить одновременно
  провайдеры کی تعداد cap ہو۔
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` метрики и тег کرتا ہے،
  `OrchestratorConfig::with_transport_policy` CLI или `--transport-policy` флаг и зеркало.
  `TransportPolicy::SoranetPreferred` Поверхности CLI/SDK поставляются по умолчанию. `TransportPolicy::DirectOnly`
  Для перехода на более раннюю версию необходимо следовать директиве о соответствии требованиям, установленной `SoranetStrict`.
  Пилоты только для PQ или явное одобрение и резерв.
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` set کریں تاکہ PQ-only uploads Force ہوں؛ транспорт помощника/анонимность
  политики کو خودکار طور پر продвигать کرے گا جب تک واضح override نہ ہو۔
- `SorafsGatewayFetchOptions::policy_override` Запрос на временный транспорт
  یا значок уровня анонимности ہو جائے؛ کسی بھی поле کے دینے سے понижение пониженного напряжения пропустить ہوتا ہے اور اگر
  запрошенный уровень удовлетворяет نہ ہو سکے تو error آتا ہے۔
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) и JavaScript (`sorafsMultiFetchLocal`)
  привязки, планировщик и повторное использование, а также помощники и `return_scoreboard=true` set کریں تاکہ
  Получение фрагментов расчетных весов
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP-поток для записи или пакет принятия
  بنایا تھا۔ Если опустить клиент, можно получить `region:<telemetry_region>` (`chain:<chain_id>`) получить کرتا ہے
  تاکہ метаданные میں ہمیشہ описательная метка رہے۔

## Получение через `iroha::Client`

Rust SDK — помощник по выборке шлюза Дескрипторы провайдера манифеста (токены потока).
Для клиента с диском выборки из нескольких источников можно использовать:

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
```Загрузите классические реле سے انکار کرنا ہو تو `transport_policy` کو `Some(TransportPolicy::SoranetStrict)`
Установите соединение с SoraNet для обхода канала `Some(TransportPolicy::DirectOnly)`.
`scoreboard.persist_path`, чтобы освободить каталог артефактов, и указать, как исправить `scoreboard.now_unix_secs`, как исправить.
Контекст захвата `scoreboard.metadata` (метки фиксаторов, цель Torii и т. д.)
`cargo xtask sorafs-adoption-check` SDK для детерминированного JSON, использующего SF-6c и BLOB-объект происхождения
включить رہے۔ `Client::sorafs_fetch_via_gateway` — метаданные, идентификатор манифеста, необязательный ожидаемый CID манифеста,
Флаг `gateway_manifest_provided` позволяет расширить возможности с помощью предоставленного `GatewayFetchConfig`, а также подписанного конверта манифеста.
Захватывает требования к доказательствам SF-6c и создает поля для дублирования.

## Помощники манифеста

`ManifestBuilder` и полезные нагрузки Norito программно собирают канонические или канонические файлы:

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

Службы создания манифестов на лету Создание встроенного конструктора встраивания детерминированные конвейеры и CLI и рекомендуемый путь ہے۔