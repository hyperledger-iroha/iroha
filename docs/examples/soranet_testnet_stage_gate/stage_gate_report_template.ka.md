---
lang: ka
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 Stage-Gate ანგარიში (T?_→T?_)

> ჩაანაცვლეთ ყველა ჩანაცვლება (პუნქტები კუთხური ფრჩხილებში) გაგზავნამდე. შეინახეთ
> განყოფილების სათაურები, რათა მართვის ავტომატიზაციამ შეძლოს ფაილის გაანალიზება.

## 1. მეტამონაცემები

| ველი | ღირებულება |
|-------|-------|
| ხელშეწყობა | `<T0→T1 or T1→T2>` |
| ანგარიშის ფანჯარა | `<YYYY-MM-DD → YYYY-MM-DD>` |
| რელეები მოცულობით | `<count + IDs or “see appendix A”>` |
| პირველადი კონტაქტი | `<name/email/Matrix handle>` |
| წარდგენის არქივი | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| არქივი SHA-256 | `<sha256:...>` |

## 2. მეტრიკის შეჯამება

| მეტრული | დაკვირვებული | ბარიერი | საშვი? | წყარო |
|--------|---------|-----------|-------|--------|
| მიკროსქემის წარმატების კოეფიციენტი | `<0.000>` | ≥0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| ამოღება ბრუნვის კოეფიციენტი | `<0.000>` | ≤0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR mix variance | `<+0.0%>` | ≤±10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 წამი | `<0.0 s>` | ≤3 წმ | ☐ / ☑ | `telemetry/pow_window.json` |
| შეყოვნება p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ თანაფარდობა (საშ.) | `<0.00>` | ≥ სამიზნე | ☐ / ☑ | `telemetry/pq_summary.json` |

**მოთხრობა:** `<summaries of anomalies, mitigations, overrides>`

## 3. საბურღი და ინციდენტების ჟურნალი

| დროის ანაბეჭდი (UTC) | რეგიონი | ტიპი | გაფრთხილების ID | შერბილების რეზიუმე |
|-----------------|-------|-----|---------|-------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. დანართები და ჰეშები

| არტეფაქტი | ბილიკი | SHA-256 |
|----------|------|---------|
| Metrics Snapshot | `reports/metrics-window.json` | `<sha256>` |
| მეტრიკის ანგარიში | `reports/metrics-report.json` | `<sha256>` |
| გვარდიის როტაციის ჩანაწერები | `evidence/guard_rotation/*.log` | `<sha256>` |
| გასასვლელი შემაკავშირებელი გამოიხატება | `evidence/exit_bonds/*.to` | `<sha256>` |
| საბურღი მორები | `evidence/drills/*.md` | `<sha256>` |
| MASQUE მზადყოფნა (T1→T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| დაბრუნების გეგმა (T1→T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. დამტკიცებები

| როლი | სახელი | ხელმოწერილი (Y/N) | შენიშვნები |
|------|------|--------------|-------|
| ქსელის TL | `<name>` | ☐ / ☑ | `<comments>` |
| მმართველობის წარმომადგენელი | `<name>` | ☐ / ☑ | `<comments>` |
| SRE დელეგატი | `<name>` | ☐ / ☑ | `<comments>` |

## დანართი A — სარელეო სია

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## დანართი B — ინციდენტის შეჯამება

```
<Detailed context for any incidents or overrides referenced above.>
```