---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 容量シミュレーション
タイトル: 容量シミュレーションのランブック SoraFS
Sidebar_label: 容量のシミュレーションのランブック
説明: 容量 SF-2c キットのシミュレーション マーケットプレイス、備品の再生産、輸出 Prometheus およびボード Grafana を実行します。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` を参照します。 Gardez les deux は、ドキュメントのアンサンブルとスフィンクスの統合を同期させます。
:::

Ce Runbook の明示的なコメントは、SF-2C の容量を市場でシミュレーションするためのキットと結果を視覚化するツールです。 `docs/examples/sorafs_capacity_simulation/` は、クォータの交渉、フェイルオーバーの発生、および試合中の修正の決定を有効にします。容量を使用したペイロード `sorafs_manifest_stub capacity`; `iroha app sorafs toolkit pack` は、マニフェスト/CAR のフラックス ダンプを利用します。

## 1. アーティファクト CLI の生成

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` カプセル化 `sorafs_manifest_stub capacity` ペイロード Norito、ブロブ Base64、リクエスト Torii および履歴 JSON を注ぎます:

- ノルマ交渉のトロワ宣言参加者。
- 定期的な準備を整えるためのマニフェストの複製を作成します。
- 基本的な事前準備のスナップショット、定期的なメンテナンス、およびフェイルオーバーの復旧。
- パンヌシミュレーション後のペイロード要求を解除します。

Tous les artefacts Sont déposés sous `./artifacts` (remplacez en passant un autre répertoire en premier argument)。状況に応じて、`_summary.json` を検査してください。

## 2. 結果と評価の評価

```bash
./analyze.py --artifacts ./artifacts
```

L'analyseur 製品:

- `capacity_simulation_report.json` - 割り当ての統合、フェイルオーバーのデルタ、および問題の管理。
- `capacity_simulation.prom` - テキストファイル Prometheus (`sorafs_simulation_*`) は、テキストファイル コレクタとノード エクスポータを統合し、スクレイピング ジョブに依存しません。

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

テキストファイル コレクターと `capacity_simulation.prom` を比較します (`--collector.textfile.directory` 経由でノード エクスポーターを利用し、レパートリー パスをコピーします)。

## 3. インポーター ファイル ダッシュボード Grafana

1. Grafana で、`dashboards/grafana/sorafs_capacity_simulation.json` をインポートします。
2. データソース `Prometheus` の変数とスクレイピング構成の関連付け。
3. パンノーの検証:
   - **クォータ割り当て (GiB)** は、担当者/担当者に申請書を添付します。
   - **フェイルオーバー トリガー** は、*フェイルオーバー アクティブ* の詳細を確認します。
   - **停止中の稼働時間の低下** トレース ラ パーテ アン プールセント プール ル フォーニッサー `alpha`。
   - **要求されたスラッシュ パーセンテージ** は、改善策の比率と費用を視覚化します。

## 4. 検証の出席者

- `sorafs_simulation_quota_total_gib{scope="assigned"}` est égal à `600` ant que le total engage reste >=600。
- `sorafs_simulation_failover_triggered` は、`1` と前もって交換する方法 `beta` を示します。
- `sorafs_simulation_slash_requested` インデックス `0.15` (15 % デ スラッシュ) `alpha` の識別子を注ぎます。

Exécutez `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` は、スキーマ CLI を受け入れて、フィクスチャの確認を実行します。