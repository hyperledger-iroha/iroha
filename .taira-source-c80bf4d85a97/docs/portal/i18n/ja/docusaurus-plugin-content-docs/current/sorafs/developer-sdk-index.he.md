---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71e2894a26f4f2220e32615a7b25f10e048687c5a3d139a5bc1a5cee380d2a0a
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: developer-sdk-index
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/developer/sdk/index.md` を反映しています。レガシーの Sphinx セットが退役するまで両方を同期してください。
:::

このハブで、SoraFS ツールチェーンに同梱される言語別ヘルパーを追跡できます。
Rust 向けのスニペットは [Rust SDK snippets](./developer-sdk-rust.md) を参照してください。

## 言語別ヘルパー

- **Python** — `sorafs_multi_fetch_local`（ローカルオーケストレーターのスモークテスト）と
  `sorafs_gateway_fetch`（gateway E2E 演習）は、CLI の rollout ノブに合わせて、
  任意の `telemetry_region` と `transport_policy` の override
  (`"soranet-first"`, `"soranet-strict"`, `"direct-only"`) を受け付けます。
  ローカル QUIC プロキシが起動すると、`sorafs_gateway_fetch` はブラウザ manifest を
  `local_proxy_manifest` に返すため、テストが trust bundle をブラウザアダプタへ
  渡せます。
- **JavaScript** — `sorafsMultiFetchLocal` は Python helper を反映し、payload bytes と
  受領サマリーを返します。`sorafsGatewayFetch` は Torii gateways を実行し、ローカル
  プロキシ manifest を引き回し、CLI と同じ telemetry/transport override を露出します。
- **Rust** — サービスは `sorafs_car::multi_fetch` を通じて scheduler を直接埋め込めます。
  proof-stream helper と orchestrator 統合については
  [Rust SDK snippets](./developer-sdk-rust.md) を参照してください。
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` は Torii の HTTP executor を再利用し、
  `GatewayFetchOptions` を尊重します。`ClientConfig.Builder#setSorafsGatewayUri` と
  PQ アップロードヒント（`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`）を併用し、
  アップロードが PQ-only 経路に固定されるべき場合に使ってください。

## Scoreboard とポリシーノブ

Python（`sorafs_multi_fetch_local`）と JavaScript（`sorafsMultiFetchLocal`）のヘルパーは、
CLI が使うテレメトリ対応の scheduler scoreboard を公開しています:

- 本番バイナリでは scoreboard がデフォルトで有効です。fixtures を再生する際に
  `use_scoreboard=True`（または `telemetry` エントリ）を設定し、helper が advert メタデータと
  最近のテレメトリスナップショットからプロバイダーの重み付き順序を導出できるようにします。
- `return_scoreboard=True` を設定して、算出済みの重みを chunk receipt と一緒に受け取り、
  CI ログが診断情報を記録できるようにします。
- `deny_providers` または `boost_providers` 配列で peer を拒否したり、scheduler が
  プロバイダーを選ぶ際に `priority_delta` を加算できます。
- 既定の `"soranet-first"` を維持し、downgrade をステージングする場合のみ変更します。
  `"direct-only"` はコンプライアンス領域がリレー回避を求める場合や SNNet-5a フォールバックの
  リハーサル時に限定し、`"soranet-strict"` はガバナンス承認のある PQ-only パイロット向けに
  予約します。
- gateway helper は `scoreboardOutPath` と `scoreboardNowUnixSecs` も公開します。
  `scoreboardOutPath` を設定すると算出済み scoreboard を永続化します（CLI の `--scoreboard-out` に対応）。
  これにより `cargo xtask sorafs-adoption-check` が SDK アーティファクトを検証できます。
  `scoreboardNowUnixSecs` は fixtures が安定した `assume_now` 値を必要とする場合に使います。
  JavaScript helper では `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` も設定可能で、
  ラベルが省略されると `region:<telemetryRegion>`（フォールバックは `sdk:js`）を導出します。
  Python helper は scoreboard を永続化する際に `telemetry_source="sdk:python"` を自動付与し、
  暗黙メタデータは無効のままにします。

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
