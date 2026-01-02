<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: he
direction: rtl
source: docs/source/nexus_overview.md
status: complete
translator: LLM (Codex)
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי עבור docs/source/nexus_overview.md -->

# סקירת Nexus והקשר למפעילים

**קישור לרודמפ:** NX-14 — תיעוד ורנבוקים למפעילי Nexus  
**סטטוס:** נוסח 24.03.2026 (תואם את `docs/source/nexus_operations.md`)  
**קהל יעד:** מנהלי תוכנית, מהנדסי תפעול וצוותי שותפים הזקוקים לתקציר של ארכיטקטורת Sora Nexus (Iroha 3) לפני קריאה במסמכי העומק (`docs/source/nexus.md`, `docs/source/nexus_lanes.md`, `docs/source/nexus_transition_notes.md`).

## 1. קווי שחרור וכלים משותפים

- **Iroha 2** נשאר הקו המיועד לפריסות קונסורציום/On‑prem עצמאיות.
- **Iroha 3 / Sora Nexus** מוסיף ביצוע רב־נתיבי, Data Spaces ומודל ממשל משותף. אותו ריפו, כלי build ו־CI בונים גם את Iroha 2 וגם את Nexus, ולכן תיקונים ב־IVM, בקומפיילר Kotodama או ב־SDKים זמינים לשני הקווים בו־זמנית.
- **ארטיפקטים:** חבילות `iroha3-<version>-<os>.tar.zst` ותמונות OCI כוללות בינאריים, קונפיגים לדוגמה ומטא־דאטה של פרופיל Nexus. פעלו לפי `docs/source/sora_nexus_operator_onboarding.md` בעת אימות הארכיון מקצה לקצה.
- **ממשק SDK משותף:** SDKים ל־Rust‏/Python‏/JS/TS‏/Swift‏/Android צורכים את אותם סכימות Norito ואת אותן דוגמאות כתובות (`fixtures/account/address_vectors.json`), כך שארנקים ואוטומציה יכולים לעבור בין רשתות Iroha 2 ל־Nexus בלי התאמות פורמט.

## 2. אבני הבניין הארכיטקטוניות

| רכיב | תיאור | מקורות מרכזיים |
|------|-------|----------------|
| **Data Space (DS)** | תחום ביצוע שמוגדר על ידי הממשל: קובע חברות ולידטורים, רמת פרטיות, מדיניות עמלות ופרופיל DA. כל DS מחזיק נתיב אחד או יותר. | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | שבריר דטרמיניסטי של מצב וביצוע. מניפסט הנתיב מציין ולידטורים, ווים לסליקה, מטא־דאטה לטלמטריה והרשאות ניתוב. טבעת הקונצנזוס הגלובלית מסדרת את קומיטמנטים הנתיב. | `docs/source/nexus_lanes.md` |
| **Space Directory** | חוזה ו־CLI ששומרים מניפסטים, רוטציות ולידטורים ומסמכי יכולות. ההיסטוריה חתומה כדי שאודיטורים יוכלו לשחזר מצב. | `docs/source/nexus.md#space-directory` |
| **Lane Catalog** | החלק `[nexus]` בקובץ `config.toml` שממפה מזהי נתיבים לכינויים, מדיניות ניתוב ופרמטרי שמירה. ניתן להדפיס את הקטלוג הפעיל עם `irohad --sora --config … --trace-config`. | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | מנתב תנועות XOR בין נתיבים (לדוגמה בין נתיבי CBDC פרטיים לבין נתיבי נזילות ציבוריים). ערכי ברירת המחדל מתועדים ב־`docs/source/cbdc_lane_playbook.md`. | `docs/source/cbdc_lane_playbook.md` |
| **טלמטריה ו‑SLO** | לוחות המחוונים וסט חוקי ההתראה ב־`dashboards/grafana/nexus_*.json` מכסים גובה נתיב, צבר DA, השהיית סליקה ועומק תורי ממשל. תוכנית התיקון מפורטת ב־`docs/source/nexus_telemetry_remediation_plan.md`. | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### מחלקות נתיבים ו־Data Spaces

