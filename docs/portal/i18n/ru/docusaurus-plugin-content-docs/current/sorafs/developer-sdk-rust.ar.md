---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: developer-sdk-rust
title: مقتطفات Rust SDK
sidebar_label: مقتطفات Rust
description: أمثلة Rust دنيا لاستهلاك تدفقات الأدلة والمانيفستات.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/developer/sdk/rust.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

تغذّي حزم Rust في هذا المستودع واجهة CLI ويمكن تضمينها داخل مُنسِّقات أو خدمات مخصّصة.
تسلط المقتطفات التالية الضوء على المساعدات التي يطلبها معظم المطورين.

## مساعد تدفق الأدلة

أعد استخدام محلل تدفق الأدلة الحالي لتجميع المقاييس من استجابة HTTP:

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

تتوفر النسخة الكاملة (مع الاختبارات) في `docs/examples/sorafs_rust_proof_stream.rs`.
تنتج `ProofStreamSummary::to_json()` نفس JSON للمقاييس الذي يصدره CLI، ما يسهل تغذية
منصات الرصد أو فرضيات CI.

## تقييم الجلب متعدد المصادر

يعرض الموديول `sorafs_car::multi_fetch` مُجدول الجلب غير المتزامن المستخدم في CLI.
نفّذ `sorafs_car::multi_fetch::ScorePolicy` ومرّره عبر `FetchOptions::score_policy`
لضبط ترتيب المزوّدين. يوضح اختبار الوحدة
`multi_fetch::tests::score_policy_can_filter_providers` كيفية فرض تفضيلات مخصصة.

عناصر أخرى تعكس أعلام CLI:

- `FetchOptions::per_chunk_retry_limit` يطابق علم `--retry-budget` لتشغيلات CI التي
  تحد عدد المحاولات عمدًا.
- اجمع بين `FetchOptions::global_parallel_limit` و`--max-peers` لتقييد عدد المزوّدين
  المتزامنين.
- يقوم `OrchestratorConfig::with_telemetry_region("region")` بوسم مقاييس
  `sorafs_orchestrator_*`، بينما يعكس `OrchestratorConfig::with_transport_policy`
  علم CLI `--transport-policy`. تُشحن `TransportPolicy::SoranetPreferred` كافتراضي عبر
  أسطح CLI/SDK؛ استخدم `TransportPolicy::DirectOnly` فقط عند اختبار downgrade أو اتباع
  توجيه امتثال، واحجز `SoranetStrict` لطيارين PQ-only بموافقة صريحة.
- اضبط `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` لفرض رفع PQ-only؛ سيعزز المساعد سياسات
  النقل/إخفاء الهوية تلقائيًا ما لم يتم تجاوزها صراحةً.
- استخدم `SorafsGatewayFetchOptions::policy_override` لتثبيت طبقة نقل أو إخفاء هوية
  مؤقتة لطلب واحد؛ تمرير أي حقل يتجاوز تخفيض brownout ويفشل عندما يتعذر تلبية الطبقة
  المطلوبة.
- تستخدم روابط Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) وJavaScript
  (`sorafsMultiFetchLocal`) نفس المُجدول، لذا اضبط `return_scoreboard=true` في تلك المساعدات
  لاسترجاع الأوزان المحسوبة مع إيصالات الـ chunk.
- يسجل `SorafsGatewayScoreboardOptions::telemetry_source_label` تدفق OTLP الذي أنتج
  حزمة تبنٍ. عند الإغفال، يستنتج العميل `region:<telemetry_region>` (أو `chain:<chain_id>`) تلقائيًا
  حتى تحمل الميتاداتا دائمًا وسمًا وصفيًا.

## الجلب عبر `iroha::Client`

تضم حزمة Rust SDK مساعد الجلب عبر البوابة؛ مرر مانيفست مع أوصاف المزوّدين
(بما في ذلك رموز البث) ودع العميل يدير الجلب متعدد المصادر:

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
الرفوعات المرحلات الكلاسيكية، أو `Some(TransportPolicy::DirectOnly)` عندما يجب تجاوز
SoraNet بالكامل. وجّه `scoreboard.persist_path` إلى دليل آرتيفاكتات الإصدار، واضبط
اختياريًا `scoreboard.now_unix_secs`، واملأ `scoreboard.metadata` بسياق الالتقاط
(وسوم fixtures، هدف Torii، إلخ) حتى يستهلك `cargo xtask sorafs-adoption-check` JSON
حتميًا عبر SDKs مع ملف المصدر الذي يتوقعه SF-6c.
تقوم `Client::sorafs_fetch_via_gateway` الآن بإثراء تلك الميتاداتا بمعرف المانيفست،
وتوقع CID المانيفست الاختياري، وعلم `gateway_manifest_provided` عبر فحص
`GatewayFetchConfig` المقدم، بحيث تلبي الالتقاطات التي تتضمن ظرف مانيفست موقع متطلبات
دليل SF-6c دون تكرار تلك الحقول يدويًا.

## مساعدات المانيفست

يظل `ManifestBuilder` الطريقة المعتمدة لتجميع حمولات Norito برمجيًا:

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

ضمّن المنشئ حيثما احتاجت الخدمات إلى توليد المانيفستات ديناميكيًا؛ تظل CLI هي المسار
الموصى به للخطوط الحتمية.
