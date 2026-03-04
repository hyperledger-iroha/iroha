---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-SDK-インデックス
タイトル: SoraFS SDK ガイド
Sidebar_label: SDK ガイド
説明: SoraFS アーティファクトは、スニペットを統合します。
---

:::note メモ
:::

SoraFS ツールチェーン、言語ヘルパー トラック、SoraFS 言語ヘルパー トラック
Rust のスニペット لیے [Rust SDK スニペット](./developer-sdk-rust.md) دیکھیں۔

## 言語ヘルパー

- **Python** — `sorafs_multi_fetch_local` (ローカル オーケストレーターのスモーク テスト)
  `sorafs_gateway_fetch` (ゲートウェイ E2E 演習) オプション `telemetry_region`
  `transport_policy` オーバーライド
  (`"soranet-first"`、`"soranet-strict"`、`"direct-only"`) CLI ロールアウト ノブ
  ローカル QUIC プロキシと `sorafs_gateway_fetch` ブラウザ マニフェスト
  `local_proxy_manifest` テスト トラスト バンドル テスト ブラウザ アダプタ
  پہنچا سکیں۔
- **JavaScript** — `sorafsMultiFetchLocal` Python ヘルパーのミラーリング ペイロード バイト
  領収書の要約 `sorafsGatewayFetch` Torii ゲートウェイの演習 ہے،
  ローカル プロキシ マニフェスト スレッド スレッド CLI テレメトリ/トランスポート オーバーライドの公開
- **Rust** — サービス スケジューラ `sorafs_car::multi_fetch` 埋め込み سکتے ہیں؛
  プルーフストリームヘルパーとオーケストレーターの統合 [Rust SDK スニペット](./developer-sdk-rust.md)
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii HTTP エグゼキュータの再利用
  `GatewayFetchOptions` おめでとうございます ہے۔ `ClientConfig.Builder#setSorafsGatewayUri`
  PQ アップロード ヒント (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) アップロードを結合する
  PQ 専用パス پر رکھنا ضروری ہو۔

## スコアボードのポリシーノブ

Python (`sorafs_multi_fetch_local`) JavaScript (`sorafsMultiFetchLocal`) ヘルパー CLI
テレメトリ対応スケジューラ スコアボードを公開する:

- 実稼働バイナリ スコアボードのデフォルトの有効化試合のリプレイ
  `use_scoreboard=True` (`telemetry` エントリ) ヘルパー広告メタデータ 最近のテレメトリ
  スナップショット、重み付けされたプロバイダーの順序付け、導出
- `return_scoreboard=True` 計算された重みチャンク レシートと CI ログを設定します
  診断キャプチャ
- `deny_providers` `boost_providers` 配列 `priority_delta` ピアは拒否します
  スケジューラー プロバイダーを追加します。選択します。
- デフォルト `"soranet-first"` 姿勢 ステータス ダウングレード ステージ`"direct-only"` 認証済み
  コンプライアンス地域、リレー、SNNet-5a フォールバック リハーサル、`"soranet-strict"`
  PQ のみのパイロット ガバナンスの承認 準備金
- ゲートウェイ ヘルパー `scoreboardOutPath` アイコン `scoreboardNowUnixSecs` を公開する`scoreboardOutPath`
  計算されたスコアボードを設定します (CLI `--scoreboard-out` フラグ کی طرح)
  `cargo xtask sorafs-adoption-check` SDK アーティファクトによる検証 `scoreboardNowUnixSecs` 評価
  フィクスチャ 再現可能なメタデータ 安定した `assume_now` 値 ہو۔ JavaScript ヘルパー
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` بھی セット کیے جا سکتے ہیں؛ラベル省略 ہو تو
  `region:<telemetryRegion>` は ہے を導出します (フォールバック `sdk:js`)。 Python ヘルパーのスコアボードの永続化
  `telemetry_source="sdk:python"` 暗黙的なメタデータの発行、無効化、無効化

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```