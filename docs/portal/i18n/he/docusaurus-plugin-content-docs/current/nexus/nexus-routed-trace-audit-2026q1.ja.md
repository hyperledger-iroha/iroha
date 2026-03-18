---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cf84b1b7c844155c4fa2e25dfb8162bc0f0de387414dae94c97f16850349fe3
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-routed-trace-audit-2026q1
title: דוח ביקורת routed-trace לרבעון Q1 2026 (B1)
description: מראה של `docs/source/nexus_routed_trace_audit_report_2026q1.md`, המכסה את תוצאות חזרות הטלמטריה הרבעוניות.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note מקור קנוני
עמוד זה משקף את `docs/source/nexus_routed_trace_audit_report_2026q1.md`. שמרו על יישור שתי הגרסאות עד להגעת התרגומים הנותרים.
:::

# דוח ביקורת Routed-Trace לרבעון Q1 2026 (B1)

פריט הרודמפ **B1 - Routed-Trace Audits & Telemetry Baseline** דורש סקירה רבעונית של תוכנית routed-trace של Nexus. דוח זה מתעד את חלון הביקורת Q1 2026 (ינואר-מרץ) כדי שמועצת הממשל תאשר את מצב הטלמטריה לפני חזרות ההשקה של Q2.

## היקף ולוח זמנים

| Trace ID | חלון (UTC) | מטרה |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | אימות היסטוגרמות קבלת lane, gossip של תורים וזרימת התראות לפני הפעלת multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | אימות replay OTLP, פריטי parity של diff bot וקליטת טלמטריה של SDK לקראת אבני דרך AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | אישור דלתאות `iroha_config` שאושרו ממשל והיערכות rollback לפני חיתוך RC1. |

כל חזרה רצה בטופולוגיה דמוית פרודקשן עם אינסטרומנטציה routed-trace מופעלת (טלמטריה `nexus.audit.outcome` + מונים של Prometheus), כללי Alertmanager טעונים, והוכחות שיוצאו ל-`docs/examples/`.

## מתודולוגיה

1. **איסוף טלמטריה.** כל הצמתים הפיקו את האירוע המובנה `nexus.audit.outcome` ואת המדדים הנלווים (`nexus_audit_outcome_total*`). העזר `scripts/telemetry/check_nexus_audit_outcome.py` עקב אחר לוג JSON, אימת את סטטוס האירוע וארכב את ה-payload תחת `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **אימות התראות.** `dashboards/alerts/nexus_audit_rules.yml` וה-harness של הבדיקה שלו הבטיחו שספי רעש ההתראה ותבניות ה-payload נשמרו עקביות. CI מריץ `dashboards/alerts/tests/nexus_audit_rules.test.yml` בכל שינוי; אותם כללים הופעלו ידנית במהלך כל חלון.
3. **לכידת דשבורדים.** המפעילים ייצאו את פאנלי routed-trace מתוך `dashboards/grafana/soranet_sn16_handshake.json` (בריאות handshake) ואת דשבורדי סקירת הטלמטריה כדי לקשר את בריאות התורים לתוצאות הביקורת.
4. **הערות מבקרים.** מזכירת הממשל רשמה את ראשי התיבות של הסוקרים, ההחלטה וכרטיסי המיתון ב-[Nexus transition notes](./nexus-transition-notes) וב-tracker של דלתאות קונפיג (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## ממצאים

| Trace ID | תוצאה | ראיות | הערות |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | צילומי התראות fire/recover (קישור פנימי) + replay של `dashboards/alerts/tests/soranet_lane_rules.test.yml`; דיפי טלמטריה נרשמו ב-[Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 קבלת תורים נשאר 612 ms (יעד <=750 ms). אין המשך נדרש. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | payload מאורכב `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` יחד עם hash של replay OTLP שנרשם ב-`status.md`. | salts של redaction ב-SDK התאימו לבסיס Rust; diff bot דיווח על אפס דלתאות. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | רשומה ב-tracker הממשל (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest של פרופיל TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest של חבילת הטלמטריה (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | הריצה מחדש ב-Q2 גיבבה את פרופיל ה-TLS המאושר ואישרה אפס מתעכבים; manifest הטלמטריה מתעד טווח slots 912-936 ו-workload seed `NEXUS-REH-2026Q2`. |

כל ה-traces הפיקו לפחות אירוע `nexus.audit.outcome` אחד בתוך החלונות שלהם, מה שעומד ב-guardrails של Alertmanager (`NexusAuditOutcomeFailure` נשאר ירוק לאורך הרבעון).

## משימות המשך

- נספח routed-trace עודכן עם hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; המיתון `NEXUS-421` נסגר ב-transition notes.
- להמשיך לצרף replays גולמיים של OTLP וארטיפקטי diff של Torii לארכיון כדי לחזק ראיות פריטי parity לבדיקות Android AND4/AND7.
- לאשר שהחזרות הבאות `TRACE-MULTILANE-CANARY` משתמשות באותו helper טלמטריה כדי שחתימת Q2 תיהנה מהתהליך המאומת.

## אינדקס ארטיפקטים

| נכס | מיקום |
|-------|----------|
| מאמת טלמטריה | `scripts/telemetry/check_nexus_audit_outcome.py` |
| כללי התראה ובדיקות | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Payload outcome לדוגמה | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker של דלתאות קונפיג | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| לוח routed-trace והערות | [Nexus transition notes](./nexus-transition-notes) |

דוח זה, הארטיפקטים לעיל ויצואי התראות/טלמטריה צריכים להיות מצורפים ליומן החלטות הממשל כדי לסגור את B1 לרבעון.
