---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
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

اس ذخیرے میں زنگ آلود خانے سی ایل آئی کو کھانا کھاتے ہیں اور رواج میں بنایا جاسکتا ہے
آرکیسٹریٹرز یا خدمات۔ نیچے کے ٹکڑوں میں مددگاروں کو نمایاں کیا جاتا ہے جو اکثر اکثر ہوتے ہیں
ڈویلپرز کی ضرورت ہے۔

## پروف اسٹریم کے لئے مددگار

موجودہ پروف اسٹریم پارسر کو مجموعی میٹرکس سے دوبارہ استعمال کریں
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
`ProofStreamSummary::to_json()` اسی میٹرکس JSON کو CLI کی طرح پیش کرتا ہے ، جس سے یہ آسان ہوجاتا ہے
مشاہدہ کرنے والے پسدید یا CI کے دعووں پر ڈیٹا جمع کروانا۔

## ملٹی سورس بازیافت اسکورنگ

ماڈیول `sorafs_car::multi_fetch` استعمال شدہ غیر متزلزل بازیافت شیڈولر کو بے نقاب کرتا ہے
سی ایل آئی۔ `sorafs_car::multi_fetch::ScorePolicy` کو نافذ کریں اور اسے گزریں
`FetchOptions::score_policy` فراہم کنندگان کے آرڈر کو تشکیل دینے کے لئے۔ یونٹ ٹیسٹ
`multi_fetch::tests::score_policy_can_filter_providers` میں داخل ہونے کا طریقہ دکھاتا ہے
کسٹم ترجیحات۔

دوسرے نوبس سی ایل آئی کے جھنڈوں سے مطابقت رکھتے ہیں:

- `FetchOptions::per_chunk_retry_limit` CI کے لئے `--retry-budget` پرچم سے مطابقت رکھتا ہے
  لانچ کرتا ہے جو جان بوجھ کر دوبارہ کوشش کرتا ہے۔
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ جوڑیں
  بیک وقت فراہم کرنے والوں کی تعداد۔
- `OrchestratorConfig::with_telemetry_region("region")` میٹرکس کو نشانہ بناتا ہے
  `sorafs_orchestrator_*` ، اور `OrchestratorConfig::with_transport_policy` آئینے
  CLI پرچم `--transport-policy`۔ `TransportPolicy::SoranetPreferred` پہلے سے طے شدہ ہے
  CLI/SDK سطحوں میں ؛ صرف اسٹیجنگ کے لئے `TransportPolicy::DirectOnly` استعمال کریں
  ڈاون گریڈ یا تعمیل ہدایت کے حصے کے طور پر ، اور `SoranetStrict` کے لئے ریزرو
  واضح منظوری کے ساتھ صرف پی کیو صرف پائلٹ۔
- سیٹ `sorafsgatewayfetchptions :: write_mode_hint =
  کچھ (Writemodehint :: اپلوڈپقونلی) P PQ- صرف اپ لوڈ کو مجبور کرنا ؛ مددگار خود بخود
  اگر وہ واضح طور پر زیر نہیں ہیں تو نقل و حمل/گمنامی کی پالیسیاں میں اضافہ کریں گے۔
- عارضی طور پر محفوظ کرنے کے لئے `SorafsGatewayFetchOptions::policy_override` استعمال کریں
  ایک درخواست کے لئے ٹرانسپورٹ یا گمنامی کا درجہ ؛ کسی بھی فیلڈ کو پیش کرنا غائب ہے
  براؤن آؤٹ ڈیموشن اور کریش ہوتا ہے اگر درخواست کردہ درجے مطمئن نہیں ہوسکتے ہیں۔
- ازگر (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) اور جاوا اسکرپٹ
  (`sorafsMultiFetchLocal`) بائنڈنگ ایک ہی شیڈولر کا استعمال کرتے ہیں ، لہذا سیٹ کریں
  `return_scoreboard=true` ان مددگاروں میں حساب شدہ وزن حاصل کرنے کے لئے اگلے
  حص rech ے کی رسیدیں۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` ریکارڈز OTLP اسٹریم ،
  گود لینے کا بنڈل تشکیل دیا۔ اگر اس کی وضاحت نہیں کی گئی ہے تو ، کلائنٹ خود بخود دکھاتا ہے
  `region:<telemetry_region>` (یا `chain:<chain_id>`) تاکہ میٹا ڈیٹا ہمیشہ چلتا ہو
  وضاحتی لیبل

## `iroha::Client` کے ذریعے بازیافت کریں

مورچا ایس ڈی کے میں گیٹ وے بازیافت کے لئے ایک مددگار شامل ہے۔ مینی فیسٹ اور فراہم کنندہ کے بیان کرنے والے پاس کریں
(بشمول اسٹریم ٹوکن) اور کلائنٹ کو ملٹی سورس بازیافت کا انتظام کرنے دیں:

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
کلاسک ریلے کو مسترد کرنا چاہئے ، یا `Some(TransportPolicy::DirectOnly)` میں جب
سورانیٹ کو مکمل طور پر نظرانداز کرنے کی ضرورت ہے۔ `scoreboard.persist_path` کو ڈائرکٹری کی طرف اشارہ کریں
نمونے جاری کریں ، اختیاری طور پر `scoreboard.now_unix_secs` کو درست کریں اور پُر کریں
`scoreboard.metadata` کیپچر سیاق و سباق کے ذریعہ (لیبل فکسچر ، ٹارگٹ Torii ، وغیرہ) تو ، تو ،
`cargo xtask sorafs-adoption-check` کرنے کے لئے SDKs کے مابین جینیاتی JSON استعمال کریں
بلاب پروویژن کے ساتھ ، جو SF-6C کی توقع کرتا ہے۔
`Client::sorafs_fetch_via_gateway` اب اس میٹا ڈیٹا کو منشور شناخت کنندہ کے ساتھ پورا کرتا ہے ،
تجزیہ کے ذریعہ مینی فیسٹ سی آئی ڈی اور پرچم `gateway_manifest_provided` کی اختیاری توقع
پاس کیا `GatewayFetchConfig` تاکہ دستخط شدہ لفافے کے ساتھ گرفتاری سے مطمئن ہوجائے
SF-6C شواہد کی ضرورت کو دستی طور پر ان شعبوں کی نقل تیار کیے بغیر۔

## ظاہر کے لئے مددگار

`ManifestBuilder` پروگرام کے مطابق Norito پے لوڈ کو جمع کرنے کا بنیادی طریقہ ہے:

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

ایمبیڈ بلڈر جہاں بھی خدمات کو مکھی پر ظاہر کرنے کی ضرورت ہے۔ سی ایل آئی
تعی .ن پائپ لائنوں کے لئے تجویز کردہ راستہ رہتا ہے۔