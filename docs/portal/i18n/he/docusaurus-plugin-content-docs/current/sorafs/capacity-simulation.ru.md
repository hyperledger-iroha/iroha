---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: סימולציית קיבולת
כותרת: Ранбук симуляции емкости SoraFS
sidebar_label: Ранбук симуляции емкости
תיאור: Запуск набора симуляции рынка емкости SF-2c с воспроизводимыми фикстурами, экспортами Prometheus Grafana.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Держите обе копии синхронизированными, пока устаревший набор документации Sphinx полностью не будест перевший.
:::

Этот ранбук объясняет, как запускать набор симуляции рынка емкости SF-2c и визуализировать поличентны. Он проверяет переговоры по квотам, обработку failover ו-ремедиацию חיתוך מקצה לקצה, используя детерминированные I108000X. מטענים емкости по-прежнему используют `sorafs_manifest_stub capacity`; используйте `iroha app sorafs toolkit pack` для потоков упаковки manifest/CAR.

## 1. Сгенерировать CLI-артефакты

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` оборачивает `sorafs_manifest_stub capacity`, чтобы выпускать Norito מטענים, base64-блоб, тела запросов Prometheus ל:

- Трех деклараций провайдеров, участвующих в сценарии переговоров по квотам.
- Одного распоряжения о репликации, распределяющего מבוים-מנופיסט между провайдерами.
- טלפונים של סינמקולים ל-Baзовой линии до сбоя, интервала сбоя ו-восстановления failover.
- מטען спора с запросом на slashing после смоделированного сбоя.

Все артефакты помещаются в `./artifacts` (можно переопределить, передав другой каталог первмым). Проверьте файлы `_summary.json` для читаемого контекста.

## 2. Агрегировать результаты и выпустить метрики

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор формирует:

- `capacity_simulation_report.json` - התקנות תקינות, תקלות תקלות ושירותים מתקדמים.
- `capacity_simulation.prom` - метрики textfile Prometheus (`sorafs_simulation_*`).

Пример конфигурации לגרד Prometheus:

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

צור אספן קבצי טקסט ב-`capacity_simulation.prom` (באמצעות יצואנים צומתים מסודרים בקטלוג, переданный через000200X000020X).

## 3. Импортировать дашборд Grafana

1. В Grafana импортируйте `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Привяжите переменную מקור נתונים `Prometheus` к указанной выше цели לגרד.
3. הצג לוחות:
   - **הקצאת מכסות (GiB)** показывает баланс commit/assign для каждого провайдера.
   - **טריגר כשל בכשל** переключается на *failover Active*, когда поступают метрики сбоя.
   - **ירידה בזמן פעילות בזמן הפסקה** отображает процент потери для провайдера `alpha`.
   - **אחוז נטוי מבוקש** визуализирует коэффициент ремедиации из фикстуры спора.

## 4. Ожидаемые проверки

- `sorafs_simulation_quota_total_gib{scope="assigned"}` равен `600`, צפה ב-commit остаётся >=600.
- `sorafs_simulation_failover_triggered` показывает `1`, а метрика заменяющего провайдера выделяет `beta`.
- `sorafs_simulation_slash_requested` показывает `0.15` (15% נטוי) עבור идентификатора провайдера `alpha`.

Запустите `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, чтобы подтвердить, что фикстуры по‑прежнему принимаются схемой CLI.