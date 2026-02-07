---
lang: he
direction: rtl
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2026-01-03T18:07:59.245516+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# נוהל הזמנת מעבדת מכשירי Android (AND6/AND7)

ספר משחק זה מתאר כיצד צוות אנדרואיד מזמין, מאשר ובודק את המכשיר
זמן מעבדה לאבני דרך **AND6** (CI והקשחת תאימות) ו**AND7**
(מוכנות לצפייה). זה משלים את הכניסה למקריים
`docs/source/compliance/android/device_lab_contingency.md` על ידי הבטחת קיבולת
חוסרים נמנעים מלכתחילה.

## 1. מטרות והיקף

- שמור את ה-StrongBox + מאגרי המכשירים הכלליים מעל 80% המחייב מפת הדרכים
  יעד קיבולת לאורך חלונות הקפאה.
- ספק לוח שנה דטרמיניסטי כך ש-CI, אישורים גורפים וכאוס
  החזרות לעולם לא מתחרות על אותה חומרה.
- לכידת שביל שניתן לביקורת (בקשות, אישורים, הערות לאחר ריצה) שמזין
  רשימת התיוג של AND6 ויומן ההוכחות.

הליך זה מכסה את נתיבי ה-Pixel הייעודיים, את הבריכה המשותפת, וכן
שומר המעבדה החיצוני של StrongBox המוזכר במפת הדרכים. אמולטור אד-הוק
השימוש הוא מחוץ לתחום.

## 2. הזמנות חלונות

| בריכה / נתיב | חומרה | אורך חריץ ברירת מחדל | זמן אספקת הזמנה | בעלים |
|-------------|--------|---------------------|------------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4 שעות | 3 ימי עסקים | מוביל מעבדת חומרה |
| `pixel8a-ci-b` | Pixel8a (CI כללי) | 2 שעות | 2 ימי עסקים | Android Foundations TL |
| `pixel7-fallback` | בריכה משותפת Pixel7 | 2 שעות | יום עסקים אחד | הנדסת שחרור |
| `firebase-burst` | תור עשן של מעבדת הבדיקות של Firebase | שעה אחת | יום עסקים אחד | Android Foundations TL |
| `strongbox-external` | מגן מעבדה StrongBox חיצוני | 8 שעות | 7 ימים קלנדריים | מוביל תוכנית |

משבצות מוזמנות ב-UTC; הזמנות חופפות דורשות אישור מפורש
ממנהל מעבדת החומרה.

## 3. בקשת זרימת עבודה

1. **הכן הקשר**
   - עדכון `docs/source/sdk/android/android_strongbox_device_matrix.md` עם
     המכשירים שאתה מתכנן להתאמן ותג המוכנות
     (`attestation`, `ci`, `chaos`, `partner`).
   - אסוף את תמונת המצב העדכנית ביותר של הקיבולת
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **שלח בקשה**
   - הגש כרטיס בתור `_android-device-lab` באמצעות התבנית ב
     `docs/examples/android_device_lab_request.md` (בעלים, תאריכים, עומסי עבודה,
     דרישת החזרה).
   - צרף כל תלות רגולטורית (לדוגמה, סריקה של אישור AND6, AND7
     תרגיל טלמטריה) וקישור לערך מפת הדרכים הרלוונטי.
3. **אישור**
   - חוות דעת של מובילי מעבדת חומרה תוך יום עסקים אחד, מאשרת חריץ ב-
     לוח שנה משותף (`Android Device Lab – Reservations`), ומעדכן את
     `device_lab_capacity_pct` בעמודה
     `docs/source/compliance/android/evidence_log.csv`.
4. **ביצוע**
   - הפעל את העבודות המתוזמנות; רשום מזהי ריצה של Buildkite או יומני כלים.
   - שימו לב לכל סטיות (החלפות חומרה, חריגות).
5. **סגירה**
   - הגיבו על הכרטיס עם חפצים/קישורים.
   - אם ההרצה הייתה קשורה לתאימות, עדכן
     `docs/source/compliance/android/and6_compliance_checklist.md` והוסף שורה
     ל-`evidence_log.csv`.

בקשות שמשפיעות על הדגמות של שותפים (AND8) חייבות להיות ב-cc Partner Engineering.

## 4. שינוי וביטול- **תזמון מחדש:** פתח מחדש את הכרטיס המקורי, הצע משבצת חדשה ועדכן את הכרטיס
  הזנת לוח שנה. אם החריץ החדש נמצא בתוך 24 שעות, שלח ping Hardware Lab Lead + SRE
  ישירות.
- **ביטול חירום:** יש לפעול לפי תוכנית המגירה
  (`device_lab_contingency.md`) ורשום את שורות ההדק/פעולה/מעקב.
- **חריפות:** אם ריצה חורגת מהמשבצת שלה ב->15 דקות, פרסם עדכון ואשר
  האם ניתן להמשיך בהזמנה הבאה; אחרת מסור ל-fallback
  בריכה או נתיב פרץ של Firebase.

## 5. ראיות וביקורת

