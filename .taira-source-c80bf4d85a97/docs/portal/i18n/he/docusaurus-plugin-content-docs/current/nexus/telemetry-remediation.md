---
lang: he
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-telemetry-remediation
title: תוכנית שיקום טלמטריה של Nexus (B2)
description: מראה של `docs/source/nexus_telemetry_remediation_plan.md`, המתעד מטריצת פערי טלמטריה וזרימת עבודה תפעולית.
---

# סקירה

פריט הרודמפ **B2 - בעלות על פערי טלמטריה** מחייב תוכנית מפורסמת הקושרת כל פער טלמטריה שנותר ב-Nexus לאות, guardrail להתראה, בעלים, מועד יעד וארטיפקט אימות לפני תחילת חלונות הביקורת של Q1 2026. עמוד זה משקף את `docs/source/nexus_telemetry_remediation_plan.md` כדי ש-release engineering, telemetry ops ובעלי ה-SDK יאשרו כיסוי לפני חזרות routed-trace ו-`TRACE-TELEMETRY-BRIDGE`.

# מטריצת פערים

| מזהה פער | אות ו-guardrail להתראה | בעלים / הסלמה | יעד (UTC) | ראיות ואימות |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | היסטוגרמה `torii_lane_admission_latency_seconds{lane_id,endpoint}` עם התראה **`SoranetLaneAdmissionLatencyDegraded`** שמופעלת כאשר `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` למשך 5 דקות (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (אות) + `@telemetry-ops` (התראה); הסלמה דרך on-call routed-trace של Nexus. | 2026-02-23 | בדיקות התראה תחת `dashboards/alerts/tests/soranet_lane_rules.test.yml` יחד עם תיעוד חזרת `TRACE-LANE-ROUTING` המציגה התראה שנדלקה/התאוששה ו-scrape של Torii `/metrics` בארכיון [Nexus transition notes](./nexus-transition-notes). |
| `GAP-TELEM-002` | מונה `nexus_config_diff_total{knob,profile}` עם guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` שחוסם פריסות (`docs/source/telemetry.md`). | `@nexus-core` (אינסטרומנטציה) -> `@telemetry-ops` (התראה); קצין תורנות ממשלתי מקבל דף כשמונה עולה באופן לא צפוי. | 2026-02-26 | פלטי dry-run של ממשל נשמרים ליד `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; רשימת הבקרה של השחרור כוללת צילום שאילתת Prometheus יחד עם קטע לוג שמוכיח ש-`StateTelemetry::record_nexus_config_diff` הפיק את ההפרש. |
| `GAP-TELEM-003` | אירוע `TelemetryEvent::AuditOutcome` (מדד `nexus.audit.outcome`) עם התראה **`NexusAuditOutcomeFailure`** כאשר כשלונות או תוצאות חסרות נמשכים יותר מ-30 דקות (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) בהסלמה ל-`@sec-observability`. | 2026-02-27 | שער CI `scripts/telemetry/check_nexus_audit_outcome.py` מאחסן payloads NDJSON ונכשל כאשר חלון TRACE חסר אירוע הצלחה; צילומי התראות מצורפים לדוח routed-trace. |
| `GAP-TELEM-004` | Gauge `nexus_lane_configured_total` עם guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` שמזין את רשימת הבקרה של SRE on-call. | `@telemetry-ops` (gauge/export) בהסלמה ל-`@nexus-core` כאשר צמתים מדווחים על גדלי קטלוג לא עקביים. | 2026-02-28 | בדיקת טלמטריית המתזמן `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` מוכיחה פליטה; מפעילים מצרפים diff של Prometheus + קטע לוג של `StateTelemetry::set_nexus_catalogs` לחבילת חזרת TRACE. |

# תהליך עבודה תפעולי

1. **מיון שבועי.** בעלי המשימה מדווחים על התקדמות בשיחת מוכנות Nexus; חסמים וארטיפקטי בדיקות התראה נרשמים ב-`status.md`.
2. **הרצות התראה.** כל כלל התראה נשלח יחד עם רשומת `dashboards/alerts/tests/*.test.yml` כדי ש-CI יריץ `promtool test rules` בכל שינוי guardrail.
3. **ראיות ביקורת.** במהלך חזרות `TRACE-LANE-ROUTING` ו-`TRACE-TELEMETRY-BRIDGE` התורן לוכד תוצאות שאילתות Prometheus, היסטוריית התראות ופלטי סקריפטים רלוונטיים (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` עבור אותות קשורים) ושומר אותם עם ארטיפקטי routed-trace.
4. **הסלמה.** אם guardrail מופעל מחוץ לחלון חזרה, הצוות האחראי פותח כרטיס תקרית Nexus שמפנה לתוכנית זו, כולל snapshot של המדד ושלבי המיתון לפני חידוש ביקורות.

עם פרסום מטריצה זו - ובהפניה אל `roadmap.md` ו-`status.md` - פריט roadmap **B2** עומד כעת בקריטריוני הקבלה "אחריות, מועד יעד, התראה, אימות".
