<!-- Hebrew translation of docs/source/runtime_upgrades.md -->

---
lang: he
direction: rtl
source: docs/source/runtime_upgrades.md
status: complete
translator: manual
---

<div dir="rtl">

# שדרוגי Runtime (IVM + Host) — ללא Downtime וללא Hardfork

המסמך מגדיר מנגנון דטרמיניסטי בשליטת ממשל להוספת יכולות IVM/Host חדשות בלי לעצור את הרשת ובלי Hardfork. הנודים מריצים בינארי מעודכן מראש; ההפעלה מתואמת on-chain בתוך חלון גבהים. חוזים קיימים ממשיכים לפעול; יכולות חדשות נשלטות ע״י גרסת ABI ומדיניות.

## יעדים
- הפעלה דטרמיניסטית בחלון גבהים וביצוע אדמ״ס.
- קיום במקביל של גרסאות ABI; אין שבירה רטרואקטיבית.
- כללי קבלה/ביצוע שמונעים הפעלה מוקדמת.
- פריסה ידידותית למפעילים עם חשיפה טלמטרית ומצבי כשל ברורים.

## לא-יעדים
- שינוי מספרי syscall או מזהי pointer (אסור).
- Patch בזמן אמת ללא בינארי חדש.

## הגדרות
- **גרסת ABI**: ערך ב-`ProgramMetadata.abi_version` שמחלץ `SyscallPolicy` והרשאות pointer.
- **ABI Hash**: digest דטרמיניסטי של הממשק (`compute_abi_hash`).
- **מדיניות syscall**: קובעת אילו מספרים מותרים לכל גרסת ABI.
- **חלון הפעלה**: `[start, end)` עם הפעלה בדיוק ב-`start`.

## אוביקטים
- `RuntimeUpgradeId` = Blake2b-256 של המניפסט.
- `RuntimeUpgradeManifest`: שם, תיאור, `abi_version`, `abi_hash`, רשימת syscalls/types חדשים, `start_height`, `end_height`.
- `RuntimeUpgradeRecord`: המניפסט, סטטוס, מציע, גובה יצירה.
- תנאים: `end>start`, גרסה גדולה מהפעילות, רשימות חדשות אינן חופפות.

## אחסון
`world.runtime_upgrades` – מפת MVCC עם ערכים Norito.

## הוראות
- `ProposeRuntimeUpgrade`: מוסיפה רשומה Proposed אם לא קיים. בודקת חפיפה ותנאים.
- `ActivateRuntimeUpgrade`: דורשת Proposed, הפעלה ב-`start_height`, מצב הופך `ActivatedAt` והגרסה מצטרפת לסט הפעיל.
- `CancelRuntimeUpgrade`: ניתן לפני תחילת החלון.

## אירועים
`RuntimeUpgradeEvent::{Proposed, Activated, Canceled}`.

## חוקים לקבלה
- מניפסט עם `abi_version=v`: לפני ההפעלה נדחה (`AbiVersionNotActive`); אחרי ההפעלה בודקים `abi_hash`.
- הוראות שדרוג דורשות הרשאות על ומניעת חפיפה.

## חוקים בביצוע
- ה-Host גוזר `SyscallPolicy` מ-`abi_version`; קריאות לא מוכרות → `UnknownSyscall`.
- רשימת pointer מותרת נגזרת מאותה גרסה.
- לאחר ההפעלה, טרנזקציות באותו בלוק כבר רואות את הפוליסה החדשה.

## דטרמיניזם ובטיחות
- הפעלה רק ב-`start_height`, אדמ״ס לריאורגים.
- גרסאות קיימות נשארות פעילות; חדשות מוסיפות לסט.
- אין ניהול יכולות שמשנה סדר ביצוע.

## תהליך פריסה
1. הפצת בינארי עם תמיכה בגרסה `v+1`.
2. מעקב טלמטריה.
3. הגשת `ProposeRuntimeUpgrade` מוקדם.
4. הפעלה ב-`start_height` – נודים שלא שודרגו דוחים ABI חדש.
5. לאחר ההפעלה אפשר לקמפל ולהפריס חוזים ל-`v+1`.

## Torii ו-CLI
- Torii: `GET /v1/runtime/abi/active`, `GET /v1/runtime/abi/hash`, `GET /v1/runtime/upgrades`, `POST /v1/runtime/upgrades/{propose|activate|cancel}`.
- CLI: `iroha runtime abi active/hash`, `runtime upgrade list/propose/activate/cancel`.

## שאילתת Core
`FindActiveAbiVersions` – מחזירה `{ active_versions, default_compile_target }` (דוגמה: `docs/source/samples/...`).

## שינויי קוד נחוצים
- DataModel: סוגים, הוראות, אירועים, קודקים.
- Core: רישום, מטפלים, חוקים לקבלה, פוליסת VM.
- IVM: הגדרת ABI חדשה, עדכון טבלאות, גולדן טסטים.
- CLI/Torii: פקודות ואנדפוינטים.
- Kotodama: תמיכה בגרסה החדשה והטבעת ה-hash.

## טלמטריה
הוספת `runtime.active_abi_versions` ו-`runtime.upgrade_events_total{kind}`.

## אבטחה
- רק root/sudo רשאים להפעיל הוראות; המניפסט חתום.
- חלון הפעלה מונע מרוץ.
- `abi_hash` קושר את הממשק כדי למנוע היסחף.

## קריטריונים
- לפני ההפעלה: דחיית `v+1`.
- אחרי ההפעלה: קבלה והרצה; גרסאות ישנות ממשיכות.
- טסטים גולדן עוברים בארכיטקטורות שונות.
- ההפעלה אדמ״ס גם בריאורגים.

</div>
