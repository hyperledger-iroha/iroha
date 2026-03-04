---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/capacity-simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1c6723cbc4f1416b17c64525975de2cc91a768259e8b13431f67c96ab7c7741
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: capacity-simulation
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS 容量シミュレーション・ランブック
sidebar_label: 容量シミュレーション・ランブック
description: 再現可能なフィクスチャ、Prometheus エクスポート、Grafana ダッシュボードを用いて SF-2c 容量マーケットプレイスのシミュレーションツールキットを実行する。
---

:::note 正典のソース
このページは `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` を反映しています。レガシーの Sphinx ドキュメント一式が完全に移行されるまで、両方のコピーを同期してください。
:::

このランブックは、SF-2c 容量マーケットプレイスのシミュレーションキットを実行し、得られたメトリクスを可視化する方法を説明します。`docs/examples/sorafs_capacity_simulation/` の決定的なフィクスチャを用いて、クォータ交渉、フェイルオーバー処理、スラッシング是正をエンドツーエンドで検証します。容量ペイロードは引き続き `sorafs_manifest_stub capacity` を使用します。manifest/CAR のパッケージングには `iroha app sorafs toolkit pack` を使ってください。

## 1. CLI アーティファクトを生成

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` は `sorafs_manifest_stub capacity` をラップし、Norito ペイロード、base64 ブロブ、Torii リクエストボディ、JSON サマリーを生成します:

- クォータ交渉シナリオに参加する 3 つのプロバイダ宣言。
- 準備済みマニフェストをプロバイダ間に割り当てるレプリケーション注文。
- 障害前のベースライン、障害区間、フェイルオーバー回復を捉えるテレメトリ・スナップショット。
- シミュレーション障害後のスラッシングを要求する紛争ペイロード。

すべてのアーティファクトは `./artifacts` に出力されます（第 1 引数で別ディレクトリを指定可能）。`_summary.json` ファイルで人間が読めるコンテキストを確認してください。

## 2. 結果を集計してメトリクスを出力

```bash
./analyze.py --artifacts ./artifacts
```

分析ツールは次を生成します:

- `capacity_simulation_report.json` - 集計された割当、フェイルオーバーの差分、紛争メタデータ。
- `capacity_simulation.prom` - Prometheus の textfile メトリクス (`sorafs_simulation_*`)。node-exporter の textfile collector または単独の scrape job に利用できます。

Prometheus の scrape 設定例:

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

textfile collector を `capacity_simulation.prom` に向けます（node-exporter を使う場合は `--collector.textfile.directory` で指定したディレクトリにコピーしてください）。

## 3. Grafana ダッシュボードをインポート

1. Grafana で `dashboards/grafana/sorafs_capacity_simulation.json` をインポートします。
2. `Prometheus` データソース変数を上記の scrape ターゲットに紐付けます。
3. パネルを確認します:
   - **Quota Allocation (GiB)** は各プロバイダの commit/assign バランスを表示します。
   - **Failover Trigger** は障害メトリクスが入ると *Failover Active* に切り替わります。
   - **Uptime Drop During Outage** はプロバイダ `alpha` の損失率を描画します。
   - **Requested Slash Percentage** は紛争フィクスチャから抽出した是正比率を可視化します。

## 4. 期待されるチェック

- `sorafs_simulation_quota_total_gib{scope="assigned"}` は、コミット合計が >=600 の間は `600` になります。
- `sorafs_simulation_failover_triggered` は `1` を返し、置換プロバイダのメトリクスが `beta` を強調します。
- `sorafs_simulation_slash_requested` はプロバイダ識別子 `alpha` に対して `0.15`（15% slash）を返します。

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` を実行し、フィクスチャが CLI スキーマで引き続き受理されることを確認してください。
