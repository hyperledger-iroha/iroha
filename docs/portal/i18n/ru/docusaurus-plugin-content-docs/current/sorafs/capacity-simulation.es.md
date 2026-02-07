---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: моделирование емкости
название: Runbook моделирования мощности SoraFS
Sidebar_label: Runbook моделирования производительности
описание: Воспользуйтесь набором инструментов для моделирования рынка емкости SF-2c с воспроизводимыми приспособлениями, экспортом Prometheus и приборными панелями Grafana.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Нам пришлось скопировать синхронизированные копии того, что комплект документации был передан в Сфинкс, если он полностью мигрировал.
:::

Это объяснение Runbook включает в себя набор для моделирования рынка производительности SF-2c и визуализацию полученных результатов. Подтвердите согласование настроек, управление аварийным переключением и исправление экстремальных и экстремальных значений с использованием детерминированных приборов в `docs/examples/sorafs_capacity_simulation/`. Потеря полезной нагрузки при использовании `sorafs_manifest_stub capacity`; США `iroha app sorafs toolkit pack` для упаковки в манифест/CAR.

## 1. Генерация артефактов CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` конвертирует `sorafs_manifest_stub capacity` для отправки полезных данных Norito, больших двоичных объектов base64, запроса на Torii и резюме JSON для:

- Три заявления проверяющих, участвующих в сценарии переговоров.
- Порядок репликации, который назначается для проявления в постановке между этими поставщиками.
- Снимки телеметрии для предыдущего базового состояния, интервала между событиями и восстановления для аварийного переключения.
- Полезная нагрузка по запросу спора разрезает имитацию tras la caída.

Все артефакты записаны как `./artifacts` (можно повторно указать другую директорию в качестве первого аргумента). Проверьте архивы `_summary.json` для разборчивости контекста.

## 2. Совокупность результатов и показателей выбросов

```bash
./analyze.py --artifacts ./artifacts
```

El analizador производит:

- `capacity_simulation_report.json` — назначение агрегатов, изменений при отказе и метаданных спора.
- `capacity_simulation.prom` — метрики текстового файла Prometheus (`sorafs_simulation_*`), подходящие для сборщика текстовых файлов с помощью узла-экспортера или независимого задания очистки.

Пример конфигурации очистки Prometheus:

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

Подключите сборщик текстовых файлов к `capacity_simulation.prom` (если вы используете экспортер узлов, скопируйте его в папку, расположенную через `--collector.textfile.directory`).

## 3. Импортируйте панель управления Grafana.

1. En Grafana, импорт `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Выделите переменную источника данных `Prometheus` и выберите цель очистки, настроенную заранее.
3. Проверка панелей:
   - **Распределение квот (ГиБ)** должно осуществляться по компромиссным/назначаемым каждодневным счетам.
   - **Триггер аварийного переключения** активируется в режиме *Аварийное переключение* при входе в систему показателей.
   - **Снижение времени бесперебойной работы во время отключения**
   - **Запрошенный процент сокращения** позволяет визуализировать долю дополнительного исправления спорных моментов.

## 4. Эсперадас Comprobaciones- `sorafs_simulation_quota_total_gib{scope="assigned"}` эквивалентен `600` при общем снижении содержания мантиена >=600.
- `sorafs_simulation_failover_triggered` отчет `1` и метрика поставщика повторной замены `beta`.
- `sorafs_simulation_slash_requested` сообщает `0.15` (15% косой черты) для идентификатора подтверждения `alpha`.

Выведите `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, чтобы подтвердить, что все приборы были приняты для выполнения команды CLI.