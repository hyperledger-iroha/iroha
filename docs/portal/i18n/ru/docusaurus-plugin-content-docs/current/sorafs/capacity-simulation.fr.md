---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: моделирование емкости
название: Runbook de Simulation de Capacité SoraFS
Sidebar_label: Runbook моделирования производительности
описание: Комплект для моделирования рыночной мощности SF-2c с воспроизводимыми светильниками, экспортом Prometheus и таблицами Grafana.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Обратите внимание на две синхронизированные копии, которые представляют собой ансамбль документации Sphinx, унаследованный так, чтобы он мигрировал.
:::

Этот runbook поясняет комментарии, выполняет набор моделирования рыночной мощности SF-2c и визуализирует результаты. Действует согласование квот, жест переключения при отказе и исправление сокращения боя в режиме помощи детерминированным приборам в `docs/examples/sorafs_capacity_simulation/`. Емкость полезной нагрузки, используемая сегодня `sorafs_manifest_stub capacity`; используйте `iroha app sorafs toolkit pack` для манифеста потока заполнения/CAR.

## 1. Создание артефактов CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` инкапсуляция `sorafs_manifest_stub capacity` для измерения полезных данных Norito, больших двоичных объектов base64, запрошенных корпусов Torii и резюме JSON для:

- Трое деклараций участников сценария переговоров по квотам.
- Порядок репликации размещается в манифесте среди четырех специалистов.
- Снимки телеметрии для базовой линии предварительной настройки, интервала панорамирования и восстановления после сбоя.
- Полезная нагрузка по судебному делу требует резких движений после одновременной работы.

Все артефакты были спрятаны под `./artifacts` (заменены на проходе и в другом репертуаре в главном аргументе). Проверьте файлы `_summary.json`, чтобы увидеть контекст.

## 2. Сбор результатов и измерение показателей

```bash
./analyze.py --artifacts ./artifacts
```

Продукт анализатора:

- `capacity_simulation_report.json` — общие распределения, отклонения отработки отказа и метадонические процессы.
- `capacity_simulation.prom` - текстовый файл метрик Prometheus (`sorafs_simulation_*`) адаптирован для сборщика текстовых файлов из узла-экспортера или для независимой очистки данных.

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

Укажите сборщик текстовых файлов версии `capacity_simulation.prom` (если вы используете node-exporter, скопируйте его в прошлый репертуар через `--collector.textfile.directory`).

## 3. Импортер панели управления Grafana

1. Данс Grafana, импорт `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Свяжите переменную источника данных `Prometheus` с конфигурируемым кабелем очистки данных.
3. Проверьте панно:
   - **Распределение квот (ГиБ)** указывает на тех, кто занимается/назначается для каждой операции.
   - **Триггер аварийного переключения** переходит к *Активному аварийному переключению* по поступающим показателям панели.
   - **Снижение времени безотказной работы во время простоя**. Отслеживайте состояние работоспособности `alpha`.
   - **Запрошенный процент сокращения** визуализирует коэффициент дополнительного возмещения ущерба по судебному делу.

## 4. Участники проверки- `sorafs_simulation_quota_total_gib{scope="assigned"}` равен `600`, если общее количество оставшихся >=600.
- `sorafs_simulation_failover_triggered` Индикация `1` и метрика замены с учетом `beta`.
- `sorafs_simulation_slash_requested` Индикация `0.15` (15 % косой черты) для идентификатора пользователя `alpha`.

Выполните команду `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit`, чтобы подтвердить, что все приспособления, которые уже приняты, соответствуют схеме CLI.