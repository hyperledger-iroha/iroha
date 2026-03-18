---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ڈویلپر-SDK-RUST
عنوان: زنگ SDK اسنیپٹس
سائڈبار_لیبل: زنگ کے ٹکڑوں
تفصیل: ڈائرکٹری اسٹریمز اور ظاہر ہونے کے لئے کم سے کم مورچا کی مثالیں۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/developer/sdk/rust.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ اس وقت تک دونوں کاپیاں ہم آہنگی میں رکھیں جب تک کہ پرانا اسفنکس کلسٹر ریٹائر نہ ہوجائے۔
:::

اس ذخیرے میں زنگ آلود پیکیجز سی ایل آئی میں کھانا کھاتے ہیں اور کسٹم فارمیٹرز یا خدمات کے اندر سرایت کرسکتے ہیں۔
مندرجہ ذیل اقتباسات زیادہ تر ڈویلپرز کی ضرورت کی مدد کو اجاگر کرتے ہیں۔

## ثبوت بہاؤ اسسٹنٹ

HTTP جواب سے میٹرکس جمع کرنے کے لئے اپنے موجودہ شواہد کے بہاؤ پارسر کا دوبارہ استعمال کریں:

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

مکمل ورژن (ٹیسٹوں کے ساتھ) `docs/examples/sorafs_rust_proof_stream.rs` پر دستیاب ہے۔
`ProofStreamSummary::to_json()` CLI کی طرح ایک ہی میٹرکس JSON تیار کرتا ہے ، جس سے کھانا کھلانا آسان ہوجاتا ہے
مانیٹرنگ پلیٹ فارم یا سی آئی فرضی تصورات۔

## کثیر سورس بازیافت کا اندازہ کریں

ماڈیول `sorafs_car::multi_fetch` CLI میں استعمال ہونے والے غیر متزلزل بازیافت شیڈیولر کو دکھاتا ہے۔
`sorafs_car::multi_fetch::ScorePolicy` پر عمل کریں اور اسے `FetchOptions::score_policy` کے ذریعے منتقل کریں
فراہم کرنے والوں کے آرڈر کو ایڈجسٹ کرنے کے لئے۔ یونٹ کی جانچ کا مظاہرہ کرتا ہے
`multi_fetch::tests::score_policy_can_filter_providers` اپنی مرضی کے مطابق ترجیحات کو کس طرح مجبور کریں۔

دوسرے عناصر جو CLI جھنڈوں کی عکاسی کرتے ہیں:

- `FetchOptions::per_chunk_retry_limit` CI رنز کے لئے `--retry-budget` پرچم سے میل کھاتا ہے
  جان بوجھ کر کوششوں کی تعداد کو محدود کریں۔
- فراہم کنندگان کی تعداد کو محدود کرنے کے لئے `FetchOptions::global_parallel_limit` اور `--max-peers` کو یکجا کریں
  ہم وقت ساز
- `OrchestratorConfig::with_telemetry_region("region")` ٹیگز میٹرکس
  `sorafs_orchestrator_*` ، جبکہ `OrchestratorConfig::with_transport_policy` کی عکاسی ہوتی ہے
  CLI پرچم `--transport-policy`۔ `TransportPolicy::SoranetPreferred` جہاز بطور ڈیفالٹ کے ذریعے
  CLI/SDK کھالیں ؛ جب صرف ڈاون گریڈ یا فالو کی جانچ کرتے ہو تو `TransportPolicy::DirectOnly` استعمال کریں
  تعمیل ہدایت ، اور واضح منظوری کے ساتھ صرف PQ- صرف پائلٹوں کے لئے `SoranetStrict`۔
- سیٹ `sorafsgatewayfetchptions :: write_mode_hint =
  کچھ (Writemodehint :: اپلوڈپقونلی) `صرف PQ-Olly اپ لوڈ پر مجبور کرنا ؛ اسسٹنٹ پالیسیوں کو فروغ دے گا
  جب تک واضح طور پر اوورڈ نہ ہو تب تک خود بخود منتقلی/گمنام کریں۔
- ٹرانسپورٹ یا گمنام پرت انسٹال کرنے کے لئے `SorafsGatewayFetchOptions::policy_override` استعمال کریں
  ایک ہی آرڈر کے لئے عارضی ؛ کسی بھی فیلڈ سے گزرنا جو براؤن آؤٹ میں کمی سے تجاوز کرتا ہے ناکام ہوجاتا ہے جب پرت مطمئن نہیں ہوسکتی ہے
  ضروری ہے۔
- ازگر (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) اور جاوا اسکرپٹ بائنڈنگ استعمال کرتا ہے
  (`sorafsMultiFetchLocal`) same scheduler, so set `return_scoreboard=true` in that helper
  حص can ہ کی رسیدوں کے ساتھ حساب شدہ وزن کو بازیافت کرنے کے لئے۔
-`SorafsGatewayScoreboardOptions::telemetry_source_label` نے تیار کردہ OTLP اسٹریم کو ریکارڈ کیا
  گھاس کا ایک بنڈل۔ غلطی پر ، کلائنٹ خود بخود `region:<telemetry_region>` (یا `chain:<chain_id>`) کا پتہ لگاتا ہے۔
  تو میٹا ڈیٹا ہمیشہ وضاحتی ٹیگ اٹھاتا ہے۔

## `iroha::Client` کے ذریعے بازیافت کریں

مورچا SDK میں گیٹ وے بازیافت کرنے والا مددگار شامل ہے۔ فراہم کنندگان کی تفصیل کے ساتھ ظاہر کے ذریعے سکرول کریں
(بشمول براڈکاسٹ کوڈز) اور کلائنٹ کو ملٹی سورس بازیافت کا انتظام کرنے دیں:

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

`transport_policy` کو `Some(TransportPolicy::SoranetStrict)` پر سیٹ کریں جب اسے مسترد کرنا چاہئے
کلاسیکی ریلے ، یا `Some(TransportPolicy::DirectOnly)` جب ان کو نظرانداز کیا جانا چاہئے
سورانیٹ مکمل طور پر۔ `scoreboard.persist_path` کو ریلیز نمونے کی ڈائرکٹری ، اور سیٹ پر پوائنٹ کریں
اختیاری طور پر `scoreboard.now_unix_secs` ، اور `scoreboard.metadata` کو کیپچر سیاق و سباق سے بھریں
.
SF-6C کے ذریعہ متوقع ماخذ فائل کے ساتھ SDKs کے ذریعے ناگزیر۔
`Client::sorafs_fetch_via_gateway` اب مینی فیسٹ ID کے ساتھ میٹا ڈیٹا کو افزودہ کرتا ہے ،
سی آئی ڈی کی توقع اختیاری مینی فیسٹ میں کی جاتی ہے ، اور `gateway_manifest_provided` کو امتحان کے ذریعے جھنڈا لگایا جاتا ہے
`GatewayFetchConfig` فراہم کی گئی ہے ، تاکہ اس پک اپس جس میں دستخط شدہ مینی فیسٹ لفافہ شامل ہوتا ہے اس کی ضروریات کو پورا کرتے ہیں
SF-6C دستی دستی طور پر ان کھیتوں کو نقل کیے بغیر۔

## ظاہر امداد

`ManifestBuilder` Norito پے لوڈ کو پروگرام کے مطابق مرتب کرنے کے لئے منظور شدہ طریقہ ہے:```rust
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

تعمیر کنندہ کو شامل کریں جہاں بھی خدمات کو متحرک طور پر ظاہر کرنے کی ضرورت ہے۔ سی ایل آئی راہ باقی ہے
لازمی لائنوں کے لئے تجویز کردہ۔