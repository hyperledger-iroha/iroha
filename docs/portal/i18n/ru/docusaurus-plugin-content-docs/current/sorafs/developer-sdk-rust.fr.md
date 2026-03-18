---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-sdk-rust
Название: Экстраитсы SDK Rust
Sidebar_label: Экстраиты Rust
описание: Примеры Rust minimaux для проверки потоков и манифестов.
---

:::note Источник канонический
:::

Ящики Rust de ce dépôt alimentent le CLI и могут быть помещены в
оркестраторы или обслуживающий персонал. Les extraits ci-dessous mettent en avant
Помощники и самые востребованные.

## Поток проверки помощника

Повторное использование существующего потока проверки синтаксического анализатора для объединения имеющихся метрик
ответ HTTP:

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

Полная версия (с учетом тестов) соответствует `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` отображает JSON-метрические метки, которые есть в CLI, вот что
облегчить питание бэкендов наблюдения или утверждений CI.

## Оценка выборки из нескольких источников

Модуль `sorafs_car::multi_fetch` предоставляет доступ к используемому планировщику асинхронной выборки.
для CLI. Внедрить `sorafs_car::multi_fetch::ScorePolicy` и пройти через
`FetchOptions::score_policy` для настройки порядка поставщиков. Унитарный тест
`multi_fetch::tests::score_policy_can_filter_providers` монтре комментарий импозёр де
персональные предпочтения.

Другие ручки выравниваются по флагам CLI:

- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` для этого
  запускает CI, который противоречит добровольным повторным попыткам.
- Комбинация `FetchOptions::global_parallel_limit` с `--max-peers` для плафона
  количество одновременных поставщиков.
- `OrchestratorConfig::with_telemetry_region("region")` теги метрик
  `sorafs_orchestrator_*`, тогда как `OrchestratorConfig::with_transport_policy`
  отобразить флаг CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` есть
  livré comme valeur по умолчанию с CLI/SDK ; использовать `TransportPolicy::DirectOnly`
  уникальность при переходе на более раннюю версию или в соответствии с директивой о соответствии и резерве
  `SoranetStrict` дополнительные пилотные сигналы только для PQ с явной апробацией.
- Определено `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` для принудительной загрузки файлов только PQ; помощник
  Автоматически активировать транспортную/анонимную политику, чтобы отменить явное изменение.
- Используйте `SorafsGatewayFetchOptions::policy_override` для проверки уровня
  транспортировка или временная анонимная перевозка по запросу; Фурнир на полях
  Контур деградации затемнения и отголосков, если уровень требований не может быть больше
  удовлетворение.
- Привязки Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) и др.
  JavaScript (`sorafsMultiFetchLocal`) повторно использует планировщик мемов; определенный
  `return_scoreboard=true` в этих помощниках для восстановления веса, вычисленного в памяти
  время, когда получены квитанции.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` зарегистрировать поток OTLP
  Этот продукт является пакетом усыновления. Если это не так, клиент запускается автоматически
  `region:<telemetry_region>` (или `chain:<chain_id>`) в поисках метадонных предзнаменований
  toujours une étiquette описательный.

## Получение через `iroha::Client`

SDK Rust включает помощника по выборке шлюза; Fournissez Un Manifest Plus Des
описания провайдеров (включая потоковые токены) и отсутствие пилотного пилотного проекта клиента
получить несколько источников:

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
```Définissez `transport_policy` на `Some(TransportPolicy::SoranetStrict)` lorsque les
загружает отказ от классических реле, или на `Some(TransportPolicy::DirectOnly)`
Когда SoraNet делает контур контурным. Pointez `scoreboard.persist_path` версия le
репертуар артефактов выпуска, исправление событий `scoreboard.now_unix_secs` и др.
renseignez `scoreboard.metadata` с контекстом захвата (этикетки светильников, кабель
Torii и т. д.), пока `cargo xtask sorafs-adoption-check` не превратится в определенный JSON.
Все SDK содержат информацию о происхождении для SF-6c.
`Client::sorafs_fetch_via_gateway` обогащает эти метадоны с идентификацией
манифест, событие манифеста CID и флаг `gateway_manifest_provided` ru
Инспектор le `GatewayFetchConfig` forni, de sorte que les captures incluant une envelope
Демонстрация удовлетворения потребности в преуве SF-6c без дублирования на этих полях на основных полях.

## Помощники манифеста

`ManifestBuilder` остается каноническим фасадом ассемблера полезных нагрузок Norito фасада
программный:

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

Интегрируйте сборщик отдельных служб, создающих манифесты по желанию;
CLI рекомендуется оставить для детерминированных конвейеров.