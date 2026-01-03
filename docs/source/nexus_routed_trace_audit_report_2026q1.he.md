<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: he
direction: rtl
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/nexus_routed_trace_audit_report_2026q1.md -->

# דוח ביקורת Routed-Trace לרבעון 1 2026 (B1)

פריט מפת הדרכים **B1 — Routed-Trace Audits & Telemetry Baseline** דורש סקירה
רבעונית של תוכנית routed-trace ב-Nexus. דוח זה מתעד את חלון הביקורת של Q1 2026
(ינואר–מרץ) כדי שמועצת הממשל תאשר את עמדת הטלמטריה לפני חזרות ההשקה של Q2.

## היקף ולוח זמנים

| Trace ID | חלון (UTC) | יעד |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | לאמת היסטוגרמות כניסת lanes, gossip של תורים וזרימת התראות לפני הפעלת multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | לאמת OTLP replay, פריטיות diff bot וקליטת טלמטריה SDK לפני אבני הדרך AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | לאשר deltas של `iroha_config` שאושרו בממשל ומוכנות rollback לפני חיתוך RC1. |

כל rehearsal בוצע על טופולוגיה דמוית production עם אינסטרומנטציה routed-trace
פעילה (טלמטריה `nexus.audit.outcome` + מוני Prometheus), כללי Alertmanager
טעונים, וראיות שיוצאות ל-`docs/examples/`.

## מתודולוגיה

1. **איסוף טלמטריה.** כל הצמתים שידרו את האירוע המובנה `nexus.audit.outcome`
   ואת המדדים הנלווים (`nexus_audit_outcome_total*`). הכלי
   `scripts/telemetry/check_nexus_audit_outcome.py` עקב אחר יומן ה-JSON, אימת את סטטוס האירוע
   וארכב את ה-payload תחת `docs/examples/nexus_audit_outcomes/`
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **אימות התראות.** `dashboards/alerts/nexus_audit_rules.yml` וה-harness הבדיקה שלו הבטיחו
   שספי רעש ותבניות payload נשארים עקביים. CI מריץ
   `dashboards/alerts/tests/nexus_audit_rules.test.yml` בכל שינוי; אותם כללים הופעלו ידנית בכל חלון.
3. **לכידת דשבורדים.** מפעילים ייצאו את לוחות routed-trace מתוך
   `dashboards/grafana/soranet_sn16_handshake.json` (בריאות handshake) ואת דשבורדי הטלמטריה
   כדי לקשר בין בריאות התורים לתוצאות הביקורת.
4. **הערות סוקרים.** מזכירות הממשל רשמה ראשי תיבות של סוקרים, החלטה וטיקטים של mitigation
   ב-`docs/source/nexus_transition_notes.md` וב-tracker של config delta
   (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## ממצאים

| Trace ID | תוצאה | ראיות | הערות |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | צילומי fire/recover של התראה (קישור פנימי) + replay של `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diff טלמטריה נרשמו ב-`docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule`. | P95 של queue-admission נשאר 612 ms (יעד <=750 ms). אין צורך במעקב. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | payload מאורכב `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` יחד עם hash של OTLP replay שנרשם ב-`status.md`. | מלחי redaction של ה-SDK תאמו ל-baseline של Rust; diff bot דיווח על אפס deltas. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | רשומה ב-governance tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest של TLS profile (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest של telemetry pack (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | ריצה מחדש ב-Q2 ביצעה hash ל-TLS profile המאושר ואישרה אפס stragglers; manifest הטלמטריה מתעד slot range 912–936 ו-workload seed `NEXUS-REH-2026Q2`. |

כל ה-traces הפיקו לפחות אירוע אחד של `nexus.audit.outcome` בתוך חלונותיהם,
ועמדו ב-guardrails של Alertmanager (`NexusAuditOutcomeFailure` נשאר ירוק לאורך הרבעון).

## המשכים

- עודכן נספח routed-trace עם TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  (ראו `nexus_transition_notes.md`); mitigation `NEXUS-421` נסגר.
- להמשיך לצרף OTLP replays גולמיים וארטיפקטים של Torii diff לארכיון כדי לחזק
  ראיות פריטיות לבדיקות AND4/AND7.
- לוודא שחזרות `TRACE-MULTILANE-CANARY` הבאות משתמשות באותו helper טלמטריה כדי
  שה-sign-off של Q2 יהנה מה-workflow המאומת.

## אינדקס ארטיפקטים

| נכס | מיקום |
|-------|----------|
| מאמת טלמטריה | `scripts/telemetry/check_nexus_audit_outcome.py` |
| כללי התראה ובדיקות | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| payload outcome לדוגמה | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker של config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| לוח routed-trace והערות | `docs/source/nexus_transition_notes.md` |

דוח זה, הארטיפקטים שלעיל ויצואי ההתראה/טלמטריה צריכים להיצמד ללוג החלטות
הממשל כדי לסגור את B1 לרבעון.

</div>
