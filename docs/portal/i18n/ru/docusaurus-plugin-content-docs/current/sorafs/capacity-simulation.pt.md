---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: моделирование емкости
title: Runbook de Simulação de Capacidade do SoraFS
Sidebar_label: Runbook моделирования производительности
описание: Выполните набор инструментов для моделирования рынка возможностей SF-2c с воспроизводимыми светильниками, экспортом для Prometheus и панелями мониторинга для Grafana.
---

:::примечание Fonte canônica
Эта страница написана `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Мантенья представился как копиас синхронизадас.
:::

Этот Runbook поясняется в качестве исполнителя или комплекта моделирования рынка производительности SF-2c и визуализируется в виде результирующих показателей. Он действителен при согласовании котировок, обработке аварийного переключения и исправлении разрыва моста и моста с использованием определенных приспособлений в `docs/examples/sorafs_capacity_simulation/`. Os payloads de capacidade ainda usam `sorafs_manifest_stub capacity`; используйте `iroha app sorafs toolkit pack` для потоков упаковки манифеста/CAR.

## 1. Исправления ошибок CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` инкапсула `sorafs_manifest_stub capacity` для отправляемых полезных данных Norito, blobs base64, корпус заявки для Torii и резюме в формате JSON:

- Тремя заявлениями об участии в плане переговоров по котировкам.
- Укажите порядок репликации, в котором будет размещен манифест в постановке между всеми участниками.
- Снимки телеметрии для базовой линии перед сбоем, интервала сбоя и восстановления после отказа.
- Um payload de disputa solicitando, рубящийся в результате имитации.

Все артефаты для `./artifacts` (замените проход в другом направлении, как первый аргумент). Проверьте архивы `_summary.json` для легального контекста.

## 2. Совокупность результатов и показателей выбросов

```bash
./analyze.py --artifacts ./artifacts
```

О анализаторе продукта:

- `capacity_simulation_report.json` — локальные агрегаты, изменения аварийного переключения и метаданные спора.
- `capacity_simulation.prom` - метрики текстового файла для Prometheus (`sorafs_simulation_*`) подходят для сборщика текстовых файлов, экспортера узлов или независимого задания очистки.

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

Используйте сборщик текстовых файлов для `capacity_simulation.prom` (используйте node-exporter, скопируйте его в папку, проходящую через `--collector.textfile.directory`).

## 3. Импортируйте панель управления с помощью Grafana.

1. Нет Grafana, импортируйте `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Выберите вариант источника данных `Prometheus` для очистки целевой конфигурации.
3. Проверка боли:
   - **Распределение квот (ГиБ)** обеспечивает компромиссные условия/принадлежности каждого дохода.
   - **Триггер аварийного переключения** используется для *Активного переключения при отказе*, когда используются соответствующие показатели.
   - **Снижение времени безотказной работы во время простоя** представляет собой процентное соотношение `alpha`.
   - **Запрошенный процент сокращения** позволяет визуализировать возможность дополнительного исправления спора.

## 4. Проверка результатов

- `sorafs_simulation_quota_total_gib{scope="assigned"}` эквивалентен `600`, или общая постоянная компрометация >=600.
- `sorafs_simulation_failover_triggered` сообщает `1` и использует метрику для проверки или замены оставшегося `beta`.
- `sorafs_simulation_slash_requested` сообщает `0.15` (15 % косой черты) для идентификатора поставщика `alpha`.Выполните команду `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, чтобы подтвердить, что все устройства действительно работают в CLI.