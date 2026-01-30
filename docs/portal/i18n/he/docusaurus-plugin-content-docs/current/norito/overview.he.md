---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e1bd3b5102498088da1dcf75eadfa62b20cb29ce73343e74a2659c93fe5ab87
source_last_modified: "2025-11-04T12:03:18.017835+00:00"
translation_last_reviewed: 2026-01-30
---

# סקירה כללית של Norito

Norito היא שכבת סריאליזציה בינארית המשמשת בכל Iroha: היא מגדירה כיצד מבני נתונים מקודדים על הרשת, נשמרים בדיסק ומוחלפים בין חוזים ומארחים. כל crate במרחב העבודה נשען על Norito במקום `serde` כדי שעמיתים על חומרה שונה יפיקו בתים זהים.

סקירה זו מסכמת את החלקים המרכזיים ומקשרת להפניות הקנוניות.

## ארכיטקטורה במבט מהיר

- **כותרת + payload** – כל הודעת Norito נפתחת בכותרת מו"מ על יכולות (flags, checksum) ואחריה ה-payload הגולמי. פריסות ארוזות ודחיסה מתואמות דרך ביטי הכותרת.
- **קידוד דטרמיניסטי** – `norito::codec::{Encode, Decode}` מממשים את הקידוד הבסיסי. אותו layout משמש גם בעת עטיפת payloads בכותרות כך שהגיבוב והחתימה נשארים דטרמיניסטיים.
- **סכמה + derives** – `norito_derive` מייצר מימושי `Encode`, `Decode` ו-`IntoSchema`. מבנים/רצפים ארוזים מופעלים כברירת מחדל ומתועדים ב-`norito.md`.
- **רשומת multicodec** – המזהים של hashes, סוגי מפתחות ותיאורי payload נמצאים ב-`norito::multicodec`. הטבלה הרשמית מתוחזקת ב-`multicodec.md`.

## כלים

| משימה | פקודה / API | הערות |
| --- | --- | --- |
| בדיקת כותרת/סעיפים | `ivm_tool inspect <file>.to` | מציג גרסת ABI, flags ו-entrypoints. |
| קידוד/דקידוד ב-Rust | `norito::codec::{Encode, Decode}` | מיושם לכל סוגי data model המרכזיים. |
| interop JSON | `norito::json::{to_json_pretty, from_json}` | JSON דטרמיניסטי שמבוסס על ערכי Norito. |
| יצירת docs/specs | `norito.md`, `multicodec.md` | תיעוד מקור אמת בשורש הריפו. |

## תהליך פיתוח

1. **הוספת derives** – העדיפו `#[derive(Encode, Decode, IntoSchema)]` עבור מבני נתונים חדשים. הימנעו מסריאליזרים ידניים אלא אם יש הכרח מוחלט.
2. **אימות layouts ארוזים** – השתמשו ב-`cargo test -p norito` (ובמטריצת packed features שב-`scripts/run_norito_feature_matrix.sh`) כדי לוודא שה-layouts החדשים נשארים יציבים.
3. **חידוש docs** – כאשר הקידוד משתנה, עדכנו את `norito.md` ואת טבלת multicodec, ואז רעננו את דפי הפורטל (`/reference/norito-codec` וסקירה זו).
4. **שמירה על Norito-first בבדיקות** – בדיקות אינטגרציה צריכות להשתמש בעזרי ה-JSON של Norito במקום `serde_json` כדי להפעיל את אותם נתיבים של פרודקשן.

## קישורים מהירים

- מפרט: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- הקצאות multicodec: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- סקריפט מטריצת features: `scripts/run_norito_feature_matrix.sh`
- דוגמאות layout ארוז: `crates/norito/tests/`

שלבו את הסקירה הזו עם מדריך ההתחלה המהירה (`/norito/getting-started`) למסלול מעשי של קומפילציה והרצת bytecode שמשתמש ב-payloads של Norito.
