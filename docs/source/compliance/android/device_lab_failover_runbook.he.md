---
lang: he
direction: rtl
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2026-01-03T18:07:59.262670+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ספר ריצת תרגילי כשל במכשירי Android (AND6/AND7)

ספר הפעלה זה לוכד את ההליך, דרישות הראיות ומטריצת הקשר
משמש בעת מימוש **תוכנית המגירה של מעבדת התקן** המוזכרת ב
`roadmap.md` ("אישורי חפצים רגולטוריים ותקרית מעבדה"). זה משלים
זרימת העבודה של ההזמנה (`device_lab_reservation.md`) ויומן האירועים
(`device_lab_contingency.md`) אז בודקי ציות, יועץ משפטי ו-SRE
יש מקור אחד של אמת לאופן שבו אנו מאמתים מוכנות לכשל.

## מטרה וקצב

- הדגימו שמאגרי ה-Android StrongBox + כללי המכשיר עלולים להיכשל
  לנתיבי ה-Pixel החלופיים, לבריכה המשותפת, לתור התפרצות של Firebase Test Lab, ו
  מגן StrongBox חיצוני ללא SLA AND6/AND7 חסרים.
- הפק צרור ראיות שמשפטי יכול לצרף להגשות ETSI/FISC
  לקראת בדיקת התאימות בפברואר.
- הפעל לפחות פעם ברבעון, בתוספת כל פעם שרשימת החומרה של המעבדה משתנה
  (מכשירים חדשים, פרישה או תחזוקה יותר מ-24 שעות).

| מזהה תרגיל | תאריך | תרחיש | צרור עדויות | סטטוס |
|--------|------|--------|----------------|--------|
| DR-2026-02-Q1 | 2026-02-20 | הפסקת נתיב Pixel8Pro מדומה + צבר אישורים עם חזרת טלמטריה AND7 | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ הושלם - חבילות גיבוב מוקלט ב-`docs/source/compliance/android/evidence_log.csv`. |
| DR-2026-05-Q2 | 22-05-2026 (מתוכנן) | חפיפה תחזוקה StrongBox + Nexus חזרה | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(בהמתנה)* — כרטיס `_android-device-lab` **AND6-DR-202605** מחזיק בהזמנות; החבילה תאוכלס לאחר התרגיל. | 🗓 מתוכנן - בלוק לוח שנה נוסף ל-"Android Device Lab - הזמנות" לפי קדנס AND6. |

## נוהל

### 1. הכנה לפני תרגיל

1. אשר את קיבולת הבסיס ב-`docs/source/sdk/android/android_strongbox_capture_status.md`.
2. ייצא את לוח ההזמנות עבור שבוע היעד ISO באמצעות
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. קובץ `_android-device-lab` כרטיס
   `AND6-DR-<YYYYMM>` עם היקף ("תרגיל כשל"), חריצים מתוכננים ומושפעים
   עומסי עבודה (אישור, עשן CI, כאוס טלמטריה).
4. עדכן את תבנית יומן המגירה ב-`device_lab_contingency.md` עם א
   שורת מציין מיקום עבור תאריך התרגיל.

### 2. הדמיית מצבי כשל

1. השבת או הסר את המסלול הראשי (`pixel8pro-strongbox-a`) בתוך המעבדה
   מתזמן ותייגו את רשומת ההזמנה כ"תרגיל".
2. הפעל התראת הפסקה מדומה ב-PagerDuty (שירות `AND6-device-lab`) ו
   ללכוד את ייצוא ההודעות עבור חבילת הראיות.
3. הערות על עבודות Buildkite שצורכות בדרך כלל את הנתיב
   (`android-strongbox-attestation`, `android-ci-e2e`) עם מזהה התרגיל.

### 3. ביצוע כשל1. קדם את מסלול ה-Pixel7 הנלווה ליעד CI ראשי ותזמן את
   עומסי עבודה מתוכננים נגדו.
2. הפעל את חבילת התפרצות של Firebase Test Lab דרך נתיב `firebase-burst` עבור
   בדיקות העשן של ארנק הקמעונאות בזמן שהכיסוי StrongBox עובר למשותף
   נתיב. ללכוד את קריאת ה-CLI (או ייצוא המסוף) בכרטיס לביקורת
   זוגיות.
