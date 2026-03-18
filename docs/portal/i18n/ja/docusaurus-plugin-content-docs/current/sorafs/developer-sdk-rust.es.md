---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-sdk-rust
タイトル: SDK の Rust のフラグメント
サイドバーラベル: 錆びた断片
説明: 証明ストリームとマニフェストを消費する Rust のミニモス。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/developer/sdk/rust.md` のページを参照してください。マンテン・アンバス・コピアス・シンクロニザダス。
:::

ロス クレイツとエステ リポジトリのインパルサン エル CLI およびクラスター デントロ デ プエデン
オルケスタドーレスまたはパーソナルサービス。ロス フラグメントス デ アバホ レサルタン ロス ヘルパーズ
ケ・マス・ピデン・ロス・デサロラドーレス。

## 証明ストリームのヘルパー

HTTP 解析メトリクスの存在を証明するストリームの解析機能:

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

バージョンが完了しました (テスト中) `docs/examples/sorafs_rust_proof_stream.rs` で生き続けてください。
`ProofStreamSummary::to_json()` CLI からのメトリクスに関する JSON のレンダリング、ロケール
CI の観察や研究を容易にするための食品バックエンド。

## マルチソースフェッチの句読点

`sorafs_car::multi_fetch` モジュールのスケジューラーのフェッチとクロノメーターの実行
CLI。 `sorafs_car::multi_fetch::ScorePolicy` を実装してください。
`FetchOptions::score_policy` 正義の秩序を証明するために。エルテストユニタリオ
`multi_fetch::tests::score_policy_can_filter_providers` 夢の世界
個人的な好み。

CLI からの Otros ノブのリフレジャン フラグ:

- `FetchOptions::per_chunk_retry_limit` 一致コンエルフラグ `--retry-budget` パラ
  正しい規制を解除してください。
- Combina `FetchOptions::global_parallel_limit` コン `--max-peers` パラ リミッター ラ
  兼務議員。
- `OrchestratorConfig::with_telemetry_region("region")` メトリカスのエチケット
  `sorafs_orchestrator_*`、ミエントラスク `OrchestratorConfig::with_transport_policy`
  リフレジャ エル フラグ `--transport-policy` デル CLI。 `TransportPolicy::SoranetPreferred`
  CLI/SDK の表面に欠陥がある場合は、これを確認してください。アメリカ
  `TransportPolicy::DirectOnly` 単独でのダウングレードの準備と指示の確認
  コンプライアンス、Y 予約 `SoranetStrict` パラ パイロット PQ のみの明示的な違反。
- `SorafsGatewayFetchOptions::write_mode_hint = を設定します。
  Some(WriteModeHint::UploadPqOnly)` は PQ 専用です。エルヘルパープロモーション
  交通機関の政治的自動化/緊急一斉射撃の自動化
  エクスプリシタメンテ。
- 米国 `SorafsGatewayFetchOptions::policy_override` フィハルル輸送機関向け
  アノニマト・テンポラル・パラ・ウナ・ソラ・ソリトゥド。アル・プロポーシオナル・クアルキエラ・デ・ロス・カンポス
  ブラウンアウトによる劣化を防ぎ、安全性を追求しません
  満足です。
- Python のバインディングの喪失 (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`)
  JavaScript (`sorafsMultiFetchLocal`) ミスモ スケジューラの再利用、構成として
  `return_scoreboard=true` en esos helpers para recuperar los pesos calculados junto con
  ロス・レシボス・デ・チャンク。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` レジストレーション ストリーム OTLP キュー
  採用のバンドルを作成します。クアンド・セ・オミテ、エル・クライエンテ・デリバ
  `region:<telemetry_region>` (または `chain:<chain_id>`) 自動処理
  メタデータは、エチケットの説明を参照できます。

## `iroha::Client` 経由で取得

Rust の SDK にはゲートウェイフェッチのヘルパーが組み込まれています。マニフェスト ロスに比例する
証明された記述子 (ストリーム トークンを含む) とクライアントの取り出し
el フェッチマルチソース:

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

`transport_policy` を構成 `Some(TransportPolicy::SoranetStrict)` クアンドラス
スビダス デバン レチャザール リレー クラシコス、o `Some(TransportPolicy::DirectOnly)` cuando
SoraNet では完全に省略されています。 Apunta `scoreboard.persist_path` ディレクトリ
リリースのアーティファクト、必要に応じて `scoreboard.now_unix_secs` を完了
キャプチャのコンテキスト `scoreboard.metadata` (フィクスチャのエチケット、ターゲット Torii など)
パラメータ `cargo xtask sorafs-adoption-check` コンスマ JSON 決定要素 SDK コン
SF-6c の手順を確認します。
`Client::sorafs_fetch_via_gateway` アホラの識別情報を補完するメタデータ
 マニフェスト、期待されるオプションのマニフェスト CID およびフラグ
`gateway_manifest_provided` 検査および検査 `GatewayFetchConfig` 管理、モード
キャプチャーを含めて、マニフェスト・ファームド・クムプランを要求する必要があります
プルエバス SF-6c シン デュプリカー EOS カンポス マニュアルメンテ。

## マニフェストのヘルパー

`ManifestBuilder` 形式的なペイロードの形式 Norito 形式
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

ビルダーは、必要な一般的なマニフェストをアルブエロに組み込みます。エル
CLI は、パイプラインの決定に関する推奨事項を示します。