---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-SDK-インデックス
title: ガイド SDK SoraFS
サイドバーラベル: ガイド SDK
説明: アーティファクト SoraFS を統合した言語による拡張機能。
---

:::note ソースカノニク
:::

ツールチェーン SoraFS を使用して、言語ライブラリのヘルパーをサポートするハブを利用します。
Rust のスニペットをすべて注いでください。[Rust SDK スニペット](./developer-sdk-rust.md)。

## 言語によるヘルパー

- **Python** — `sorafs_multi_fetch_local` (ローカルのオーケストラの煙をテスト)
  `sorafs_gateway_fetch` (ゲートウェイの E2E 演習) 承認済みのデータ
  `telemetry_region` オプションと `transport_policy` のオーバーライド解除
  (`"soranet-first"`、`"soranet-strict"` または `"direct-only"`)、ノブの詳細
  CLI によるロールアウト。 Lorsqu'un プロキシ QUIC ローカル デマーレ、
  `sorafs_gateway_fetch` renvoie ファイル マニフェスト ナビゲーター経由
  `local_proxy_manifest` 信頼バンドル補助の送信テストを完了します
  適応者ナビゲーター。
- **JavaScript** — `sorafsMultiFetchLocal` ヘルパー Python を参照、参照
  ペイロードのバイトと履歴、タンディス QUE `sorafsGatewayFetch` 演習
  ゲートウェイ Torii、プロキシ ローカルのマニフェストをパスし、ミームを公開します
  CLI のテレメトリー/トランスポートをオーバーライドします。
- **Rust** — スケジューラーの指示を介して、システムのサービスを公開します
  `sorafs_car::multi_fetch` ;参照して相談する
  [Rust SDK スニペット](./developer-sdk-rust.md) ヘルパーのプルーフストリームなどを注ぐ
  オーケストラの統合。
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` HTTP を再利用
  Torii および `GatewayFetchOptions`。コンバインル・アベック
  `ClientConfig.Builder#setSorafsGatewayUri` およびアップロード PQ のインデックス
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) lorsque les アップロード doivent
  Rester sur des chemins PQ のみ。

## スコアボードと政治のノブ

レスヘルパー Python (`sorafs_multi_fetch_local`) と JavaScript
(`sorafsMultiFetchLocal`) スケジューラのベースとなるスコアボードの利用方法を公開
CLI の場合:

- デフォルトのスコアボードでの生産活動のビネール。定義
  `use_scoreboard=True` (メイン料理の 4 つ `telemetry`) 再生デス
  備品は、プロバイダーとパートーレの順序を決定するためのヘルパーを提供します。
  最近の広告やスナップショットの情報。
- Définissez `return_scoreboard=True` は、平均的な計算結果を受け取ります
  ログの CI キャプチャ ファイル診断に関するチャンクの調査。
- テーブルを使用して `deny_providers` または `boost_providers` を注ぎます
  `priority_delta` でプロバイダーのスケジューラーを選択してください。
- ダウングレード時のデフォルト `"soranet-first"` の姿勢を維持します。フルニセ
  `"direct-only"` seulement lorsqu'une 地域に準拠して、リレーを実行してください
  フォールバック SNNet-5a の繰り返しと `"soranet-strict"` 補助パイロット
  PQ のみの avec 承認と統治。
- ヘルパー ゲートウェイがオーストラリア `scoreboardOutPath` および `scoreboardNowUnixSecs` を公開しています。
  Définissez `scoreboardOutPath` 持続的なスコアボードの計算 (旗のミロワール)
  CLI `--scoreboard-out`) `cargo xtask sorafs-adoption-check` の有効なアーティファクト
  SDK および `scoreboardNowUnixSecs` を使用して、必要なフィクスチャが必要になります
  `assume_now` 安定した pour des metadonnées の再生産可能。ヘルパー JavaScript については、
  オーストラリアの定義 `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  ラベルが正しくなく、`region:<telemetryRegion>` を取得します (`sdk:js` に対する avec フォールバック)。
  Le helper Python émet automatiquement `telemetry_source="sdk:python"` チャックフォワキル
  スコアボードとメタドンの持続は、非アクティブ化を暗黙的に示します。

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