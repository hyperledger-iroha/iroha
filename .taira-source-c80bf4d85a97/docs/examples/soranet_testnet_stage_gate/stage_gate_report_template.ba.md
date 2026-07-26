---
lang: ba
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 Этап-ҡапҡа отчеты (Т?_→Т?_)

> Ҡабул итеү алдынан һәр урынды (элементтар мөйөшлө йәйәләрҙә) алмаштырығыҙ. Һаҡларға
> бүлек башлыҡтары шулай идара итеү автоматлаштырыу файлды анализлау мөмкин.

## 1. Метадата

| Ялан | Ҡиммәте |
|------|-------|
| Промоушен | `<T0→T1 or T1→T2>` |
| Отчет тәҙрә | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Релелар масштабында | `<count + IDs or “see appendix A”>` |
| Беренсел бәйләнеш | `<name/email/Matrix handle>` |
| Тапшырыу архивы | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Архив ША-256 | I18NI0000007X |

## 2. Метрика резюме

| Метрика | Күҙәтелгән | Эроболь | Үтергә? | Сығанаҡ |
|-------|----------|------------|-------|---------||
| Сюжет уңыш нисбәте | `<0.000>` | ≥0,95 | ☐ / ☑ | `reports/metrics-report.json` |
| Браунут нисбәте | `<0.000>` | ≤0.01 | ☐ / ☑ | I18NI0000011X |
| GAR ҡатнашмаһы дисперсияһы | I18NI0000012X | ≤±10% | ☐ / ☑ | I18NI0000013X |
| PoW p95 секунд | I18NI0000014X | ≤3 с | ☐ / ☑ | `telemetry/pow_window.json` |
| Латентлыҡ p95 | `<0 ms>` | <200 мс | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ нисбәте (вег) | `<0.00>` | ≥ маҡсат | ☐ / ☑ | `telemetry/pq_summary.json` |

**Хикәйәләү:** `<summaries of anomalies, mitigations, overrides>`

## 3. Дрель & инциденттар журналы

| Ваҡыт тамғаһы (UTC) | Төбәк | Тип | Иҫкәртмә идентификаторы | Йомшартыу резюмеһы |
|---------------|------------------------------|------------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Ҡушымталар һәм хеш

| Артефакт | Юл | SHA-256 |
|--------|-------|---------|
| Метрика снимок | `reports/metrics-window.json` | `<sha256>` |
| Метрика отчеты | `reports/metrics-report.json` | `<sha256>` |
| Гвардия әйләнеш стенограммалары | I18NI000000030X | `<sha256>` |
| Сығыу бәйләнеше күренә | `evidence/exit_bonds/*.to` | `<sha256>` |
| Дренаж журналдар | `evidence/drills/*.md` | I18NI000000035X |
| МАСКИЯ әҙерлеге (Т1→Т2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Rollback планы (Т1→Т2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Раҫлаусылар

| Роль | Исем | Ҡултамға (Й/Н) | Иҫкәрмәләр |
|-----|------|--------------|-------|
| Селтәрле TL | `<name>` | ☐ / ☑ | `<comments>` |
| Идара итеү реп | `<name>` | ☐ / ☑ | `<comments>` |
| SRE делегаты | `<name>` | ☐ / ☑ | `<comments>` |

## Ҡушымта А — Реле исемлеге

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Ҡушымта В — Инцидент резюме

```
<Detailed context for any incidents or overrides referenced above.>
```