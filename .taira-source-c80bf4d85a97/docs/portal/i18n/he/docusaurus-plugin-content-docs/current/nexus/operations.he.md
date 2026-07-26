---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 429297c84c8a996c906665e513239cd86c30d5b2164167e61db788e364668296
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/nexus/operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-operations
title: Runbook לתפעול Nexus
description: סיכום תפעולי לשטח של תהליך העבודה למפעיל Nexus, המשקף את `docs/source/nexus_operations.md`.
---

השתמשו בעמוד זה כאח מהיר-עיון ל-`docs/source/nexus_operations.md`. הוא מזקק את רשימת הבדיקה התפעולית, נקודות החיבור לניהול שינויים ודרישות כיסוי הטלמטריה שעל מפעילי Nexus לעקוב אחריהן.

## רשימת בדיקה למחזור חיים

| שלב | פעולות | ראיות |
|-------|--------|----------|
| טרום-טיסה | אימות hash/חתימות של release, אישור `profile = "iroha3"`, והכנת תבניות קונפיגורציה. | פלט `scripts/select_release_profile.py`, יומן checksum, bundle מניפסטים חתום. |
| יישור קטלוג | עדכון קטלוג `[nexus]`, מדיניות ניתוב וספי DA לפי מניפסט המועצה, ואז לכידת `--trace-config`. | פלט `irohad --sora --config ... --trace-config` שמאוחסן עם כרטיס ה-onboarding. |
| Smoke & cutover | להריץ `irohad --sora --config ... --trace-config`, לבצע smoke ב-CLI (`FindNetworkStatus`), לאמת יצואי טלמטריה ולבקש קבלה. | לוג smoke-test + אישור Alertmanager. |
| מצב יציב | לנטר dashboards/התראות, לסובב מפתחות לפי קצב הממשל, ולסנכרן configs/runbooks כאשר מניפסטים משתנים. | פרוטוקולים של סקירה רבעונית, צילומי dashboards, IDs של כרטיסי סבב. |

Onboarding מפורט (החלפת מפתחות, תבניות ניתוב, שלבי פרופיל release) נשאר ב-`docs/source/sora_nexus_operator_onboarding.md`.

## ניהול שינויים

1. **עדכוני release** - לעקוב אחרי הודעות ב-`status.md`/`roadmap.md`; לצרף את רשימת ה-onboarding לכל PR של release.
2. **שינויים במניפסטי lane** - לאמת bundles חתומים מ-Space Directory ולארכב תחת `docs/source/project_tracker/nexus_config_deltas/`.
3. **דלתות קונפיגורציה** - כל שינוי ב-`config/config.toml` דורש כרטיס שמפנה ל-lane/data-space. לשמור עותק מצונזר של הקונפיגורציה האפקטיבית כשצמתים מצטרפים או מתעדכנים.
4. **תרגילי rollback** - לבצע רבעונית תרגול stop/restore/smoke; לתעד תוצאות תחת `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **אישורי תאימות** - lanes פרטיים/CBDC חייבים לקבל אישור תאימות לפני שינוי מדיניות DA או knobs של redaction בטלמטריה (ראו `docs/source/cbdc_lane_playbook.md`).

## טלמטריה ו-SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, בתוספת תצוגות SDK ייעודיות (לדוגמה `android_operator_console.json`).
- Alerts: `dashboards/alerts/nexus_audit_rules.yml` וכללי Torii/Norito transport (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- מדדים למעקב:
  - `nexus_lane_height{lane_id}` - התרעה על אפס התקדמות לשלושה slots.
  - `nexus_da_backlog_chunks{lane_id}` - התרעה מעל ספי lane (ברירת מחדל 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - התרעה כאשר P99 חוצה 900 ms (public) או 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` - התרעה אם יחס שגיאות 5 דקות גדול מ-2%.
  - `telemetry_redaction_override_total` - Sev 2 מיידי; ודאו שיש כרטיסי תאימות עבור overrides.
- הריצו את רשימת הטלמטריה ב-[Nexus telemetry remediation plan](./nexus-telemetry-remediation) לפחות אחת לרבעון והצמידו את הטופס המלא להערות סקירת תפעול.

## מטריצת תקריות

| חומרה | הגדרה | תגובה |
|----------|------------|----------|
| Sev 1 | הפרת בידוד data-space, עצירת settlement מעל 15 דקות, או השחתת הצבעת ממשל. | להפעיל Nexus Primary + Release Engineering + Compliance, להקפיא קבלה, לאסוף ארטיפקטים, לפרסם תקשורת <=60 דקות, RCA <=5 ימי עסקים. |
| Sev 2 | הפרת SLA של backlog ב-lane, נקודת עיוורון בטלמטריה >30 דקות, rollout מניפסט כושל. | להפעיל Nexus Primary + SRE, להקטין <=4 שעות, להגיש מעקבים בתוך יומיים עסקים. |
| Sev 3 | סטייה לא חוסמת (docs, alerts). | לרשום ב-tracker, לתזמן תיקון בתוך הספירנט. |

כרטיסי תקרית חייבים לכלול IDs של lane/data-space שנפגעו, hashes של מניפסטים, ציר זמן, מטריקות/לוגים תומכים, ומשימות/בעלים להמשך.

## ארכיון ראיות

- לאחסן bundles/manifestes/יצואי טלמטריה תחת `artifacts/nexus/<lane>/<date>/`.
- לשמור configs מצונזרים + פלט `--trace-config` לכל release.
- לצרף פרוטוקולי מועצה + החלטות חתומות כאשר חלים שינויי config או מניפסט.
- לשמור snapshots שבועיים של Prometheus הרלוונטיים למטריקות Nexus במשך 12 חודשים.
- לתעד עריכות runbook ב-`docs/source/project_tracker/nexus_config_deltas/README.md` כדי שמבקרים ידעו מתי האחריות השתנתה.

## חומר קשור

- סקירה: [Nexus overview](./nexus-overview)
- מפרט: [Nexus spec](./nexus-spec)
- גיאומטריית lane: [Nexus lane model](./nexus-lane-model)
- מעבר ו-shims של ניתוב: [Nexus transition notes](./nexus-transition-notes)
- Onboarding למפעילים: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Remediation טלמטריה: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