- `default_public`: נתיבים ציבוריים לגמרי תחת בקרה של פרלמנט סורה.
- `public_custom`: נתיבים שקופים עם כלכלה מותאמת.
- `private_permissioned`: נתיבי CBDC/קונסורציום שמפרסמים רק קומיטמנטים והוכחות.
- `hybrid_confidential`: נתיבים שמשלבים הוכחות אפס־ידע עם חשיפה סלקטיבית.

כל נתיב מצהיר על:

1. **מניפסט נתיב:** מטא־דאטה חתומה שמנוהלת ב־Space Directory.
2. **מדיניות זמינות נתונים:** פרמטרי קידוד מחיקות, תהליכי שחזור ודרישות ביקורת.
3. **פרופיל טלמטריה:** לוחות מחוונים ורנבוקים לאיש משמרת; חובה לעדכן בכל שינוי גוברננס.

## 3. ציר זמן של ההשקה

| שלב | מוקד | קריטריון יציאה |
|------|------|----------------|
| **N0 – בטא סגורה** | רשם מנוהל ע"י המועצה, סיומת `.sora` בלבד, קליטת מפעילים ידנית. | מניפסטים חתומים, קטלוג נתיבים קבוע, תרגילי גוברננס מתועדים. |
| **N1 – השקה ציבורית** | מוסיף `.nexus`, מכרזים ורשם בשירות עצמי. הסליקה מתחברת לאוצר ה‑XOR. | בדיקות סינכרון רזולבר/שער ירוקות, לוחות חיוב חיים, תרגיל דיספיוט הושלם. |
| **N2 – הרחבה** | מוסיף `.dao`, ממשקי ריסיילר, אנליטיקה, פורטל תלונות ודו"חות ציון. | ארטיפקטי ציות בגרסאות, ערכת Policy Jury פעילה, דו"חות שקיפות אוצר מפורסמים. |
| **שער NX-12/13/14** | מנוע ציות, טלמטריה ודוקומנטציה חייבים לנחות יחד לפני פתיחת פיילוט השותפים. | `docs/source/nexus_overview.md` ו־`docs/source/nexus_operations.md` פורסמו, לוחות מחוונים מחוברים להתראות, ומנוע המדיניות משולב בגוברננס. |

## 4. אחריות המפעיל

| אחריות | תיאור | אסמכתא |
|---------|-------|--------|
| היגיינת קונפיגורציה | שמירה על `config/config.toml` מסונכרן עם קטלוג הנתיבים וה‑DS שפורסם; תיעוד הדלתות. | פלט `irohad --sora --config … --trace-config` בארכיון השחרור. |
| מעקב מניפסט | ניטור עדכוני Space Directory וריענון המטמונים/רשימות המורשות. | חבילת מניפסט חתומה מחוברת לטיקט הקליטה. |
| כיסוי טלמטריה | הבטחת גישה ללוחות שבסעיף 2, חיבור ההתראות ל‑PagerDuty ותיעוד סקירות רבעוניות. | פרוטוקול סקירת משמרת + יצוא Alertmanager. |
| דיווח תקלות | שימוש במטריצת החומרה ב־`docs/source/nexus_operations.md` והגשת דו"ח תוך 5 ימי עסקים. | תבנית דו"ח לפי מזהה אירוע. |
| מוכנות גוברננס | השתתפות בהצבעות המועצה כשהמדיניות נוגעת לפריסה, ותרגול הוראות חירום פעם ברבעון. | נוכחות ותיאורי תרגול תחת `docs/source/project_tracker/nexus_config_deltas/`. |

## 5. מסמכים קשורים

- **מסמך עומק:** `docs/source/nexus.md`
- **גאומטריית נתיבים ואחסון:** `docs/source/nexus_lanes.md`
- **תוכנית מעבר וניטוב זמני:** `docs/source/nexus_transition_notes.md`
- **קליטת מפעילים:** `docs/source/sora_nexus_operator_onboarding.md`
- **מדיניות נתיבי CBDC וסליקה:** `docs/source/cbdc_lane_playbook.md`
- **תוכנית תיקון טלמטריה:** `docs/source/nexus_telemetry_remediation_plan.md`
- **רנבוק/ניהול אירועים:** `docs/source/nexus_operations.md`

יש לעדכן את הסקירה יחד עם פריט הרודמפ NX-14 בכל פעם שמתווספים סוגי נתיבים חדשים, תזרימי ממשל או שינויים מהותיים במסמכי הייחוס.

</div>
