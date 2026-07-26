---
lang: pt
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e631786283968acdc9b79b675cb7331b346d27b6311f25d93b15a7e49d744661
source_last_modified: "2026-01-18T05:31:56+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# רפרנס לקודק Norito

Norito היא שכבת הסריאליזציה הקנונית של Iroha. כל הודעת on-wire, כל payload על דיסק וכל API בין רכיבים משתמשים ב-Norito כך שהנודים מסכימים על אותם בתים גם כשהם רצים על חומרה שונה. הדף הזה מסכם את החלקים המרכזיים ומפנה למפרט המלא ב-`norito.md`.

## פריסת ליבה

| רכיב | מטרה | מקור |
| --- | --- | --- |
| **Header** | ממסגר payloads עם magic/version/schema hash, CRC64, אורך ותג דחיסה; v1 מחייב `VERSION_MINOR = 0x00` ומאמת header flags מול המסכה הנתמכת (ברירת מחדל `0x00`). | `norito::header` — ראו `norito.md` ("Header & Flags", שורש המאגר) |
| **Payload חשוף** | קידוד ערכים דטרמיניסטי המשמש ל-hashing/השוואה. התעבורה on-wire תמיד משתמשת ב-header; בתים חשופים הם פנימיים בלבד. | `norito::codec::{Encode, Decode}` |
| **דחיסה** | Zstd אופציונלי (והאצה ניסיונית ב-GPU) שנבחר דרך בייט הדחיסה ב-header. | `norito.md`, “Compression negotiation” |

רישום ה-flags של ה-layout (packed-struct, packed-seq, field bitset, compact lengths) נמצא ב-`norito::header::flags`. V1 משתמש ב-flags `0x00` כברירת מחדל אך מקבל flags מפורשים בתוך המסכה הנתמכת; ביטים לא מוכרים נדחים. `norito::header::Flags` נשמר לצורכי בדיקה פנימית ולגרסאות עתידיות.

## תמיכת derive

`norito_derive` מספק derives של `Encode`, `Decode`, `IntoSchema` ועוזרי JSON. עקרונות מרכזיים:

- ה-derives מייצרים מסלולי AoS ו-packed; v1 משתמש בפריסת AoS כברירת מחדל (flags `0x00`) אלא אם header flags בוחרים בגרסאות packed. המימוש נמצא ב-`crates/norito_derive/src/derive_struct.rs`.
- יכולות המשפיעות על ה-layout (`packed-struct`, `packed-seq`, `compact-len`) הן opt-in דרך header flags וחייבות להיות מקודדות/מפוענחות באופן עקבי בין peers.
- עוזרי JSON (`norito::json`) מספקים JSON דטרמיניסטי מגובה Norito ל-API ציבוריים. השתמשו ב-`norito::json::{to_json_pretty, from_json}` — לעולם לא `serde_json`.

## Multicodec וטבלאות מזהים

Norito שומר את שיוכי ה-multicodec ב-`norito::multicodec`. טבלת הייחוס (hashes, סוגי מפתחות, מתארי payload) נשמרת ב-`multicodec.md` בשורש המאגר. כאשר מוסיפים מזהה חדש:

1. עדכנו את `norito::multicodec::registry`.
2. הרחיבו את הטבלה ב-`multicodec.md`.
3. צרו מחדש binding-ים downstream (Python/Java) אם הם צורכים את המפה.

## יצירת docs ו-fixtures מחדש

כשהפורטל מארח כרגע תקציר פרוזאי, השתמשו במקורות ה-Markdown המקוריים כמקור אמת:

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

כאשר אוטומציית Docusaurus תעלה לאוויר, הפורטל יעודכן דרך סקריפט sync (מעקב ב-`docs/portal/scripts/`) שמושך את הנתונים מהקבצים האלה. עד אז, שמרו את הדף הזה מיושר ידנית בכל שינוי במפרט.
