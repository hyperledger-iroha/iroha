---
lang: ru
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/04-telemetry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a947c289c13c15b09dfbbf28c23ae1539fd3e29ca3943fa8522c3eca32c28bf5
source_last_modified: "2025-11-04T17:24:14.014911+00:00"
translation_last_reviewed: 2026-01-01
---

# Требования к телеметрии

## Цели Prometheus

Снимайте метрики relay и orchestrator со следующими метками:

```yaml
- job_name: "soranet-relay"
  static_configs:
    - targets: ["relay-host:9898"]
      labels:
        region: "testnet-t0"
        role: "relay"
- job_name: "sorafs-orchestrator"
  static_configs:
    - targets: ["orchestrator-host:9797"]
      labels:
        region: "testnet-t0"
        role: "orchestrator"
```

## Обязательные дашборды

1. `dashboards/grafana/soranet_testnet_overview.json` *(будет опубликован)* — загрузите JSON и импортируйте переменные `region` и `relay_id`.
2. `dashboards/grafana/soranet_privacy_metrics.json` *(актив SNNet-8)* — убедитесь, что панели privacy bucket отображаются без пробелов.

## Правила алертов

Пороги должны соответствовать ожиданиям playbook:

- Увеличение `soranet_privacy_circuit_events_total{kind="downgrade"}` > 0 за 10 минут вызывает `critical`.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` > 5 за 30 минут вызывает `warning`.
- `up{job="soranet-relay"}` == 0 в течение 2 минут вызывает `critical`.

Загрузите правила в Alertmanager с receiver `testnet-t0`; проверьте через `amtool check-config`.

## Оценка метрик

Агрегируйте snapshot за 14 дней и передайте его валидатору SNNet-10:

```
cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json --out metrics-report.json
```

- Замените sample файл на свой экспортированный snapshot при запуске на живых данных.
- Результат `status = fail` блокирует продвижение; исправьте отмеченные проверки перед повтором.

## Отчетность

Каждую неделю загружайте:

- Снимки запросов (`.png` или `.pdf`), показывающие PQ ratio, успешность circuit и гистограмму решения PoW.
- Выход recording rule Prometheus для `soranet_privacy_throttles_per_minute`.
- Краткое описание сработавших алертов и шагов по смягчению (включая timestamps).
