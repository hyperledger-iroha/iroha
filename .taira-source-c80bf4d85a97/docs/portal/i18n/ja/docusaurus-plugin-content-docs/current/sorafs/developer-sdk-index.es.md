---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-SDK-インデックス
タイトル: SoraFS の SDK の説明
サイドバーラベル: SDK のガイド
説明: SoraFS の統合アーティファクトに関する特殊なフラグメント。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/developer/sdk/index.md` のページを参照してください。マンテン・アンバス・コピアス・シンクロニザダス。
:::

米国のエステハブは、SoraFS のツールチェーンの言語を選択し、ヘルパーをサポートします。
Rust 固有のスニペットは [Rust SDK スニペット](./developer-sdk-rust.md) です。

## 言語ヘルパー

- **Python** — `sorafs_multi_fetch_local` (ローカルの人間テスト) y
  `sorafs_gateway_fetch` (ゲートウェイからの E2E の解放) `telemetry_region` によるアホラ アセプタン
  `transport_policy` をオーバーライドするオプション
  (`"soranet-first"`、`"soranet-strict"` または `"direct-only"`)、ノブの反射率
  CLI のロールアウト。 QUIC ローカルのプロキシを選択できます。
  `sorafs_gateway_fetch` マニフェスト デル ナベガドールのデブエルブ
  `local_proxy_manifest` パラケ ロス テスト エントレギュエン エル トラスト バンドル ロス アダプタレス
  デル・ナベガドール。
- **JavaScript** — `sorafsMultiFetchLocal` Python のヘルパーを参照、詳細
  ペイロードのバイト数とレシボス、ミエントラ `sorafsGatewayFetch` の再開
  Torii のゲートウェイ、プロキシ ローカルのエンカデナ マニフェスト、ミスモス オーバーライドの説明
  CLI によるテレメトリ/トランスポート。
- **Rust** — スケジューラ経由で直接アクセスできるサービスを提供します
  `sorafs_car::multi_fetch`;参照先を参照してください
  [Rust SDK スニペット](./developer-sdk-rust.md) 証明ストリームと統合のパラ ヘルパー
  デル・オルケスタドール。
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` HTTP 再利用
  Torii と `GatewayFetchOptions` を比較します。コンビナロコン
  `ClientConfig.Builder#setSorafsGatewayUri` PQ に関するヒント
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) クアンド・ラス・スビーダス・デバン・セニルセ
  ルタスソロPQ。

## スコアボードと政治のノブ

Python のヘルパー (`sorafs_multi_fetch_local`) と JavaScript
(`sorafsMultiFetchLocal`) 米国テレメトリのスケジューラーとスコアボードの指数
ポートCLI:

- 欠陥のあるスコアボードの生産性の損失。安定
  `use_scoreboard=True` (`telemetry` の内部構造) の再現
  フィクスチャーパラケエルヘルパーは、エルオルデンポンダーラドデプロフェドールをパルティールデに導きます
  広告とテレメトリのスナップショットのメタデータ。
- Establece `return_scoreboard=True` パラ レシビル ロス ペソス ジュント ア ロス
  CI キャプチャされた診断ログのログを取得できます。
- 米国アレグロス `deny_providers` または `boost_providers` パラレチャザールピアまたはアニャディル国連
  `priority_delta` cuando el スケジューラーの選択は、proveedores です。
- 事前決定の準備を整えて `"soranet-first"` 一斉射撃がダウングレードの準備をします。
  proporciona `"direct-only"` ソロ クアンド ウナ リージョン デ コンプライアンス デバ エビタール リレー
  フォールバック SNNet-5a、予備 `"soranet-strict"` パラ パイロット PQ のみ
  ゴベルナンサの不正行為。
- ゲートウェイ タンビエン指数 `scoreboardOutPath` と `scoreboardNowUnixSecs` のヘルパーを失います。
  `scoreboardOutPath` の永続的なスコアボード計算の設定 (リフレハ エル フラグ)
  `--scoreboard-out` del CLI) パラケ `cargo xtask sorafs-adoption-check` 有効なアーティファクト
  SDK、米国 `scoreboardNowUnixSecs` cuando los フィクスチャが必要な安定性
  `assume_now` パラメタデータの再現可能。 JavaScript ヘルプのヘルプ
  エスタブレサー `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;クアンド・セ・オミテ
  `region:<telemetryRegion>` の派生情報 (`sdk:js` のフォールバック)。エルヘルパーデ
  Python はスコアボードを自動的に出力 `telemetry_source="sdk:python"` を保持します
  y mantiene deshabilitados los metadatos implícitos。

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