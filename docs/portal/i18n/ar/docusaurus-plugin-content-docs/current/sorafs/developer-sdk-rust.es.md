---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-sdk-الصدأ
العنوان: Fragmentos de SDK de Rust
Sidebar_label: أجزاء الصدأ
الوصف: أمثلة على الحد الأدنى من الصدأ للتدفقات والبيانات المقاومة للاستهلاك.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/developer/sdk/rust.md`. احتفظ بنسخ متزامنة.
:::

صناديق الصدأ في هذا المستودع تحفز CLI ويمكن أن تستقر فيها
orquestadores o servicios Personalizados. يتم إرسال أجزاء الجزء العلوي إلى المساعدين
ما هو أكثر دعمًا للمطورين.

## تدفق مساعد الإثبات

إعادة استخدام محلل تدفق الإثبات الموجود لتجميع مقاييس استجابة HTTP:

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

النسخة الكاملة (مع الاختبارات) موجودة في `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` يعرض نفس JSON من المقاييس مثل CLI، كما هو
تسهيل تشغيل الواجهات الخلفية للمراقبة أو عمليات CI.

## نقطة الجلب متعددة المصادر

تعرض الوحدة `sorafs_car::multi_fetch` جدولة الجلب غير المتزامنة المستخدمة من أجل
سطر الأوامر. قم بتنفيذ `sorafs_car::multi_fetch::ScorePolicy` وانتقل عبر
`FetchOptions::score_policy` لضبط أمر الموردين. الاختبار الوحدوي
`multi_fetch::tests::score_policy_can_filter_providers` يعمل كجهاز
التفضيلات الشخصية.

مقابض أخرى أعلام reflejan ديل CLI:- `FetchOptions::per_chunk_retry_limit` يتزامن مع العلم `--retry-budget` الفقرة
  عمليات تشغيل CI التي تقيّد عمليات الإبقاء على الاقتراح.
- الجمع بين `FetchOptions::global_parallel_limit` مع `--max-peers` للحد من ذلك
  عدد الموردين المتزامنين.
- `OrchestratorConfig::with_telemetry_region("region")` آداب المقاييس
  `sorafs_orchestrator_*`، بينما `OrchestratorConfig::with_transport_policy`
  يعكس العلم `--transport-policy` del CLI. `TransportPolicy::SoranetPreferred`
  se entrega مثل قيمة العيوب والسطوح CLI/SDK؛ الولايات المتحدة الأمريكية
  `TransportPolicy::DirectOnly` فقط للتحضير للرجوع إلى إصدار أقدم أو متابعة التوجيه
  الامتثال، وحجز `SoranetStrict` للطيارين PQ فقط مع الموافقة الصريحة.
- التكوين `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` لفرز ملفات PQ الفرعية فقط؛ المساعد الترويجي
  سياسات النقل/الطلقات المجهولة التي يتم تسميتها تلقائيًا
  بشكل واضح.
- الولايات المتحدة الأمريكية `SorafsGatewayFetchOptions::policy_override` لتشغيل وسيلة نقل أو مستوى
  مجهول مؤقت لطلب واحد فقط؛ بشكل متناسب بين المجالات
  إذا حذفت التدهور الناتج عن اللون البني وسقطت إذا لم يكن من الممكن الحصول على المستوى المطلوب
  com.satisfacerse.
- روابط بايثون (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) ذ
  يقوم JavaScript (`sorafsMultiFetchLocal`) بإعادة استخدام نفس المجدول، كما يتم تكوينه
  `return_scoreboard=true` وهذه الأدوات المساعدة لاسترداد العملات المحسوبة جنبًا إلى جنب مع
  تلقيت القطعة.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` قم بتسجيل تيار OTLP الذيأنتج حزمة اعتماد. عندما يحذف العميل
  `region:<telemetry_region>` (o `chain:<chain_id>`) تلقائيًا حتى تتمكن من ذلك
  ستظل البيانات التعريفية تحتوي على اتيكيت وصفي.

## جلب عبر `iroha::Client`

تشتمل مجموعة SDK الخاصة بـ Rust على مساعد جلب البوابة؛ تقديم بيان أكثر من ذلك
واصفات الموردين (بما في ذلك رموز الدفق المميزة) وتأكد من قيام العميل بالتنفيذ
الجلب متعدد المصادر:

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

التكوين `transport_policy` كما `Some(TransportPolicy::SoranetStrict)` عند الانتهاء
subidas deban rechazar يرحل الكلاسيكيات، o `Some(TransportPolicy::DirectOnly)` cuando
SoraNet يجب حذفه بالكامل. Apunta `scoreboard.persist_path` في الدليل
مصنوعات الإصدار، متاحة اختياريًا `scoreboard.now_unix_secs` وكاملة
`scoreboard.metadata` مع سياق الالتقاط (علامات التثبيت، الهدف Torii، وما إلى ذلك)
لكي `cargo xtask sorafs-adoption-check` يستهلك JSON بين مجموعات SDK
فقاعة الإجراءات التي تتوقعها من SF-6c.
`Client::sorafs_fetch_via_gateway` الآن تكملة لهذه البيانات الوصفية مع المعرف
 في البيان، التوقع الاختياري لبيان CID والعلم
`gateway_manifest_provided` يتم فحص `GatewayFetchConfig` بالطريقة الصحيحة
التي تلتقطها تتضمن محضر بيان ثابت يلبي متطلباتها
يتم اختبار SF-6c بدون تكرار هذا المجال يدويًا.

## مساعدين البيان

`ManifestBuilder` يبقي على شكل الكنسي لتجميع الحمولات Norito من الشكل
برمجية:

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
```قم بدمج المنشئ عندما تظهر الخدمات التي تحتاج إلى إنشاء بيانات في السيارة؛ ش
CLI سيبقى على المسار الموصى به لتحديد خطوط الأنابيب.