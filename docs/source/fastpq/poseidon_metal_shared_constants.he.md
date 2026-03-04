---
lang: he
direction: rtl
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Poseidon Metal Shared Constants

גרעיני מתכת, גרעיני CUDA, מוכיח חלודה וכל מתקן SDK חייבים לשתף
אותם פרמטרים של Poseidon2 בדיוק על מנת לשמור על מואצת החומרה
hashing דטרמיניסטי. מסמך זה מתעד את תמונת המצב הקנונית, כיצד לעשות זאת
ליצור מחדש את זה, וכיצד צינורות GPU צפויים לבלוע את הנתונים.

## מניפסט תמונת מצב

הפרמטרים מתפרסמים כמסמך `PoseidonSnapshot` RON. עותקים הם
נשמר תחת בקרת גרסאות כך ששרשרת כלים ו-SDK של GPU לא מסתמכים על זמן בנייה
יצירת קוד.

| נתיב | מטרה | SHA-256 |
|------|--------|--------|
| `artifacts/offline_poseidon/constants.ron` | תמונת מצב קנונית שנוצרה מ-`fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}`; מקור האמת לבניית GPU. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | משקף את תמונת המצב הקנונית כך שבדיקות יחידת Swift ורתמת העשן XCFramework טוענים את אותם הקבועים שגרעיני המתכת מצפים להם. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | אביזרי אנדרואיד/קוטלין חולקים את המניפסט הזהה עבור בדיקות זוגיות והסדרה. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

כל צרכן חייב לאמת את ה-hash לפני חיבור הקבועים ל-GPU
צינור. כאשר המניפסט משתנה (סט פרמטרים או פרופיל חדש), ה-SHA ו
יש לעדכן את המראות במורד הזרם בשלב נעילה.

## התחדשות

המניפסט נוצר ממקורות Rust על ידי הפעלת ה-`xtask`
עוזר. הפקודה כותבת גם את הקובץ הקנוני וגם את מראות ה-SDK:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

השתמש ב-`--constants <path>`/`--vectors <path>` כדי לעקוף את היעדים או
`--no-sdk-mirror` בעת יצירה מחדש של תמונת המצב הקנונית בלבד. העוזר יעשה זאת
שיקוף את החפצים לתוך עצי Swift ואנדרואיד כאשר הדגל מושמט,
מה ששומר על ה-hash מיושר עבור CI.

## האכלת מתכת/CUDA Builds

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` ו
  יש ליצור מחדש את `crates/fastpq_prover/cuda/fastpq_cuda.cu` מה-
  מתבטא בכל פעם שהטבלה משתנה.
- קבועים מעוגלים וקבועים MDS מבוימים לתוך `MTLBuffer`/`__constant` רציפים
  קטעים התואמים את פריסת המניפסט: `round_constants[round][state_width]`
  ואחריו מטריצת MDS 3x3.
- `fastpq_prover::poseidon_manifest()` טוען ומאמת את תמונת המצב ב-
  זמן ריצה (במהלך חימום מתכת) כך שכלי אבחון יכולים לקבוע כי
  קבועי הצללה תואמים ל-hash שפורסם באמצעות
  `fastpq_prover::poseidon_manifest_sha256()`.
- קוראי אביזרי SDK (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) ו
  כלי העבודה הלא מקוונים של Norito מסתמכים על אותו מניפסט, מה שמונע רק GPU
  פרמטר מזלגות.

## אימות

1. לאחר יצירת המניפסט מחדש, הפעל את `cargo test -p xtask` כדי להפעיל את
   בדיקות יחידת ייצור גופי פוסידון.
2. רשמו את ה-SHA-256 החדש במסמך זה ובכל לוחות מחוונים שמנטרים
   חפצי GPU.
3. ניתוחים של `cargo test -p fastpq_prover poseidon_manifest_consistency`
   `poseidon2.metal` ו-`fastpq_cuda.cu` בזמן הבנייה וטוען כי
   קבועים מסודרים תואמים את המניפסט, שומרים על טבלאות CUDA/Metal ו
   תמונת המצב הקנונית בשלב נעילה.שמירה על המניפסט לצד הוראות בניית ה-GPU מעניקה את המתכת/CUDA
זרימות עבודה לחיצת יד דטרמיניסטית: הגרעינים חופשיים לייעל את הזיכרון שלהם
פריסה כל עוד הם בולעים את גוש הקבועים המשותפים וחושפים את ה-hash פנימה
טלמטריה לבדיקות זוגיות.