---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-rust
כותרת: Сниппеты SDK на Rust
sidebar_label: חלודה сниппеты
תיאור: Минимальные примеры на Rust для потребления proof streams и מניפסטים.
---

:::note Канонический источник
:::

ארגזי חלודה в этом репозитории питают CLI и могут быть встроены в кастомные
מתזמרים или сервисы. Сниппеты ниже выделяют עוזרים, которые чаще всего
нужны разработчикам.

## עוזר לזרם הוכחה

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
`ProofStreamSummary::to_json()` פנה ל- же JSON метрик, что ו-CLI, что упрощает
מידע על קצה קצה של צפיות או קביעות CI.

## אחזור ריבוי מקורות ניקוד

Модуль `sorafs_car::multi_fetch` раскрывает асинхронный אחזור מתזמן, используемый
CLI. Реализуйте `sorafs_car::multi_fetch::ScorePolicy` и передайте его через
`FetchOptions::score_policy`, чтобы настроить порядок провайдеров. Юнит-тест
`multi_fetch::tests::score_policy_can_filter_providers` צילום, חזור
кастомные предпочтения.

Другие כפתורים соответствуют CLI флагам:

- `FetchOptions::per_chunk_retry_limit` соответствует флагу `--retry-budget` ל-CI
  запусков, которые намеренно ограничивают מנסה שוב.
- Скомбинируйте `FetchOptions::global_parallel_limit` с `--max-peers`, чтобы ограничить
  количество одновременных провайдеров.
- `OrchestratorConfig::with_telemetry_region("region")` מדדים
  `sorafs_orchestrator_*`, א `OrchestratorConfig::with_transport_policy` זרקליט
  флаг CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` идет по умолчанию
  в поверхностях CLI/SDK; используйте `TransportPolicy::DirectOnly` только при בימוי
  שדרוג לאחור или в рамках הוראת תאימות, и резервируйте `SoranetStrict` для
  PQ-only пилотов с явным одобрением.
- תקן `SorafsGatewayFetchOptions::write_mode_hint =
  כמה (WriteModeHint::UploadPqOnly)` чтобы форсировать העלאות PQ בלבד; עוזר автоматически
  повысит תחבורה/אנונימיות политики, если они не דרוס явно.
- Используйте `SorafsGatewayFetchOptions::policy_override` чтобы закрепить временный
  תחבורה или דרג אנונימיות для одного запроса; подача любого поля пропускает
  ירידה בדרגה בראייה и падает, если запрошенный tier не может быть удовлетворен.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) ו-JavaScript
  (`sorafsMultiFetchLocal`) כריכות используют тот же מתזמן, поэтому задайте
  `return_scoreboard=true` в этих עוזרים, чтобы получить рассчитанные веса рядом с
  חתיכות קבלות.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` זרם OTLP זרם,
  חבילת אימוץ сформировавший. Если он не задан, клиент автоматически выводит
  `region:<telemetry_region>` (או `chain:<chain_id>`) чтобы metadata всегда несла
  תווית описательный.

## אחזר את `iroha::Client`

Rust SDK включает עוזר ל-gateway fech; передайте manifest и дескрипторы провайдеров
(включая זרם אסימוני) и дайте клиенту управлять אחזור רב מקורות:

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
```Установите `transport_policy` в `Some(TransportPolicy::SoranetStrict)`, העלאות когда
должны отвергать классические ממסרים, или в `Some(TransportPolicy::DirectOnly)`, когда
SoraNet нужно полностью обходить. Укажите `scoreboard.persist_path` לדירקטוריו
релизных артефактов, опционально зафиксируйте `scoreboard.now_unix_secs` и заполните
`scoreboard.metadata` контекстом захвата (תוויות גופי, цель Torii и т.д.) так,
чтобы `cargo xtask sorafs-adoption-check` потреблял детерминированный JSON между SDK
с כתם מקור, который ожидает SF-6c.
`Client::sorafs_fetch_via_gateway` теперь дополняет эти מניפסט מטא נתונים идентификатором,
опциональным ожиданием מניפסט CID и флагом `gateway_manifest_provided` путем анализа
переданного `GatewayFetchConfig`, чтобы захваты с подписанным מניפסט מעטפה удовлетворяли
требованию доказательств SF-6c ללא ручного дублирования этих полей.

## עוזרים למניפסט

`ManifestBuilder` остается каноничным способом программно собирать Norito מטענים:

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

Встраивайте בונה везде, где сервисам нужно генерировать מניפסטים на лету; CLI
остается рекомендуемым путем для детерминированных צינורות.