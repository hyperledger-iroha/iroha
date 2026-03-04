---
lang: ur
direction: rtl
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md کا اردو ترجمہ -->

# SNNet-10 Stage-Gate رپورٹ (T?_ -> T?_)

> جمع کرانے سے پہلے ہر placeholder (زاویہ بریکٹس میں موجود آئٹمز) بدل دیں۔ سیکشن ہیڈرز برقرار رکھیں تاکہ governance automation فائل کو parse کر سکے.

## 1. Metadata

| فیلڈ | ویلیو |
|------|-------|
| Promotion | `<T0->T1 یا T1->T2>` |
| Reporting window | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| Relays in scope | `<count + IDs یا "appendix A دیکھیں">` |
| Primary contact | `<name/email/Matrix handle>` |
| Submission archive | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Archive SHA-256 | `<sha256:...>` |

## 2. Metrics summary

| Metric | Observed | Threshold | Pass? | Source |
|--------|----------|-----------|-------|--------|
| Circuit success ratio | `<0.000>` | >=0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| Fetch brownout ratio | `<0.000>` | <=0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| GAR mix variance | `<+0.0%>` | <=+/-10% | ☐ / ☑ | `reports/metrics-report.json` |
| PoW p95 seconds | `<0.0 s>` | <=3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| Latency p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| PQ ratio (avg) | `<0.00>` | >= target | ☐ / ☑ | `telemetry/pq_summary.json` |

**Narrative:** `<summaries of anomalies, mitigations, overrides>`

## 3. Drill & incident log

| Timestamp (UTC) | Region | Type | Alert ID | Mitigation summary |
|-----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Attachments and hashes

| Artefact | Path | SHA-256 |
|----------|------|---------|
| Metrics snapshot | `reports/metrics-window.json` | `<sha256>` |
| Metrics report | `reports/metrics-report.json` | `<sha256>` |
| Guard rotation transcripts | `evidence/guard_rotation/*.log` | `<sha256>` |
| Exit bonding manifests | `evidence/exit_bonds/*.to` | `<sha256>` |
| Drill logs | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Rollback plan (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Approvals

| Role | Name | Signed (Y/N) | Notes |
|------|------|--------------|-------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Governance rep | `<name>` | ☐ / ☑ | `<comments>` |
| SRE delegate | `<name>` | ☐ / ☑ | `<comments>` |

## Appendix A - Relay roster

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Appendix B - Incident summaries

```
<Detailed context for any incidents or overrides referenced above.>
```

</div>
