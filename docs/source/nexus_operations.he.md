<!-- Hebrew translation for docs/source/nexus_operations.md -->

---
lang: he
direction: rtl
source: docs/source/nexus_operations.md
status: draft
translator: LLM (Codex)
---

# רנבוק תפעולי ל‑Nexus (NX-14)

**קישור לרודמפ:** NX-14 — תיעוד ורנבוקים למפעילי Nexus  
**סטטוס:** נוסח 24.03.2026 — מסונכרן עם `docs/source/nexus_overview.md` וזרימות הקליטה ב־`docs/source/sora_nexus_operator_onboarding.md`.  
**קהל יעד:** מפעילי רשת, מהנדסי SRE/On-call ומתאמי ממשל.

הרנבוק מסכם את מחזור החיים התפעולי של צמתים ב‑Sora Nexus (Iroha 3). הוא אינו מחליף את הספציפיקציה המלאה (`docs/source/nexus.md`) או מדריכים ייעודיים לנתיבים (למשל `docs/source/cbdc_lane_playbook.md`), אלא מרכז את הצ'ק‑ליסטים, חיבורי הטלמטריה ודרישות האסמכתא שנדרשים לפני הקמה, שדרוג או שחזור.

## 1. מחזור חיים תפעולי

| שלב | צ'ק-ליסט | אסמכתא |
|------|-----------|---------|
| **Pre-flight** | אימות חתימות/גיבובי ארטיפקטים, וידוא `profile = "iroha3"` והכנת תבניות קונפיגורציה. | פלט `scripts/select_release_profile.py`, לוג checksums וחבילת מניפסט חתומה. |
| **יישור קטלוג** | עדכון קטלוג הנתיבים וה‑DS תחת `[nexus]`, כולל מדיניות ניתוב ורף DA, בהתאם למניפסט המאושר. | פלט `irohad --sora --config … --trace-config` שמצורף לטיקט. |
| **Smoke & Cutover** | הרצת `irohad --sora --config … --trace-config`, בדיקות CLI (לדוגמה `FindNetworkStatus`), בדיקת נקודות טלמטריה ואז שליחת בקשת admission. | לוג בדיקות + אישור Alertmanager silence. |
| **Steady state** | ניטור לוחות/התראות, רוטציית מפתחות בקצב הגוברננס, ויישור קונפיגים/רנבוקים עם גרסת המניפסט העדכנית. | פרוטוקול סקירה רבעונית, צילומי מסך, מזהי טיקט לרוטציות. |

הוראות הקליטה המלאות (החלפת מפתחות, דוגמאות למדיניות ניתוב, אימות פרופיל שחרור) נמצאות ב־`docs/source/sora_nexus_operator_onboarding.md`. בכל שינוי ארטיפקט או כלי יש לפעול לפי מסמך זה.

## 2. ניהול שינויים וחיבורי ממשל

1. **עדכוני שחרור**
   - לעקוב אחרי הודעות ב־`status.md` וב־`roadmap.md`.
   - כל PR שחרור חייב לצרף את הצ'ק‑ליסט מה־onboarding.
2. **שינויי מניפסט נתיב**
   - הממשל מפרסם חבילות חתומות דרך Space Directory.
   - המפעילים מאמתים חתימות, מרעננים את הקטלוג ומאחסנים את הקבצים תחת `docs/source/project_tracker/nexus_config_deltas/`.
3. **דלתות קונפיגורציה**
   - כל עריכה של `config/config.toml` דורשת טיקט עם מזהה נתיב וכינוי DS.
   - יש לשמור עותק מצונזר של הקונפיג האפקטיבי בעת הצטרפות או שדרוג.
4. **תרגילי Rollback**
   - פעם ברבעון: עצירה, שחזור חבילה קודמת, יישום קונפיג מחדש והרצת בדיקות עישון. לתעד ב־`docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **אישורי ציות**
   - שינויי מדיניות DA או אנונימיזציה בנתיבי CBDC/פרטיים מחייבים אישור ציות לפי `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. טלמטריה וכיסוי SLO

לוחות המחוונים וחוקי ההתראה מנוהלים תחת `dashboards/` ומתועדים ב־`docs/source/nexus_telemetry_remediation_plan.md`. חובות המפעיל:

- חיבור PagerDuty/On-call לכללי `dashboards/alerts/nexus_audit_rules.yml` ולחוקי הבריאות של Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- פרסום הלוחות הבאים בפורטל התפעולי:
  - `nexus_lanes.json` – גובה נתיב, צבר DA וחיווי זמינות.
  - `nexus_settlement.json` – השהיית סליקה, דלתות מול האוצר.
  - לוחות SDK (למשל `android_operator_console.json`) כאשר הנתיב תלוי בטלמטריית מובייל.