| חפץ | מיקום | הערות |
|--------|--------|-------|
| הזמנת כרטיסים | תור `_android-device-lab` (Jira) | ייצוא סיכום שבועי; קישור מזהי כרטיסים ביומן ראיות. |
| ייצוא יומן | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | הפעל `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` בכל יום שישי; העוזר שומר את קובץ ה-`.ics` המסונן בתוספת סיכום JSON לשבוע ISO כך שביקורות יכולות לצרף את שני החפצים ללא הורדות ידניות. |
| תמונות קיבולת | `docs/source/compliance/android/evidence_log.csv` | עדכון לאחר כל הזמנה/סגירה. |
| הערות לאחר ריצה | `docs/source/compliance/android/device_lab_contingency.md` (במקרה חירום) או הערה לכרטיס | נדרש לביקורות. |

במהלך סקירות תאימות רבעוניות, צרף את ייצוא היומן, סיכום הכרטיסים,
וקטע יומן ראיות להגשת רשימת ה-AND6.

### אוטומציה של ייצוא יומן

1. השג את כתובת האתר של עדכון ICS (או הורד קובץ `.ics`) עבור "מעבדת מכשירי אנדרואיד - הזמנות".
2. בצע

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   התסריט כותב את שני `artifacts/android/device_lab/<YYYY-WW>-calendar.ics`
   ו-`...-calendar.json`, ללכוד את שבוע ה-ISO שנבחר.
3. העלה את הקבצים שנוצרו עם חבילת ההוכחות השבועית ועיין ב-
   סיכום JSON ב-`docs/source/compliance/android/evidence_log.csv` כאשר
   קיבולת מכשיר רישום-מעבדה.

## 6. סולם הסלמה

1. מוביל מעבדת חומרה (ראשי)
2. Android Foundations TL
3. הנדסת הובלת תוכנית/שחרור (עבור חלונות הקפאה)
4. איש קשר חיצוני למעבדה StrongBox (כאשר ה-Retainer מופעל)

יש לרשום הסלמות בכרטיס ולשקף באנדרואיד השבועי
דואר סטטוס.

## 7. מסמכים קשורים

- `docs/source/compliance/android/device_lab_contingency.md` - יומן אירועים עבור
  מחסור בקיבולת.
- `docs/source/compliance/android/and6_compliance_checklist.md` - מאסטר
  צ'ק רשימת תוצרים.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` - חומרה
  מעקב אחר כיסוי.
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  עדות אישור StrongBox שהתייחסו אליו על ידי AND6/AND7.

שמירה על הליך הזמנה זה עונה על פריט הפעולה של מפת הדרכים "הגדר
הליך הזמנת מכשיר-מעבדה" ושומר על חפצי תאימות מול שותפים
מסונכרן עם שאר תוכנית המוכנות של אנדרואיד.

## 8. נוהל תרגיל ואנשי קשר

פריט מפת הדרכים AND6 מצריך גם חזרה רבעונית לכשל על כשל. המלא,
הוראות שלב אחר שלב חיות
`docs/source/compliance/android/device_lab_failover_runbook.md`, אבל הגבוה
זרימת העבודה ברמה מסוכמת להלן כך שמבקשים יוכלו לתכנן תרגילים לצדם
הזמנות שגרתיות.1. **תזמן את התרגיל:** חסום את הנתיבים המושפעים (`pixel8pro-strongbox-a`,
   מאגר חילופין, `firebase-burst`, מחזיק StrongBox חיצוני) במשותף
   לוח שנה ותור `_android-device-lab` לפחות 7 ימים לפני התרגיל.
2. **לדמות הפסקה:** הסר את הנתיב הראשי, הפעל את ה-PagerDuty
   (`AND6-device-lab`) תקרית, ולציין את העבודות התלויות ב-Buildkite עם
   מזהה התרגיל המצוין בספר ההפעלה.
3. **נכשל:** קדם את נתיב ה-Pixel7, התחל את פרץ Firebase
   חבילה, וערב את השותף החיצוני StrongBox בתוך 6 שעות. לכידת
   כתובות URL להרצת Buildkite, ייצוא Firebase ואישורי שמירה.
4. **אמת ושחזר:** אימות זמני ריצה של אישור + CI, הפעל מחדש את
   נתיבים מקוריים, ועדכון `device_lab_contingency.md` בתוספת יומן ההוכחות
   עם נתיב החבילה + סיכומי ביקורת.

### הפנייה ליצירת קשר והסלמה

| תפקיד | איש קשר ראשי | ערוצים | סדר הסלמה |
|------|----------------|----------------|----------------|
| מוביל מעבדת חומרה | פריה רמנתן | `@android-lab` Slack · +81-3-5550-1234 | 1 |
| Device Lab Ops | מתאו קרוז | תור `_android-device-lab` | 2 |
| Android Foundations TL | אלנה וורובבה | `@android-foundations` Slack | 3 |
| הנדסת שחרור | אלכסיי מורוזוב | `release-eng@iroha.org` | 4 |
| מעבדת StrongBox חיצונית | Sakura Instruments NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

הסלמה ברצף אם המקדחה מגלה בעיות חסימה או אם יש נפילה כלשהי
לא ניתן להעלות את הליין לאינטרנט תוך 30 דקות. רשום תמיד את ההסלמה
הערות בכרטיס `_android-device-lab` ושיקוף אותם ביומן המגירה.