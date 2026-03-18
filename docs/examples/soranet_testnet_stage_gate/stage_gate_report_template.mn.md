---
lang: mn
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 Тайзны Хаалганы тайлан (T?_→T?_)

> Илгээхийн өмнө орлуулагч бүрийг (өнцгийн хаалтанд байгаа зүйл) солино уу. Хадгалах
> хэсгийн толгойнууд нь засаглалын автоматжуулалт нь файлыг задлан шинжлэх боломжтой.

## 1. Мета өгөгдөл

| Талбай | Үнэ цэнэ |
|-------|-------|
| Урамшуулал | `<T0→T1 or T1→T2>` |
| Мэдээлэх цонх | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Хамрах хүрээний реле | `<count + IDs or “see appendix A”>` |
| Үндсэн харилцагч | `<name/email/Matrix handle>` |
| Оруулсан архив | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Архив SHA-256 | `<sha256:...>` |

## 2. Метрикийн хураангуй

| Метрик | Ажигласан | Босго | Дамжуулах уу? | Эх сурвалж |
|--------|----------|-----------|-------|--------|
| Хэлхээний амжилтын харьцаа | `<0.000>` | ≥0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| Борлуулах харьцаа | `<0.000>` | ≤0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR хольцын хэлбэлзэл | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 секунд | `<0.0 s>` | ≤3 сек | ☐ / ☑ | `telemetry/pow_window.json` |
| Хоцролт p95 | `<0 ms>` | <200 мс | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ харьцаа (дундаж) | `<0.00>` | ≥ зорилтот | ☐ / ☑ | `telemetry/pq_summary.json` |

**Өгүүлбэр:** `<summaries of anomalies, mitigations, overrides>`

## 3. Өрөмдлөг ба ослын бүртгэл

| Цагийн тэмдэг (UTC) | Бүс | Төрөл | Анхааруулга ID | Зөрчлийг бууруулах тойм |
|-----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Хавсралт ба хэш

| Олдвор | Зам | SHA-256 |
|----------|------|---------|
| Хэмжилтийн агшин зуурын зураг | `reports/metrics-window.json` | `<sha256>` |
| хэмжүүрийн тайлан | `reports/metrics-report.json` | `<sha256>` |
| Харуулын эргэлтийн хуулбар | `evidence/guard_rotation/*.log` | `<sha256>` |
| Гарах бонд манифест | `evidence/exit_bonds/*.to` | `<sha256>` |
| Өрөмдлөгийн бүртгэл | `evidence/drills/*.md` | `<sha256>` |
| Маск бэлэн байдал (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Буцах төлөвлөгөө (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Зөвшөөрөл

| Үүрэг | Нэр | Гарын үсэг зурсан (Y/N) | Тэмдэглэл |
|------|------|--------------|-------|
| Сүлжээний TL | `<name>` | ☐ / ☑ | `<comments>` |
| Засаглалын төлөөлөгч | `<name>` | ☐ / ☑ | `<comments>` |
| SRE төлөөлөгч | `<name>` | ☐ / ☑ | `<comments>` |

## Хавсралт А — Буухианы жагсаалт

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Хавсралт В — Осол явдлын хураангуй

```
<Detailed context for any incidents or overrides referenced above.>
```