- שמירה על OTEL exporters תואמי `docs/source/torii/norito_rpc_telemetry.md` כאשר מופעלת תעבורת Norito RPC.
- ביצוע צ'ק‑ליסט תיקון טלמטריה לפחות רבעונית (סעיף 5 במסמך התיקון) והצמדת הטופס המלא לפרוטוקול הסקירה.

### מדדים מרכזיים

| מדד | תיאור | סף התראה |
|------|--------|-----------|
| `nexus_lane_height{lane_id}` | גובה הראש לכל נתיב; מזהה ולידטורים תקועים. | אין התקדמות 3 סלוטים רצופים. |
| `nexus_da_backlog_chunks{lane_id}` | כמות צ'אנקים לא מעובדים. | מעל ברירת המחדל (64 ציבורי / 8 פרטי). |
| `nexus_settlement_latency_seconds{lane_id}` | זמן מהקומיט לנתיב ועד הסליקה הגלובלית. | P99>900 ms ציבורי או >1200 ms פרטי. |
| `torii_request_failures_total{scheme="norito_rpc"}` | ספירת שגיאות Norito RPC. | יחס שגיאות>2 % בחמש דקות. |
| `telemetry_redaction_override_total` | מספר חריגות לאנונימיזציה. | התראה מיידית (Sev 2) ופתיחת טיקט ציות. |

## 4. ניהול אירועים

| חומרה | הגדרה | פעולות נדרשות |
|-------|--------|----------------|
| **Sev 1** | פגיעה בבידוד DS, עצירת סליקה>15 דקות או שחיתות הצבעה. | הזעקת Nexus Primary + Release Engineering + Compliance, הקפאת admission, איסוף מדדים/לוגים, הודעה תוך 60 דקות ו‑RCA תוך 5 ימי עסקים. |
| **Sev 2** | צבר נתיב חורג מה‑SLA, עיוורון טלמטריה>30 דקות, rollout מניפסט כושל. | הזעקת Nexus Primary + SRE, טיפול תוך 4 שעות ותיעוד משימות המשך תוך 2 ימי עסקים. |
| **Sev 3** | רגרסיות לא חוסמות (מסמכים לא מעודכנים, אזעקה שגויה). | רישום בטיקט ותיקון במהלך הספרינט. |

כל טיקט אירוע חייב לכלול:

1. מזהי נתיב/DS מושפעים וה־hash של המניפסט.
2. ציר זמן ב‑UTC: גילוי, בלימה, שחזור ותקשורת.
3. גרפים/צילומי מסך שמבססים את הגילוי.
4. משימות המשך עם בעלים/תאריך ועדכון האם יש צורך בשינויים באוטומציה או ברנבוקים.

## 5. אסמכתאות ושרשרת ביקורת

- **ארכיון ארטיפקטים:** `artifacts/nexus/<lane>/<date>/` עבור חבילות, מניפסטים וייצוא טלמטריה.
- **צילום קונפיגורציה:** קובץ `config.toml` מצונזר + פלט `trace-config` לכל שחרור.
- **קישוריות ממשל:** פרוטוקולי מועצה והחלטות חתומות שמוזכרות בטיקט הקליטה/אירוע.
- **ייצוא טלמטריה:** צילום שבועי של נתוני Prometheus הרלוונטיים לנתיב, נשמר ל‑12 חודשים לפחות.
- **ניהול גרסאות רנבוק:** כל שינוי במסמך זה דורש רשומה ב־`docs/source/project_tracker/nexus_config_deltas/README.md`.

## 6. מקורות קשורים

- `docs/source/nexus_overview.md` — סקירה ארכיטקטונית.
- `docs/source/nexus.md` — הספציפיקציה הטכנית.
- `docs/source/nexus_lanes.md` — גיאומטריית נתיבים.
- `docs/source/nexus_transition_notes.md` — תוכנית המעבר.
- `docs/source/cbdc_lane_playbook.md` — מדיניות נתיבי CBDC.
- `docs/source/sora_nexus_operator_onboarding.md` — תהליך שחרור וקליטה.
- `docs/source/nexus_telemetry_remediation_plan.md` — קווי הגנה לטלמטריה.

יש לעדכן רנבוק זה בכל פעם שפריט הרודמפ NX-14 מתקדם או כאשר מתווספים סוגי נתיבים, כללי טלמטריה או חיבורי ממשל חדשים.
