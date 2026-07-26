---
lang: he
direction: rtl
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 רתמת SLO

שורת השחרור Iroha 3 נושאת SLOs מפורשים עבור הנתיבים הקריטיים של Nexus:

- משך משבצת סופי (NX-18 קצב)
- אימות הוכחה (אישורי התחייבות, אישורי JDG, הוכחות גשר)
- הוכחה לטיפול בנקודת קצה (פרוקסי נתיב Axum באמצעות חביון אימות)
- מסלולי עמלות והימור (תזרימי משלם/נותן ואג"ח/חותך)

## תקציבים

תקציבים חיים ב-`benchmarks/i3/slo_budgets.json` וממפים ישירות לספסל
תרחישים בסוויטת I3. היעדים הם יעדי p99 לכל שיחה:

- עמלה/הימור: 50ms לשיחה (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- אישור התחייבות / JDG / אימות גשר: 80ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- הרכבת אישור התחייבות: 80ms (`commit_cert_assembly`)
- מתזמן גישה: 50ms (`access_scheduler`)
- פרוקסי הוכחה לנקודת קצה: 120ms (`torii_proof_endpoint`)

רמזים לקצב צריבה (`burn_rate_fast`/`burn_rate_slow`) מקודדים את ה-14.4/6.0
יחסי ריבוי חלונות להתראות מול התראות על כרטיסים.

## רתמה

הפעל את הרתמה באמצעות `cargo xtask i3-slo-harness`:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

פלטים:

- `bench_report.json|csv|md` - תוצאות גולמיות של חבילת ספסל I3 (git hash + תרחישים)
- `slo_report.json|md` - הערכת SLO עם יחס מעבר/נכשל/תקציב לכל יעד

הרתמה צורכת את קובץ התקציבים ואוכפת את `benchmarks/i3/slo_thresholds.json`
במהלך ריצת הספסל להיכשל במהירות כאשר מטרה נסוגה.

## טלמטריה ודשבורדים

- סופיות: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- אימות הוכחה: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

לוחות המתנע Grafana חיים ב-`dashboards/grafana/i3_slo.json`. Prometheus
התראות על קצב צריבה מסופקות ב-`dashboards/alerts/i3_slo_burn.yml` עם ה-
תקציבים לעיל אפויים (סופיות 2s, אימות הוכחה 80ms, הוכחת proxy של נקודת קצה
120ms).

## הערות תפעוליות

- הפעל את הרתמה בשידורי לילה; פרסם את `artifacts/i3_slo/<stamp>/slo_report.md`
  לצד הספסל חפצי אומנות לראיות ממשל.
- אם תקציב נכשל, השתמש בסימון הספסל כדי לזהות את התרחיש ולאחר מכן תרגיל
  לתוך הלוח/התראה התואם Grafana כדי לתאם עם מדדים חיים.
- SLOs הוכחה של נקודות קצה משתמשים בהשהיית האימות כפרוקסי כדי להימנע מכל מסלול
  פיצוץ קרדינליות; יעד ההשוואה (120ms) תואם לשמירה/DoS
  מעקות בטיחות ב-API ההוכחה.