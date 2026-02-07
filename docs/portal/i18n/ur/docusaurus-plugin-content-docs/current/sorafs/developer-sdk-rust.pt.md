---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
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
یہ صفحہ `docs/source/sorafs/developer/sdk/rust.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

اس ذخیرے میں زنگ آلود خانے سی ایل آئی کو طاقت دیتے ہیں اور اس میں سرایت کی جاسکتی ہیں
آرکیسٹریٹرز یا اپنی مرضی کے مطابق خدمات۔ نیچے دیئے گئے ٹکڑوں نے مددگاروں کو اجاگر کیا
زیادہ تر ڈویلپر کی درخواستوں میں ظاہر ہوتے ہیں۔

## پروف اسٹریم مددگار

موجودہ پروف اسٹریم پارسر کو HTTP کے جواب سے مجموعی میٹرکس کے لئے دوبارہ استعمال کریں:

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
`ProofStreamSummary::to_json()` CLI میٹرکس کی طرح JSON کو پیش کرتا ہے ، جس سے یہ آسان ہوجاتا ہے
پاور مشاہدہ کرنے کی پشت پناہی یا سی آئی کے دعوے۔

## ملٹی سورس بازیافت اسکورنگ

`sorafs_car::multi_fetch` ماڈیول استعمال شدہ غیر متزلزل بازیافت شیڈولر کو بے نقاب کرتا ہے
سی ایل آئی۔ `sorafs_car::multi_fetch::ScorePolicy` کو نافذ کریں اور پاس کریں
فراہم کنندگان کے آرڈر کو ایڈجسٹ کرنے کے لئے `FetchOptions::score_policy`۔ یونٹ ٹیسٹ
`multi_fetch::tests::score_policy_can_filter_providers` میں ترجیحات مسلط کرنے کا طریقہ دکھاتا ہے
اپنی مرضی کے مطابق.

دوسرے نوبس آئینے سی ایل آئی جھنڈے:

- `FetchOptions::per_chunk_retry_limit` رنز کے لئے `--retry-budget` پرچم سے مطابقت رکھتا ہے
  CIS جو مقصد پر کوششوں کو محدود کرتا ہے۔
- نمبر کو محدود کرنے کے لئے `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ جوڑیں
  مسابقتی فراہم کرنے والوں سے۔
- `OrchestratorConfig::with_telemetry_region("region")` میٹرکس کو نشان زد کرتا ہے
  `sorafs_orchestrator_*` ، جبکہ `OrchestratorConfig::with_transport_policy` آئینہ
  CLI پرچم `--transport-policy`۔ `TransportPolicy::SoranetPreferred` اور پہلے سے طے شدہ
  CLI/SDK سطحیں ؛ جب صرف ڈاون گریڈ تیار کرتے ہو تو `TransportPolicy::DirectOnly` استعمال کریں
  یا تعمیل ہدایت کی پیروی کریں ، اور صرف PQ- صرف پائلٹوں کے لئے `SoranetStrict` کو محفوظ کریں
  واضح منظوری کے ساتھ۔
- سیٹ `sorafsgatewayfetchptions :: write_mode_hint =
  کچھ (Writemodehint :: اپلوڈپقونلی) P PQ- صرف اپ لوڈ کو مجبور کرنا ؛ مددگار فروغ دیتا ہے
  خود بخود ٹرانسپورٹ/گمنامی کی پالیسیاں تبدیل کریں جب تک کہ وہ واضح طور پر نہ ہوں
  اوور رائٹ۔
- عارضی درجے کو ترتیب دینے کے لئے `SorafsGatewayFetchOptions::policy_override` کا استعمال کریں
  کسی ایک درخواست کے لئے ٹرانسپورٹ یا گمنامی ؛ اسکیپ فیلڈز میں سے کسی کو بھی فراہم کریں
  براؤن آؤٹ ڈیموشن ناکام ہوجاتا ہے جب درخواست کردہ درجے کو پورا نہیں کیا جاسکتا۔
- ازگر بائنڈنگز (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) اور
  جاوا اسکرپٹ (`sorafsMultiFetchLocal`) اسی شیڈولر کو دوبارہ استعمال کریں ، لہذا وضاحت کریں
  `return_scoreboard=true` ان مددگاروں میں حساب شدہ وزن کے ساتھ ساتھ بازیافت کرنے کے لئے
  حص rech ے کی رسیدیں۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP اسٹریم کو ریکارڈ کرتا ہے
  گود لینے کا بنڈل تیار کیا۔ جب اسے چھوڑ دیا جاتا ہے تو ، مؤکل خود بخود اخذ کرتا ہے
  `region:<telemetry_region>` (یا `chain:<chain_id>`) تاکہ ہمیشہ میٹا ڈیٹا
  ایک وضاحتی لیبل اپ لوڈ کریں۔

## `iroha::Client` کے ذریعے بازیافت کریں

مورچا SDK میں بازیافت گیٹ وے مددگار شامل ہے۔ ایک مینی فیسٹ پلس ڈسریکٹر فراہم کرتا ہے
فراہم کنندگان (بشمول اسٹریم ٹوکن) اور مؤکل کو ملٹی سورس بازیافت کرنے دیں:

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
```اپ لوڈ کرتے وقت `transport_policy` کو `Some(TransportPolicy::SoranetStrict)` پر سیٹ کریں
کلاسک ریلے سے انکار کرنے کی ضرورت ہے ، یا `Some(TransportPolicy::DirectOnly)` جب
سورانیٹ کو مکمل طور پر نظرانداز کرنے کی ضرورت ہے۔ `scoreboard.persist_path` کو پوائنٹ پر
نمونے کی ڈائرکٹری جاری کریں ، اختیاری طور پر `scoreboard.now_unix_secs` سیٹ کریں اور پُر کریں
`scoreboard.metadata` کیپچر سیاق و سباق کے ساتھ (فکسچر لیبل ، ٹارگٹ Torii ، وغیرہ)
تاکہ `cargo xtask sorafs-adoption-check` SDKs میں عین مطابق JSON کھائے
SF-6C کی توقع کے مطابق پروویژن بلاب کے ساتھ۔
`Client::sorafs_fetch_via_gateway` اب اس میٹا ڈیٹا کو شناخت کنندہ کے ساتھ بڑھا دیتا ہے
ظاہر ، اختیاری ظاہر سی آئی ڈی کی توقع ، اور `gateway_manifest_provided` پرچم
فراہم کردہ `GatewayFetchConfig` کا معائنہ کرنا ، تاکہ اس کی گرفت میں
مینی فیسٹ پر دستخط شدہ لفافے SF-6C ثبوت کی ضرورت کو پورا کریں بغیر ان کی نقل تیار کیے
کھیت دستی طور پر۔

## ظاہر مددگار

`ManifestBuilder` Norito ایکس پے لوڈ کو جمع کرنے کا ایک اہم طریقہ ہے
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

جب بھی خدمات کو حقیقی وقت میں ظاہر کرنے کی ضرورت ہوتی ہے تو بلڈر کو شامل کریں۔
سی ایل آئی ڈٹرمینسٹک پائپ لائنوں کے لئے تجویز کردہ راستہ بنی ہوئی ہے۔