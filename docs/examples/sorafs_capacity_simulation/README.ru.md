---
lang: ru
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-11-05T17:59:15.481814+00:00"
translation_last_reviewed: 2026-01-01
---

# Набор для симуляции емкости SoraFS

Этот каталог содержит воспроизводимые artefacts для симуляции рынка емкости SF-2c. Набор упражняет переговоры по квотам, обработку failover и ремедиацию slashing с использованием production CLI helpers и легкого скрипта анализа.

## Предварительные требования

- Rust toolchain, способный запускать `cargo run` для членов workspace.
- Python 3.10+ (только стандартная библиотека).

## Быстрый старт

```bash
# 1. Сгенерировать канонические CLI artefacts
./run_cli.sh ./artifacts

# 2. Аггрегировать результаты и вывести метрики Prometheus
./analyze.py --artifacts ./artifacts
```

Скрипт `run_cli.sh` вызывает `sorafs_manifest_stub capacity` для построения:

- Детерминированных объявлений провайдеров для набора fixtures переговоров по квотам.
- Порядка репликации, соответствующего сценарию переговоров.
- Снимков телеметрии для окна failover.
- Dispute payload, фиксирующего запрос на slashing.

Скрипт записывает Norito bytes (`*.to`), base64 payloads (`*.b64`), Torii request bodies
и читаемые summaries (`*_summary.json`) в выбранный каталог artefacts.

`analyze.py` потребляет сгенерированные summaries, производит агрегированный отчет
(`capacity_simulation_report.json`) и выпускает Prometheus textfile
(`capacity_simulation.prom`), содержащий:

- `sorafs_simulation_quota_*` gauges, описывающие согласованную емкость и долю
  распределения по провайдерам.
- `sorafs_simulation_failover_*` gauges, выделяющие deltas downtime и выбранного
  провайдера замены.
- `sorafs_simulation_slash_requested`, записывающий процент ремедиации из dispute payload.

Импортируйте Grafana bundle из `dashboards/grafana/sorafs_capacity_simulation.json`
и укажите datasource Prometheus, который сканирует сгенерированный textfile (например,
через textfile collector node-exporter). Runbook в
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` описывает полный workflow,
включая советы по настройке Prometheus.

## Fixtures

- `scenarios/quota_negotiation/` — спецификации объявлений провайдеров и порядок репликации.
- `scenarios/failover/` — окна телеметрии для основного сбоя и подъема failover.
- `scenarios/slashing/` — спецификация спора, ссылающаяся на тот же порядок репликации.

Эти fixtures проверяются в `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`,
чтобы гарантировать синхронизацию со схемой CLI.