3. הפעל את מחזיק המעבדה החיצוני של StrongBox לבדיקת אישור קצר;
   יומן אישור איש קשר כמתואר להלן.
4. רשום את כל מזהי הריצה של Buildkite, כתובות האתרים של משימות Firebase ותמלולי השמירה ב-
   כרטיס `_android-device-lab` והמניפסט של חבילת הראיות.

### 4. אימות והחזרה

1. השווה זמני ריצה של אישור/CI מול קו הבסיס; דגל דלתות >10% ל-
   מוביל מעבדת חומרה.
2. שחזר את הנתיב הראשי ועדכן את תמונת הקיבולת בתוספת המוכנות
   מטריצה לאחר שהאימות עובר.
3. הוסף את השורה האחרונה ל-`device_lab_contingency.md` עם טריגר, פעולות,
   ומעקבים.
4. עדכן את `docs/source/compliance/android/evidence_log.csv` עם:
   נתיב חבילה, מניפסט SHA-256, מזהי ריצת Buildkite, Hash לייצוא PagerDuty, ו
   חתימת מבקר.

## פריסת חבילת ראיות

| קובץ | תיאור |
|------|-------------|
| `README.md` | סיכום (מזהה תרגיל, היקף, בעלים, ציר זמן). |
| `bundle-manifest.json` | מפת SHA-256 עבור כל קובץ בחבילה. |
| `calendar-export.{ics,json}` | יומן הזמנות לשבוע ISO מתוך סקריפט הייצוא. |
| `pagerduty/incident_<id>.json` | ייצוא תקרית PagerDuty המציג ציר זמן של התראה + אישור. |
| `buildkite/<job>.txt` | כתובות URL ויומנים להרצת Buildkite עבור עבודות מושפעות. |
| `firebase/burst_report.json` | סיכום ביצוע פרץ של Firebase Test Lab. |
| `retainer/acknowledgement.eml` | אישור ממעבדת StrongBox החיצונית. |
| `photos/` | תמונות/צילומי מסך אופציונליים של טופולוגיית מעבדה אם החומרה הוכנסה לכבלים מחדש. |

אחסן את החבילה ב
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` והקלט
סכום הבדיקה המניפסט בתוך יומן ההוכחות בתוספת רשימת הבדיקה של תאימות AND6.

## מטריצת יצירת קשר והסלמה

| תפקיד | איש קשר ראשי | ערוצים | הערות |
|------|----------------|--------|-------|
| מוביל מעבדת חומרה | פריה רמנתן | `@android-lab` Slack · +81-3-5550-1234 | בעל פעולות באתר ועדכוני לוח שנה. |
| Device Lab Ops | מתאו קרוז | תור `_android-device-lab` | מתאם הזמנת כרטיסים + העלאות חבילות. |
| הנדסת שחרור | אלכסיי מורוזוב | Release Eng Slack · `release-eng@iroha.org` | מאמת עדויות Buildkite + מפרסמת hashes. |
| מעבדת StrongBox חיצונית | Sakura Instruments NOC | `noc@sakura.example` · +81-3-5550-9876 | קשר ריטיינר; אשר זמינות בתוך 6 שעות. |
| מתאם פרץ Firebase | טסה רייט | `@android-ci` Slack | מפעיל אוטומציה של Firebase Test Lab כאשר יש צורך ב-fallback. |

הסלמה בסדר הבא אם תרגיל מגלה בעיות חסימה:
1. מוביל מעבדת חומרה
2. Android Foundations TL
3. מוביל תוכנית / הנדסת שחרור
4. מוביל ציות + ייעוץ משפטי (אם התרגיל מגלה סיכון רגולטורי)

## דיווח ומעקבים- קשר את ספר ההפעלה הזה לצד הליך ההזמנה בכל פעם שאתה מפנה
  מוכנות לכשל ב-`roadmap.md`, `status.md`, ומנות ניהול.
- שלח בדוא"ל את תקציר התרגיל הרבעוני אל Compliance + Legal עם חבילת הראיות
  טבלת hash וצרף את ייצוא הכרטיסים `_android-device-lab`.
- שיקוף מדדי מפתח (זמן עד כשל, עומסי עבודה שוחזרו, פעולות יוצאות דופן)
  בתוך `status.md` ו-AND7 מעקב אחר הרשימה החמה כדי שהבודקים יוכלו לאתר את
  תלות לחזרה קונקרטית.