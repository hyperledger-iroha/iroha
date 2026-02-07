---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-sdk-الصدأ
العنوان: قصاصات SDK من الصدأ
Sidebar_label: قصاصات الصدأ
الوصف: أمثلة بسيطة عن الصدأ لتدفقات وبيانات إثبات الاهتزاز.
---

:::note Канонический источник
:::

صناديق الصدأ في تلك المستودعات تضغط على CLI ويمكن أن تكون ثابتة في المستودع
منسقين أو خدمات. قصاصات جديدة من المساعدين، كل شيء على الإطلاق
نحن بحاجة إلى مهندسين.

## مساعد لإثبات الدفق

استخدم محلل دفق الدليل الفعلي لتجميع المقاييس منه
إجابة HTTP:

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

النسخة الكاملة (بشهادة) تأتي في `docs/examples/sorafs_rust_proof_stream.rs`.
يُظهر `ProofStreamSummary::to_json()` أنه مقياس JSON وCLI الذي يتم شراؤه
تقديم البيانات في الواجهة الخلفية لقابلية الملاحظة أو تأكيدات CI.

## سجل جلب متعدد المصادر

تقوم الوحدة `sorafs_car::multi_fetch` بإعادة جدولة الجلب غير المتزامنة، وهي متاحة
سطر الأوامر. تحقيق `sorafs_car::multi_fetch::ScorePolicy` والمضي قدمًا عبره
`FetchOptions::score_policy`، لإنشاء محققين. اختبار الوحدة
يظهر `multi_fetch::tests::score_policy_can_filter_providers` كيف تتدفق
افتراضات عادية.

المقابض الأخرى تدعم علم CLI:- `FetchOptions::per_chunk_retry_limit` علم الاتصال `--retry-budget` لـ CI
  بادئ ذي بدء، الذي يقوم بإعادة المحاولة.
- قم بدمج `FetchOptions::global_parallel_limit` مع `--max-peers` للإلغاء
  مجموعة من المقدمين الفريدين.
- `OrchestratorConfig::with_telemetry_region("region")` قياس المقاييس
  `sorafs_orchestrator_*`، و`OrchestratorConfig::with_transport_policy`
  علم CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` فكرة التشجيع
  في أعلى سطر الأوامر/SDK؛ استخدم `TransportPolicy::DirectOnly` فقط قبل التدريج
  الرجوع إلى إصدار سابق أو توجيه الامتثال في النطاق، وحجز `SoranetStrict` لذلك
  طيارو PQ فقط مع موافقة مجانية.
- تثبيت `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` чтобы форсировать تحميلات PQ فقط؛ مساعد آلي
  استخدم سياسات النقل/عدم الكشف عن هويتك، إذا لم يتم تجاوزها تمامًا.
- استخدم `SorafsGatewayFetchOptions::policy_override` لتقطيع الوقت
  النقل أو طبقة عدم الكشف عن هويته لسبب واحد؛ مساعدة أي شخص آخر في الشراء
  تخفيض رتبة Brownout واستبدالها، إذا لم تتمكن الطبقة المتأخرة من أن تكون سعيدًا.
- بايثون (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) وجافا سكريبت
  تستخدم الارتباطات (`sorafsMultiFetchLocal`) بمثابة جدولة، ثم تضاف
  `return_scoreboard=true` في هؤلاء المساعدين للحصول على أفضل النتائج مع
  إيصالات قطعة.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` يسجل دفق OTLP،
  حزمة التبني المتوافقة. إذا لم يكن الأمر كذلك، فإن العميل يراجع تلقائيًا
  `region:<telemetry_region>` (أو `chain:<chain_id>`) للحصول على بيانات التعريف حتى الآنالتسمية الوصفية.

## الجلب عبر `iroha::Client`

يتضمن Rust SDK مساعدًا لجلب البوابة؛ قم بقراءة البيان ومقدمي الوصفات
(بما في ذلك الرموز المميزة للتيار) ويتحكم العميل في الجلب متعدد المصادر:

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

تثبيت `transport_policy` في `Some(TransportPolicy::SoranetStrict)`، عند التحميلات
يجب تجاوز المرحلات الكلاسيكية، أو في `Some(TransportPolicy::DirectOnly)`، عندما
SoraNet بحاجة إلى المزيد من الخدمة. قم بتحميل `scoreboard.persist_path` إلى الدليل
قطع أثرية فعالة، قم بتأكيد `scoreboard.now_unix_secs` وتأمينها اختياريًا
سياق `scoreboard.metadata` (تركيبات التسميات، كل Torii وما إلى ذلك) إلخ،
من أجل `cargo xtask sorafs-adoption-check`، من المحتمل تحديد JSON بين مجموعات SDK
من مصدر النقطة، الذي ينتمي إلى SF-6c.
`Client::sorafs_fetch_via_gateway` يكمل بيان التعريف التعريفي هذا،
يمكن إجراء تحليل اختياري لبيان CID والعلامة `gateway_manifest_provided`
تم إرسال `GatewayFetchConfig` مسبقًا للتأكيد على بيان المغلف المرفق
يجب أن يتم دعم SF-6c بدون دبلن كبير من هذا القطب.

## مساعدون للبيان

يحتوي `ManifestBuilder` على البرنامج الأساسي الخاص بالاشتراك في حمولات Norito:

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

قم بإنشاء منشئ مرة أخرى حيث تحتاج الخدمة إلى إنشاء قوائم على متن الطائرة; سطر الأوامر
هناك خط أنابيب موصى به لتحديد خطوط الأنابيب.