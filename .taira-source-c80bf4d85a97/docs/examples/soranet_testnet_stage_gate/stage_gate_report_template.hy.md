---
lang: hy
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 Stage-Gate հաշվետվություն (T?_→T?_)

> Փոխարինեք յուրաքանչյուր տեղապահ (կետերը անկյունային փակագծերում) նախքան ներկայացնելը: Պահել
> բաժնի վերնագրերը, որպեսզի կառավարման ավտոմատացումը կարողանա վերլուծել ֆայլը:

## 1. Մետատվյալներ

| Դաշտային | Արժեք |
|-------|-------|
| Խթանում | `<T0→T1 or T1→T2>` |
| Հաշվետվության պատուհան | `<YYYY-MM-DD → YYYY-MM-DD>` |
| Ռելեներ տիրույթում | `<count + IDs or “see appendix A”>` |
| Առաջնային կապ | `<name/email/Matrix handle>` |
| Ներկայացման արխիվ | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Արխիվ SHA-256 | `<sha256:...>` |

## 2. Չափումների ամփոփում

| Մետրական | Դիտարկված | շեմ ​​| Անձնագիր? | Աղբյուր |
|--------|---------|-----------|-------|--------|
| Շղթայի հաջողության գործակիցը | `<0.000>` | ≥0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| Բեռնել խզման հարաբերակցությունը | `<0.000>` | ≤0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| ԳԱՐ խառնուրդի շեղում | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 վայրկյան | `<0.0 s>` | ≤3 վ | ☐ / ☑ | `telemetry/pow_window.json` |
| Լատենտություն p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ հարաբերակցությունը (միջին) | `<0.00>` | ≥ թիրախ | ☐ / ☑ | `telemetry/pq_summary.json` |

**Պատմություն.** `<summaries of anomalies, mitigations, overrides>`

## 3. Հորատման և միջադեպերի մատյան

| Ժամացույց (UTC) | Մարզ | Տեսակ | Զգուշացման ID | Մեղմացման ամփոփագիր |
|---------------------------|-----|----------|-------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Հավելվածներ և հեշեր

| Արտեֆակտ | Ճանապարհ | SHA-256 |
|----------|------|---------|
| Չափման ակնարկ | `reports/metrics-window.json` | `<sha256>` |
| Չափման հաշվետվություն | `reports/metrics-report.json` | `<sha256>` |
| Պահակների ռոտացիայի վերծանումներ | `evidence/guard_rotation/*.log` | `<sha256>` |
| Ելքի կապը դրսևորվում է | `evidence/exit_bonds/*.to` | `<sha256>` |
| Հորատման գերաններ | `evidence/drills/*.md` | `<sha256>` |
| ԴԻՄԱԿԱՆ պատրաստվածություն (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Հետադարձ պլան (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Հաստատումներ

| Դերը | Անունը | Ստորագրված (Y/N) | Ծանոթագրություններ |
|------|------|--------------|-------|
| Ցանցային TL | `<name>` | ☐ / ☑ | `<comments>` |
| Կառավարման ներկայացուցիչ | `<name>` | ☐ / ☑ | `<comments>` |
| SRE պատվիրակ | `<name>` | ☐ / ☑ | `<comments>` |

## Հավելված Ա — Էստաֆետային ցուցակ

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Հավելված Բ — Միջադեպերի ամփոփումներ

```
<Detailed context for any incidents or overrides referenced above.>
```