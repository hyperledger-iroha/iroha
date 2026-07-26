---
lang: kk
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 Stage-Gate Report (T?_→T?_)

> Жібермес бұрын әрбір толтырғышты (бұрыштық жақшадағы элементтер) ауыстырыңыз. Сақтау
> бөлім тақырыптары, сондықтан басқаруды автоматтандыру файлды талдай алады.

## 1. Метадеректер

| Өріс | Мән |
|-------|-------|
| жылжыту | `<T0→T1 or T1→T2>` |
| Есеп беру терезесі | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Ауқымдағы релелер | `<count + IDs or “see appendix A”>` |
| Негізгі байланыс | `<name/email/Matrix handle>` |
| Жіберу мұрағаты | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Мұрағат SHA-256 | `<sha256:...>` |

## 2. Көрсеткіштер жиыны

| метрикалық | Байқалған | Табалдырық | Өту? | Дереккөз |
|--------|----------|-----------|-------|--------|
| Схема сәттілігінің коэффициенті | `<0.000>` | ≥0,95 | ☐ / ☑ | `reports/metrics-report.json` |
| Қою коэффициентін алу | `<0.000>` | ≤0,01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR қоспасының дисперсиясы | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 секунд | `<0.0 s>` | ≤3 с | ☐ / ☑ | `telemetry/pow_window.json` |
| Кідіріс p95 | `<0 ms>` | <200 мс | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ қатынасы (орташа) | `<0.00>` | ≥ мақсат | ☐ / ☑ | `telemetry/pq_summary.json` |

**Баяндама:** `<summaries of anomalies, mitigations, overrides>`

## 3. Бұрғылау және оқиғалар журналы

| Уақыт белгісі (UTC) | Аймақ | |түрі Ескерту идентификаторы | Жеңілдетуге арналған қорытынды |
|----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Қосымшалар мен хэштер

| Артефакт | Жол | SHA-256 |
|----------|------|---------|
| Көрсеткіштердің суреті | `reports/metrics-window.json` | `<sha256>` |
| Көрсеткіштер туралы есеп | `reports/metrics-report.json` | `<sha256>` |
| Күзет ротациясының транскрипттері | `evidence/guard_rotation/*.log` | `<sha256>` |
| Шығу байланысы | `evidence/exit_bonds/*.to` | `<sha256>` |
| Бұрғылау журналдары | `evidence/drills/*.md` | `<sha256>` |
| МАСҚА дайындығы (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Қайтару жоспары (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Бекітулер

| Рөл | Аты | Қол қойылған (Ж/Ж) | Ескертпелер |
|------|------|--------------|-------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Басқару өкілі | `<name>` | ☐ / ☑ | `<comments>` |
| SRE делегаты | `<name>` | ☐ / ☑ | `<comments>` |

## А қосымшасы — Эстафеталық тізім

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## В қосымшасы — Оқиғаның қорытындылары

```
<Detailed context for any incidents or overrides referenced above.>
```