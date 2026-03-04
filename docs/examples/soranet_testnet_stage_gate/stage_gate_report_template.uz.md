---
lang: uz
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

> Taqdim etishdan oldin har bir joy ushlagichni (burchakdagi qavslardagi elementlar) almashtiring. Saqlash
> bo'lim sarlavhalari, shuning uchun boshqaruvni avtomatlashtirish faylni tahlil qilishi mumkin.

## 1. Metadata

| Maydon | Qiymat |
|-------|-------|
| Rag'batlantirish | `<T0→T1 or T1→T2>` |
| Hisobot oynasi | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Ko'lamdagi o'rni | `<count + IDs or “see appendix A”>` |
| Asosiy aloqa | `<name/email/Matrix handle>` |
| Taqdim etish arxivi | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Arxiv SHA-256 | `<sha256:...>` |

## 2. Ko'rsatkichlar xulosasi

| Metrik | Kuzatilgan | Ostona | O'tishmi? | Manba |
|--------|----------|-----------|-------|--------|
| Devren muvaffaqiyati nisbati | `<0.000>` | ≥0,95 | ☐ / ☑ | `reports/metrics-report.json` |
| Qo'ng'iroq qilish nisbati | `<0.000>` | ≤0,01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR aralashmasi dispersiyasi | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 soniya | `<0.0 s>` | ≤3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| Kechikish p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ nisbati (o'rtacha) | `<0.00>` | ≥ maqsad | ☐ / ☑ | `telemetry/pq_summary.json` |

**Hikoya:** `<summaries of anomalies, mitigations, overrides>`

## 3. Matkap va hodisalar jurnali

| Vaqt tamg'asi (UTC) | Hudud | Tur | Ogohlantirish identifikatori | Yumshatish haqida xulosa |
|----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Qo'shimchalar va xeshlar

| Artefakt | Yo'l | SHA-256 |
|----------|------|---------|
| Koʻrsatkichlar surati | `reports/metrics-window.json` | `<sha256>` |
| Ko'rsatkichlar bo'yicha hisobot | `reports/metrics-report.json` | `<sha256>` |
| Qo'riqchilarning aylanish transkriptlari | `evidence/guard_rotation/*.log` | `<sha256>` |
| Chiqish bog'lanish manifestlari | `evidence/exit_bonds/*.to` | `<sha256>` |
| Burg'ulash jurnallari | `evidence/drills/*.md` | `<sha256>` |
| MASQA tayyorligi (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Orqaga qaytarish rejasi (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Tasdiqlashlar

| Rol | Ism | Imzolangan (Y/N) | Eslatmalar |
|------|------|--------------|-------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Boshqaruv vakili | `<name>` | ☐ / ☑ | `<comments>` |
| SRE delegati | `<name>` | ☐ / ☑ | `<comments>` |

## A ilovasi - Estafeta ro'yxati

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## B ilovasi — Voqealarning xulosalari

```
<Detailed context for any incidents or overrides referenced above.>
```