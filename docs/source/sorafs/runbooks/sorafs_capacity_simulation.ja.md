---
lang: ja
direction: ltr
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS 容量シミュレーション・ランブック

このランブックは、SF-2c 容量マーケットプレイスのシミュレーションツールキットを実行し、得られたメトリクスを可視化する方法を説明します。目的は、`docs/examples/sorafs_capacity_simulation/` にある再現可能なフィクスチャを使って、クォータ交渉、フェイルオーバー処理、スラッシング是正をエンドツーエンドで検証することです。容量ペイロードは引き続き `sorafs_manifest_stub capacity` を使用します。manifest/CAR のパッケージングには `iroha app sorafs toolkit pack` を使ってください。

## 1. CLI アーティファクトを生成

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

このスクリプトは `sorafs_manifest_stub capacity` を呼び出し、決定的な Norito ペイロード、base64 エンコード、Torii リクエストボディ、JSON サマリーを生成します:

- クォータ交渉シナリオに参加する 3 つのプロバイダ宣言。
- 準備済みマニフェストをプロバイダ間に割り当てるレプリケーション注文。
- 障害前のベースライン、障害ウィンドウ、フェイルオーバー回復を捉えるテレメトリ・スナップショット。
- シミュレーション障害後のスラッシングを要求する紛争ペイロード。

アーティファクトは `./artifacts` に書き込まれます（第 1 引数にパスを渡すと変更可能）。`_summary.json` ファイルで人間が読める状態を確認してください。

## 2. 結果を集計してメトリクスを出力

```bash
./analyze.py --artifacts ./artifacts
```

分析スクリプトは次を生成します:

- `capacity_simulation_report.json` — 集計された割当、フェイルオーバーの差分、紛争メタデータ。
- `capacity_simulation.prom` — Prometheus の textfile メトリクス (`sorafs_simulation_*`)。node-exporter の textfile collector または独立した Prometheus scrape job にインポートできます。

`prometheus.yml` の scrape 設定例:

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

生成された `.prom` を textfile collector に向けます（node-exporter の場合は `--collector.textfile.directory` にコピーしてください）。

## 3. Grafana ダッシュボードをインポート

1. Grafana で `dashboards/grafana/sorafs_capacity_simulation.json` をインポートします。
2. `Prometheus` データソース入力を上記の scrape 設定に紐付けます。
3. パネルを確認します:
   - **Quota Allocation (GiB)** は各プロバイダの commit/assign バランスを表示します。
   - **Failover Trigger** は障害メトリクスが読み込まれると *Failover Active* に切り替わります。
   - **Uptime Drop During Outage** はプロバイダ `alpha` の損失率を描画します。
   - **Requested Slash Percentage** は紛争フィクスチャから抽出した是正比率を可視化します。

## 4. 期待されるチェック

- `sorafs_simulation_quota_total_gib{scope="assigned"}` は、コミット合計が ≥600 の間は 600 になります。
- `sorafs_simulation_failover_triggered` は `1` を返し、置換プロバイダのメトリクスが `beta` を強調します。
- `sorafs_simulation_slash_requested` はプロバイダ識別子 `alpha` に対して `0.15`（15% slash）を返します。

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` を実行し、フィクスチャが CLI スキーマで引き続き検証されることを確認してください。
