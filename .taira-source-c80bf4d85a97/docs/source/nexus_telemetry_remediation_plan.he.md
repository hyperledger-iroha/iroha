---
lang: he
direction: rtl
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/nexus_telemetry_remediation_plan.md -->

% תוכנית תיקון טלמטריה של Nexus (שלב B2)

# סקירה כללית

פריט מפת הדרכים **B2 - telemetry gap ownership** מחייב תוכנית מפורסמת שמקשרת כל פער טלמטריה פתוח
ב-Nexus לאות, guardrail התראה, בעלים, מועד יעד וארטיפקט אימות לפני תחילת חלונות הביקורת של
Q1 2026. מסמך זה מרכז את המטריצה כדי ש-release engineering, telemetry ops ובעלי SDK יאשרו כיסוי
לפני חזרות routed-trace ו-`TRACE-TELEMETRY-BRIDGE`.

# מטריצת פערים

| מזהה פער | אות ו-guardrail התראה | בעלים / הסלמה | יעד (UTC) | ראיות ואימות |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | היסטוגרמה `torii_lane_admission_latency_seconds{lane_id,endpoint}` עם התראה **`SoranetLaneAdmissionLatencyDegraded`** שנדלקת כאשר `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` למשך 5 דקות (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (אות) + `@telemetry-ops` (התראה) - הסלמה דרך כוננות Nexus routed-trace. | 2026-02-23 | בדיקות התראה ב-`dashboards/alerts/tests/soranet_lane_rules.test.yml` וצילום חזרת `TRACE-LANE-ROUTING` שמציג התראה נדלקת/מתאוששת, לצד scrape של Torii `/metrics` המאוחסן ב-`docs/source/nexus_transition_notes.md`. |
| `GAP-TELEM-002` | מונה `nexus_config_diff_total{knob,profile}` עם guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` שחוסם פריסות (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (התראה) - קצין תורן של ממשל מקבל התראה כאשר המונה עולה באופן לא צפוי. | 2026-02-26 | פלטי dry-run של ממשל נשמרים לצד `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; רשימת הבדיקות לשחרור כוללת צילום שאילתת Prometheus + קטע לוג שמוכיח ש-`StateTelemetry::record_nexus_config_diff` פלט את ה-diff. |
| `GAP-TELEM-003` | אירוע `TelemetryEvent::AuditOutcome` (מדד `nexus.audit.outcome`) עם התראה **`NexusAuditOutcomeFailure`** כאשר כשלונות או תוצאות חסרות נמשכים מעל 30 דקות (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) עם הסלמה ל-`@sec-observability`. | 2026-02-27 | שער CI `scripts/telemetry/check_nexus_audit_outcome.py` מאחסן payloads בפורמט NDJSON ונכשל כאשר חלון TRACE חסר אירוע הצלחה; צילומי התראה מצורפים לדוח routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` במעקב עם guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` (מתועד ב-`docs/source/telemetry.md`) שמזין את צ'ק-ליסט כוננות SRE. | `@telemetry-ops` (gauge/export) עם הסלמה ל-`@nexus-core` כאשר צמתים מדווחים על גדלי קטלוג לא עקביים. | 2026-02-28 | בדיקת טלמטריה של ה-scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` מוכיחה פליטה; מפעילים מצרפים diff של Prometheus וקטע לוג `StateTelemetry::set_nexus_catalogs` לחבילת חזרת TRACE. |

# תקציב יצוא ומגבלות OTLP

- **החלטה (2026-02-11):** להגביל יצואני OTLP ל-**5 MiB/min לכל צומת** או
  **25,000 spans/min**, הנמוך מביניהם, עם גודל batch של 256 spans ו-timeout יצוא של 10 שניות.
  יצוא מעל 80% מהתקרה מפעיל את ההתראה `NexusOtelExporterSaturated` ב-`dashboards/alerts/nexus_telemetry_rules.yml`
  ומייצר אירוע `telemetry_export_budget_saturation` ללוגי ביקורת.
- **אכיפה:** כללי Prometheus קוראים את המונים `iroha.telemetry.export.bytes_total`
  ו-`iroha.telemetry.export.spans_total`; פרופיל ה-OTLP collector מגיע עם אותם גבולות כברירת מחדל,
  ותצורות על הצומת אינן רשאיות להעלות אותם ללא פטור ממשל.
- **ראיות:** וקטורי בדיקות התראה והגבולות המאושרים נשמרים תחת `docs/source/nexus_transition_notes.md`
  לצד ארטיפקטי ביקורת routed-trace. קבלת B2 מתייחסת כעת לתקציב היצוא כאל סגור.

# תהליך תפעולי

1. **מיון שבועי.** בעלי אחריות מדווחים על התקדמות בשיחת Nexus readiness; חסימות וארטיפקטי בדיקות
   התראה נרשמים ב-`status.md`.
2. **Dry-runs של התראות.** כל כלל התראה מגיע עם ערך ב-`dashboards/alerts/tests/*.test.yml` כדי
   ש-CI יריץ `promtool test rules` בכל שינוי של guardrail.
3. **ראיות ביקורת.** במהלך חזרות `TRACE-LANE-ROUTING` ו-`TRACE-TELEMETRY-BRIDGE` הכונן אוסף תוצאות
   שאילתות Prometheus, היסטוריית התראות ופלטי סקריפטים רלוונטיים
   (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py`
   עבור אותות מתואמים) ושומר אותם יחד עם ארטיפקטי routed-trace.
4. **הסלמה.** אם guardrail כלשהו מופעל מחוץ לחלון חזרה, הצוות האחראי פותח כרטיס אירוע Nexus
   שמפנה לתוכנית זו, כולל snapshot מדדים ושלבי מיתון לפני חידוש הביקורות.

עם פרסום מטריצה זו והפניה אליה מ-`roadmap.md` ו-`status.md`, פריט מפת הדרכים **B2** עומד כעת
בקריטריוני הקבלה "אחריות, מועד יעד, התראה, אימות".

</div>
