---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-SDK-インデックス
タイトル: SoraFS の SDK の開発
サイドバー ラベル: SDK の開発
説明: SoraFS の統合された技術に関する言語のトレコス。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/developer/sdk/index.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

SoraFS のツールチェーンの言語用に、companhar os ヘルパーのエステ ハブを使用します。
Rust 固有のスニペット、[Rust SDK スニペット](./developer-sdk-rust.md) のパラグラフ。

## 言語ヘルパー

- **Python** - `sorafs_multi_fetch_local` (スモーク テストは orquestrador ローカルで実行されます)
  `sorafs_gateway_fetch` (ゲートウェイでの E2E 演習) `telemetry_region`
  `transport_policy` のオプションのオーバーライド
  (`"soranet-first"`、`"soranet-strict"` または `"direct-only"`)、OS ノブを操作します
  ロールアウトは CLI を実行します。 Quando um プロキシ QUIC ローカル ソベ、`sorafs_gateway_fetch` レトルナ
  ブラウザマニフェスト em `local_proxy_manifest` para que os testes passem o trust Bundle
  パラ・アダプタドレス・デ・ナベガドール。
- **JavaScript** - `sorafsMultiFetchLocal` Python のヘルパー、レトルナンドの説明
  ペイロードのバイト数とレシーボの履歴、エンクアント `sorafsGatewayFetch` の実行
  ゲートウェイ Torii、encadeia マニフェスト、プロキシ ローカル、expoe os mesmos オーバーライド
  テレメトリ/トランスポートは CLI を実行します。
- **Rust** - サービス ポデム エンブティルまたはスケジューラー ディレタメント経由
  `sorafs_car::multi_fetch`;リファレンスを参照してください
  [Rust SDK スニペット](./developer-sdk-rust.md) プルーフストリームのパラヘルパー
  インテグラソン・ド・オルケストラドール。
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` 実行者 HTTP を再利用する
  do Torii e honra `GatewayFetchOptions`。コンバインコム
  `ClientConfig.Builder#setSorafsGatewayUri` PQ アップロードのヒント
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) Quando はプレシサレムをアップロードします
  フィカル エム カミーニョス ソメンテ PQ。

## スコアボードと政治のノブ

Python の OS ヘルパー (`sorafs_multi_fetch_local`) と JavaScript
(`sorafsMultiFetchLocal`) エクスポエムまたはスコアボード、スケジューラー コム テレメトリア アメリカ
ペロ CLI:

- パドラオによるスコアボードの生産活動;デフィナ `use_scoreboard=True`
  (ou forneca entradas `telemetry`) ヘルパーの派生パラメータによる再現フィクスチャ
  広告やスナップショットのメタデータを検証するための計画を立てる
  最近のテレメトリー。
- Defina `return_scoreboard=True` の受信料ペソ計算と com recibos の計算
  チャンク、CI キャプチャー診断ログの許可。
- 配列 `deny_providers` または `boost_providers` をピアまたはアディショナルとして使用します
  `priority_delta` スケジューラーの選択が完了しました。
- Mantenha a postura Padrao `"soranet-first"` a menos que esteja preparando um ダウングレード;
  forneca `"direct-only"` アペナス クアンド ウマ レギアオ デ コンプライアンス プレシザー エヴィーター リレー
  フォールバック SNNet-5a、予約 `"soranet-strict"` パラ パイロット PQ のみ
  com aprovacao de Governmenta。
- ゲートウェイのヘルパーは、`scoreboardOutPath` および `scoreboardNowUnixSecs` を説明します。
  Defina `scoreboardOutPath` 持続性スコアボード計算 (エスペルハ フラグ)
  `--scoreboard-out` CLI を実行) パラメータ `cargo xtask sorafs-adoption-check` 有効
  SDK の芸術品、`scoreboardNowUnixSecs` Quando フィクスチャを正確に使用
  valor `assume_now` はメタデータを再現します。 JavaScript のヘルパーはありません。
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; 声を上げてください。
  ラベルを省略し、派生 `region:<telemetryRegion>` (フォールバック パラ `sdk:js`)。
  Python のヘルパーが自動で出力する `telemetry_source="sdk:python"` Quando
  スコアボードと暗黙のメタデータを保持します。

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