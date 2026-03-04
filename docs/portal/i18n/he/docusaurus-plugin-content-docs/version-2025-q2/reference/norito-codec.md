---
lang: he
direction: rtl
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8de31f9e066b729fda8324b8847badba23de926888574d02a44fb0e6d4472f77
source_last_modified: "2026-01-18T05:31:56+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito הפניה ל-Codec

Norito היא שכבת הסריאליזציה הקנונית של Iroha. כל הודעה על חוט, בדיסק
עומס, וממשק API חוצה רכיבים משתמש ב-Norito כך שהצמתים מסכימים על בתים זהים
גם כשהם פועלים על חומרה שונה. עמוד זה מסכם את החלקים הנעים
ומצביע על המפרט המלא ב-`norito.md`.

## פריסת ליבה

| רכיב | מטרה | מקור |
| --- | --- | --- |
| **כותרת** | מסגרות מטענים עם hash של קסם/גרסה/סכמה, CRC64, אורך ותג דחיסה; v1 דורש `VERSION_MINOR = 0x00` ומאמת את דגלי הכותרת מול המסכה הנתמכת (ברירת המחדל `0x00`). | `norito::header` — ראה `norito.md` ("כותרת ודגלים", שורש מאגר) |
| **מטען חשוף** | קידוד ערך דטרמיניסטי המשמש לגיבוב/השוואה. הובלה על חוט משתמשת תמיד בכותרת; בתים חשופים הם פנימיים בלבד. | `norito::codec::{Encode, Decode}` |
| **דחיסה** | Zstd אופציונלי (והאצת GPU ניסיוני) שנבחרה באמצעות בייט דחיסת הכותרת. | `norito.md`, "משא ומתן דחיסה" |

רישום דגל הפריסה (packed-struct, packed-seq, field bitset, compact
longs) חי ב-`norito::header::flags`. ברירת המחדל של V1 היא דגלים `0x00` אבל
מקבל דגלי כותרת מפורשים בתוך המסכה הנתמכת; ביטים לא ידועים הם
נדחה. `norito::header::Flags` נשמר לבדיקה פנימית ו
גרסאות עתידיות.

## קבל תמיכה

`norito_derive` נשלחים ל-`Encode`, `Decode`, `IntoSchema` ו-JSON עוזר נגזרת.
מוסכמות מפתח:

- נגזרות מייצרות גם AoS וגם נתיבי קוד ארוזים; ברירת המחדל של v1 היא AoS
  פריסה (דגלים `0x00`) אלא אם כן דגלי כותרת יבחרו בגרסאות עמוסות.
  היישום מתקיים ב-`crates/norito_derive/src/derive_struct.rs`.
- תכונות המשפיעות על פריסה (`packed-struct`, `packed-seq`, `compact-len`) הן
  הצטרפות באמצעות דגלי כותרת וחייב להיות מקודד/פענוח באופן עקבי בין עמיתים.
- עוזרי JSON (`norito::json`) מספקים JSON דטרמיניסטי עם גיבוי Norito עבור
  ממשקי API פתוחים. השתמש ב-`norito::json::{to_json_pretty, from_json}` - לעולם לא ב-`serde_json`.

## מולטי-קודקים וטבלאות מזהים

Norito שומר את הקצאות המולטי-קודקים שלו ב-`norito::multicodec`. ההתייחסות
טבלה (hashes, סוגי מפתחות, מתארי מטען) נשמרת ב-`multicodec.md`
בשורש המאגר. כאשר מוסיפים מזהה חדש:

1. עדכון `norito::multicodec::registry`.
2. הרחב את הטבלה ב-`multicodec.md`.
3. צור מחדש קישורים במורד הזרם (Python/Java) אם הם צורכים את המפה.

## חידוש מסמכים ומתקנים

כאשר הפורטל מארח כעת סיכום פרוזה, השתמש ב-Markdown במעלה הזרם
מקורות כמקור האמת:

- **מפרט**: `norito.md`
- **שולחן מולטי-קודק**: `multicodec.md`
- **אמות מידה**: `crates/norito/benches/`
- **מבחני זהב**: `crates/norito/tests/`

כאשר האוטומציה Docusaurus תעלה לאוויר, הפורטל יעודכן באמצעות
סקריפט סינכרון (במעקב ב-`docs/portal/scripts/`) שמושך את הנתונים מאלה
קבצים. עד אז, השאר את הדף הזה מיושר באופן ידני בכל פעם שהמפרט משתנה.