---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-sdk-rust
Название: Обновление Rust SDK
Sidebar_label: Добавлено Rust
описание: Раст играет в фильме "Ржавчина".
---

:::примечание
Был установлен на `docs/source/sorafs/developer/sdk/rust.md`. Он был создан в честь Сфинкса.
:::

Создан в Rust в интерфейсе CLI и создан в 2017 году. خدمات مخصّصة.
Он сказал, что хочет, чтобы это произошло с ним.

## مساعد تدفق الأدلة

Вы можете получить доступ к веб-сайту по протоколу HTTP:

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

Установите флажок (в виде файла) для `docs/examples/sorafs_rust_proof_stream.rs`.
Создайте `ProofStreamSummary::to_json()` для JSON-файла в CLI и воспользуйтесь файлами CLI.
Создан в CI.

## تقييم الجلب متعدد المصادر

Установите `sorafs_car::multi_fetch` для изменения настроек в CLI.
Код `sorafs_car::multi_fetch::ScorePolicy` и код `FetchOptions::score_policy`
لضبط ترتيب المزوّدين. يوضح اختبار الوحدة
`multi_fetch::tests::score_policy_can_filter_providers` был отправлен на сервер.

Откройте интерфейс командной строки:

- `FetchOptions::per_chunk_retry_limit` для `--retry-budget` для CI التي
  تحد عدد المحاولات عمدًا.
- اجمع بين `FetchOptions::global_parallel_limit` و`--max-peers` لتقييد عدد المزوّدين
  المتزامنين.
- يقوم `OrchestratorConfig::with_telemetry_region("region")` بوسم مقاييس
  `sorafs_orchestrator_*`, بينما يعكس `OrchestratorConfig::with_transport_policy`
  Используйте CLI `--transport-policy`. Код `TransportPolicy::SoranetPreferred`
  Использование CLI/SDK; استخدم `TransportPolicy::DirectOnly` для понижения версии до более ранней версии.
  Для этого используется `SoranetStrict`, предназначенный только для PQ.
- اضبط `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` для PQ-only; سيعزز المساعد سياسات
  Он/она сказал, что он был в восторге от Трэвиса.
- استخدم `SorafsGatewayFetchOptions::policy_override` لتثبيت طبقة نقل أو إخفاء هوية
  مؤقتة لطلب واحد؛ Он сыграл с Дэвидом Уилсоном, когда он отключился от электросети, и Сэнсэй Уоррен в 2007 году.
  المطلوبة.
- Поддержка Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) и JavaScript.
  (`sorafsMultiFetchLocal`) Зарегистрируйтесь, чтобы установить `return_scoreboard=true` в приложении.
  Нажмите на кусок куска.
- Код `SorafsGatewayScoreboardOptions::telemetry_source_label` для OTLP-файла.
  حزمة تبنٍ. Установите флажок `region:<telemetry_region>` (или `chain:<chain_id>`)
  Он сказал ему, что он Уайт.

## الجلب عبر `iroha::Client`

Использование Rust SDK для создания новых приложений Создатель Миссисипи в Сан-Франциско
(Джон и Дональд Трамп)

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

اضبط `transport_policy` إلى `Some(TransportPolicy::SoranetStrict)` عندما يجب أن ترفض
الرفوعات المرحلات الكلاسيكية, أو `Some(TransportPolicy::DirectOnly)` عندما يجب تجاوز
SoraNet بالكامل. وجّه `scoreboard.persist_path` إلى دليل آرتيفاكتات الإصدار, واضبط
Код `scoreboard.now_unix_secs`, а также `scoreboard.metadata`.
(Светильники, Torii, إلخ) حتى يستهلك `cargo xtask sorafs-adoption-check` JSON
Созданы SDK для разработки SF-6c.
تقوم `Client::sorafs_fetch_via_gateway` в разделе "Программное обеспечение".
Для CID используется код `gateway_manifest_provided`.
`GatewayFetchConfig`. متطلبات
На борту SF-6c был Дэниел Тэхен в Уэльсе.

## مساعدات المانيفست

Для `ManifestBuilder` установите флажок Norito:```rust
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

Он выступил в роли президента США Дональда Трампа в Вашингтоне. Использование CLI в интерфейсе
Он сказал, что это не так.