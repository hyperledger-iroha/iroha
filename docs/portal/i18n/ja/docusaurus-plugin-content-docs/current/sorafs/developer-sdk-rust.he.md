---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5523126b97c4e64d2f63830d496d36fd00df696070e3a3126ddb961def6f5157
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: developer-sdk-rust
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/developer/sdk/rust.md` を反映しています。レガシーの Sphinx セットが退役するまで両方を同期してください。
:::

このリポジトリの Rust クレートは CLI を支え、カスタムオーケストレーターや
サービスに埋め込めます。以下のスニペットでは、開発者が求める代表的な helper を
紹介します。

## proof stream helper

既存の proof stream パーサーを再利用して HTTP レスポンスからメトリクスを集計します:

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

完全版（テスト付き）は `docs/examples/sorafs_rust_proof_stream.rs` にあります。
`ProofStreamSummary::to_json()` は CLI と同じメトリクス JSON を出力するため、
観測基盤や CI アサーションに容易に取り込めます。

## マルチソース fetch のスコアリング

`sorafs_car::multi_fetch` モジュールは CLI が使う非同期 fetch スケジューラを
公開します。`sorafs_car::multi_fetch::ScorePolicy` を実装し、
`FetchOptions::score_policy` で渡すことでプロバイダーの順序を調整できます。
ユニットテスト `multi_fetch::tests::score_policy_can_filter_providers` が
カスタム嗜好の適用方法を示します。

他のノブも CLI フラグに対応します:

- `FetchOptions::per_chunk_retry_limit` は、リトライを意図的に制限する CI 実行で
  `--retry-budget` フラグと一致します。
- `FetchOptions::global_parallel_limit` と `--max-peers` を組み合わせ、
  同時プロバイダー数を上限設定します。
- `OrchestratorConfig::with_telemetry_region("region")` は `sorafs_orchestrator_*`
  メトリクスをタグ付けし、`OrchestratorConfig::with_transport_policy` は
  CLI の `--transport-policy` を反映します。`TransportPolicy::SoranetPreferred`
  は CLI/SDK の既定値として提供されます。`TransportPolicy::DirectOnly` は
  downgrade を段階的に行う場合か、コンプライアンス指示に従う場合のみ使用し、
  `SoranetStrict` はガバナンス承認のある PQ-only パイロット向けに予約してください。
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` を設定すると PQ-only アップロードを強制できます。
  helper は明示的に override されない限り transport/anonymity ポリシーを自動的に強化します。
- `SorafsGatewayFetchOptions::policy_override` を使うと、単一リクエストに対して
  一時的な transport/anonymity tier を固定できます。どちらかのフィールドを渡すと
  brownout デモーションがスキップされ、要求 tier を満たせない場合は失敗します。
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) と JavaScript
  (`sorafsMultiFetchLocal`) のバインディングは同じ scheduler を再利用するため、
  `return_scoreboard=true` を設定して計算済みの重みを chunk receipt と一緒に取得します。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` は、adoption bundle を生成した
  OTLP ストリームを記録します。省略時は `region:<telemetry_region>`（または
  `chain:<chain_id>`）を自動導出し、メタデータに説明的なラベルを必ず付与します。

## `iroha::Client` 経由の fetch

Rust SDK には gateway fetch helper が含まれています。manifest と provider
descriptor（stream token を含む）を渡し、クライアントにマルチソース fetch を
実行させます:

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

アップロードが従来のリレーを拒否すべき場合は `transport_policy` を
`Some(TransportPolicy::SoranetStrict)` に、SoraNet を完全に迂回する必要がある場合は
`Some(TransportPolicy::DirectOnly)` に設定します。`scoreboard.persist_path` を
リリースアーティファクトディレクトリに向け、必要に応じて
`scoreboard.now_unix_secs` を固定し、`scoreboard.metadata` にキャプチャ文脈
（fixture ラベル、Torii ターゲットなど）を入れておくと、
`cargo xtask sorafs-adoption-check` が SDK 間で決定的 JSON を消費し、
SF-6c が求める provenance blob を維持できます。
`Client::sorafs_fetch_via_gateway` は供給された `GatewayFetchConfig` を検査して、
manifest 識別子、任意の manifest CID 期待値、`gateway_manifest_provided` フラグを
メタデータに追加するため、署名済み manifest エンベロープを含むキャプチャでも
これらのフィールドを手動で重複させずに SF-6c の証跡要件を満たせます。

## manifest helper

`ManifestBuilder` は Norito payload をプログラムで組み立てる正規の手段です:

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

サービスがオンザフライで manifest を生成する必要がある場合は builder を組み込み、
決定的なパイプラインには引き続き CLI を推奨します。
