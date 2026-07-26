---
lang: ru
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

# Отчет о stage-gate SNNet-10 (T?_ -> T?_)

> Замените все placeholder (элементы в угловых скобках) перед отправкой. Сохраните заголовки разделов, чтобы автоматизация governance могла разобрать файл.

## 1. Метаданные

| Поле | Значение |
|------|---------|
| Повышение | `<T0->T1 или T1->T2>` |
| Период отчета | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| Relays в области | `<count + IDs или "см. приложение A">` |
| Основной контакт | `<name/email/Matrix handle>` |
| Архив отправки | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Архив SHA-256 | `<sha256:...>` |

## 2. Сводка метрик

| Метрика | Наблюдаемое | Порог | Прошел? | Источник |
|--------|-------------|-------|--------|---------|
| Доля успеха circuit | `<0.000>` | >=0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| Доля brownout fetch | `<0.000>` | <=0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| Дисперсия mix GAR | `<+0.0%>` | <=+/-10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 в секундах | `<0.0 s>` | <=3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| Латентность p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ ratio (avg) | `<0.00>` | >= target | ☐ / ☑ | `telemetry/pq_summary.json` |

**Нарратив:** `<summaries of anomalies, mitigations, overrides>`

## 3. Журнал drill и инцидентов

| Timestamp (UTC) | Region | Type | Alert ID | Mitigation summary |
|-----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Приложения и хеши

| Артефакт | Путь | SHA-256 |
|----------|------|---------|
| Snapshot метрик | `reports/metrics-window.json` | `<sha256>` |
| Отчет по метрикам | `reports/metrics-report.json` | `<sha256>` |
| Транскрипты guard rotation | `evidence/guard_rotation/*.log` | `<sha256>` |
| Exit bonding manifests | `evidence/exit_bonds/*.to` | `<sha256>` |
| Логи drill | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| План rollback (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Одобрения

| Роль | Имя | Подписано (Y/N) | Примечания |
|------|-----|----------------|------------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Governance rep | `<name>` | ☐ / ☑ | `<comments>` |
| SRE delegate | `<name>` | ☐ / ☑ | `<comments>` |

## Приложение A — Relay roster

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Приложение B — Incident summaries

```
<Detailed context for any incidents or overrides referenced above.>
```
