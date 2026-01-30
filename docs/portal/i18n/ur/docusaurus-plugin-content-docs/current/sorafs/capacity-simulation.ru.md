---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: capacity-simulation
title: Ранбук симуляции емкости SoraFS
sidebar_label: Ранбук симуляции емкости
description: Запуск набора симуляции рынка емкости SF-2c с воспроизводимыми фикстурами, экспортами Prometheus и дашбордами Grafana.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Держите обе копии синхронизированными, пока устаревший набор документации Sphinx полностью не будет перенесен.
:::

Этот ранбук объясняет, как запускать набор симуляции рынка емкости SF-2c и визуализировать полученные метрики. Он проверяет переговоры по квотам, обработку failover и ремедиацию slashing end-to-end, используя детерминированные фикстуры в `docs/examples/sorafs_capacity_simulation/`. Payloads емкости по-прежнему используют `sorafs_manifest_stub capacity`; используйте `iroha app sorafs toolkit pack` для потоков упаковки manifest/CAR.

## 1. Сгенерировать CLI-артефакты

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` оборачивает `sorafs_manifest_stub capacity`, чтобы выпускать Norito payloads, base64-блоб, тела запросов Torii и JSON-сводки для:

- Трех деклараций провайдеров, участвующих в сценарии переговоров по квотам.
- Одного распоряжения о репликации, распределяющего staged‑манифест между провайдерами.
- Снимков телеметрии для базовой линии до сбоя, интервала сбоя и восстановления failover.
- Payload спора с запросом на slashing после смоделированного сбоя.

Все артефакты помещаются в `./artifacts` (можно переопределить, передав другой каталог первым аргументом). Проверьте файлы `_summary.json` для читаемого контекста.

## 2. Агрегировать результаты и выпустить метрики

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор формирует:

- `capacity_simulation_report.json` - агрегированные распределения, дельты failover и метаданные спора.
- `capacity_simulation.prom` - метрики textfile Prometheus (`sorafs_simulation_*`), подходящие для textfile collector node-exporter или отдельного scrape job.

Пример конфигурации scrape Prometheus:

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

Укажите textfile collector на `capacity_simulation.prom` (при использовании node-exporter скопируйте его в каталог, переданный через `--collector.textfile.directory`).

## 3. Импортировать дашборд Grafana

1. В Grafana импортируйте `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Привяжите переменную datasource `Prometheus` к указанной выше цели scrape.
3. Проверьте панели:
   - **Quota Allocation (GiB)** показывает баланс commit/assign для каждого провайдера.
   - **Failover Trigger** переключается на *Failover Active*, когда поступают метрики сбоя.
   - **Uptime Drop During Outage** отображает процент потери для провайдера `alpha`.
   - **Requested Slash Percentage** визуализирует коэффициент ремедиации из фикстуры спора.

## 4. Ожидаемые проверки

- `sorafs_simulation_quota_total_gib{scope="assigned"}` равен `600`, пока общий commit остаётся >=600.
- `sorafs_simulation_failover_triggered` показывает `1`, а метрика заменяющего провайдера выделяет `beta`.
- `sorafs_simulation_slash_requested` показывает `0.15` (15% slash) для идентификатора провайдера `alpha`.

Запустите `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, чтобы подтвердить, что фикстуры по‑прежнему принимаются схемой CLI.
