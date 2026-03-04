---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-sdk-rust
title: SDK Rustのスニペット
Sidebar_label: Rust のスニペット
説明: 消費者証明ストリームとマニフェストに関する Rust ミニモスの例。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/developer/sdk/rust.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

OS クレート Rust ネスト リポジトリの管理、CLI およびポデム サーバーの管理
オルケストラドールやサービスのカスタマイズ。 OS スニペット abaixo destacam os ヘルパー クエリ
あなたの要求はデセンボルベドーレスにあります。

## 証明ストリームのヘルパー

HTTP レポートの集計メトリクスに存在する証明ストリームのパーサーを再利用します。

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

A versao completa (com testes) は `docs/examples/sorafs_rust_proof_stream.rs` で生き続けます。
`ProofStreamSummary::to_json()` CLI を実行するための JSON メトリクスのレンダリング、容易化
CI のアサーションを観察するための食品バックエンド。

## マルチソースフェッチのスコアリング

O modulo `sorafs_car::multi_fetch` expoe o scheduler de fetch assincrono usado pelo
CLI。 `sorafs_car::multi_fetch::ScorePolicy` を経由して実装します
`FetchOptions::score_policy` 裁判官の権限を証明します。ああ、テスト・ユニタリオ
`multi_fetch::tests::score_policy_can_filter_providers` 最も重要な優遇措置
カスタマイズ。

Outros ノブ espelham flags は CLI を実行します。

- `FetchOptions::per_chunk_retry_limit` は、ao フラグに対応します。 `--retry-budget` パラ実行
  提案された CI の制限を解除します。
- `FetchOptions::global_parallel_limit` com `--max-peers` パラメーターの数値を結合します。
  デ・プロヴェドール・コンコルレンテス。
- メトリカとしての `OrchestratorConfig::with_telemetry_region("region")` マルカ
  `sorafs_orchestrator_*`、エンクアント `OrchestratorConfig::with_transport_policy` エスペルハ
  o フラグ CLI `--transport-policy`。 `TransportPolicy::SoranetPreferred` e o デフォルトの nas
  表面機能 CLI/SDK。 `TransportPolicy::DirectOnly` を使用してダウングレードの準備をします
  コンプライアンスの遵守、電子予約 `SoranetStrict` パラ パイロット PQ のみ
  com aprovacao明示的。
- `SorafsGatewayFetchOptions::write_mode_hint = を定義します。
  Some(WriteModeHint::UploadPqOnly)` パラは、PQ のみをアップロードします。 o 促進するヘルパー
  交通機関の政治としての自動管理/匿名性による明示的な管理
  ソブレスクリタス。
- 一時的な固定層として `SorafsGatewayFetchOptions::policy_override` を使用します
  ユニコのリクエストに応じて輸送します。フォーネセル クアルケル ウム ドス カンポス プーラ
  ブラウンアウト降格とファルハ・クアンド、ティア・ソリシタド・ナオ・ポデ・サー・テンディド。
- OS バインディング Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`)
  JavaScript (`sorafsMultiFetchLocal`) 再利用または管理スケジューラ、定義済み
  `return_scoreboard=true` nesses helpers para recuperar os pesos calculados junto com
  塊の領収書。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` レジストラ ストリーム OTLP キュー
  製品採用バンドル。自動的にクライアントを削除します
  `region:<telemetry_region>` (ou `chain:<chain_id>`) メタデータのパラメータ
  カレーグ・ウム・ロトゥーロ・ディスクリティーヴォ。

## `iroha::Client` 経由で取得

O SDK Rustにはゲートウェイフェッチのヘルパーが含まれています。マニフェストには記述があります
プローベドア (インクルード ストリーム トークンを含む)、デシェ、クライアント、マルチソースのフェッチを実行します。

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

Defina `transport_policy` コモ `Some(TransportPolicy::SoranetStrict)` Quando アップロード
プレシサレム レキュサー リレー クラシコ、OU `Some(TransportPolicy::DirectOnly)` Quando
SoraNet は完全に回避できます。アポンテ `scoreboard.persist_path` パラ o
`scoreboard.now_unix_secs` のリリース管理、オプションの修正
`scoreboard.metadata` com contexto de captura (ラベルとフィクスチャ、alvo Torii など)
`cargo xtask sorafs-adoption-check` consuma JSON の決定的な SDK のパラメータ
SF-6c espera の起源を示すブロブ。
`Client::sorafs_fetch_via_gateway` 識別情報のメタデータを確認する
マニフェスト、マニフェスト CID およびフラグ `gateway_manifest_provided` の予想オプション
検査を行ってください `GatewayFetchConfig` フォルネシド、キャプチャーを含めてください
マニフェストエンベロープアシナドアテンダム青証拠証明要求 SF-6c sem duplicar esses
カンポスマニュアルメンテ。

## マニフェストのヘルパー

`ManifestBuilder` 形式の正規ペイロードを送信し続ける Norito 形式
プログラマティカ:

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

正確なサービスを構築するために、実際のテンポを明示します。ああ
CLI は、パイプラインの決定性に関する推奨事項を送信します。