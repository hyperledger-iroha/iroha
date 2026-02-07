---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 容量シミュレーション
タイトル: SoraFS による容量シミュレーションのランブック
Sidebar_label: 容量性シミュレーションのランブック
説明: SF-2c の容量をマーケットプレイスでシミュレートするツールキットの実行、エクスポートは Prometheus で、ダッシュボードは Grafana です。
---

:::ノート フォンテ カノニカ
エスペルハ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` を参照してください。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

Este Runbook は、SF-2C 容量のマーケットプレイスを実行するキットとシミュレーション結果をメトリクス結果として視覚化して説明します。 `docs/examples/sorafs_capacity_simulation/` の OS フィクスチャを決定するために、必要なフェイルオーバーの有効性を確認し、スラッシュの修復メディアを使用します。静電容量の OS ペイロードは、`sorafs_manifest_stub capacity` を使用します。マニフェスト/CAR のエンパコメントに `iroha app sorafs toolkit pack` を使用します。

## 1. CLI のアーティファトを作成する

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` カプセル化 `sorafs_manifest_stub capacity` パラメータ エミミール ペイロード Norito、BLOB Base64、必要な資料 Torii JSON パラメータの履歴:

- 参加者としての交渉の承認を宣言します。
- ステージング・エンターテイナーのマニフェストを複製するかどうかを決定します。
- 本番前の基本的なテレメトリ、フェイルオーバーの回復期間中のテレメトリのスナップショット。
- ファルハ シミュレーションの後に、ペイロードの紛争要請をスラッシュします。

`./artifacts` に関する重要事項 (次の引数を置き換えてください)。 `_summary.json` の状況を検査します。

## 2. 結果と排出量の集計

```bash
./analyze.py --artifacts ./artifacts
```

おおアナリサドール製品:

- `capacity_simulation_report.json` - 集約、フェイルオーバーのデルタ、および議論のメタデータ。
- `capacity_simulation.prom` - Prometheus (`sorafs_simulation_*`) のテキストファイルのメトリクスは、テキストファイルコレクターとして、ノードエクスポーターやスクレイピングジョブに依存しません。

Prometheus のスクレイピング構成の例:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

`capacity_simulation.prom` のテキストファイル コレクターをサポートします (ノード エクスポーターを使用し、`--collector.textfile.directory` 経由でディレクトリ パスをコピーします)。

## 3. ダッシュボードのインポート Grafana

1. Grafana はいいえ、`dashboards/grafana/sorafs_capacity_simulation.json` をインポートします。
2. Vincule は、データソース `Prometheus` を実行し、ターゲット設定 acima をスクレイピングします。
3. 痛みを確認します:
   - **クォータ割り当て (GiB)** ほとんどのコスト/パフォーマンスが証明されています。
   - **フェイルオーバー トリガー** は、メトリカス デ ファルハ チェガムとして *フェイルオーバー アクティブ* を実行する必要があります。
   - **停止中の稼働時間の低下** については、`alpha` がパーセントで証明されています。
   - **要求されたスラッシュ パーセンテージ** は、紛争の解決策を視覚的に表示します。

## 4. エスペラダの検証

- `sorafs_simulation_quota_total_gib{scope="assigned"}` は、`600` の合計永続性 >=600 と同等です。
- `sorafs_simulation_failover_triggered` レポート `1` は、`beta` の代わりにメトリックを証明しました。
- `sorafs_simulation_slash_requested` レポート `0.15` (15% デ スラッシュ) `alpha` の識別情報。

CLI を確認するために `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` を実行します。