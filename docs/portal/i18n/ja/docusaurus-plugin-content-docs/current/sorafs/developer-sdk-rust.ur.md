---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-sdk-rust
タイトル: Rust SDK スニペット
Sidebar_label: Rust スニペット
説明: 証明ストリームとマニフェストは最小限の Rust サンプルを消費します
---

:::note メモ
:::

リポジトリ Rust クレート CLI 電源 カスタム オーケストレーター サービス 埋め込み ہیں۔
فیچے والے スニペット ان ヘルパー کو ハイライト کرتے ہیں جن کی طلب سب سے زیادہ ہوتی ہے۔

## プルーフストリームヘルパー

HTTP 応答とメトリクスの集約と既存のプルーフ ストリーム パーサーの再利用:

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

フルバージョン (テスト) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` メトリクス JSON レンダリングの実行 CLI の実行
可観測性バックエンド CI アサーション フィード آسان ہوتا ہے۔

## マルチソースフェッチスコアリング

`sorafs_car::multi_fetch` モジュールと非同期フェッチ スケジューラを公開する CLI を公開する
`sorafs_car::multi_fetch::ScorePolicy` 実装する `FetchOptions::score_policy` パスする
プロバイダーの注文曲 ہو سکے۔単体テスト `multi_fetch::tests::score_policy_can_filter_providers`
カスタム設定を強制する

その他のノブ CLI フラグとミラーリング:

- `FetchOptions::per_chunk_retry_limit` CI が実行されます。 `--retry-budget` フラグが一致します。
  再試行回数 制限回数 制限回数
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ 結合 同時実行
  プロバイダー キャップ ہو۔
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` メトリクスのタグ付け
  `OrchestratorConfig::with_transport_policy` CLI کے `--transport-policy` フラグ ミラー ہے۔
  `TransportPolicy::SoranetPreferred` CLI/SDK サーフェスはデフォルトで出荷されます`TransportPolicy::DirectOnly`
  ダウングレード ステージ コンプライアンス指令に従ってください `SoranetStrict` کو
  PQ のみのパイロット 明示的な承認 予備の承認
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` を設定すると、PQ のみのアップロードが強制されます。ヘルパー輸送/匿名性
  ポリシーを促進し、承認を上書きします。
- `SorafsGatewayFetchOptions::policy_override` 緊急輸送リクエスト 緊急輸送
  یا 匿名層ピン ہو جائے؛フィールド دینے ブラウンアウト降格 スキップ ہوتا ہے اور اگر
  リクエストされた層は満足しました 成功しました 失敗しました
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`)、JavaScript (`sorafsMultiFetchLocal`)
  バインディング、スケジューラ、再利用、バインディング、ヘルパー、`return_scoreboard=true` セット、再利用、バインディング
  計算された重みチャンクのレシート
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP ストリーム記録採用バンドル
  और देखें اگر 省略 ہو تو client خودکار طور پر `region:<telemetry_region>` (یا `chain:<chain_id>`) کرتا ہے を導出
  メタデータ میں ہمیشہ 説明ラベル رہے۔

## `iroha::Client` 経由で取得

Rust SDK ゲートウェイフェッチヘルパーの機能マニフェスト プロバイダー記述子 (ストリーム トークン)
クライアントとマルチソースフェッチドライブの接続:

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

古典的なリレーのアップロード、`transport_policy`、`Some(TransportPolicy::SoranetStrict)`
پر セット کریں، یا جب SoraNet کو مکمل bypass کرنا ہو تو `Some(TransportPolicy::DirectOnly)` پر۔
`scoreboard.persist_path` アーティファクト ディレクトリをリリースします。 `scoreboard.now_unix_secs` 修正します。
`scoreboard.metadata` キャプチャ コンテキスト (フィクスチャ ラベル、Torii ターゲット) を取得します。
`cargo xtask sorafs-adoption-check` SDK は、決定論的な JSON を使用して、SF-6c の出所 BLOB を消費します。
を含む`Client::sorafs_fetch_via_gateway` メタデータ、マニフェスト識別子、オプションのマニフェスト CID の期待値
提供された `GatewayFetchConfig` 署名付きマニフェスト封筒を介して、`gateway_manifest_provided` フラグを拡張します。
SF-6c の証拠要件をキャプチャします。 フィールドの重複を確認します。

## マニフェストヘルパー

`ManifestBuilder` ペイロード Norito ペイロードは、プログラムによって正規のペイロードをアセンブルします。

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

サービス オンザフライマニフェスト生成 ビルダー埋め込み決定的パイプライン ٩ے لیے CLI ابھی بھی 推奨パス ہے۔