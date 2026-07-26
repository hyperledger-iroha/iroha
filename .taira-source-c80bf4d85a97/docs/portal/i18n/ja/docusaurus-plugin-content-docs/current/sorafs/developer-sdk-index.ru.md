---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-SDK-インデックス
タイトル: Руководства по SDK SoraFS
サイドバーラベル: SDK の標準
説明: Языковые сниппеты для интеграции артефактов SoraFS.
---

:::note Канонический источник
:::

言語ヘルパー、ツールチェーン SoraFS を使用できます。
Rust から [Rust SDK スニペット](./developer-sdk-rust.md) を参照してください。

## Языковые ヘルパー

- **Python** — `sorafs_multi_fetch_local` (スモーク テスト локального оркестратора)
  `sorafs_gateway_fetch` (ゲートウェイ E2E упражнения) теперь принимают опциональный
  `telemetry_region` オーバーライド `transport_policy`
  (`"soranet-first"`、`"soranet-strict"` または `"direct-only"`)、ロールアウト ノブ
  CLI。 QUIC プロキシ、`sorafs_gateway_fetch` を使用してください。
  ブラウザー マニフェスト、`local_proxy_manifest`、トラスト バンドル
  そうです。
- **JavaScript** — `sorafsMultiFetchLocal` отражает Python ヘルパー、ペイロード バイトの計算
  および概要 квитанций、тогда как `sorafsGatewayFetch` упражняет Torii ゲートウェイ、
  プロキシとテレメトリ/トランスポートのオーバーライドをマニフェストします。
  と CLI。
- **Rust** — スケジューラ `sorafs_car::multi_fetch`;
  最低。 [Rust SDK スニペット](./developer-sdk-rust.md) 証明ストリーム ヘルパーと説明
  оркестратора。
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` переиспользует Torii HTTP エグゼキュータ
  `GatewayFetchOptions`。 Комбинируйте с
  `ClientConfig.Builder#setSorafsGatewayUri` または PQ アップロードのヒント
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) は、PQ のみの機能です。

## スコアボードとポリシーノブ

Python (`sorafs_multi_fetch_local`) および JavaScript (`sorafsMultiFetchLocal`) ヘルパー
テレメトリ対応スケジューラー スコアボード、および CLI:

- プロダクションスコアボードの表示。 `use_scoreboard=True`
  (`telemetry` エントリ) フィクスチャ、ヘルパーの機能
  メタデータ広告とテレメトリ スナップショットを確認できます。
- Установите `return_scoreboard=True`、チャンクのレシートを取得する、
  позволяя CI логам фиксировать диагностику。
- Используйте массивы `deny_providers` または `boost_providers` はピアと同じです。
  `priority_delta`、スケジューラが機能しています。
- Сохраняйте позу `"soranet-first"` по умолчанию, если только не готовите ダウングレード;
  указывайте `"direct-only"` когда コンプライアンス регион обязан избегать リレー или при
  フォールバック SNNet-5a、`"soranet-strict"` および PQ のみの機能
  統治。
- ゲートウェイ ヘルパーは `scoreboardOutPath` および `scoreboardNowUnixSecs` です。
  Задайте `scoreboardOutPath` для сохранения вычисленного スコアボード (соответствует флагу)
  CLI `--scoreboard-out`)、`cargo xtask sorafs-adoption-check` の結果
  SDK は、`scoreboardNowUnixSecs`、フィクスチャを備えています。
  `assume_now` メタデータを参照してください。 JavaScript ヘルパー Вжно
  дополнительно установить `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  ラベル опущен、он выводит `region:<telemetryRegion>` (フォールバック на `sdk:js`)。 Python ヘルパー
  `telemetry_source="sdk:python"` のスコアボードと держит
  暗黙的なメタデータ。

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