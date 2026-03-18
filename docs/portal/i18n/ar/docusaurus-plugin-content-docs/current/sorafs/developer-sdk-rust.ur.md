---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-sdk-الصدأ
العنوان: مقتطفات Rust SDK
Sidebar_label: مقتطفات الصدأ
الوصف: تدفقات الأدلة والبيانات تستهلك الحد الأدنى من أمثلة الصدأ.
---

:::ملاحظة مستند ماخذ
:::

يقوم مستودعه الذي يحتوي على صناديق الصدأ وCLI وبطاقة الطاقة والمنسقين أو الخدمات المخصصة بتضمين أي شيء في أي مكان.
تم إضافة المزيد من المقتطفات المساعدة التي تسلط الضوء على البطاقة وهي عبارة عن طلب سريع.

## مساعد دفق الإثبات

تعمل مقاييس استجابة HTTP على تجميع البيانات لإعادة استخدام محلل دفق الدليل الحالي:

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

النسخة الكاملة (الاختبارات سميت) `docs/examples/sorafs_rust_proof_stream.rs` متاحة.
`ProofStreamSummary::to_json()` ومقاييس عرض JSON وCLI ديتا، جيس س
الواجهات الخلفية لقابلية الملاحظة أو تغذية تأكيدات CI سهلة الاستخدام.

## سجل جلب متعدد المصادر

تعرض الوحدة `sorafs_car::multi_fetch` وجدولة الجلب غير المتزامنة استخدام CLI.
`sorafs_car::multi_fetch::ScorePolicy` تنفيذ البطاقة و`FetchOptions::score_policy` تمرير البطاقة
تاكہ مزود طلب اللحن ہو سکے۔ اختبار الوحدة `multi_fetch::tests::score_policy_can_filter_providers`
يتم فرض التفضيلات المخصصة بطريقة مختلفة.

المقابض الأخرى: أعلام CLI ومرآة:- `FetchOptions::per_chunk_retry_limit` CI يعمل على تطابق علامة `--retry-budget` مع كرتا ہے
  قم بإعادة المحاولة من أجل تقييد البطاقة.
- `FetchOptions::global_parallel_limit` و`--max-peers` يعملان على دمج البطاقات المتزامنة
  مقدمي الخدمة يبلغ عددهم الحد الأقصى.
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` مقاييس علامة كرتا ہے،
  علامة `OrchestratorConfig::with_transport_policy` CLI `--transport-policy` مرآة ومرآة.
  أسطح `TransportPolicy::SoranetPreferred` CLI/SDK يتم شحنها افتراضيًا؛ `TransportPolicy::DirectOnly`
  اتبع مرحلة الرجوع إلى إصدار أقدم أو توجيه الامتثال لإصدار أقدم، و`SoranetStrict`
  يحصل طيارو PQ فقط على موافقة صريحة على الاحتفاظ بالاحتياطي المستمر.
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` set قوة تحميل PQ فقط؛ النقل المساعد / عدم الكشف عن هويته
  السياسات التي تعمل على الترويج للأموال لا ينبغي تجاوزها بشكل واضح.
- `SorafsGatewayFetchOptions::policy_override` استخدام کریں تاکہ ایک طلب کے لیے النقل المؤقت
  أو دبوس طبقة عدم الكشف عن هويته؛ هذا الحقل أيضًا هو خفض رتبة براونوت، تخطي أوتا، وما إلى ذلك
  المستوى المطلوب يرضي لا يوجد فشل في ذلك اتا ہے۔
- بايثون (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) وجافا سكريبت (`sorafsMultiFetchLocal`)
  يتم استخدام أداة الجدولة هذه لإعادة استخدام البطاقات، وهي عبارة عن مجموعة مساعدات `return_scoreboard=true`
  الأوزان المحسوبة هي عبارة عن إيصالات مقطوعة.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP تيار کو سجل کرتا ہے جس نے حزمة التبنيبنايا تھ۔ إذا قمت بحذف العميل الخاص بك وفقًا لـ `region:<telemetry_region>` (أو `chain:<chain_id>`) استنتج هذه الرسالة
  تحتوي هذه البيانات الوصفية على تسمية وصفية.

## جلب عبر `iroha::Client`

يتضمن Rust SDK مساعد جلب البوابة؛ واصفات موفر البيان (رموز الدفق المميزة)
والعميل محرك الجلب متعدد المصادر:

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

كيفية تحميل المرحلات الكلاسيكية إلى `transport_policy` إلى `Some(TransportPolicy::SoranetStrict)`
قم بتعيين بطاقة الائتمان أو SoraNet لتجاوز تجاوز كامل إلى `Some(TransportPolicy::DirectOnly)`.
`scoreboard.persist_path` الذي يقوم بتحرير دليل العناصر عند نقطة معينة، وهو أمر ضروري وإصلاح `scoreboard.now_unix_secs`،
و`scoreboard.metadata` يحتوي على سياق الالتقاط (تسميات التركيبات، Torii الهدف، وما إلى ذلك) يتضمن هذه التقنية
`cargo xtask sorafs-adoption-check` SDKs الحتمية JSON تستهلك الطاقة وSF-6c والمصدر blob
تشمل رہے۔ `Client::sorafs_fetch_via_gateway` يستخدم بيانات التعريف كمعرف البيان، وتوقع CID للبيان الاختياري،
وعلامة `gateway_manifest_provided` تضاف إلى كرتا عبر `GatewayFetchConfig` الموردة، وذلك مظروف بيان موقع
يلتقط السجل متطلبات أدلة SF-6c بعد تضمين الحقول التي يتم إعدادها مسبقًا لتكرارها.

## مساعدين واضحين

تقوم الحمولات `ManifestBuilder` أيضًا بتجميع Norito برمجيًا بالطريقة الأساسية:

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
```تعمل خدمات الجوّال التي يتم إنشاؤها أثناء التنقل على إنشاء ملفات تعريف الارتباط ومنشئ التضمين؛ خطوط الأنابيب الحتمية هي المسار الموصى به لـ CLI أيضًا.