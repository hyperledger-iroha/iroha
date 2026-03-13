---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-default-lane-quickstart
タイトル: デフォルトのラレーン ガイド (NX-5)
Sidebar_label: デフォルトの高速ガイド
説明: 構成者と検証者は、Nexus のデフォルトのレーンのフォールバック、Torii および SDK のレーン ID シュール レーン パブリックを管理します。
---

:::note ソースカノニク
Cette ページの再表示 `docs/source/quickstart/default_lane.md`。 Gardez les deux のコピーは、ローカリゼーション ジュスカ チェ ケ ル バレイヤージュ シュール ポルテイルに到着します。
:::

# デフォルトの高速ガイド (NX-5)

> **コンテキストロードマップ:** NX-5 - デフォルトでの公開統合。ランタイムは、フォールバック `nexus.routing_policy.default_lane` のエンドポイント REST/gRPC や Torii などの安全性を保証する `lane_id` のトラフィックを公開します。 CE ガイドは、コンフィギュラーのカタログ、検証者のフォールバック、`/status` およびテスターのコンポートメント クライアントのカタログに付属しています。

## 前提条件

- Sora/Nexus から `irohad` (ランサー `irohad --sora --config ...`) をアンビルドします。
- セクション `nexus.*` からのデポ設定へのアクセスを解除します。
- `iroha_cli` パーラー au クラスターケーブルを構成します。
- `curl`/`jq` (同等) インスペクター ファイル ペイロード `/status` と Torii。

## 1. レーンとデータスペースのカタログを決定する

レーンとデータスペースが存在するかどうかを宣言します。 L'extrait ci-dessous (タイヤ de `defaults/nexus/config.toml`) は、データスペースの特派員の公開情報を登録します。

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

Chaque `index` は、ユニークで継続的なものです。 64 ビットのデータスペースの情報。サンプル ci-dessus utilisent les memes valeurs numeriques que les インデックス デ レーン プール プラス デ クラルテ。

## 2. デフォルトのルート料金と追加料金オプションの定義

セクション `nexus.routing_policy` では、フォールバックのレーンと追加料金の設定を制御し、ルーティングの指示とコンパイルのプレフィックスを指定します。これに対応し、トランザクションに対するスケジューラー ルートが `default_lane` および `default_dataspace` で設定されます。 `crates/iroha_core/src/queue/router.rs` のルータの論理と、Torii の REST/gRPC の透明なマニエールのアップリケ。

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

Lorsque vous ajoutez と tard de nouvelles レーン、mettez d'abord a jour le Catalogue、puis etendez les regles de Routage。レーン ド フォールバックは、ポインタとレーン パブリック ポートを継続して、主要なトラフィック ユーティリティを維持し、SDK 遺産を継続的に機能させます。

## 3. Demarrer un noeud avec la politique アップリケ

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Le noeud Journalise la politique de routage derivee au demarrage.検証エラー (インデックスのマンクォント、エイリアス重複、無効なデータスペース ID) は、ゴシップのデビュー前に実行されます。

## 4. レーンの統治を確認する者

必要に応じて、デフォルトの安全管理 (マニフェスト料金) とトラフィックを管理するための検証ツールであるヘルパー CLI を利用します。 La vue recap affiche une ligne par LANE :

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

デフォルトの添付書類 `sealed` を使用して、交通管理の外部管理者向けの管理用ランブックを作成します。 CI を使用してフラグ `--fail-on-sealed` を取得します。

## 5. Torii の法定ペイロード検査官

応答 `/status` は、スケジューラーごとの瞬時のルート管理の政治を公開します。 `curl`/`jq` は、デフォルト設定の値と検証者によるフォールバック製品のテレメトリの確認者を利用します:

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

検査官の検査官がライブ デュ スケジューラーを流し、レーン `0` を注ぎます:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Cela は、瞬時の TEU、エイリアスのメタドン、および整列アベニュー構成の明示を確認します。ミーム ペイロードは、レーンのダッシュボードからのパノー Grafana の取り込みを利用します。

## 6. 顧客のデフォルトの評価をテストする- **Rust/CLI.** `iroha_cli` et le crate client Rust omettent le Champ `lane_id` lorsque vous ne passez pas `--lane-id` / `LaneSelector`. `default_lane` を経由してキューを復元します。 `--lane-id`/`--dataspace-id` のフラグを使用して、デフォルトではないレーンを明確に示します。
- **JS/Swift/Android.** SDK traitent `laneId`/`lane_id` のバージョンの詳細は、`/status` の評価に関する通知とオプションを参照してください。再構成が必要な場合は、モバイル アプリのステージングと本番環境でルーティングの同期を管理します。
- **パイプライン/SSE テスト。** トランザクション受け入れ条件のフィルター `tx_lane_id == <u32>` (`docs/source/pipeline.md`)。 Abonnez-vous a `/v2/pipeline/events/transactions` avec ce filtre pour prouver que les ecritures envoyees sans 車線を明示的に到着し、レーン デ フォールバックを確認します。

## 7. 監視権と統治権の監視

- `/status` 公開オーストラリア `nexus_lane_governance_sealed_total` と `nexus_lane_governance_sealed_aliases` は、アラート マネージャーがレーン パード ソンのマニフェストを回避します。 Gardez ces は、開発ネット上のアクティブなミームを警告します。
- スケジューラーのテレメトリとレーンの管理ダッシュボード (`dashboards/grafana/nexus_lanes.json`) のカタログのエイリアス/スラッグのアテンダント。 Si vous renommez un alias, reetiquetez les repertoires 倉特派員は、conservent des chemins deterministes (suvi sous NX-1) を参照して監査を行います。
- ロールバックの計画を含め、デフォルトのレーンを承認する議会を承認します。マニフェストのハッシュと統治の優先事項を登録し、クイックスタートと投票ランブックの操作を実行して、将来のローテーションを決定し、必要な情報を決定します。

安全な検証を終了し、裏切り者 `nexus.routing_policy.default_lane` を SDK のソースから直接提供し、単一レーンの遺産のコードの無効化を開始します。