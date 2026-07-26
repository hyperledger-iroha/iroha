---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-default-lane-quickstart
タイトル: デフォルト レーン (NX-5)
サイドバーラベル: デフォルトレーン
説明: Nexus デフォルト レーン フォールバック 構成 検証 Torii SDK パブリック レーン レーン ID 省略
---

:::note 正規ソース
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ローカリゼーション スイープを実行する ローカリゼーション スイープを実行する 位置を調整する
:::

# デフォルトレーン (NX-5)

> **ロードマップのコンテキスト:** NX-5 - デフォルトのパブリック レーン統合ランタイム `nexus.routing_policy.default_lane` フォールバック Torii REST/gRPC エンドポイント SDK `lane_id` محفوظ طریقے سے 省略 کر سکیں جب ٹریفک 正規のパブリック レーン سے تعلق رکھتا ہو۔演算子 カタログの構成 `/status` フォールバックの検証 エンドツーエンドのクライアント動作演習 エンドツーエンドのクライアント動作の演習

## 前提条件

- `irohad` کا Sora/Nexus ビルド ( `irohad --sora --config ...` چلائیں )。
- 構成リポジトリの編集 `nexus.*` セクションの編集
- `iroha_cli` ターゲット クラスターが構成されました。
- Torii `/status` ペイロード検査 `curl`/`jq` (同等)。

## 1. レーン データスペース カタログ

ネットワーク アクセス レーン データスペース 宣言 宣言スニペット (`defaults/nexus/config.toml` ) と一致するデータスペース エイリアス レジスタのパブリック レーンと次のコード:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

ہر `index` منفرد اور 連続 ہونا چاہیے۔データスペース ID の 64 ビット値レーン インデックス 数値 数値

## 2. ルーティングのデフォルトとオプションのオーバーライド

`nexus.routing_policy` フォールバック レーンの制御 アカウント プレフィックスの指示 ルーティング オーバーライド ルーティング オーバーライドルール一致ルール スケジューラ設定 `default_lane` ルート `default_dataspace` ルート スケジュールルーター ロジック `crates/iroha_core/src/queue/router.rs` میں ہے اور Torii REST/gRPC サーフェス پر پالیسی شفاف انداز میں apply کرتا ہے۔

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. ノードブートの実行

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

ノードの起動と派生ルーティング ポリシーの実行検証エラー (インデックスの欠落、エイリアスの重複、無効なデータスペース ID) の噂話

## 4. レーンガバナンス状態

ノードオンライン ونے کے بعد، CLI ヘルパー الستعمال کریں تاکہ デフォルトレーンが封印されている (マニフェストがロードされている) اور トラフィック کے لئے 準備完了 ہو۔サマリ ビューのレーンと行の概要:

```bash
iroha_cli app nexus lane-report --summary
```

出力例:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

デフォルト レーン `sealed` 外部トラフィックを許可するレーン ガバナンス ランブック`--fail-on-sealed` フラグ CI کے لئے مفید ہے۔

## 5. Torii ステータス ペイロードの検査

`/status` 応答ルーティング ポリシーがレーン スケジューラのスナップショットを公開する`curl`/`jq` 構成済みのデフォルト値 フォールバック レーン テレメトリー生成❷:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

出力例:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

レーン `0` ライブ スケジューラー カウンターの数:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

TEU スナップショット、エイリアス メタデータ、マニフェスト フラグ設定、整列、整列ペイロード Grafana パネル レーン取り込みダッシュボード میں استعمال ہوتا ہے۔

## 6. クライアントのデフォルトの演習

- **Rust/CLI.** `iroha_cli` Rust クライアント クレート `lane_id` フィールドを省略します。 `--lane-id` / `LaneSelector` はパスします。ありがとうキュー ルーター `default_lane` フォールバック明示的な `--lane-id`/`--dataspace-id` フラグ (デフォルト以外のレーン) とターゲット (ターゲット) のフラグ。
- **JS/Swift/Android。** SDK リリース `laneId`/`lane_id` オプションの値 `/status` 値のフォールバックありがとうございますルーティング ポリシー、ステージング、本番環境、同期、モバイル アプリケーション、緊急時の再構成
- **パイプライン/SSE テスト。** トランザクション イベント フィルター `tx_lane_id == <u32>` 述語 قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`)。 `/v1/pipeline/events/transactions` フィルター チャンネル登録 チャンネル登録 明示的なレーン チャンネル フォールバック レーン ID を書き込みますحت پہنچتی ہیں۔

## 7. 可観測性とガバナンスのフック- `/status` `nexus_lane_governance_sealed_total` 警告 `nexus_lane_governance_sealed_aliases` 警告を発行する アラートマネージャーが警告する 警告する レーンを警告する マニフェストを警告するऔर देखेंアラート、devnets、有効化されたアラート、devnets が有効になりました
- スケジューラ テレメトリ マップ、レーン ガバナンス ダッシュボード (`dashboards/grafana/nexus_lanes.json`) カタログ、エイリアス/スラッグ フィールド、期待値エイリアスの名前変更、名前の変更、クラ ディレクトリのラベル変更、監査人の決定論的パス、 (NX-1 のトラック ہوتا ہے)۔
- デフォルトレーンの議会承認とロールバック計画の承認マニフェスト ハッシュ ガバナンスの証拠 クイックスタート オペレータ ランブック 記録 将来のローテーション 状態 ステータス ステータス