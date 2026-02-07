---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 容量シミュレーション
タイトル: SoraFS の容量シミュレーションのランブック
Sidebar_label: 容量シミュレーションのランブック
説明: 静電容量 SF-2c コンフィクスチャの再現品の市場シミュレーション、Prometheus および Grafana のダッシュボードのエクスポート用ツールキットの取り出し。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` のページを参照してください。スフィンクスの記録は完全に記録されており、すべての記録が保存されています。
:::

Este Runbook は、SF-2C 容量の市場シミュレーションと、結果のメトリクスを視覚化するためのキットの詳細を説明します。 `docs/examples/sorafs_capacity_simulation/` では、必要な調整、フェイルオーバーの有効性、および極端なスラッシュ修正の検証が行われます。 `sorafs_manifest_stub capacity` での容量損失ペイロード。米国 `iroha app sorafs toolkit pack` マニフェスト/CAR のフルホス デ エンパケタド。

## 1. CLI の一般的な成果物

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` envuelve `sorafs_manifest_stub capacity` para Emiir payloads Norito、blobbase64、cuerpos de solicitud para Torii y resúmenes JSON para:

- 交渉のシナリオに参加するための証明を宣言します。
- 証明されたイベントをステージングするために、複製を作成する必要があります。
- ベースライン前のテレメトリ、フェールオーバーの回復期間中のスナップショット。
- ペイロードの紛争要求を解除して、トラフィックのシミュレーションをスラッシュします。

Todos los artefactos se escriben bajo `./artifacts` (puedes reemplazarlo pasando undirectio diferente como primer argumento)。 `_summary.json` のコンテキストを読み取れるように検査します。

## 2. 結果と排出量の集計

```bash
./analyze.py --artifacts ./artifacts
```

エル・アナリザドール・プロデュース:

- `capacity_simulation_report.json` - 集計、フェイルオーバーの差分、および議論のメタデータを割り当てます。
- `capacity_simulation.prom` - Prometheus (`sorafs_simulation_*`) のテキストファイルのメトリクスは、ノードエクスポータのテキストファイルコレクターと独立したジョブのスクレイピングをサポートします。

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

テキストファイル コレクター `capacity_simulation.prom` (ノード エクスポーターを使用し、`--collector.textfile.directory` 経由でディレクトリをコピーします)。

## 3. Grafana のダッシュボードのインポート

1. En Grafana、importa `dashboards/grafana/sorafs_capacity_simulation.json`。
2. Vincula はデータソース `Prometheus` の変数からターゲット設定を取得します。
3. ベリフィカ・ロス・パネル:
   - **クォータ割り当て (GiB)** は、comprometidos/asignados de cada proveedor のバランスを保ちます。
   - **フェイルオーバー トリガー** カンビアと *フェイルオーバー アクティブ* が実行されます。
   - **停止中の稼働時間の低下** グラフィック `alpha`。
   - **要求されたスラッシュ パーセンテージ** は、紛争の修復に必要な割合を視覚化します。

## 4. コンプロバシオネス エスペラダス

- `sorafs_simulation_quota_total_gib{scope="assigned"}` は、`600` mientras el total comprometido se mantiene >=600 と同等です。
- `sorafs_simulation_failover_triggered` レポート `1` は、`beta` の指標を証明します。
- `sorafs_simulation_slash_requested` レポート `0.15` (15% スラッシュ) 証明者 `alpha` の識別子。

Ejecuta `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` は、CLI の検査結果を確認するためのフィクスチャを確認します。