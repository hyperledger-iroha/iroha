---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-SDK-RUST
عنوان: زنگ SDK اسنیپٹس
سائڈبار_لیبل: زنگ کے ٹکڑے
تفصیل: پروف اسٹریمز اور ظاہر ہونے کے لئے زنگ میں کم سے کم مثالیں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/developer/sdk/rust.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

اس ذخیرے میں زنگ آلود خانے سی ایل آئی کو طاقت دیتے ہیں اور اس کے اندر سرایت کرسکتے ہیں
آرکسٹریٹر یا ذاتی خدمات۔ نیچے دیئے گئے ٹکڑوں میں مددگاروں کو اجاگر کیا گیا ہے
ڈویلپرز زیادہ تر کیا کہتے ہیں۔

## پروف اسٹریم مددگار

HTTP جواب سے میٹرکس شامل کرنے کے لئے موجودہ پروف اسٹریم پارسر کا دوبارہ استعمال کریں:

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

مکمل ورژن (ٹیسٹوں کے ساتھ) `docs/examples/sorafs_rust_proof_stream.rs` پر رہتا ہے۔
`ProofStreamSummary::to_json()` ایک ہی میٹرکس JSON کو CLI کی طرح پیش کرتا ہے ، جو
مشاہدہ کرنے کے لئے بیک اینڈ یا سی آئی کے دعوے کو کھانا کھلانا آسان بناتا ہے۔

## ملٹی سورس بازیافت اسکور

ماڈیول `sorafs_car::multi_fetch` کے ذریعہ استعمال ہونے والے غیر متزلزل بازیافت شیڈیولر کو بے نقاب کیا گیا ہے
سی ایل آئی۔ `sorafs_car::multi_fetch::ScorePolicy` کو نافذ کریں اور اسے کے ذریعے پاس کریں
سپلائر آرڈر کو ایڈجسٹ کرنے کے لئے `FetchOptions::score_policy`۔ یونٹ ٹیسٹ
`multi_fetch::tests::score_policy_can_filter_providers` دکھاتا ہے کہ کس طرح مجبور کیا جائے
ذاتی ترجیحات۔

دوسرے نوبس سی ایل آئی کے جھنڈوں کی عکاسی کرتے ہیں:

- `FetchOptions::per_chunk_retry_limit` `--retry-budget` پرچم سے میل کھاتا ہے
  سی آئی چلتا ہے جو مقصد پر دوبارہ کوشش کرتا ہے۔
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ جوڑیں تاکہ محدود کریں
  ہم آہنگی سپلائرز کی تعداد۔
- `OrchestratorConfig::with_telemetry_region("region")` لیبل میٹرکس
  `sorafs_orchestrator_*` ، جبکہ `OrchestratorConfig::with_transport_policy`
  CLI پرچم `--transport-policy` کی عکاسی کرتا ہے۔ `TransportPolicy::SoranetPreferred`
  سی ایل آئی/ایس ڈی کے سطحوں میں بطور ڈیفالٹ فراہم کیا گیا۔ استعمال کریں
  `TransportPolicy::DirectOnly` صرف جب نیچے کی تیاری یا کسی پالیسی کی پیروی کرتے ہو
  تعمیل ، اور واضح منظوری کے ساتھ صرف PQ- صرف پائلٹوں کے لئے `SoranetStrict`۔
- سیٹ `sorafsgatewayfetchptions :: write_mode_hint =
  کچھ (Writemodehint :: اپلوڈپقونلی) P PQ- صرف اپ لوڈ کو مجبور کرنا ؛ مددگار فروغ پائے گا
  نقل و حمل/گمنامی کی پالیسیاں خود بخود جب تک اوور رائٹ نہ ہوں
  واضح طور پر
- نقل و حمل یا درجے کو ترتیب دینے کے لئے `SorafsGatewayFetchOptions::policy_override` استعمال کریں
  ایک ہی درخواست کے لئے عارضی گمنامی ؛ کسی بھی فیلڈ کو فراہم کرکے
  براؤن آؤٹ ڈاون گریڈ کو چھوڑ دیا جاتا ہے اور ناکام ہوجاتا ہے اگر درخواست کردہ درجے نہیں کرسکتا ہے
  مطمئن رہیں۔
- ازگر بائنڈنگز (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) اور
  جاوا اسکرپٹ (`sorafsMultiFetchLocal`) اسی شیڈولر کو دوبارہ استعمال کریں ، لہذا تشکیل کریں
  `return_scoreboard=true` ان مددگاروں میں حساب شدہ وزن کے ساتھ ساتھ بازیافت کرنے کے لئے
  حص rech ے کی رسیدیں۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP اسٹریم کو ریکارڈ کرتا ہے
  گود لینے کا بنڈل تیار کیا۔ جب چھوڑ دیا جاتا ہے تو ، مؤکل اخذ کرتا ہے
  `region:<telemetry_region>` (یا `chain:<chain_id>`) خود بخود تاکہ
  میٹا ڈیٹا ہمیشہ وضاحتی ٹیگ اٹھاتا ہے۔

## `iroha::Client` کے ذریعے بازیافت کریں

مورچا SDK میں بازیافت گیٹ وے مددگار شامل ہے۔ ایک اور منشور فراہم کرتا ہے
فراہم کنندہ ڈسریکٹر (بشمول اسٹریم ٹوکن) اور مؤکل کو عملی جامہ پہنانے دیں
ملٹی سورس بازیافت:

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
````transport_policy` کو `Some(TransportPolicy::SoranetStrict)` پر سیٹ کرتا ہے
اپ لوڈز کو کلاسک ریلے ، یا `Some(TransportPolicy::DirectOnly)` کو مسترد کرنا چاہئے
سورانیٹ کو مکمل طور پر چھوڑنا چاہئے۔ `scoreboard.persist_path` کو ڈائرکٹری پر پوائنٹ کریں
نمونے جاری کرتے ہیں ، اختیاری طور پر `scoreboard.now_unix_secs` سیٹ کرتا ہے اور مکمل کرتا ہے
`scoreboard.metadata` کیپچر سیاق و سباق کے ساتھ (فکسچر لیبل ، ٹارگٹ Torii ، وغیرہ)
`cargo xtask sorafs-adoption-check` کے ساتھ SDKs کے مابین جینیاتی JSON استعمال کرنے کے لئے
پروویژن بلاب کی توقع SF-6C کے ذریعہ ہے۔
`Client::sorafs_fetch_via_gateway` اب اس کی تکمیل کرتا ہے جو شناخت کنندہ کے ساتھ میٹا ڈیٹا ہے
 ظاہر ، اختیاری مینی فیسٹ سی آئی ڈی کی توقع ، اور پرچم
`gateway_manifest_provided` فراہم کردہ `GatewayFetchConfig` کا معائنہ کرکے ، تو
اس کی گرفت جس میں دستخط شدہ مینی فیسٹ ریپر شامل ہیں اس کی ضرورت کو پورا کریں
SF-6C ٹیسٹ دستی طور پر ان کھیتوں کو نقل کیے بغیر۔

## ظاہر مددگار

`ManifestBuilder` اب بھی Norito پے لوڈ کو جمع کرنے کا ایک اہم طریقہ ہے
پروگرامی:

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

بلڈر کو شامل کریں جہاں خدمات کو مکھی پر ظاہر کرنے کی ضرورت ہے۔
سی ایل آئی اب بھی عین مطابق پائپ لائنوں کے لئے تجویز کردہ راستہ ہے۔