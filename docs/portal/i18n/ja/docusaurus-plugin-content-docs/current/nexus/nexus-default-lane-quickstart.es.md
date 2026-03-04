---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-default-lane-quickstart
タイトル: 事前決定ギア (NX-5)
サイドバーラベル: 事前に決定された速度の変化
説明: Nexus パラケ Torii のレーンの事前決定でフォールバックを検証し、SDK の公開レーンのレーン ID を省略します。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/quickstart/default_lane.md`。 Manten は、ローカルリーグのポータルで、さまざまな情報を共有しています。
:::

# 事前決定ギア (NX-5)

> **ロードマップの内容:** NX-5 - 事前に公開される統合。ランタイム アホラは、フォールバック `nexus.routing_policy.default_lane` パラケ ロス エンドポイント REST/gRPC デ Torii および SDK プエダン省略コンセグリダード Un `lane_id` で、トラフィック パーテネシー アラート パブリック キャノニコを提供します。 `/status` の `/status` で、設定されたカタログ、検証されたフォールバック、および極端なクライアントの安全な操作が可能です。

## 前提条件

- Sora/Nexus を `irohad` でビルドします (`irohad --sora --config ...` を取り出します)。
- 編集セクション `nexus.*` の設定リポジトリにアクセスします。
- `iroha_cli` クラスター オブジェクトのハブラー コンフィギュレーションを設定します。
- `curl`/`jq` (同等) ペイロード `/status` と Torii。

## 1. カタログとデータスペースの説明

レーンとデータスペースが存在することを宣言します。断片的な情報 (`defaults/nexus/config.toml` の記録) データスペースの別名公開登録:

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

Cada `index` は、連続したユニコです。 64 ビットのデータスペースの価値が失われます。ロス・エジェンプロス・アンテリオレス・米国、ロス・ミスモス・バロールズ、クラリダード・パラ・マヨール・クラリダード・レーン・デ・レーンの指標。

## 2. 必要なルールを事前に決定し、必要なルールを設定する

セクション `nexus.routing_policy` は、フォールバック レーンの制御を許可するための特別な命令を実行するための事前許可です。スケジュールが一致しているため、`default_lane` および `default_dataspace` 設定のトランザクション スケジュールが実行されています。 `crates/iroha_core/src/queue/router.rs` でのルータの論理は、Torii の REST/gRPC の透明性のある形式の政治に適用されます。

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

クアンド・マス・アデランテは、ヌエバス・レーンに同意し、実際のカタログやルールを延長します。フォールバックのデベセギルは、市長の交通機関の集中路での公共交通機関の安全性を確認するためのSDKの情報を提供します。

## 3. アランカは政治の問題を解決する

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

長期にわたる政治政治の登録。検証エラー (インデックスのファルタンテス、エイリアス重複、無効なデータスペース ID) は、ゴシップに関する安全性を保証します。

## 4. レーンの確認

Una vez que el nodo este en linea、usa el helper del CLI para verificar que el LANE predeterminado este sellado (manifest cargado) y listo para trafico。素晴らしい景色を再開してください:

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

レーンの事前決定事項 `sealed`、外出先での交通許可のためのランブックです。 El フラグ `--fail-on-sealed` es util para CI。

## 5. Torii のペイロードの検査

La respuesta `/status` は、スケジューラごとに瞬時にポリティカ デ エンルタミエントを実行します。米国 `curl`/`jq` は、事前決定された設定とコンプロバー クエリ レーン デ フォールバック サービスのテレメトリの生産性を確認します。

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

レーンごとの生体内スケジューラの検査 `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

TEU のインスタントを確認し、別名やフラグのマニフェスト ラインコンフィギュレーションを確認します。レーン取り込みのダッシュボードにある Grafana のパネルのミスモ ペイロード。

## 6. 事前決定の緊急命令- **Rust/CLI.** `iroha_cli` は、Rust を使用しないクライアントのクレート `lane_id` は、`--lane-id` / `LaneSelector` をパスしません。ルーター デ コラス ポル ロ タントが `default_lane` を繰り返します。米国ロス フラグ明示的 `--lane-id`/`--dataspace-id` ソロ クアンド アプンテス、ウナ レーン、事前決定なし。
- **JS/Swift/Android.** SDK トラタン `laneId`/`lane_id` の最終バージョンは、`/status` に関するフォールバックの安全性を保証します。非常に重要な政治的問題は常に準備されており、アプリや映画の制作に必要な緊急時の再構成は必要ありません。
- **パイプライン/SSE テスト。** トランザクション イベントのフィルタリング アセプタン プレディカド `tx_lane_id == <u32>` (バージョン `docs/source/pipeline.md`)。 `/v1/pipeline/events/transactions` を購読して、デモストラル クエリ ラス エスクリチュラス エンヴィアダのレーンを明示的に表示し、レーンのフォールバックを確認してください。

## 7. 観察可能性とガンチョス・デ・ゴベルナンサ

- `/status` tambien publica `nexus_lane_governance_sealed_total` y `nexus_lane_governance_sealed_aliases` para que Alertmanager pueda avisar cuando una lane pierde su マニフェスト。開発ネットを含むハビリタダのアラートを管理します。
- テレメトリとスケジューラのマップとレーンのダッシュボード (`dashboards/grafana/nexus_lanes.json`) エスペラン ロス カンポスのエイリアス/スラッグ デル カタログ。 Si renombras un alias, vuelve a etiquetar los Directorios Kurarespondentes para que los Auditores mantengan rutas deterministas (seguido bajo NX-1).
- ロールバック計画を含めた事前決定は、レーンごとに行われる必要があります。レジストラ エル ハッシュ デル マニフェストと、すぐに証拠を提出し、クイック スタートとランブックのオペレーター パラ ケラス ロタシオネス フューチュラスを登録し、管理者が要求する必要はありません。

私は、SDK の構成を変更するために、`nexus.routing_policy.default_lane` との競争を回避し、赤ラン ユニコの制御を開始します。