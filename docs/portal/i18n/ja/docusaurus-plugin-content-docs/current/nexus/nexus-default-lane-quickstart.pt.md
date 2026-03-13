---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-default-lane-quickstart
タイトル: ギア ラピダ ド レーン パドラオ (NX-5)
サイドバーラベル: ギア・ラピダ・ド・レーン・パドラオ
説明: Nexus パラメータ Torii の SDK を使用してレーン パドラオのフォールバックを構成し、lane_id レーンの公開を省略します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/quickstart/default_lane.md`。マンテンハ アンバスは、ポータルのローカル情報を確認しながら、コピア アリンハダスを食べました。
:::

# ギア・ラピダ・ド・レーン・パドラオ (NX-5)

> **ロードマップのコンテキスト:** NX-5 - レーン公共パドラオの統合。ランタイム アゴラは、フォールバック `nexus.routing_policy.default_lane` パラケ エンドポイント REST/gRPC で Torii を実行し、SDK を安全に実行できます。`lane_id` は、トラフィックとトラフィックのパフォーマンスを公開します。 `/status` のクライアントのコンフィギュレーション、カタログ、検証、フォールバックの操作を実行します。

## 前提条件

- うーん、Sora/Nexus を `irohad` でビルドします (`irohad --sora --config ...` を実行します)。
- `nexus.*` 編集用のリポジトリ設定にアクセスします。
- `iroha_cli` クラスター全体の構成を設定します。
- `curl`/`jq` (同等) ペイロード `/status` は Torii を検査します。

## 1. レーンとデータスペースのカタログの説明

存在しないようにレーンとデータスペースを宣言します。 O trecho abaixo (recortado de `defaults/nexus/config.toml`) 登録レーンの公開データスペースの別名:

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

Cada `index` 開発者ユニコと継続。 64 ビットのデータスペースの値。アメリカ合衆国の政府機関は、主要なクラレザの指標を数値化します。

## 2. 問題を解決するためのパドロエス デ ロテアメントを定義する

`nexus.routing_policy` はフォールバック レーンを制御し、プレフィックスの特定の指示に従って安全な制御を許可します。 `default_lane` および `default_dataspace` 構成を転送するためのスケジューラの管理。論理的にルータは `crates/iroha_core/src/queue/router.rs` を生き、REST/gRPC が Torii を行う上層部として透明な政治的アプリケーションを実現します。

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

将来の新星レーンをすべて選択し、定期的に最初のカタログと定期的なメンテナンスを実現してください。フォールバックは、SDK の代替の永続的な互換性を維持するために、主要な部分でのレーンの公開集中を継続的に実行します。

## 3. 政治的用途の開始

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

起動期間中の政治的役割を担うノード登録。不正な不正行為 (有効なインデックス、重複したエイリアス、無効なデータスペース ID) は、ゴシップの開始前に行われます。

## 4. レーンの統治を確認する

オンラインでノードを取得し、レーン パドラオ エスタ セレド (マニフェスト カレガド) を実行して、トラフィックを確認するために CLI を実行するヘルパーを使用します。馬リンハポーレーンでの最高のビザ:

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

レーン パドラオのほとんどの `sealed`、政府のランブックがレーンの事前許可で外部からの輸送を許可します。フラグ `--fail-on-sealed` e util para CI。

## 5. OS ペイロードのステータスを検査 Torii

レスポスタ `/status` は、スケジューラーごとにスナップショットを作成し、政治的役割を果たします。 `curl`/`jq` を使用して、パドロエス構成を確認し、フォールバック エスタ製品テレメトリを確認します。

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

パラ検査OSコンタドール生体内スケジューラーパラオレーン`0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

TEU のスナップショットを確認し、マニフェスト アリナムのエイリアス メタデータと OS フラグを設定します。レーン取り込みのダッシュボードで Grafana を実行するためのペイロードの管理。

## 6. クライアントを実行します。- **Rust/CLI.** `iroha_cli` クライアントを Rust から削除する `lane_id` は、すぐに実行できます `--lane-id` / `LaneSelector`。おおルータ・デ・フィラス、ポルタント、カイ・エム・`default_lane`。 `--lane-id`/`--dataspace-id` アペナス アオ ミララム レーン ナオ パドラオとして明示的にフラグを使用します。
- **JS/Swift/Android。** SDK トラタム `laneId`/`lane_id` の最終リリースとして、`/status` の有効性と安全性を考慮したフォールバックが可能です。政治は非常に重要な役割を果たし、非常に正確な再構成を行うための準備と生産を開始します。
- **パイプライン/SSE テスト。** トランザクション監視イベントのフィルター `tx_lane_id == <u32>` (veja `docs/source/pipeline.md`)。 Assine `/v2/pipeline/events/transactions` は、フォールバックを回避するために、安全性を確認するためのフィルターを作成します。

## 7. 統治の監視

- `/status` は、`nexus_lane_governance_sealed_total` と `nexus_lane_governance_sealed_aliases` パラグラフを表示し、アラート マネージャーは、マニフェストごとに 1 つずつアクセスします。 Mantenha は、開発ネットワークの管理に関するアラートを発行します。
- スケジューラを実行するテレメトリと、レーンを管理するダッシュボード (`dashboards/grafana/nexus_lanes.json`) のエスペラム オス カンポスのエイリアス/スラッグをカタログに表示します。エイリアスを再確認し、オーディオ マンテナム カミーニョの決定者としてのクラ特派員を再確認してください (rastreado sob NX-1)。
- ロールバックを含む、レーン間のパドラオ開発を支援します。登録は、政府機関の証拠を明示し、迅速なスタートを切り、未来のロタコスのオペレーターの実行手順書を作成し、必要な管理を行います。

認証を取得し、SDK の設定を行って、SDK を使用して、代替コードを使用して、別の方法で `nexus.routing_policy.default_lane` を呼び出します。