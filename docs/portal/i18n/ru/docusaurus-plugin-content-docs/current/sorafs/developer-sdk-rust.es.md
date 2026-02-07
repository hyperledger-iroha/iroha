---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-sdk-rust
Название: Фрагменты SDK de Rust
Sidebar_label: Фрагменты ржавчины
описание: Минимальные примеры на Rust для потоков и манифестов потребительской проверки.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/developer/sdk/rust.md`. Mantén ambas copyas sincronizadas.
:::

Ящики Rust в этом импульсном репозитории CLI и можно инкрустировать их
Оркестадоры или персонализированные услуги. Лос-фрагменты абахо заменены помощниками
que más piden los desarrolladores.

## Поток помощника по проверке

Повторно используйте анализатор существующего потока доказательств для объединения показателей ответа HTTP:

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

Полная версия (с тестами) живет в `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` визуализирует JSON-метрику, которую использует CLI, вот что
облегчение питания серверов наблюдения или подключений CI.

## Пункт получения данных из нескольких источников

Модуль `sorafs_car::multi_fetch` отображает планировщик автоматической выборки, используемый для него
Интерфейс командной строки. Реализация `sorafs_car::multi_fetch::ScorePolicy` и прохождение через
`FetchOptions::score_policy` для настройки порядка проверки. Эль-тест унитарио
`multi_fetch::tests::score_policy_can_filter_providers` нужно как использовать
персонализированные предпочтения.

Другие регуляторы отображения флагов CLI:

- `FetchOptions::per_chunk_retry_limit` совпадают с флагом `--retry-budget` пункта
  выбросы CI, которые ограничивают повторное намерение.
- Комбинация `FetchOptions::global_parallel_limit` с `--max-peers` для ограничения
  cantidad deprovedores concurrentes.
- `OrchestratorConfig::with_telemetry_region("region")` этикетка метрик
  `sorafs_orchestrator_*`, сейчас, когда `OrchestratorConfig::with_transport_policy`
  Отобразится флаг `--transport-policy` CLI. `TransportPolicy::SoranetPreferred`
  se entrega como valor por дефекто в поверхностях CLI/SDK; США
  `TransportPolicy::DirectOnly` только для подготовки к переходу на более раннюю версию или продолжению директивы
  соответствие требованиям и резерв `SoranetStrict` для пилотов только для PQ с явным одобрением.
- Настройка `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` для субсидирования PQ-only; помощник по продвижению
  автоматический политический/анонимный залп, который будет записан
  явно.
- США `SorafsGatewayFetchOptions::policy_override` для транспортировки или уровня
  anonimato temporal para una sola solicitud; аль-пропорционар куалькиера-де-лос-кампос
  если вы опустите деградацию из-за отключения электроэнергии и падения, если уровень не будет требоваться
  удовлетворенный.
- Потеря привязок Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) y
  JavaScript (`sorafsMultiFetchLocal`) повторно использует планировщик мисмо, как в конфигурации
  `return_scoreboard=true` и эти помощники для восстановления вычисленных песо
  лос-рекибос-де-кусок.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` регистрирует поток OTLP que
  создание пакета усыновления. Когда ты упускаешь возможность получить клиента
  `region:<telemetry_region>` (или `chain:<chain_id>`) автоматически для этого
  метаданные всегда соответствуют описательному этикету.

## Получить через `iroha::Client`

SDK de Rust включает помощника выборки шлюза; пропорционально большему количеству проявлений
дескрипторы доказательств (включая токены потока) и то, что клиент извлекает
el fetch из нескольких источников:

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
```Конфигурация `transport_policy` как `Some(TransportPolicy::SoranetStrict)` когда-либо
subidas deban rechazar реле классикос, o `Some(TransportPolicy::DirectOnly)` cuando
SoraNet полностью опущен. Apunta `scoreboard.persist_path` в директории
артефакты выпуска, дополнительная опция `scoreboard.now_unix_secs` и полная версия
`scoreboard.metadata` с контекстом захвата (этики приборов, целевой Torii и т. д.)
для того, чтобы `cargo xtask sorafs-adoption-check` использовал JSON, определенный всеми SDK
эль-блоб процесса, который ожидал SF-6c.
`Client::sorafs_fetch_via_gateway` сейчас дополняет эти метаданные с идентификатором
 манифест, дополнительный ожидаемый манифест CID и флаг
`gateway_manifest_provided` Проверка `GatewayFetchConfig`, выполненная в режиме реального времени
que capturas que incluyen un Envoltorio de Manifest Firmado cumplan el Requisito de
Pruebas SF-6c не дублируется вручную.

## Помощники манифеста

`ManifestBuilder` означает, что форма канонического набора полезных нагрузок Norito формы
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

Включите застройщика в список необходимых услуг, представленный в виде деклараций; эль
CLI всегда рекомендует руту для детерминированных конвейеров.