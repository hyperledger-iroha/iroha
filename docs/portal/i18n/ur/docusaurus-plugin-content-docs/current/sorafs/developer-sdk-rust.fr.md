---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-SDK-RUST
عنوان: زنگ SDK اسنیپٹس
سائڈبار_لیبل: زنگ کے ٹکڑوں
تفصیل: پروف اسٹریمز اور ظاہر ہونے کے لئے کم سے کم زنگ کی مثالیں۔
---

::: نوٹ کینونیکل ماخذ
:::

اس ذخیرے سے آنے والے زنگ آلود خانے سی ایل آئی کو طاقت دیتے ہیں اور اس میں سرایت کرسکتے ہیں
آرکسٹریٹر یا ذاتی خدمات۔ روشنی کے نیچے نچوڑ
سب سے زیادہ درخواست کردہ مددگار۔

## مددگار پروف اسٹریم

موجودہ پروف اسٹریم پارسر کو ایک سے مجموعی میٹرکس کے لئے دوبارہ استعمال کریں
HTTP جواب:

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

مکمل ورژن (ٹیسٹوں کے ساتھ) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` ایک ہی میٹرکس JSON کو CLI کی طرح پیش کرتا ہے ، جو
طاقت کے مشاہدہ کرنے کے لئے بیک اینڈ یا سی آئی کے دعوے کو آسان بناتا ہے۔

## ملٹی سورس بازیافت اسکورنگ

ماڈیول `sorafs_car::multi_fetch` استعمال شدہ غیر متزلزل بازیافت شیڈولر کو بے نقاب کرتا ہے
بذریعہ CLI `sorafs_car::multi_fetch::ScorePolicy` کو نافذ کریں اور اسے گزریں
فراہم کنندگان کے آرڈر کو ایڈجسٹ کرنے کے لئے `FetchOptions::score_policy`۔ یونٹ ٹیسٹ
`multi_fetch::tests::score_policy_can_filter_providers` ظاہر کرتا ہے کہ کس طرح مسلط کیا جائے
ذاتی ترجیحات۔

دوسرے نوبس سی ایل آئی کے جھنڈوں کے ساتھ منسلک ہیں:

- `FetchOptions::per_chunk_retry_limit` پرچم `--retry-budget` کے لئے مساوی ہے
  سی آئی چلتی ہے جو رضاکارانہ طور پر دوبارہ کوشش کرتی ہے۔
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ جوڑیں
  مسابقتی فراہم کرنے والوں کی تعداد۔
- `OrchestratorConfig::with_telemetry_region("region")` ٹیگز میٹرکس
  `sorafs_orchestrator_*` ، جبکہ `OrchestratorConfig::with_transport_policy`
  CLI پرچم `--transport-policy` کی عکاسی کرتا ہے۔ `TransportPolicy::SoranetPreferred` ہے
  سی ایل آئی/ایس ڈی کے سائیڈ پر بطور ڈیفالٹ فراہم کیا گیا۔ `TransportPolicy::DirectOnly` استعمال کریں
  صرف ڈاون گریڈ کے دوران یا تعمیل ہدایت ، اور ریزرو پر
  `SoranetStrict` واضح منظوری کے ساتھ صرف PQ-Olly ڈرائیوروں کو۔
- سیٹ `sorafsgatewayfetchptions :: write_mode_hint =
  کچھ (Writemodehint :: اپلوڈپقونلی) P PQ- صرف اپ لوڈ کو مجبور کرنا ؛ مددگار
  خود بخود نقل و حمل/گمنامی کی پالیسیوں کو فروغ دیتا ہے جب تک کہ واضح اوور رائڈ نہ ہو۔
- ایک تہائی کو لاک کرنے کے لئے `SorafsGatewayFetchOptions::policy_override` استعمال کریں
  درخواست کے لئے نقل و حمل یا عارضی طور پر نام ظاہر نہ کرنا ؛ کھیتوں میں سے ایک فراہم کریں
  براؤن آؤٹ انحطاط کو نظرانداز کرتا ہے اور ناکام ہوجاتا ہے اگر درخواست کی گئی درجہ بندی نہیں ہوسکتی ہے
  مطمئن
- ازگر بائنڈنگز (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) اور
  جاوا اسکرپٹ (`sorafsMultiFetchLocal`) اسی شیڈولر کو دوبارہ استعمال کریں۔ وضاحت کریں
  `return_scoreboard=true` ان مددگاروں میں ایک ہی وقت میں حساب شدہ وزن کو بازیافت کرنے کے لئے
  وقت کے طور پر وقت۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP کے بہاؤ کو بچاتا ہے
  جس نے گود لینے کا بنڈل تیار کیا۔ اگر اسے چھوڑ دیا جاتا ہے تو ، مؤکل خود بخود بہہ جاتا ہے
  `region:<telemetry_region>` (یا `chain:<chain_id>`) تاکہ میٹا ڈیٹا لے جائے
  ہمیشہ ایک وضاحتی لیبل۔

## `iroha::Client` کے ذریعے بازیافت کریں

مورچا SDK میں گیٹ وے بازیافت کرنے والا مددگار شامل ہے۔ ایک منشور پلس فراہم کریں
فراہم کنندہ ڈسریکٹر (جس میں اسٹریم ٹوکن بھی شامل ہیں) اور کلائنٹ کو چلانے دیں
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
````transport_policy` کو `Some(TransportPolicy::SoranetStrict)` پر سیٹ کریں
اپ لوڈز کو کلاسک ریلے سے انکار کرنا چاہئے ، یا `Some(TransportPolicy::DirectOnly)` پر
جب سورانیٹ کو مکمل طور پر نظرانداز کیا جانا چاہئے۔ `scoreboard.persist_path` کو پوائنٹ پر
نمونے کی ڈائرکٹری جاری کریں ، اختیاری طور پر `scoreboard.now_unix_secs` اور سیٹ کریں
کیپچر سیاق و سباق (فکسچر لیبل ، ہدف کے ساتھ `scoreboard.metadata` درج کریں
Torii ، وغیرہ) تاکہ `cargo xtask sorafs-adoption-check` عین مطابق JSON کھائے
SF-6C کے ذریعہ متوقع بلاب کے ساتھ SDKs کے درمیان۔
`Client::sorafs_fetch_via_gateway` اب اس میٹا ڈیٹا کو شناخت کنندہ کے ساتھ افزودہ کرتا ہے
منشور ، مینی فیسٹ سی آئی ڈی اور پرچم `gateway_manifest_provided` میں ممکنہ انتظار کریں
فراہم کردہ `GatewayFetchConfig` کا معائنہ کرنا ، تاکہ لفافے سمیت کیچز
دستخط شدہ منشور SF-6C پروف کی ضرورت کو پورا کریں بغیر ان فیلڈز کو ہاتھ سے نقل کیا۔

## ظاہر مددگار

`ManifestBuilder` پے لوڈ کو جمع کرنے کا ایک طرح سے Norito کو جمع کرنے کا ایک طریقہ ہے
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

جہاں بھی خدمات کو مکھی پر ظاہر کرنے کی ضرورت ہے وہاں بلڈر کو مربوط کریں۔
سی ایل آئی ڈٹرمینسٹک پائپ لائنوں کے لئے تجویز کردہ راستہ بنی ہوئی ہے۔