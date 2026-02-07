---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-sdk-rust
заголовок: Фрагменты SDK Rust
Sidebar_label: Фрагменты Rust
описание: Минимальные примеры Rust для потребительских потоков и манифестов.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/developer/sdk/rust.md`. Мантенья представился как копиас синхронизадас.
:::

Ящики с Rust содержат репозитории продуктов питания или CLI и могут быть вставлены в них.
организаторы или индивидуальные услуги. Фрагменты фрагментов, которые можно найти в помощниках
больше всего нам известно о наших просьбах к разработчикам.

## Поток помощника по проверке

Повторно используйте парсер существующего потока доказательств для объединения метрик ответа HTTP:

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

Полная версия (с тестами) Vive Em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` Упрощенная визуализация или JSON-метрика CLI
питание серверной части наблюдения или утверждений CI.

## Оценка выборки из нескольких источников

По модулю `sorafs_car::multi_fetch` выставляется или планировщик выборки, используемый в качестве синхронного
Интерфейс командной строки. Внедрить `sorafs_car::multi_fetch::ScorePolicy` и пройти через
`FetchOptions::score_policy` для настройки порядка проверки. О teste unitario
`multi_fetch::tests::score_policy_can_filter_providers` используется как импортные предпочтения
настройки.

Outros регулирует флаги espelham для CLI:

- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` для запуска
  де CI, который ограничивает наши предложения.
- Объедините `FetchOptions::global_parallel_limit` с `--max-peers` для ограничения или числа.
  deprovores concorrentes.
- `OrchestratorConfig::with_telemetry_region("region")` марка как метрика
  `sorafs_orchestrator_*`, например `OrchestratorConfig::with_transport_policy`
  o флаг CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` e o значение по умолчанию
  интерфейс CLI/SDK; используйте `TransportPolicy::DirectOnly` для подготовки к переходу на более раннюю версию
  или следите за соблюдением требований и резервируйте `SoranetStrict` для пилотов только для PQ
  com явное подтверждение.
- Defina `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` для загрузки только PQ; о помощник в продвижении
  автоматически, как политика транспорта/анонимность, когда я говорю это открыто
  собрескритас.
- Используйте `SorafsGatewayFetchOptions::policy_override` для исправления временного уровня.
  транспортировать или анонимно для единого запроса; fornecer qualquer um dos Campos пула
  о понижении в должности и о понижении уровня, который может быть запрошен.
- Привязки ОС Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) повторное использование или планировщик сообщений, определенный
  `return_scoreboard=true` нужны помощники для восстановления вычисленных песо в junto com
  чаевые квитанции.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` регистрация или поток OTLP очереди
  produziu um пакет усыновления. Когда опущено, клиент автоматически извлекается
  `region:<telemetry_region>` (или `chain:<chain_id>`) для постоянных метаданных
  carregue um rotulo descritivo.

## Получение через `iroha::Client`

В SDK Rust включен помощник по выборке шлюза; Forneca um Manifest mais descritores de
Проверки (включая токены потока) и deixe или cliente conduzir или выборка из нескольких источников:

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
```Defina `transport_policy` как `Some(TransportPolicy::SoranetStrict)` при каждой загрузке
Реле Precisarem recusar classicos или `Some(TransportPolicy::DirectOnly)` quando
SoraNet может быть полностью отключен. Апонте `scoreboard.persist_path` пункт o
директория по выпуску артефактов, дополнительные исправления `scoreboard.now_unix_secs` и дополнительная информация
`scoreboard.metadata` в контексте захвата (метки приборов, дополнительный Torii и т. д.)
для того, чтобы `cargo xtask sorafs-adoption-check` использовал детерминированный JSON в SDK
как источник происхождения SF-6c.
`Client::sorafs_fetch_via_gateway` назад дополнительные метаданные в качестве идентификатора
манифест, ожидаемый дополнительный идентификатор манифеста и флаг `gateway_manifest_provided`
Проверьте `GatewayFetchConfig`, чтобы зафиксировать, что включено в
Конверт-манифест, содержащий реквизиты для доказательств SF-6c, в двух экземплярах
Кампос вручную.

## Помощники манифеста

`ManifestBuilder` продолжает отправлять форму канонических полезных нагрузок Norito формы
программатика:

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

Incorpore или строитель всегда будет выполнять точные услуги, которые будут проявляться в реальном времени; о
CLI отправляет или рекомендует использовать детерминированные конвейеры.