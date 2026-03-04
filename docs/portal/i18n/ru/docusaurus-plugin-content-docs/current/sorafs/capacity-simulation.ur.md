---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: моделирование емкости
title: SoraFS کپیسٹی سمیولیشن رَن بُک
Sidebar_label: کپیسٹی سمیولیشن رَن بُک
описание: воспроизводимые светильники, Prometheus экспорт, Grafana панели мониторинга کے ساتھ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن Как это сделать
---

:::примечание
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی آئینہ دار ہے۔ جب تک پرانا Sphinx دستاویزی مجموعہ مکمل طور پر منتقل نہیں ہو جاتا دونوں نقول کو ہم آہنگ رکھیں۔
:::

В качестве примера можно привести SF-2c, который можно использовать в качестве оружия. چلائیں اور حاصل شدہ میٹرکس کیسے دیکھیں۔ `docs/examples/sorafs_capacity_simulation/` позволяет использовать детерминированные устройства, использовать квоту и аварийное переключение, а также сквозное исправление среза. ویلیڈیٹ کرتی ہے۔ Полезные нагрузки могут быть установлены `sorafs_manifest_stub capacity`. Манифест/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. Доступ к интерфейсу командной строки

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`, `sorafs_manifest_stub capacity`, а также полезные данные Norito, большие двоичные объекты base64, тела запросов Torii и сводки JSON. ہے برائے:

- Квота и декларации поставщиков квоты.
- Порядок репликации и промежуточный манифест, а также поставщики и поставщики услуг.
- базовый уровень перед отключением, интервал простоя, аварийное восстановление и снимки телеметрии.
- симулированный сбой, позволяющий сократить нагрузку на спорную полезную нагрузку.

تمام آرٹی فیکٹس `./artifacts` کے تحت جمع ہوتے ہیں (پہلے آرگومنٹ میں مختلف ڈائریکٹری دے کر اووررائیڈ کر سکتے ہیں)۔ انسانی سمجھ کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. Найдите нужные метрики

```bash
./analyze.py --artifacts ./artifacts
```

Вот пример того, как:

- `capacity_simulation_report.json` - распределение выделений, дельты аварийного переключения, а также спорные метаданные.
- `capacity_simulation.prom` - Prometheus метрики текстового файла (`sorafs_simulation_*`) в сборщике текстовых файлов node-exporter.

Конфигурация очистки Prometheus:

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

Сборщик текстовых файлов может быть установлен в `capacity_simulation.prom` (узел-экспортер может быть установлен в `--collector.textfile.directory`). ڈائریکٹری میں کاپی کریں)۔

## 3. Grafana ڈیش بورڈ امپورٹ کریں

1. Grafana - `dashboards/grafana/sorafs_capacity_simulation.json` - Дополнительная информация
2. Источник данных `Prometheus` позволяет выполнить очистку цели для очистки.
3. Что можно сделать:
   - **Распределение квот (ГиБ)** У поставщика есть зафиксированные/назначенные балансы.
   - **Failover Trigger** Метрики сбоев в работе *Failover Active* ہو جاتا ہے۔
   - **Снижение времени безотказной работы во время отключения** от поставщика `alpha`.
   - **Запрошенный процент сокращения**

## 4. Дополнительные возможности

- `sorafs_simulation_quota_total_gib{scope="assigned"}` کی قدر `600` رہتی ہے جب تک общее количество выделенных средств >=600 رہے۔
- `sorafs_simulation_failover_triggered` قدر `1` دیتا ہے اور замена метрики поставщика `beta` نمایاں ہوتا ہے۔
- Поставщик `sorafs_simulation_slash_requested` `alpha` کے لیے `0.15` (косая черта 15%)

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` Используется для настройки светильников и схемы CLI для проверки.