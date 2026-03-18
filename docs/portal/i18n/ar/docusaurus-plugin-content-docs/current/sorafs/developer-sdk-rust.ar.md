---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-sdk-الصدأ
العنوان: مقتطفات الصدأ SDK
Sidebar_label: مقتطفات الصدأ
الوصف: أمثلة الصدأ اليومي لاستهلاك تدفقات الأدلة والبيانات.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/developer/sdk/rust.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف تشغيل مجموعة Sphinx القديمة.
:::

تغذّي حزم الصدأ في واجهة المستودع هذه CLI ويمكن تضمينها داخل مُنسِّقات أو خدمات مخصّصة.
تسلط المقتطفات التالية من المساعدات التي يطلبها معظم المطورين.

## مساعد تدفق الأدلة

دراسة استخدام محلل تدفق الأدلة الحالية لجميع الارتباطات من HTTP:

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

لكن النسخة الكاملة (مع الاختبار) في `docs/examples/sorafs_rust_proof_stream.rs`.
إنتاج `ProofStreamSummary::to_json()` نفس JSON للمقاييس التي تصدره CLI، ما نبني تغذية
لوحات الرصد أو فرضيات CI.

## تقييم جلب المصادر المتعددة

يعرض الموديول `sorafs_car::multi_fetch` مُجدول الجلب غير المتزامن المستخدم في CLI.
نفّذ `sorafs_car::multi_fetch::ScorePolicy` ومرّره عبر `FetchOptions::score_policy`
لضبط ترتيب المنظمين. يوضح الوحدة المختبرة
`multi_fetch::tests::score_policy_can_filter_providers` كيفية فرض تفضيلات مخصصة.

مبادئ أخرى لرعاية أعلام CLI:- `FetchOptions::per_chunk_retry_limit` يطابق علم `--retry-budget` تشغيلات CI التي
  تحدي عدد المحاولات العمدية.
- اجمع بين `FetchOptions::global_parallel_limit` و`--max-peers` ملتقي عدد المتحكمين
  المتزامنين.
- يقوم `OrchestratorConfig::with_telemetry_region("region")` بسم معايير
  `sorafs_orchestrator_*`، يعكس `OrchestratorConfig::with_transport_policy`
  علم CLI `--transport-policy`. تُشحن `TransportPolicy::SoranetPreferred` كافتراضي عبر
  أسطح CLI/SDK؛ استخدم `TransportPolicy::DirectOnly` فقط عند اختبار التخفيض أو اتباع
  توجيهات جديدة، واحجز `SoranetStrict` لطيارين PQ-فقط بموافقة صريحة.
- اضبط `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` لفرض رفع PQ فقط؛ سيعزز السياسات الإيجابية
  إخفاء/ إخفاء الهوية الأصلية ما لم يتم تجاوزها بصراحة.
- استخدم `SorafsGatewayFetchOptions::policy_override` لتحرك أو إخفاء هوية
  طلب طلب واحد؛ توصل إلى أي تقدم في الوصول إلى اللون البني الكبير ويفشل عندما يفشل في الاستجابة الكندية
  المطلوبة.
- تستخدم روابط Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) وJavaScript
  (`sorafsMultiFetchLocal`) نفس المُجدول، لذا اضبط `return_scoreboard=true` في تلك المساعدات
  لا يتم استرجاع الوزنة المحفوظة مع إيصالات الـ Chunk.
- تسجيل `SorafsGatewayScoreboardOptions::telemetry_source_label` تدفق OTLP الذي أنتج
  حزمة تبنٍ. عند الإغفال، يشتري العميل `region:<telemetry_region>` (أو `chain:<chain_id>`) الجديد
  حتى تحمل المذكرات الوصفية دائمًا.

## الجلب عبر `iroha::Client`

حزمة الضمان Rust SDK مساعد الجلب عبر البوابة؛ مرر بيان مع أوصاف المتحكمين
(بما في ذلك برمجيات البريد الإلكتروني) ودع العملاء يديرون جلب الموارد المتعددة:

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
```اضبط `transport_policy` إلى `Some(TransportPolicy::SoranetStrict)` عندما يجب أن ترفض
الرفوعات الممتعة الكلاسيكية، أو `Some(TransportPolicy::DirectOnly)` عندما يجب تجاوزها
سورانت بالكامل. موجه `scoreboard.persist_path` إلى دليل آرتيفاكتات الإصدار، واضبط
اختياريًا `scoreboard.now_unix_secs`، واملأ `scoreboard.metadata` بسياق التقاطع
(وسوم المباريات، هدف Torii، إلخ) حتى يستهلك `cargo xtask sorafs-adoption-check` JSON
حسمًا عبر SDKs مع الملف المصدر الذي يستخدمه SF-6c.
تقوم `Client::sorafs_fetch_via_gateway` الآن بإثراء تلك البياناتت بمعرفة المانيفست،
وتوقع CID المانيفست الاختياري، وعلم `gateway_manifest_provided` عبر الفحص
`GatewayFetchConfig` مقدم، بما في ذلك تلبيات الالتقاطات التي تتضمن ظرف مانيفيست تتطلب موقعًا
دليل SF-6c دون هذا الاختلاف باليد.

## مساعدات المانيفيست

Lose `ManifestBuilder` طريقة معتمدة لجميع حمولات Norito برمجياً:

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

بما في ذلك المنشئ حيثما انتهت الخدمات إلى توليد المانيفيستات ديناميكيًا؛ يميز CLI هو المسار
لخطوط الحامية.