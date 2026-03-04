---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: моделирование емкости
Название: Ранбук симуляции зарядов SoraFS
Sidebar_label: Ранбук симуляции емкости
описание: Запуск набора моделирования рынка емкостей SF-2c с воспроизводимыми фикстурами, экспортами Prometheus и дашбордами Grafana.
---

:::note Канонический источник
Эта страница зеркалирует `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Держите копии синхронизированными, пока открытый набор документации Sphinx полностью не будет перенесен.
:::

Этот ранбук руки позволяет запустить набор моделирования рыночных мощностей SF-2c и визуализировать полученные метрики. Недавно он по квотам обработал аварийное переключение и восстановление сквозного разрезания с помощью определенных фикстур в `docs/examples/sorafs_capacity_simulation/`. Емкости полезной нагрузки по-прежнему используются `sorafs_manifest_stub capacity`; воспользуйтесь `iroha app sorafs toolkit pack` для потоковой передачи манифеста/CAR.

## 1. Сгенерировать CLI-артефакты

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` оборачивает `sorafs_manifest_stub capacity`, чтобы выдать Norito полезные нагрузки, base64-blob, запросы тела Torii и JSON-сводки для:

- Трех деклараций провайдеров, представленных в сценариях по квотам.
- Одного распоряжения о репликации, восстановления поэтапного манифеста между провайдерами.
- Снимки телеметрии для перевода линии до сбоя, интервала сбоя и восстановления после сбоя.
- Полезная нагрузка спора с запросом на рубку после смоделированного сбоя.

Все документы находятся в `./artifacts` (можно переопределить, передав другой аргумент первым аргументом). Проверьте файлы `_summary.json` для читаемого контекста.

## 2. Агрегировать результаты и высылать метрики

```bash
./analyze.py --artifacts ./artifacts
```

Анализатор формирует:

- `capacity_simulation_report.json` - агрегированные распределения, дельты аварийного переключения и метаданные споры.
- `capacity_simulation.prom` - метрики текстового файла Prometheus (`sorafs_simulation_*`), подключаемые для узла-экспортера сборщика текстовых файлов или отдельного задания очистки.

Пример конфигурации Scrap Prometheus:

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

Укажите сборщик текстовых файлов на `capacity_simulation.prom` (при использовании node-exporter скопируйте его в каталог, передаваемый через `--collector.textfile.directory`).

##3. Импортировать дашборд Grafana

1. В Grafana импортируйте `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Привяжите переменный источник данных `Prometheus` к указанной выше цели Scrape.
3. Проверьте панель:
   - **Распределение квот (ГиБ)** показывает баланс фиксации/назначения для каждого провайдера.
   - **Триггер аварийного переключения** переключается на *Аварийное переключение активно*, когда поступают метрики сбоя.
   - **Снижение времени бесперебойной работы во время простоя** отображает процент потерь для провайдера `alpha`.
   - **Запрошенный процент слэша** визуализирует коэффициент исправления фикстур спора.

## 4. Ожидаемые проверки

- `sorafs_simulation_quota_total_gib{scope="assigned"}` равен `600`, пока общая фиксация остается >=600.
- `sorafs_simulation_failover_triggered` показывает `1`, метрика заменяющего провайдера популярных `beta`.
- `sorafs_simulation_slash_requested` показывает `0.15` (косая черта 15%) для идентификатора провайдера `alpha`.

Запустите `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, чтобы убедиться, что фикстуры по-прежнему принимаются схемой CLI.