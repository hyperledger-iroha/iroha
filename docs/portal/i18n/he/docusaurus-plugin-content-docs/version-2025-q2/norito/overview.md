---
lang: he
direction: rtl
source: docs/portal/docs/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito סקירה כללית

Norito היא שכבת ההסדרה הבינארית המשמשת על פני Iroha: היא מגדירה כיצד נתונים
מבנים מקודדים על החוט, מוחזקים בדיסק ומוחלפים ביניהם
חוזים ומארחים. כל ארגז בסביבת העבודה מסתמך על Norito במקום
`serde` כך שעמיתים בחומרה שונה מייצרים בתים זהים.

סקירה כללית זו מסכמת את חלקי הליבה וקישורים להפניות הקנוניות.

## אדריכלות במבט חטוף

- **כותרת + מטען** - כל הודעת Norito מתחילה במשא ומתן על תכונה
  header (דגלים, checksum) ואחריו המטען החשוף. פריסות עמוסות ו
  הדחיסה מתבצעת באמצעות סיביות כותרת.
- **קידוד דטרמיניסטי** - `norito::codec::{Encode, Decode}` ליישם את
  קידוד חשוף. באותה פריסה נעשה שימוש חוזר בעת עטיפת מטענים בכותרות כך
  הגיבוב והחתימה נשארים דטרמיניסטיים.
- **סכימה + נובעת** - `norito_derive` יוצר `Encode`, `Decode`, ו
  יישומי `IntoSchema`. מבנים/רצפים ארוזים מופעלים כברירת מחדל
  ומתועד ב-`norito.md`.
- **רישום Multicodec** - מזהים עבור hashes, סוגי מפתחות ומטען
  המתארים חיים ב-`norito::multicodec`. הטבלה הסמכותית היא
  מתוחזק ב-`multicodec.md`.

## כלי עבודה

| משימה | פקודה / API | הערות |
| --- | --- | --- |
| בדוק כותרת/סעיפים | `ivm_tool inspect <file>.to` | מציג גרסת ABI, דגלים ונקודות כניסה. |
| קידוד/פענוח ב-Rust | `norito::codec::{Encode, Decode}` | מיושם עבור כל סוגי מודל הנתונים המרכזיים. |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | JSON דטרמיניסטי מגובה על ידי ערכי Norito. |
| צור מסמכים/מפרטים | `norito.md`, `multicodec.md` | תיעוד מקור האמת בשורש הריפו. |

## זרימת עבודה לפיתוח

1. **הוסף נגזרות** - העדיפו `#[derive(Encode, Decode, IntoSchema)]` לנתונים חדשים
   מבנים. הימנע מתכשירי סדרות בכתב יד אלא אם כן הכרחי.
2. **אמת פריסות ארוזות** - השתמש ב-`cargo test -p norito` (ובארוזים
   מטריצת תכונה ב-`scripts/run_norito_feature_matrix.sh`) כדי להבטיח חדש
   פריסות נשארות יציבות.
3. **צור מחדש מסמכים** - כאשר הקידוד משתנה, עדכן את `norito.md` ואת
   טבלת multicodec, ולאחר מכן רענן את דפי הפורטל (`/reference/norito-codec`
   והסקירה הכללית הזו).
4. **שמור את הבדיקות Norito-first** - בדיקות האינטגרציה צריכות להשתמש ב-Norito JSON
   עוזרים במקום `serde_json` כך שהם מפעילים את אותם נתיבים כמו ייצור.

## קישורים מהירים

- מפרט: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- הקצאות מולטי-קודקים: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- סקריפט מטריצת תכונה: `scripts/run_norito_feature_matrix.sh`
- דוגמאות לפריסה ארוזה: `crates/norito/tests/`

שידוך סקירה כללית זו עם מדריך ההתחלה המהירה (`/norito/getting-started`) עבור א
הדרכה מעשית של קומפילציה והרצת bytecode המשתמשת ב-Norito
מטענים.