---
lang: he
direction: rtl
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T15:38:30.660147+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! העברת תצורת SM

# העברת תצורת SM

הפעלת מערך התכונות של SM2/SM3/SM4 דורש יותר מאשר קומפילציה עם
דגל תכונה `sm`. צמתים משדרים את הפונקציונליות מאחורי השכבות
`iroha_config` פרופילים ומצפים שהמניפסט בראשית ישא התאמה
ברירות מחדל. הערה זו תופסת את זרימת העבודה המומלצת בעת קידום של
רשת קיימת מ-"Ed25519-only" ל-"SM-enabled".

## 1. אמת את פרופיל הבנייה

- קומפל את הקבצים הבינאריים עם `--features sm`; הוסף `sm-ffi-openssl` רק כאשר אתה
  מתכננים לממש את נתיב התצוגה המקדימה של OpenSSL/Tongsuo. בונה ללא `sm`
  תכונה דחיית חתימות `sm2` במהלך הקבלה גם אם התצורה מאפשרת
  אותם.
- אשר ש-CI מפרסם את חפצי האמנות `sm` ושכל שלבי האימות (`מטען
  test -p iroha_crypto --features sm`, גופי אינטגרציה, fuzz suites) לעבור
  על הקבצים הבינאריים המדויקים שאתה מתכוון לפרוס.

## 2. עקיפות של תצורת שכבה

`iroha_config` מחיל שלוש שכבות: `defaults` → `user` → `actual`. שלח את ה-SM
עוקפים בפרופיל `actual` שהמפעילים מפיצים לאימות ול
השאר את `user` ב-Ed25519 בלבד כדי שברירות המחדל של המפתחים יישארו ללא שינוי.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

העתק את אותו בלוק למניפסט `defaults/genesis` באמצעות `kagami genesis
צור …` (add `--חתימה מותרת sm2 --default-hash sm3-256` אם אתה צריך
עוקפים) כך שהחסימה `parameters` והמטא נתונים שהוזרקו מסכימים עם
תצורת זמן ריצה. עמיתים מסרבים להתחיל כאשר המניפסט והתצורה
צילומי מצב מתפצלים.

## 3. חידוש גילויי בראשית

- הפעל את `kagami genesis generate --consensus-mode <mode>` עבור כל
  הסביבה ולבצע את ה-JSON המעודכן לצד עקיפות ה-TOML.
- חתום על המניפסט (`kagami genesis sign …`) והפצת מטען `.nrt`.
  צמתים שמבצעים אתחול ממניפסט JSON לא חתום גוזרים את קריפטו של זמן הריצה
  תצורה ישירות מהקובץ - עדיין כפופה לאותה עקביות
  המחאות.

## 4. אימות לפני תנועה

- אספקת אשכול ביניים עם הקבצים הבינאריים והתצורה החדשים, ולאחר מכן אמת:
  - `/status` חושף את `crypto.sm_helpers_available = true` לאחר הפעלה מחדש של עמיתים.
  - הכניסה ל-Torii עדיין דוחה חתימות SM2 בעוד ש-`sm2` נעדר מ-
    `allowed_signing` ומקבל אצוות Ed25519/SM2 מעורבות כאשר הרשימה
    כולל את שני האלגוריתמים.
  - `iroha_cli tools crypto sm2 export …` מעביר חומר מפתח הלוך ושוב שנזרע באמצעות החדש
    ברירות מחדל.
- הפעל את תסריטי עשן האינטגרציה המכסים חתימות דטרמיניסטיות SM2 ו
  SM3 hashing לאישור עקביות מארח/VM.

## 5. תוכנית החזרה לאחור- תעד את ההיפוך: הסר את `sm2` מ-`allowed_signing` ושחזר
  `default_hash = "blake2b-256"`. דחף את השינוי דרך אותו `actual`
  צינור פרופיל כך שכל מאמת מתהפך בצורה מונוטונית.
- שמור את גילויי ה-SM בדיסק; עמיתים שרואים לא תואמות תצורה וג'נסיס
  נתונים מסרבים להתחיל, מה שמגן מפני החזרה חלקית.
- אם מעורבת התצוגה המקדימה של OpenSSL/Tongsuo, כלול את השלבים להשבתה
  `crypto.enable_sm_openssl_preview` והסרת האובייקטים המשותפים מה-
  סביבת זמן ריצה.

## חומר עזר

- [`docs/genesis.md`](../../genesis.md) - מבנה גילוי הבראשית ו
  בלוק `crypto`.
- [`docs/source/references/configuration.md`](../references/configuration.md) -
  סקירה כללית של `iroha_config` סעיפים וברירות מחדל.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) - סוף ל
  רשימת ביקורת של מפעילי קצה עבור קריפטוגרפיה של משלוח SM.