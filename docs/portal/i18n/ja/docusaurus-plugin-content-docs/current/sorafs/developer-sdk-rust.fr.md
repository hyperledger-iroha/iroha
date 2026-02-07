---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-sdk-rust
タイトル: Extraits SDK Rust
Sidebar_label: Extraits Rust
説明: 消費者向けの Rust ミニモーの例、プルーフ ストリームとマニフェスト。
---

:::note ソースカノニク
:::

CLI および peuvent の ce dépôt alimentent le crotes de ce dépôt alimentent le embarqués dans des
オーケストレーターとサービス担当者。前衛的な情報
支援者と要求者。

## ヘルパープルーフストリーム

Réutilisez le のパーサー証明ストリームが存在し、メトリクスの精度を向上させます。
応答HTTP:

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

バージョンは `docs/examples/sorafs_rust_proof_stream.rs` で完了 (avec テスト) されました。
`ProofStreamSummary::to_json()` CLI、CE などのメトリクスの JSON をレンドします
アサーション CI の観察を容易にし、バックエンドの管理を容易にします。

## マルチソースフェッチのスコアリング

モジュール `sorafs_car::multi_fetch` は、フェッチ非同期ユーティリティのスケジューラーを公開します
CLI など。 `sorafs_car::multi_fetch::ScorePolicy` および passez-le を実装します。
`FetchOptions::score_policy` プロバイダーの注文を調整してください。ル・テスト・ユニテール
`multi_fetch::tests::score_policy_can_filter_providers` モンターコメントインポーズ者デス
人事担当者を優先します。

Autres ノブ alignés sur les flags CLI :

- `FetchOptions::per_chunk_retry_limit` は au フラグ `--retry-budget` に対応します
  CI qui contraignent volontairement les retries を実行します。
- Combinez `FetchOptions::global_parallel_limit` avec `--max-peers` pour plafonner le
  同時プロバイダーの数。
- `OrchestratorConfig::with_telemetry_region("region")` タグ・レ・メトリック
  `sorafs_orchestrator_*`、タンディスキュー `OrchestratorConfig::with_transport_policy`
  フラグ CLI `--transport-policy` を参照してください。 `TransportPolicy::SoranetPreferred` est
  CLI/SDK をデフォルトで使用できます。 `TransportPolicy::DirectOnly`を利用
  独自のダウングレード、または適合性に関する指令、および保存
  `SoranetStrict` 補助パイロット PQ のみの avec 承認が明示的です。
- 定義 `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` は PQ のみを強制的にアップロードします。ルヘルパー
  トランスポート/匿名性の自動政治政治を明示的にオーバーライドします。
- `SorafsGatewayFetchOptions::policy_override` を使用して、ティア デ ツールを使用します
  匿名で一時的にリクエストを転送してください。フルニル・ルン・デ・シャン
  劣化のブラウンアウトとそのレベルの需要の輪郭
  満足。
- Python バインディング (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) など
  JavaScript (`sorafsMultiFetchLocal`) スケジューラーを再利用します。定義
  `return_scoreboard=true` ヘルパーは、計算上の計算を実行します。
  大量の領収書を一時的に取得します。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` フラックス OTLP を登録します
  製品とバンドルの採用を検討してください。クライアントは自動化を導き出します
  `region:<telemetry_region>` (ou `chain:<chain_id>`) メタドンネの前兆が発生しました
  toujours une étiquette の説明。

## `iroha::Client` 経由で取得

SDK Rust はゲートウェイフェッチのヘルパーを開始します。フォーニセ アン マニフェスト プラス デ
プロバイダーの記述 (ストリーム トークンを含む) およびクライアント パイロットの放任
マルチソースを取得する:

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

`transport_policy` シュール `Some(TransportPolicy::SoranetStrict)` の定義
古典的なリレーの拒否者をアップロードします。`Some(TransportPolicy::DirectOnly)` です。
SoraNet はすべての輪郭を描きます。 Pointez `scoreboard.persist_path` 対ファイル
リリースのレパートリー、イベントの修正 `scoreboard.now_unix_secs` など
reNSEignez `scoreboard.metadata` キャプチャのコンテキストの取得 (フィクスチャのラベル、ケーブル)
Torii など) `cargo xtask sorafs-adoption-check` コンソメと JSON の決定
SDK は SF-6c に準拠したブロブと来歴を保持しています。
`Client::sorafs_fetch_via_gateway` 識別情報を強化します
マニフェスト、マニフェスト CID およびフラグ `gateway_manifest_provided` ja の注意事項
検査員ル `GatewayFetchConfig` 4 人、封筒を含む完全な方法でキャプチャします
マニフェスト署名は、満足のフォントの l'exigence de preuve SF-6c sans dupliker ces Champs à la main です。

## マニフェストのヘルパー

`ManifestBuilder` ペイロードのアセンブラの正式な保存 Norito
プログラム:

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

ビルダーの一部のサービスは、マニフェストの作成に必要な統合を行っています。
CLI はパイプラインを決定するための指示を返します。