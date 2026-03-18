---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: 容量シミュレーション
タイトル: Ранбук симуляции емкости SoraFS
サイドバーラベル: Ранбук симуляции емкости
説明: Запуск набора симуляции рынка емкости SF-2c с воспроизводимыми фикстурами, экспортами Prometheus и даспрордами Grafana。
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`.スフィンクスは、スフィンクスを攻撃するのに役立ちます。
:::

SF-2c を使用して、SF-2c を使用してください。フェールオーバーを実行し、エンドツーエンドでスラッシュを実行することもできます。 `docs/examples/sorafs_capacity_simulation/`。ペイロードは `sorafs_manifest_stub capacity`; `iroha app sorafs toolkit pack` マニフェスト/CAR が表示されます。

## 1. CLI を使用する

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` оборачивает `sorafs_manifest_stub capacity`、чтобы выпускать Norito ペイロード、base64-блоб、тела запросов Torii およびJSON 形式:

- Трех деклараций провайдеров, участвующих в сценарии переговоров по квотам.
- 舞台化された作品、劇場版、劇場版、劇場版。
- フェイルオーバーを実行します。
- ペイロードは、スラッシュとスラッシュの両方を必要とします。

Все артефакты помещаются в `./artifacts` (можно переопределить, передав другой каталог первым аргументом)。 Проверьте файлы `_summary.json` для читаемого контекста.

## 2. Агрегировать результаты и выпустить метрики

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор формирует:

- `capacity_simulation_report.json` - フェールオーバーが必要です。
- `capacity_simulation.prom` - テキストファイル Prometheus (`sorafs_simulation_*`)、テキストファイル コレクター ノード エクスポーター、スクレイピング ジョブ。

例: Prometheus をスクレイピング:

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

テキストファイル コレクター - `capacity_simulation.prom` (ノード エクスポーターの機能、`--collector.textfile.directory`)。

## 3. Импортировать дапорд Grafana

1. Grafana または `dashboards/grafana/sorafs_capacity_simulation.json`。
2. データソース `Prometheus` をスクレイピングします。
3. 説明:
   - **クォータ割り当て (GiB)** は、コミット/割り当てを実行します。
   - **フェイルオーバー トリガー** が *フェイルオーバー アクティブ* に該当し、これが表示されます。
   - **停止中の稼働時間の低下** отображает процент потери для провайдера `alpha`。
   - **リクエストされたスラッシュパーセンテージ** を確認してください。

## 4. Ожидаемые проверки

- `sorafs_simulation_quota_total_gib{scope="assigned"}` равен `600`、コミット остаётся >=600。
- `sorafs_simulation_failover_triggered` は `1`、`beta` と同じです。
- `sorafs_simulation_slash_requested` は `0.15` (15% スラッシュ) であり、`alpha` です。

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`、чтобы подтвердить、что фикстуры по‑прежнему принимаются схемой CLI。