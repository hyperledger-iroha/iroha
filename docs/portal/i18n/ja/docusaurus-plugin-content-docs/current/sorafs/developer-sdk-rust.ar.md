---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-sdk-rust
タイトル: Rust SDK
サイドバーラベル: Rust
説明: 錆びた錆びた金属。
---

:::note ノート
テストは `docs/source/sorafs/developer/sdk/rust.md` です。スフィンクス、スフィンクス、スフィンクス。
:::

Rust を開発し、CLI を開発しました。
最高のパフォーマンスを見せてください。

## مساعد تدفق الأدلة

HTTP:

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

テストは `docs/examples/sorafs_rust_proof_stream.rs` です。
تنتج `ProofStreamSummary::to_json()` نفس JSON للمقاييس الذي يصدره CLI، ما يسهل تغذية
CI です。

## تقييم الجلب متعدد المصادر

`sorafs_car::multi_fetch` は、CLI を実行します。
`sorafs_car::multi_fetch::ScorePolicy` ومرّره عبر `FetchOptions::score_policy`
最高です。ログイン して翻訳を追加する
`multi_fetch::tests::score_policy_can_filter_providers` كيفية فرض تفضيلات مخصصة.

CLI を使用してください:

- `FetchOptions::per_chunk_retry_limit` يطابق علم `--retry-budget` لتشغيلات CI التي
  さいごに。
- اجمع بين `FetchOptions::global_parallel_limit` و`--max-peers` لتقييد عدد المزوّدين
  ああ。
- يقوم `OrchestratorConfig::with_telemetry_region("region")` بوسم مقاييس
  `sorafs_orchestrator_*`، بينما يعكس `OrchestratorConfig::with_transport_policy`
  CLI `--transport-policy`。評価 `TransportPolicy::SoranetPreferred` كافتراضي عبر
  CLI/SDK の説明`TransportPolicy::DirectOnly` ダウングレードする
  `SoranetStrict` は PQ のみのユーザーです。
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` PQ のみ認証済み
  ログインしてください。
- استخدم `SorafsGatewayFetchOptions::policy_override` لتثبيت طبقة نقل أو إخفاء هوية
  重要な問題ブラウンアウト ويفشل عندما يتعذر تلبية الطبقة
  ああ。
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) およびJavaScript
  (`sorafsMultiFetchLocal`) فس المُجدول، لذا اضبط `return_scoreboard=true` في تلك المساعدات
  チャンクを確認してください。
- يسجل `SorafsGatewayScoreboardOptions::telemetry_source_label` تدفق OTLP الذيأنتج
  すごいね。 `region:<telemetry_region>` (`chain:<chain_id>`) の評価
  حتى تحمل الميتاداتا دائمًا وسمًا وصفيًا。

## جلبعبر `iroha::Client`

Rust SDK をインストールするعرض المزيد
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

ضبط `transport_policy` إلى `Some(TransportPolicy::SoranetStrict)` عندما يجب أن ترفض
`Some(TransportPolicy::DirectOnly)` عندما يجب تجاوز عرض المزيد
SoraNet です。 وجّه `scoreboard.persist_path` إلى دليل آرتيفاكتات الإصدار، واضبط
`scoreboard.now_unix_secs` ، واملأ `scoreboard.metadata` بسياق التقاط
(フィクスチャ Torii ) `cargo xtask sorafs-adoption-check` JSON
SDK は SF-6c に対応しています。
تقوم `Client::sorafs_fetch_via_gateway` الآن بإثراء تلك الميتاداتا بمعرف المانيفست،
CID は `gateway_manifest_provided` です。
`GatewayFetchConfig` المقدم، بحيث تلبي التقاطات التي تتضمن ظرف مانيفست موقع متطلبات
SF-6c を開発しました。

## और देखें

`ManifestBuilder` 番号 Norito 番号:

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

ضمّن المنشئ حيثما احتاجت الخدمات إلى توليد المانيفستات ديناميكيًا؛ CLI を使用してください。
ありがとうございます。