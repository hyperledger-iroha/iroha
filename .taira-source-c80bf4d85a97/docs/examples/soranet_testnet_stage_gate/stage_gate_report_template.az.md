---
lang: az
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 Mərhələ Qapısı Hesabatı (T?_→T?_)

> Təqdim etməzdən əvvəl hər bir yertutanı (bucaqlı mötərizədə olan elementlər) dəyişdirin. Saxla
> bölmə başlıqları beləliklə idarəetmənin avtomatlaşdırılması faylı təhlil edə bilsin.

## 1. Metadata

| Sahə | Dəyər |
|-------|-------|
| Tanıtım | `<T0→T1 or T1→T2>` |
| Hesabat pəncərəsi | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Əhatə dairəsində relelər | `<count + IDs or “see appendix A”>` |
| Əsas əlaqə | `<name/email/Matrix handle>` |
| Təqdimat arxivi | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Arxiv SHA-256 | `<sha256:...>` |

## 2. Metriklərin xülasəsi

| Metrik | Müşahidə | Həddi | Keçmək? | Mənbə |
|--------|----------|-----------|-------|--------|
| Dövrə müvəffəqiyyət nisbəti | `<0.000>` | ≥0,95 | ☐ / ☑ | `reports/metrics-report.json` |
| Qəbul nisbəti | `<0.000>` | ≤0,01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR qarışıq variasiyası | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 saniyə | `<0.0 s>` | ≤3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| Gecikmə p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ nisbəti (ortalama) | `<0.00>` | ≥ hədəf | ☐ / ☑ | `telemetry/pq_summary.json` |

**Hekayə:** `<summaries of anomalies, mitigations, overrides>`

## 3. Qazma və insident jurnalı

| Vaxt möhürü (UTC) | Region | Növ | Xəbərdarlıq ID | Təsirlərin azaldılması xülasəsi |
|----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Qoşmalar və hashlər

| Artefakt | Yol | SHA-256 |
|----------|------|---------|
| Metrik snapshot | `reports/metrics-window.json` | `<sha256>` |
| Metrik hesabat | `reports/metrics-report.json` | `<sha256>` |
| Mühafizə fırlanma stenoqramları | `evidence/guard_rotation/*.log` | `<sha256>` |
| Çıxış bonding təzahürləri | `evidence/exit_bonds/*.to` | `<sha256>` |
| Qazma qeydləri | `evidence/drills/*.md` | `<sha256>` |
| MASQA hazırlığı (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Geri qaytarma planı (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Təsdiqlər

| Rol | Adı | İmzalı (Y/N) | Qeydlər |
|------|------|-------------|-------|
| Şəbəkə TL | `<name>` | ☐ / ☑ | `<comments>` |
| İdarəetmə nümayəndəsi | `<name>` | ☐ / ☑ | `<comments>` |
| SRE nümayəndəsi | `<name>` | ☐ / ☑ | `<comments>` |

## Əlavə A — Relay siyahısı

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Əlavə B — Hadisələrin xülasəsi

```
<Detailed context for any incidents or overrides referenced above.>
```