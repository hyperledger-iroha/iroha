---
lang: he
direction: rtl
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# רשימת רשימת פרסום

השתמש ברשימת בדיקה זו בכל פעם שאתה מעדכן את פורטל המפתחים. זה מבטיח כי
בניית CI, פריסת דפי GitHub ומבחני עשן ידניים מכסים כל סעיף
לפני יציאת שחרור או מפת דרכים נוחתת.

## 1. אימות מקומי

- `npm run sync-openapi -- --version=current --latest` (הוסף אחד או יותר
  `--mirror=<label>` מסמן כאשר Torii OpenAPI משתנה עבור תמונת מצב קפואה).
- `npm run build` - אשר את עותק הגיבור של `Build on Iroha with confidence`
  מופיע ב-`build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - אמת את
  מניפסט checksum (הוסף `--descriptor`/`--archive` בעת בדיקת CI שהורד
  חפצי אמנות).
- `npm run serve` - משיק את עוזר התצוגה המקדימה של checksum-gated אשר מאמת
  המניפסט לפני התקשרות ל-`docusaurus serve`, כך שהבודקים לעולם אינם גולשים ב-
  תמונת מצב לא חתומה (הכינוי `serve:verified` נשאר עבור שיחות מפורשות).
- בדוק נקודתית את הסימון שבו נגעת דרך `npm run start` והטעינה מחדש בשידור חי
  שרת.

## 2. משוך בדיקות בקשה

- ודא שעבודת `docs-portal-build` הצליחה ב-`.github/workflows/check-docs.yml`.
- אשר את הפעלת `ci/check_docs_portal.sh` (יומני CI מראים את בדיקת העשן הגיבור).
- ודא שזרימת העבודה המקדימה העלתה מניפסט (`build/checksums.sha256`) ו
  סקריפט אימות התצוגה המקדימה הצליח (יומני CI מראים את
  פלט `scripts/preview_verify.sh`).
- הוסף את כתובת האתר של התצוגה המקדימה שפורסמה מסביבת דפי GitHub ל-PR
  תיאור.

## 3. חתימת מדור

| מדור | בעלים | רשימת תיוג |
|--------|-------|--------|
| דף הבית | DevRel | עיבוד עותק גיבור, כרטיסי התחלה מהירה מקשרים למסלולים חוקיים, כפתורי CTA פותרים. |
| Norito | Norito WG | סקירה כללית ומדריכי תחילת העבודה מתייחסים לדגלי ה-CLI האחרונים ולמסמכי סכימת Norito. |
| SoraFS | צוות אחסון | Quickstart פועל עד לסיומו, שדות דוח מניפסט מתועדים, אחזור הוראות סימולציה מאומתות. |
| מדריכי SDK | SDK לידים | מדריכי Rust/Python/JS מרכיבים את הדוגמאות הנוכחיות ומקשרים למחזרים חיים. |
| הפניה | Docs/DevRel | אינדקס מפרט את המפרט החדש ביותר, Norito הפניה ל-codec תואמת ל-`norito.md`. |
| תצוגה מקדימה של חפץ | Docs/DevRel | חפץ `docs-portal-preview` מצורף ל-PR, בדיקת עשן עובר, קישור משותף עם סוקרים. |
| אבטחה ונסה את זה ארגז חול | Docs/DevRel · אבטחה | התחברות לקוד התקן OAuth מוגדרת (`DOCS_OAUTH_*`), רשימת הבדיקה `security-hardening.md` בוצעה, כותרות CSP/Trusted Types אומתו באמצעות `npm run build` או `npm run probe:portal`. |

סמן כל שורה כחלק מסקירת יחסי הציבור שלך, או שים לב למשימות המשך כלשהן, כך שסטטוס
המעקב נשאר מדויק.

## 4. הערות שחרור

- כלול את `https://docs.iroha.tech/` (או את כתובת האתר של הסביבה
  מעבודת הפריסה) בהערות שחרור ועדכוני סטטוס.
- קרא במפורש כל קטע חדש או שהשתנה כדי שצוותים במורד הזרם ידעו היכן
  להריץ מחדש את בדיקות העשן שלהם.