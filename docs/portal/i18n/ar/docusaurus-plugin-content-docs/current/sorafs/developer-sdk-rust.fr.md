---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: المطور-sdk-الصدأ
العنوان: Extraits SDK Rust
Sidebar_label: إضافات الصدأ
الوصف: أمثلة على الحد الأدنى من الصدأ لاستهلاك تدفقات إثبات البيانات والبيانات.
---

:::ملاحظة المصدر الكنسي
:::

تصدأ الصناديق بسبب إيداعها في CLI ويمكن أن تكون عالقة في
الأوركسترا أو الخدمات الشخصية. يتم تنفيذ الإضافات بشكل مسبق
les helper les plus requestés.

## تيار إثبات المساعد

قم بإعادة استخدام التيار التحليلي الموجود لإضافة المقاييس بعد مرة واحدة
الرد HTTP :

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

الإصدار الكامل (مع الاختبارات) موجود في `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` يقدم نفس JSON من مقاييس CLI، وهذا ما
تسهيل تغذية الواجهات الخلفية لإمكانية الملاحظة أو التأكيدات CI.

## سجل جلب متعدد المصادر

تعرض الوحدة `sorafs_car::multi_fetch` جدولة الجلب غير المتزامنة المستخدمة
على قدم المساواة لو CLI. قم بتنفيذ `sorafs_car::multi_fetch::ScorePolicy` وتمريره عبر
`FetchOptions::score_policy` لضبط ترتيب مقدمي الخدمة. اختبار الوحدة
`multi_fetch::tests::score_policy_can_filter_providers` مونتر تعليق فرضي des
التفضيلات الشخصية.

مقابض أخرى محاذاة sur les flags CLI :- `FetchOptions::per_chunk_retry_limit` يتوافق مع العلم `--retry-budget` من أجل des
  يدير CI الذي يتناقض مع المحاولات الطوعية.
- قم بدمج `FetchOptions::global_parallel_limit` مع `--max-peers` من أجل وضع اللوح
  عدد مقدمي الخدمات المتزامنين.
- `OrchestratorConfig::with_telemetry_region("region")` علامة المقاييس
  `sorafs_orchestrator_*`، ثم `OrchestratorConfig::with_transport_policy`
  يعكس العلم CLI `--transport-policy`. تقديرات `TransportPolicy::SoranetPreferred`
  livré comme valeur par défaut côté CLI/SDK ؛ استخدم `TransportPolicy::DirectOnly`
  فقط عند الرجوع إلى إصدار سابق أو توجيه المطابقة، ثم قم بالحجز
  `SoranetStrict` aux الطيارين PQ فقط مع الموافقة الصريحة.
- تحديد `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` لفرض عمليات التحميل PQ فقط؛ لو المساعد
  تعزيز سياسات النقل/عدم الكشف عن الهوية تلقائيًا وتجاوزها بشكل صريح.
- استخدم `SorafsGatewayFetchOptions::policy_override` لقفل مستوى واحد
  نقل أو إخفاء هويتك مؤقتًا لطلب ؛ fournir l'un des champs
  تحديد التدهور البني والصدأ إذا لم يكن المستوى المطلوب موجودًا
  يرضي.
- روابط بايثون (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) وآخرون
  يقوم JavaScript (`sorafsMultiFetchLocal`) بإعادة استخدام نفس جدولة البيانات؛ تعريف
  `return_scoreboard=true` في هذه الأدوات المساعدة لاستعادة الأوزان المحسوبة بنفسي
  الوقت الذي يتم فيه استلام إيصالات القطعة.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` قم بتسجيل التدفق OTLP
  الذي هو منتج حزمة التبني. S'il est omis، العميل يستمد تلقائيًا`region:<telemetry_region>` (أو `chain:<chain_id>`) للإشارة إلى التحولات
  toujours une étiquette de description.

## جلب عبر `iroha::Client`

يقوم SDK Rust بمنع مساعد جلب البوابة؛ فورنيسيز بيان أكثر من ذلك
واصفات مقدمي الخدمة (وتشمل الرموز المميزة للتدفق) واترك العميل يقودها
جلب متعدد المصادر :

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

قم بتعريف `transport_policy` على `Some(TransportPolicy::SoranetStrict)` عند
التحميلات يجب أن ترفض التتابعات الكلاسيكية، أو على `Some(TransportPolicy::DirectOnly)`
عندما يكون SoraNet محيطًا بالكامل. Pointez `scoreboard.persist_path` فيرس لو
تم إصلاح ذخيرة القطع الأثرية في نهاية المطاف `scoreboard.now_unix_secs` و
قم بمسح `scoreboard.metadata` مع سياق الالتقاط (ملصقات التركيبات والكابلات)
Torii، وما إلى ذلك) حتى يستخدم `cargo xtask sorafs-adoption-check` محدد JSON
بين أدوات تطوير البرامج (SDKs) مع وجود نقطة المصدر على أساس SF-6c.
`Client::sorafs_fetch_via_gateway` يثري هذه الاختلالات مع المعرف
البيان، الانتظار المحتمل لبيان CID والعلامة `gateway_manifest_provided` ar
يشرح المفتش `GatewayFetchConfig` أن اللقطات تتضمن مظروفًا
تم التوقيع بشكل واضح على ضرورة توفير SF-6c بدون تكرار هذه الأبطال على المستوى الرئيسي.

## مساعدين البيان

`ManifestBuilder` بقية الطريقة التقليدية لتجميع الحمولات Norito للطريقة
برمجية :

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
```قم بدمج المنشئ من خلال الخدمات التي يجب أن تنشئ بيانات تريدها؛
يبقى CLI هو الخيار الموصى به لخطوط الأنابيب المحددة.