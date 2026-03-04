---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 容量シミュレーション
タイトル: دليل تشغيل محاكاة سعة SoraFS
サイドバーラベル: دليل محاكاة السعة
説明: フィクスチャー SF-2c フィクスチャー フィクスチャー フィクスチャー フィクスチャー フィクスチャーPrometheus 、Grafana。
---

:::note ノート
テストは `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` です。スフィンクスの正体は、スフィンクスの存在です。
:::

SF-2c を確認してください。フェイルオーバーを実行し、スラッシュを実行して、フィクスチャを実行します。 `docs/examples/sorafs_capacity_simulation/`。ペイロード数 `sorafs_manifest_stub capacity` `iroha app sorafs toolkit pack` はマニフェスト/CAR を表します。

## 1. アーティファクトの管理 CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` と `sorafs_manifest_stub capacity` の Norito ペイロードと BLOB の Base64 と Torii の JSON意味:

- 最高のパフォーマンスを見せてください。
- 最高のパフォーマンスを見せてください。
- フェールオーバーを実行します。
- ペイロードは、スラッシュとスラッシュの両方を備えています。

تُكتب كل الآرتيفاكت تحت `./artifacts` (يمكن الاستبدال بتمرير دليل مختلف كأول وسيط)。 `_summary.json` は、次のことを意味します。

## 2. テストを実行してください。

```bash
./analyze.py --artifacts ./artifacts
```

セキュリティ:

- `capacity_simulation_report.json` - フェールオーバーが発生しました。
- `capacity_simulation.prom` - テキスト ファイル Prometheus (`sorafs_simulation_*`) テキスト ファイル コレクター、ノード エクスポーター、スクレイピング ジョブ。

スクリーンショット Prometheus:

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

テキストファイル コレクター `capacity_simulation.prom` (ノード エクスポータ `--collector.textfile.directory`)。

## 3. Grafana

1. Grafana、`dashboards/grafana/sorafs_capacity_simulation.json`。
2. `Prometheus` はスクレイピング المُهيأ أعلاه.
3. 重要な点:
   - **クォータ割り当て (GiB)** 。
   - **フェイルオーバー トリガー** *フェイルオーバー アクティブ* ステータス。
   - **障害中の稼働時間の低下** `alpha`。
   - **要求されたスラッシュ パーセンテージ** は、フィクスチャ フィクスチャを表します。

## 4. ああ、

- `sorafs_simulation_quota_total_gib{scope="assigned"}` يساوي `600` طالما بقي الإجمالي الملتزم >=600。
- `sorafs_simulation_failover_triggered` يعرض `1` ويبرز مقياس المزوّد البديل `beta`。
- `sorafs_simulation_slash_requested` يعرض `0.15` ( 15% スラッシュ) لمعرّف المزوّد `alpha`。

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` は、CLI のフィクスチャをサポートしています。