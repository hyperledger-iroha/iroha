---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d33ce7d1d495b1c6c9c6f48725e75582d95125c51ab34c6985f6d20c80483f90
source_last_modified: "2025-11-14T04:43:20.384272+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正本ソース
このページは `docs/source/quickstart/default_lane.md` を反映しています。ローカリゼーションの一括更新がポータルに入るまで、両方のコピーを同期してください。
:::

# デフォルト lane クイックスタート (NX-5)

> **ロードマップの文脈:** NX-5 - default public lane の統合。ランタイムは `nexus.routing_policy.default_lane` の fallback を公開し、Torii の REST/gRPC エンドポイントと各 SDK が、トラフィックが canonical public lane に属する場合に `lane_id` を安全に省略できるようになりました。本ガイドでは、カタログの設定、`/status` での fallback 確認、クライアント挙動のエンドツーエンド検証を順に案内します。

## 前提条件

- Sora/Nexus ビルドの `irohad`（`irohad --sora --config ...` を実行）。
- `nexus.*` セクションを編集できる設定リポジトリへのアクセス。
- 対象クラスタに接続するよう設定された `iroha_cli`。
- Torii の `/status` ペイロードを確認するための `curl`/`jq`（または同等のツール）。

## 1. lane と dataspace のカタログを記述する

ネットワーク上に存在すべき lane と dataspace を宣言します。以下の抜粋（`defaults/nexus/config.toml` から抜粋）では、3 つの public lane と対応する dataspace alias を登録しています。

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

各 `index` は一意かつ連続である必要があります。dataspace の id は 64-bit 値で、上記の例では分かりやすさのため lane の index と同じ数値を使っています。

## 2. ルーティングのデフォルトと任意の上書きを設定する

`nexus.routing_policy` セクションは fallback lane を制御し、特定の instruction やアカウント prefix に対するルーティング上書きを許可します。どのルールにも一致しない場合、scheduler は設定済みの `default_lane` と `default_dataspace` にトランザクションを送ります。router のロジックは `crates/iroha_core/src/queue/router.rs` にあり、Torii の REST/gRPC 面に透過的にポリシーを適用します。

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

新しい lane を後から追加する場合は、まずカタログを更新し、その後ルーティングルールを拡張します。fallback lane は、レガシー SDK が互換性を保てるよう、ユーザートラフィックの大半を担う public lane を指し続ける必要があります。

## 3. ポリシーを適用したノードを起動する

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

ノードは起動時に導出されたルーティングポリシーをログに出力します。検証エラー（index 欠落、alias 重複、dataspace id 無効）は gossip 開始前に報告されます。

## 4. lane のガバナンス状態を確認する

ノードがオンラインになったら、CLI ヘルパーで default lane が sealed（manifest 読み込み済み）かつトラフィックに対応できる状態であることを確認します。サマリービューは lane ごとに 1 行を出力します。

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

default lane が `sealed` を示す場合は、外部トラフィックを許可する前に lane ガバナンスの runbook に従ってください。`--fail-on-sealed` フラグは CI に便利です。

## 5. Torii の status ペイロードを確認する

`/status` レスポンスはルーティングポリシーと lane ごとの scheduler スナップショットを公開します。`curl`/`jq` を使って設定済みのデフォルト値を確認し、fallback lane がテレメトリを出していることを確かめます。

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

サンプル出力:

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

lane `0` の scheduler カウンタを確認するには次を実行します。

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

これにより、TEU スナップショット、alias メタデータ、manifest フラグが設定と一致していることが確認できます。同じペイロードが Grafana の lane-ingest ダッシュボードでも使用されています。

## 6. クライアントのデフォルト挙動を確認する

- **Rust/CLI.** `iroha_cli` と Rust の client crate は、`--lane-id` / `LaneSelector` を渡さない場合に `lane_id` フィールドを省略します。queue router はそのため `default_lane` にフォールバックします。非デフォルト lane を狙うときだけ `--lane-id`/`--dataspace-id` を明示してください。
- **JS/Swift/Android.** 最新の SDK リリースでは `laneId`/`lane_id` を任意として扱い、`/status` で告知された値にフォールバックします。ステージングと本番でルーティングポリシーを同期し、モバイルアプリが緊急再設定を不要にできるようにしてください。
- **Pipeline/SSE tests.** トランザクションイベントフィルタは `tx_lane_id == <u32>` の述語を受け付けます（`docs/source/pipeline.md` を参照）。`/v1/pipeline/events/transactions` にそのフィルタで購読し、lane を明示せずに送信した書き込みが fallback lane id で到達することを確認してください。

## 7. 監視性とガバナンスのフック

- `/status` は `nexus_lane_governance_sealed_total` と `nexus_lane_governance_sealed_aliases` も公開し、Alertmanager が lane の manifest 失効を警告できるようにしています。devnet でもこれらのアラートを有効に保ってください。
- scheduler のテレメトリマップと lane ガバナンスダッシュボード（`dashboards/grafana/nexus_lanes.json`）はカタログの alias/slug フィールドを前提にしています。alias を変更する場合は、監査で決定論的なパスを維持できるよう、対応する Kura ディレクトリもリラベルしてください（NX-1 で追跡）。
- default lane の評議会承認にはロールバック計画を含めるべきです。このクイックスタートと並べて manifest ハッシュとガバナンス証跡を運用 runbook に記録し、将来のローテーションで必要状態を推測させないようにしてください。

これらの確認が完了したら、`nexus.routing_policy.default_lane` を SDK 設定の単一の真実として扱い、ネットワーク上のレガシー単一 lane コードパスの無効化を進められます。