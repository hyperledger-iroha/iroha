---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-sdk-الصدأ
العنوان: مقتطفات من SDK Rust
Sidebar_label: مقتطفات من الصدأ
الوصف: أمثلة على الحد الأدنى من الصدأ لتدفقات إثبات الاستهلاك والبيانات.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas as copias sincronzadas.
:::

الصناديق الصدئة موجودة في مخزن الطعام أو CLI وقد يتم تضمينها فيها
orquestradores أو الخدمات المخصصة. يتم إزالة المقتطفات من المساعدين الذين يقومون بذلك
نحن نظهر أيضًا أننا نطلب من المطورين.

## تدفق مساعد الإثبات

إعادة استخدام محلل دفق الدليل الموجود لتجميع مقاييس رد HTTP:

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

والعكس كامل (com الخصيتين) يعيش في `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` يعرض مقاييس JSON نفسها لـ CLI، بسهولة
قم بتزويد الواجهات الخلفية للملاحظة أو تأكيدات CI.

## سجل الجلب متعدد المصادر

تعرض الوحدة `sorafs_car::multi_fetch` جدولة الجلب المستخدمة بشكل متزامن
سطر الأوامر. تنفيذ `sorafs_car::multi_fetch::ScorePolicy` والتمرير عبر
`FetchOptions::score_policy` لضبط إعدادات النظام. يا اختبار الوحدة
`multi_fetch::tests::score_policy_can_filter_providers` يظهر كتفضيلات مهمة
تخصيص.

مقابض Outros لأعلام espelham تفعل CLI:- `FetchOptions::per_chunk_retry_limit` يتوافق مع العلم `--retry-budget` للتشغيل
  de CI que تقييد التصورات المقترحة.
- ادمج `FetchOptions::global_parallel_limit` مع `--max-peers` لتحديد الرقم
  دي بروفيدوريس المتزامنة.
- `OrchestratorConfig::with_telemetry_region("region")` ماركا كمقاييس
  `sorafs_orchestrator_*`، على سبيل المثال `OrchestratorConfig::with_transport_policy`
  o وضع علامة على CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` هو nas الافتراضي
  السطوح CLI/SDK؛ استخدم `TransportPolicy::DirectOnly` للتحضير للرجوع إلى إصدار أقدم
  أو اتبع توجيهًا للامتثال، واحتفظ بـ `SoranetStrict` للطيارين PQ فقط
  com aprovacao صريحة.
- تعريف `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` لعمليات تحميل السيارات PQ فقط؛ o مساعد في الترقية
  تلقائيًا كسياسة نقل/إخفاء الهوية إلى الحد الذي تظهر فيه بشكل صريح
  سوبرسكريتاس.
- استخدم `SorafsGatewayFetchOptions::policy_override` لإصلاح الطبقة المؤقتة
  نقل أو مجهول لطلب واحد؛ Fornecer qualquer أم دوس كامبوس بولا
  o انخفاض الرتبة وفشل عندما لا يمكن الوصول إلى المستوى المطلوب.
- روابط نظام التشغيل بايثون (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) ه
  إعادة استخدام JavaScript (`sorafsMultiFetchLocal`) أو نفس المجدول، وتحديده
  `return_scoreboard=true` هذه الأدوات المساعدة لاسترداد الأموال المحسوبة مع com
  إيصالات قطعة.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` قم بالتسجيل أو البث عبر OTLP
  إنتاج حزمة التبني. عندما يتم حذفه، يشتق العميل تلقائيًا`region:<telemetry_region>` (أو `chain:<chain_id>`) لاستمرار البيانات الوصفية
  قم بتمرير جولة وصفية.

## جلب عبر `iroha::Client`

يتضمن SDK Rust مساعد جلب البوابة؛ Forneca um واضح أكثر الوصفات دي
المثبتون (بما في ذلك الرموز المميزة للتيار) والمساعدة في توصيل العميل أو جلب مصادر متعددة:

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

Defina `transport_policy` كما `Some(TransportPolicy::SoranetStrict)` عند التحميلات
نقترح عليك إعادة ترحيل الكلاسيكيات، أو `Some(TransportPolicy::DirectOnly)` عندما
SoraNet يتطلب تجاوزه بالكامل. أبونتي `scoreboard.persist_path` ل
دليل تحرير القطع الأثرية، وإصلاح `scoreboard.now_unix_secs` اختياريًا وتحريرها
`scoreboard.metadata` مع سياق الالتقاط (ملصقات التركيبات، مثل Torii، وما إلى ذلك)
لكي `cargo xtask sorafs-adoption-check` يستهلك محددات JSON بين مجموعات SDK
مع نقطة المصدر التي يتوقعها SF-6c.
`Client::sorafs_fetch_via_gateway` الآن يزيد من البيانات الوصفية مع معرف
البيان، توقع اختياري لبيان CID والعلامة `gateway_manifest_provided`
Inspecionando o `GatewayFetchConfig` fornecido، لكي تلتقط الصور التي تتضمنها
مغلف البيان مطابق لمتطلبات الأدلة SF-6c غير مكررة
الحرم الجامعي يدويا.

## مساعدين البيان

`ManifestBuilder` يواصل إرسال نموذج Canonica لحمولات Norito النموذج
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
```

قم بتضمين المُنشئ دائمًا حيث يتم تحديد الخدمات لتظهر في وقت حقيقي; س
يتبع CLI الإرسال أو المسار الموصى به لتحديد خطوط الأنابيب.