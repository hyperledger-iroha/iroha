---
lang: he
direction: rtl
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md -->

# דוח Stage-Gate של SNNet-10 (T?_ -> T?_)

> החליפו כל placeholder (פריטים בתוך סוגריים משולשים) לפני ההגשה. שמרו על כותרות הסעיפים כדי שאוטומציית הממשל תוכל לקרוא את הקובץ.

## 1. מטאדאטה

| שדה | ערך |
|------|------|
| קידום | `<T0->T1 או T1->T2>` |
| חלון דיווח | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| relays בתחום | `<count + IDs או "ראו נספח A">` |
| איש קשר ראשי | `<name/email/Matrix handle>` |
| ארכיון הגשה | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| SHA-256 לארכיון | `<sha256:...>` |

## 2. סיכום מדדים

| מדד | נצפה | סף | עבר? | מקור |
|------|------|----|------|------|
| יחס הצלחת מעגלים | `<0.000>` | >=0.95 | ☐ / ☑ | `reports/metrics-report.json` |
| יחס brownout ל-fetch | `<0.000>` | <=0.01 | ☐ / ☑ | `reports/metrics-report.json` |
| סטיית mix של GAR | `<+0.0%>` | <=+/-10% | ☐ / ☑ | `reports/metrics-report.json` |
| שניות PoW p95 | `<0.0 s>` | <=3 s | ☐ / ☑ | `telemetry/pow_window.json` |
| השהיה p95 | `<0 ms>` | <200 ms | ☐ / ☑ | `telemetry/latency_window.json` |
| יחס PQ (ממוצע) | `<0.00>` | >= target | ☐ / ☑ | `telemetry/pq_summary.json` |

**נרטיב:** `<summaries of anomalies, mitigations, overrides>`

## 3. יומן drill ותקריות

| חותמת זמן (UTC) | אזור | סוג | מזהה התראה | תקציר מיתון |
|-----------------|------|-----|------------|-------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. מצורפים ו-hashes

| ארטיפקט | נתיב | SHA-256 |
|---------|------|---------|
| Snapshot מדדים | `reports/metrics-window.json` | `<sha256>` |
| דוח מדדים | `reports/metrics-report.json` | `<sha256>` |
| תמלילי guard rotation | `evidence/guard_rotation/*.log` | `<sha256>` |
| manifests של exit bonding | `evidence/exit_bonds/*.to` | `<sha256>` |
| לוגי drill | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| תוכנית rollback (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. אישורים

| תפקיד | שם | חתום (כן/לא) | הערות |
|-------|-----|--------------|-------|
| Networking TL | `<name>` | ☐ / ☑ | `<comments>` |
| Governance rep | `<name>` | ☐ / ☑ | `<comments>` |
| SRE delegate | `<name>` | ☐ / ☑ | `<comments>` |

## נספח A - רשימת relays

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## נספח B - סיכומי תקריות

```
<Detailed context for any incidents or overrides referenced above.>
```

</div>
